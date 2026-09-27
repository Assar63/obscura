//! An opened camera: resolves catalog features through the profile bindings.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::discovery::CameraInfo;
use crate::error::{Error, Result};
use crate::features::{FeatureId, FeatureKind};
use crate::profile::{Binding, DeviceProfile, Level, LockSpec, QueryRead, VendorBinding};
use crate::protocol::{self, encode_command, encode_query, encode_short, encode_value};
use crate::transport::{uvc_xu, v4l2_ctrl};
use crate::v4l2::VideoNode;

#[derive(Debug, Clone, Serialize)]
pub struct FeatureState {
    pub id: FeatureId,
    pub group: &'static str,
    pub label: &'static str,
    pub kind: FeatureKind,
    pub supported: bool,
    /// False when the device currently ignores the control, e.g. manual
    /// white balance temperature while Auto WB is on.
    pub active: bool,
    pub value: Option<i64>,
    /// Why the feature is unsupported or unreadable.
    pub reason: Option<String>,
}

/// Status block and query responses shared by the features being read.
struct Readback {
    status: Option<Vec<u8>>,
    queries: HashMap<(u8, [u8; 2]), Option<Vec<u8>>>,
}

/// Details reported by the camera's system module.
#[derive(Debug, Clone, Serialize)]
pub struct FirmwareInfo {
    pub version: String,
    pub serial: Option<String>,
}

pub struct Device {
    pub info: CameraInfo,
    pub profile: DeviceProfile,
    node: VideoNode,
    /// Sequence number for framed commands.
    seq: AtomicU16,
    /// Last written value of each vendor feature, and when. Used when there
    /// is no status readback, and to mask the status block's update lag.
    written: Mutex<HashMap<FeatureId, (i64, Instant)>>,
    /// Last non-zero level of each switch/level setting, by status offset,
    /// so switching it back on restores the level.
    levels: Mutex<HashMap<usize, i64>>,
    /// Values saved by a host-side lock, while locked.
    locked: Mutex<Option<Vec<(FeatureId, i64)>>>,
}

/// The status block reflects a command about 1.1 s after it is sent.
const STATUS_LAG: Duration = Duration::from_millis(2500);

impl Device {
    pub fn open(info: CameraInfo, profile: DeviceProfile) -> Result<Self> {
        let node = VideoNode::open(&info.path)?;
        Ok(Self {
            info,
            profile,
            node,
            seq: AtomicU16::new(0),
            written: Mutex::new(HashMap::new()),
            levels: Mutex::new(HashMap::new()),
            locked: Mutex::new(None),
        })
    }

    /// Reads the vendor status block (selector 6), if the device has one.
    pub fn status_block(&self) -> Option<Vec<u8>> {
        let has_vendor = self
            .profile
            .features
            .values()
            .any(|b| matches!(b, Binding::Vendor(_)));
        if !has_vendor {
            return None;
        }
        uvc_xu::get(
            &self.node,
            protocol::XU_UNIT,
            protocol::SEL_STATUS,
            crate::v4l2::UvcQuery::GetCur,
        )
        .ok()
    }

    /// Sends a query (selector 2) and returns the response payload. Only use
    /// queries seen in OBSBOT Center captures.
    pub fn query(&self, dst: u8, cmd: [u8; 2]) -> Result<Vec<u8>> {
        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        uvc_xu::set(
            &self.node,
            protocol::XU_UNIT,
            protocol::SEL_COMMAND,
            &encode_query(seq, dst, cmd),
        )?;
        // OBSBOT Center reads the response about 40 ms later.
        for _ in 0..5 {
            std::thread::sleep(Duration::from_millis(30));
            let frame = uvc_xu::get(
                &self.node,
                protocol::XU_UNIT,
                protocol::SEL_COMMAND,
                crate::v4l2::UvcQuery::GetCur,
            )?;
            if let Some(payload) = protocol::decode_response(&frame, cmd) {
                return Ok(payload.to_vec());
            }
        }
        Err(Error::Unsupported(format!(
            "no response to query {:02x}{:02x}",
            cmd[0], cmd[1]
        )))
    }

    /// Firmware version (e.g. "6.4.4.1") and serial number, for cameras
    /// whose profile says they answer the system module's queries.
    pub fn firmware_info(&self) -> Option<FirmwareInfo> {
        if !self.profile.system_info {
            return None;
        }
        let version = self.query(protocol::DST_SYSTEM, protocol::CMD_VERSION).ok()?;
        let serial = self.query(protocol::DST_SYSTEM, protocol::CMD_SERIAL).ok();
        Some(FirmwareInfo {
            version: version
                .get(..4)?
                .iter()
                .rev()
                .map(|b| b.to_string())
                .collect::<Vec<_>>()
                .join("."),
            serial: serial.map(|s| {
                String::from_utf8_lossy(&s)
                    .trim_end_matches('\0')
                    .to_string()
            }),
        })
    }

    /// Reads a vendor setting's raw value from the status block.
    fn status_value(b: &VendorBinding, block: &[u8]) -> Option<i64> {
        let v = protocol::decode_status(block, b.status?, b.status_value)?;
        Some(match b.status_mask {
            Some(m) => ((v as u8 & m) >> m.trailing_zeros()) as i64,
            None => v,
        })
    }

    /// The last level seen for a switch/level setting.
    fn level(&self, b: &VendorBinding) -> i64 {
        b.status
            .and_then(|o| self.levels.lock().unwrap().get(&o).copied())
            .or(b.default_level)
            .unwrap_or(1)
    }

    fn remember_level(&self, b: &VendorBinding, raw: i64) {
        if let (Some(o), Some(_), true) = (b.status, b.level, raw != 0) {
            self.levels.lock().unwrap().insert(o, raw.abs());
        }
    }

    /// Feature value from a raw setting.
    fn from_setting(&self, b: &VendorBinding, raw: i64) -> i64 {
        self.remember_level(b, raw);
        match b.level {
            None if b.invert => (raw == 0) as i64,
            None => raw,
            Some(Level::Switch) => (raw != 0) as i64,
            Some(Level::SignSwitch) => (raw > 0) as i64,
            Some(Level::Magnitude) if raw == 0 => self.level(b),
            Some(Level::Magnitude) => raw.abs(),
        }
    }

    /// Raw setting to write for a feature value.
    fn to_setting(&self, b: &VendorBinding, value: i64) -> i64 {
        let Some(level) = b.level else {
            return if b.invert { (value == 0) as i64 } else { value };
        };
        let current = self
            .status_block()
            .and_then(|block| Self::status_value(b, &block));
        if let Some(raw) = current {
            self.remember_level(b, raw);
        }
        match level {
            Level::Switch if value != 0 => self.level(b),
            Level::Switch => 0,
            Level::SignSwitch if value != 0 => self.level(b),
            Level::SignSwitch => -self.level(b),
            Level::Magnitude if current.is_some_and(|c| c < 0) => -value,
            Level::Magnitude => value,
        }
    }

    fn set_lock(&self, spec: &LockSpec, lock: bool) -> Result<()> {
        if lock {
            if self.locked.lock().unwrap().is_some() {
                return Ok(());
            }
            let saved = spec
                .restore
                .iter()
                .filter_map(|&id| self.feature(id).value.map(|v| (id, v)))
                .collect();
            for &id in &spec.off {
                self.set(id, 0)?;
            }
            *self.locked.lock().unwrap() = Some(saved);
        } else {
            let saved = self.locked.lock().unwrap().take();
            for (id, v) in saved.unwrap_or_default() {
                self.set(id, v)?;
            }
        }
        Ok(())
    }

    /// Records `value` for other vendor features whose frames are a subset of
    /// `b`'s (e.g. the gesture master switch covers the four sub-switches).
    fn mark_related(&self, b: &VendorBinding, value: i64) {
        if b.frames.len() < 2 {
            return;
        }
        let now = Instant::now();
        let mut written = self.written.lock().unwrap();
        for (id, other) in &self.profile.features {
            if let Binding::Vendor(o) = other {
                let covered = !o.frames.is_empty()
                    && o.frames.iter().all(|f| {
                        b.frames.iter().any(|g| {
                            g.dst == f.dst
                                && g.cmd == f.cmd
                                && f.prefix.is_empty()
                                && g.prefix.is_empty()
                        })
                    });
                if covered {
                    written.insert(*id, (value, now));
                }
            }
        }
    }

    /// Whether the camera supports velocity-based gimbal moves.
    pub fn has_gimbal_velocity(&self) -> bool {
        self.profile.gimbal_velocity.is_some()
    }

    /// Moves the gimbal at a velocity. `right` and `up` are -1..1 fractions
    /// of the profile's `max_speed`; send (0, 0) to stop. The camera expects
    /// the command to be repeated (~10 Hz) while moving.
    pub fn gimbal_velocity(&self, right: f32, up: f32) -> Result<()> {
        let g = self
            .profile
            .gimbal_velocity
            .as_ref()
            .ok_or_else(|| Error::Unsupported("gimbal velocity".into()))?;
        let yaw = right.clamp(-1.0, 1.0) * g.max_speed * g.yaw_sign;
        let pitch = up.clamp(-1.0, 1.0) * g.max_speed * g.pitch_sign;
        let mut payload = Vec::with_capacity(12);
        for v in [0.0f32, pitch, yaw] {
            payload.extend(v.to_le_bytes());
        }
        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        let packet = encode_command(seq, g.dst, g.cmd, &payload);
        uvc_xu::set(
            &self.node,
            protocol::XU_UNIT,
            protocol::SEL_COMMAND,
            &packet,
        )
    }

    fn send_vendor(&self, b: &VendorBinding, value: i64) -> Result<()> {
        for f in &b.frames {
            let mut payload = f.prefix.clone();
            payload.extend(encode_value(f.value, value, f.divisor));
            let seq = self.seq.fetch_add(1, Ordering::Relaxed);
            let packet = encode_command(seq, f.dst, f.cmd, &payload);
            uvc_xu::set(
                &self.node,
                protocol::XU_UNIT,
                protocol::SEL_COMMAND,
                &packet,
            )?;
        }
        if let Some(s) = &b.short {
            let mut payload = s.prefix.clone();
            payload.extend(encode_value(s.value, value, 1));
            let packet = encode_short(s.id, &payload);
            uvc_xu::set(&self.node, protocol::XU_UNIT, protocol::SEL_STATUS, &packet)?;
        }
        Ok(())
    }

    /// Opens a camera with the best matching builtin profile.
    pub fn open_auto(info: CameraInfo) -> Result<Self> {
        let profile = DeviceProfile::for_usb(info.vendor_id, info.product_id);
        Self::open(info, profile)
    }

    pub fn node(&self) -> &VideoNode {
        &self.node
    }

    pub fn feature(&self, id: FeatureId) -> FeatureState {
        let readback = self.readback(&[id]);
        self.feature_with_status(id, &readback)
    }

    /// Reads the status block and query responses needed by `ids`, once each.
    fn readback(&self, ids: &[FeatureId]) -> Readback {
        let vendor = || {
            ids.iter().filter_map(|id| match self.profile.features.get(id) {
                Some(Binding::Vendor(b)) => Some(b),
                _ => None,
            })
        };
        let mut rb = Readback {
            status: None,
            queries: HashMap::new(),
        };
        if vendor().any(|b| b.status.is_some() && b.query.is_none()) {
            rb.status = self.status_block();
        }
        for q in vendor().filter_map(|b| b.query.as_ref()) {
            if let std::collections::hash_map::Entry::Vacant(e) = rb.queries.entry((q.dst, q.cmd))
            {
                e.insert(self.query(q.dst, q.cmd).ok());
            }
        }
        rb
    }

    fn query_value(q: &QueryRead, rb: &Readback) -> Option<i64> {
        let payload = rb.queries.get(&(q.dst, q.cmd))?.as_ref()?;
        let bytes: Option<Vec<u8>> = q.offsets.iter().map(|&o| payload.get(o).copied()).collect();
        match bytes?.as_slice() {
            [b] => Some(*b as i64 * q.scale),
            bs => Some(bs.iter().any(|&b| b != 0) as i64),
        }
    }

    fn feature_with_status(&self, id: FeatureId, rb: &Readback) -> FeatureState {
        let status = rb.status.as_deref();
        let def = id.def();
        let mut state = FeatureState {
            id,
            group: def.group,
            label: def.label,
            kind: def.kind.clone(),
            supported: false,
            active: true,
            value: None,
            reason: None,
        };
        match self.profile.features.get(&id) {
            None => state.reason = Some(format!("not in profile `{}`", self.profile.id)),
            Some(Binding::V4l2(b)) => match self.node.query_control(b.v4l2) {
                Ok(Some(info)) if !info.is_disabled() => {
                    state.supported = true;
                    state.active = !info.is_inactive();
                    state.kind = v4l2_ctrl::describe(&info, b, &def.kind);
                    match v4l2_ctrl::get(&self.node, b) {
                        Ok(v) => state.value = Some(v),
                        Err(e) => state.reason = Some(e.to_string()),
                    }
                }
                Ok(_) => state.reason = Some("control not exposed by the camera".into()),
                Err(e) => state.reason = Some(e.to_string()),
            },
            Some(Binding::Vendor(b)) => {
                state.supported = true;
                let written = self.written.lock().unwrap().get(&id).copied();
                let raw = status.and_then(|block| Self::status_value(b, block));
                let read = match &b.query {
                    Some(q) => Self::query_value(q, rb),
                    None => raw.map(|r| self.from_setting(b, r)),
                };
                state.value = match written {
                    Some((v, at)) if at.elapsed() < STATUS_LAG => Some(v),
                    w => read.or(w.map(|(v, _)| v)),
                };
                // A level is inactive while its switch is off.
                if b.level == Some(Level::Magnitude) {
                    state.active = raw.is_none_or(|r| r > 0);
                }
            }
            Some(Binding::Lock(_)) => {
                state.supported = true;
                state.value = Some(self.locked.lock().unwrap().is_some() as i64);
            }
        }
        state
    }

    pub fn read_all(&self) -> Vec<FeatureState> {
        self.read(FeatureId::ALL)
    }

    /// Reads several features with a single status-block read.
    pub fn read(&self, ids: &[FeatureId]) -> Vec<FeatureState> {
        let readback = self.readback(ids);
        ids.iter()
            .map(|&id| self.feature_with_status(id, &readback))
            .collect()
    }

    /// Whether the camera is still there (false after unplug or reboot).
    pub fn is_connected(&self) -> bool {
        self.node.capability().is_ok()
    }

    /// Validates `value` against the feature's kind, writes it, and returns
    /// the re-read state (other controls may change as a side effect, e.g.
    /// toggling Auto WB activates Temperature).
    pub fn set(&self, id: FeatureId, value: i64) -> Result<FeatureState> {
        let current = self.feature(id);
        if !current.supported {
            return Err(Error::Unsupported(id.name().to_string()));
        }
        validate(&current.kind, value)?;
        match self.profile.features.get(&id) {
            Some(Binding::V4l2(b)) => v4l2_ctrl::set(&self.node, b, value)?,
            Some(Binding::Lock(l)) => self.set_lock(&l.lock, value != 0)?,
            Some(Binding::Vendor(b)) => {
                self.send_vendor(b, self.to_setting(b, value))?;
                if b.level == Some(Level::Magnitude) {
                    self.remember_level(b, value);
                }
                self.written
                    .lock()
                    .unwrap()
                    .insert(id, (value, Instant::now()));
                // Frames like the gesture master switch also set sub-features.
                self.mark_related(b, value);
            }
            None => return Err(Error::Unsupported(id.name().to_string())),
        }
        Ok(self.feature(id))
    }
}

fn validate(kind: &FeatureKind, value: i64) -> Result<()> {
    let (min, max) = match kind {
        FeatureKind::Toggle => (0, 1),
        FeatureKind::Range { min, max, .. } => (*min, *max),
        FeatureKind::Choice { options } => {
            if options.iter().any(|o| o.value == value) {
                return Ok(());
            }
            let min = options.iter().map(|o| o.value).min().unwrap_or(0);
            let max = options.iter().map(|o| o.value).max().unwrap_or(0);
            return Err(Error::OutOfRange { value, min, max });
        }
        FeatureKind::Action => return Ok(()),
    };
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(Error::OutOfRange { value, min, max })
    }
}
