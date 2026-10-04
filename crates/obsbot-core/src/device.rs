//! An opened camera: resolves catalog features through the profile bindings.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::discovery::CameraInfo;
use crate::error::{Error, Result};
use crate::features::{ChoiceOption, FeatureId, FeatureKind};
use crate::log;
use crate::profile::{
    AlsaControl, Binding, DeviceProfile, Level, LockSpec, QueryRead, VendorBinding,
};
use crate::protocol::{
    self, encode_command, encode_query, encode_short, encode_value, ValueEncoding,
};
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

fn hex(cmd: [u8; 2]) -> String {
    format!("{:02x}{:02x}", cmd[0], cmd[1])
}

/// A query's destination, command and payload.
type QueryKey = (u8, [u8; 2], Vec<u8>);

/// Status block and query responses shared by the features being read.
struct Readback {
    status: Option<Vec<u8>>,
    queries: HashMap<QueryKey, Option<Vec<u8>>>,
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
    /// Last value read that wasn't transient (see `VendorBinding::transient`).
    settled: Mutex<HashMap<FeatureId, i64>>,
    /// Writes not yet compared with the camera's readback, so the log can
    /// warn when the camera ignored one.
    unverified: Mutex<HashMap<FeatureId, (i64, Instant)>>,
    /// Camera-side state at the last status read, to log what the camera
    /// changed on its own (gestures, voice, auto-sleep).
    observed: Mutex<Observed>,
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
            settled: Mutex::new(HashMap::new()),
            unverified: Mutex::new(HashMap::new()),
            observed: Mutex::new(Observed::default()),
            locked: Mutex::new(None),
        })
    }

    /// Compares the camera's firmware with the latest on OBSBOT's download
    /// page. Needs network access; the error is a sentence for the user.
    #[cfg(feature = "update-check")]
    pub fn check_firmware_update(
        &self,
    ) -> std::result::Result<crate::firmware::UpdateCheck, String> {
        let source = self.profile.firmware.as_ref().ok_or_else(|| {
            format!(
                "No firmware download page is known for {}",
                self.profile.name
            )
        })?;
        let current = self
            .firmware_info()
            .ok_or("The camera didn't report its firmware version")?
            .version;
        crate::firmware::check(source, &current)
    }

    /// The camera's live state decoded from the status block, if it has one.
    pub fn live_status(&self) -> Option<crate::status::LiveStatus> {
        let block = self.status_block()?;
        self.observe(&block);
        let mut st = crate::status::decode(&block)?;
        st.ai_mode_label = match st.ai_mode {
            6 => "switching…".to_string(),
            v => self.declared_kind(FeatureId::AiMode).format(v as i64),
        };
        Some(st)
    }

    /// The wireless microphone slots (Vox SE), if the camera has a
    /// receiver: online state from the status block, battery and the rest
    /// from the mic info query.
    pub fn wireless_mics(&self) -> Option<Vec<crate::status::MicSlot>> {
        if !self.profile.wireless_mics {
            return None;
        }
        let status41 = *self.status_block()?.get(41)?;
        let info = self
            .query(protocol::DST_CAMERA, protocol::CMD_MIC_INFO)
            .unwrap_or_default();
        Some(crate::status::decode_mics(&info, status41))
    }

    /// A feature's kind as the profile declares it (catalog, or the
    /// profile's own `options`), without reading the camera.
    pub fn declared_kind(&self, id: FeatureId) -> FeatureKind {
        match self.profile.features.get(&id) {
            Some(Binding::Vendor(b)) if !b.options.is_empty() => FeatureKind::Choice {
                options: b
                    .options
                    .iter()
                    .map(|(value, label)| ChoiceOption {
                        value: *value,
                        label: label.clone(),
                    })
                    .collect(),
            },
            _ => id.def().kind,
        }
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
        self.query_with(dst, cmd, protocol::FLAGS_QUERY, &[])?
            .ok_or_else(|| Error::Unsupported(format!("query {} answered empty", hex(cmd))))
    }

    /// Sends a query with `flags` and `payload`; `None` if the camera
    /// answers that there is nothing there (e.g. an empty preset slot).
    fn query_with(
        &self,
        dst: u8,
        cmd: [u8; 2],
        flags: u8,
        payload: &[u8],
    ) -> Result<Option<Vec<u8>>> {
        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        uvc_xu::set(
            &self.node,
            protocol::XU_UNIT,
            protocol::SEL_COMMAND,
            &encode_query(seq, dst, cmd, flags, payload),
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
            match protocol::decode_response(&frame, seq, cmd) {
                Some((protocol::FLAGS_RESPONSE, p)) => return Ok(Some(p.to_vec())),
                Some((protocol::FLAGS_RESPONSE_EMPTY, _)) => return Ok(None),
                _ => {}
            }
        }
        Err(Error::Unsupported(format!(
            "no response to query {}",
            hex(cmd)
        )))
    }

    fn command(&self, dst: u8, cmd: [u8; 2], payload: &[u8]) -> Result<()> {
        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        uvc_xu::set(
            &self.node,
            protocol::XU_UNIT,
            protocol::SEL_COMMAND,
            &encode_command(seq, dst, cmd, payload),
        )
    }

    /// Number of camera-side gimbal presets (0 if unsupported).
    pub fn preset_slots(&self) -> u32 {
        self.profile.presets.as_ref().map_or(0, |p| p.slots)
    }

    fn check_slot(&self, slot: u32) -> Result<()> {
        if slot < self.preset_slots() {
            Ok(())
        } else {
            Err(Error::OutOfRange {
                value: slot as i64,
                min: 0,
                max: self.preset_slots() as i64 - 1,
            })
        }
    }

    /// Preset names by slot; `None` for empty slots.
    pub fn presets(&self) -> Result<Vec<Option<String>>> {
        (0..self.preset_slots())
            .map(|slot| {
                let name = self.query_with(
                    protocol::DST_GIMBAL,
                    protocol::CMD_PRESET_NAME_QUERY,
                    protocol::FLAGS_QUERY_SLOT,
                    &slot.to_le_bytes(),
                )?;
                Ok(name.map(|n| {
                    String::from_utf8_lossy(&n)
                        .trim_end_matches('\0')
                        .to_string()
                }))
            })
            .collect()
    }

    /// Stores the gimbal's current position and zoom in `slot`, as OBSBOT
    /// Center's "Add" does, and names it.
    pub fn save_preset(&self, slot: u32, name: &str) -> Result<()> {
        self.check_slot(slot)?;
        let pos = self
            .query_with(
                protocol::DST_GIMBAL,
                protocol::CMD_GIMBAL_POSITION,
                protocol::FLAGS_QUERY,
                &[1],
            )?
            .ok_or_else(|| Error::Unsupported("gimbal position unavailable".into()))?;
        let angle = |o: usize| -> Result<f32> {
            let b = pos
                .get(o..o + 2)
                .ok_or_else(|| Error::Unsupported("short gimbal position".into()))?;
            // x 0.1 in f32, as OBSBOT Center does (matches its bytes exactly).
            Ok(i16::from_le_bytes([b[0], b[1]]) as f32 * 0.1)
        };
        let zoom = self
            .query(protocol::DST_GIMBAL, protocol::CMD_ZOOM)?
            .get(..4)
            .map_or(1.0, |b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        let mut payload = slot.to_le_bytes().to_vec();
        for v in [angle(10)?, angle(8)?, 0.0, zoom, -1000.0] {
            payload.extend(v.to_le_bytes());
        }
        self.command(protocol::DST_GIMBAL, protocol::CMD_PRESET_SAVE, &payload)?;
        self.rename_preset(slot, name)
    }

    /// Longest preset name sent to the camera, in bytes.
    pub const PRESET_NAME_MAX: usize = 16;

    pub fn rename_preset(&self, slot: u32, name: &str) -> Result<()> {
        self.check_slot(slot)?;
        let mut end = name.len().min(Self::PRESET_NAME_MAX);
        while !name.is_char_boundary(end) {
            end -= 1;
        }
        let mut payload = slot.to_le_bytes().to_vec();
        payload.extend(&name.as_bytes()[..end]);
        self.command(protocol::DST_GIMBAL, protocol::CMD_PRESET_NAME, &payload)
    }

    /// Moves the gimbal (and zoom) to a stored preset.
    pub fn recall_preset(&self, slot: u32) -> Result<()> {
        self.check_slot(slot)?;
        let mut payload = slot.to_le_bytes().to_vec();
        for _ in 0..4 {
            payload.extend(1.0f32.to_le_bytes());
        }
        self.command(protocol::DST_GIMBAL, protocol::CMD_PRESET_RECALL, &payload)
    }

    /// Firmware version (e.g. "6.4.4.1") and serial number, for cameras
    /// whose profile says they answer the system module's queries.
    pub fn firmware_info(&self) -> Option<FirmwareInfo> {
        if !self.profile.system_info {
            return None;
        }
        let version = self
            .query(protocol::DST_SYSTEM, protocol::CMD_VERSION)
            .ok()?;
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
        b.level_status
            .or(b.status)
            .and_then(|o| self.levels.lock().unwrap().get(&o).copied())
            .or(b.default_level)
            .unwrap_or(1)
    }

    fn remember_level(&self, b: &VendorBinding, raw: i64) {
        if b.level_status.is_some() {
            return; // `raw` is the switch, not the level
        }
        if let (Some(o), Some(_), true) = (b.status, b.level, raw != 0) {
            self.levels.lock().unwrap().insert(o, raw.abs());
        }
    }

    /// Feature value from a raw setting.
    fn setting_to_feature(&self, b: &VendorBinding, raw: i64) -> i64 {
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
    fn feature_to_setting(&self, b: &VendorBinding, value: i64) -> i64 {
        let Some(level) = b.level else {
            return if b.invert { (value == 0) as i64 } else { value };
        };
        let block = self.status_block();
        let current = block
            .as_deref()
            .and_then(|block| Self::status_value(b, block));
        if let Some(raw) = current {
            self.remember_level(b, raw);
        }
        if let Some(o) = b.level_status {
            if let Some(&l) = block.as_deref().and_then(|block| block.get(o)) {
                if l != 0 {
                    self.levels.lock().unwrap().insert(o, l as i64);
                }
            }
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
            let payload = f.payload.clone().unwrap_or_else(|| {
                let mut p = f.prefix.clone();
                p.extend(encode_value(f.value, value, f.divisor));
                p
            });
            let seq = self.seq.fetch_add(1, Ordering::Relaxed);
            let packet = match f.flags {
                Some(flags) => encode_query(seq, f.dst, f.cmd, flags, &payload),
                None => encode_command(seq, f.dst, f.cmd, &payload),
            };
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
            ids.iter()
                .filter_map(|id| match self.profile.features.get(id) {
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
            if let Some(block) = &rb.status {
                self.observe(block);
            }
        }
        for q in vendor().filter_map(|b| b.query.as_ref()) {
            if let std::collections::hash_map::Entry::Vacant(e) =
                rb.queries.entry((q.dst, q.cmd, q.payload.clone()))
            {
                let flags = q.flags.unwrap_or(protocol::FLAGS_QUERY);
                e.insert(
                    self.query_with(q.dst, q.cmd, flags, &q.payload)
                        .ok()
                        .flatten(),
                );
            }
        }
        rb
    }

    fn query_value(q: &QueryRead, rb: &Readback) -> Option<i64> {
        let payload = rb
            .queries
            .get(&(q.dst, q.cmd, q.payload.clone()))?
            .as_ref()?;
        if let (ValueEncoding::F32, [o]) = (q.value, q.offsets.as_slice()) {
            let v = f32::from_le_bytes(payload.get(*o..*o + 4)?.try_into().ok()?);
            return Some((v as f64 * q.scale as f64).round() as i64);
        }
        let bytes: Option<Vec<u8>> = q.offsets.iter().map(|&o| payload.get(o).copied()).collect();
        match bytes?.as_slice() {
            [b] => {
                let b = q.mask.map_or(*b, |m| (b & m) >> m.trailing_zeros());
                let v = match q.value {
                    ValueEncoding::I8 => b as i8 as i64,
                    _ => b as i64,
                };
                Some(v * q.scale)
            }
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
                if !b.options.is_empty() {
                    state.kind = self.declared_kind(id);
                }
                let written = self.written.lock().unwrap().get(&id).copied();
                let raw = status.and_then(|block| Self::status_value(b, block));
                let read = match &b.query {
                    Some(q) => Self::query_value(q, rb),
                    None if raw.is_some_and(|r| b.transient.contains(&r)) => {
                        self.settled.lock().unwrap().get(&id).copied()
                    }
                    None => {
                        let v = raw.map(|r| self.setting_to_feature(b, r));
                        if let Some(v) = v {
                            self.settled.lock().unwrap().insert(id, v);
                        }
                        v
                    }
                };
                state.value = match written {
                    Some((v, at)) if at.elapsed() < STATUS_LAG => Some(v),
                    w => read.or(w.map(|(v, _)| v)),
                };
                let transient = raw.is_some_and(|r| b.transient.contains(&r));
                if !transient {
                    self.verify_write(id, read, &state);
                }
                // A level is inactive while its switch is off.
                if b.level == Some(Level::Magnitude) {
                    state.active = raw.is_none_or(|r| r > 0);
                }
            }
            Some(Binding::Lock(_)) => {
                state.supported = true;
                state.value = Some(self.locked.lock().unwrap().is_some() as i64);
            }
            Some(Binding::Alsa(a)) => match crate::audio::find_card(&self.info.usb_path) {
                None => state.reason = Some("no USB audio found for this camera".into()),
                Some(card) => {
                    let read = match a.alsa {
                        AlsaControl::CaptureVolume => crate::audio::volume(card),
                        AlsaControl::CaptureSwitch => crate::audio::switch(card).map(i64::from),
                    };
                    match read {
                        Ok(v) => {
                            state.supported = true;
                            state.value = Some(v);
                        }
                        Err(e) => state.reason = Some(e.to_string()),
                    }
                }
            },
        }
        state
    }

    /// Re-writes the white balance temperature after Auto WB is switched
    /// off. Tiny-series firmware uses whichever white balance control was
    /// written last (joshualambert/obsbot-tiny3-linux), so this makes the
    /// shown temperature the one in effect.
    fn reapply_white_balance_temperature(&self) {
        let Some(Binding::V4l2(t)) = self
            .profile
            .features
            .get(&FeatureId::WhiteBalanceTemperature)
        else {
            return;
        };
        if let Ok(v) = v4l2_ctrl::get(&self.node, t) {
            if v4l2_ctrl::set(&self.node, t, v).is_ok() {
                log::info(format!(
                    "Temperature: re-applied {v}K for manual white balance"
                ));
            }
        }
    }

    /// Logs what the camera did on its own since the last status read:
    /// queued events (target lost/found, …), and AI mode, power and zoom
    /// changes that weren't OBSCura's (a gesture, a voice command,
    /// auto-sleep). Status layout: docs/protocol.md.
    fn observe(&self, block: &[u8]) {
        if self.profile.event_queue {
            self.drain_events(block.get(43).copied().unwrap_or(0));
        }
        let ours = |id: FeatureId| {
            self.written
                .lock()
                .unwrap()
                .get(&id)
                .is_some_and(|(_, at)| at.elapsed() < OWN_CHANGE)
        };
        let mut seen = self.observed.lock().unwrap();

        // AI mode; 6 means a change is in progress.
        if let Some(&mode) = block.get(24).filter(|&&m| m != 6) {
            if seen.ai_mode.is_some_and(|m| m != mode) && !ours(FeatureId::AiMode) {
                log::info(format!(
                    "Camera: AI mode is now {} (gesture, voice or camera)",
                    self.declared_kind(FeatureId::AiMode).format(mode as i64)
                ));
            }
            seen.ai_mode = Some(mode);
        }

        if let Some(&power) = block.get(9) {
            if seen.power.is_some_and(|p| p != power) && !ours(FeatureId::Sleep) {
                log::info(match power {
                    1 => "Camera: woke up".to_string(),
                    3 => "Camera: went to sleep".to_string(),
                    4 => "Camera: privacy mode".to_string(),
                    v => format!("Camera: power state {v}"),
                });
            }
            seen.power = Some(power);
        }

        // Wireless microphones and the audio source; logged whoever changed
        // them, since that's what pairing experiments need to see.
        if let (Some(&mic), Some(&audio)) = (block.get(41), block.get(40)) {
            if seen.mic.is_some_and(|m| m != mic) {
                log::info(format!(
                    "Camera: wireless mic: {} (status[41] = {mic:02x})",
                    crate::status::describe_mic(mic)
                ));
            }
            if seen.audio_source.is_some_and(|a| a != audio & 7) {
                log::info(format!("Camera: audio source is now {}", audio & 7));
            }
            seen.mic = Some(mic);
            seen.audio_source = Some(audio & 7);
        }

        // Zoom ramps; log it once two reads agree.
        if let (Some(&lo), Some(&hi)) = (block.get(4), block.get(5)) {
            let ratio = u16::from_le_bytes([lo, hi]);
            if seen.zoom_seen == Some(ratio) && seen.zoom_logged != Some(ratio) {
                if seen.zoom_logged.is_some() && !ours(FeatureId::Zoom) {
                    log::info(format!(
                        "Camera: zoom is now {:.2}x (gesture, voice or tracking)",
                        1.0 + ratio.min(100) as f32 * 3.0 / 100.0
                    ));
                }
                seen.zoom_logged = Some(ratio);
            }
            seen.zoom_seen = Some(ratio);
        }
    }

    /// Pops up to `pending` queued camera events and logs them.
    fn drain_events(&self, pending: u8) {
        for _ in 0..pending.min(16) {
            match self.query_with(
                protocol::DST_CAMERA,
                protocol::CMD_EVENT,
                protocol::FLAGS_QUERY,
                &[],
            ) {
                Ok(Some(payload)) => log::info(describe_event(&payload)),
                _ => break,
            }
        }
    }

    /// Once the status lag has passed after a write, logs a warning if the
    /// camera reports something else than what was written.
    fn verify_write(&self, id: FeatureId, read: Option<i64>, state: &FeatureState) {
        let mut unverified = self.unverified.lock().unwrap();
        let Some(&(wanted, at)) = unverified.get(&id) else {
            return;
        };
        if at.elapsed() < STATUS_LAG {
            return;
        }
        unverified.remove(&id);
        match read {
            Some(got) if got != wanted => log::warn(format!(
                "{}: asked for {}, but the camera reports {}.{}",
                state.label,
                state.kind.format(wanted),
                state.kind.format(got),
                ignored_hint(id, wanted),
            )),
            _ => {}
        }
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
        let wanted = current.kind.format(value);
        let result = self.set_checked(id, value, &current);
        match &result {
            Ok(_) => log::info(format!("{}: set to {wanted}", current.label)),
            Err(e) => log::warn(format!("{}: setting {wanted} failed: {e}", current.label)),
        }
        result
    }

    fn set_checked(
        &self,
        id: FeatureId,
        value: i64,
        current: &FeatureState,
    ) -> Result<FeatureState> {
        if !current.supported {
            return Err(Error::Unsupported(id.name().to_string()));
        }
        validate(&current.kind, value)?;
        match self.profile.features.get(&id) {
            Some(Binding::V4l2(b)) => {
                v4l2_ctrl::set(&self.node, b, value)?;
                if id == FeatureId::AutoWhiteBalance && value == 0 {
                    self.reapply_white_balance_temperature();
                }
            }
            Some(Binding::Lock(l)) => self.set_lock(&l.lock, value != 0)?,
            Some(Binding::Alsa(a)) => {
                let card = crate::audio::find_card(&self.info.usb_path)
                    .ok_or_else(|| Error::Audio("no USB audio found for this camera".into()))?;
                match a.alsa {
                    AlsaControl::CaptureVolume => crate::audio::set_volume(card, value)?,
                    AlsaControl::CaptureSwitch => crate::audio::set_switch(card, value != 0)?,
                }
            }
            Some(Binding::Vendor(b)) => {
                self.send_vendor(b, self.feature_to_setting(b, value))?;
                if b.level == Some(Level::Magnitude) {
                    self.remember_level(b, value);
                }
                self.written
                    .lock()
                    .unwrap()
                    .insert(id, (value, Instant::now()));
                if b.status.is_some() || b.query.is_some() {
                    self.unverified
                        .lock()
                        .unwrap()
                        .insert(id, (value, Instant::now()));
                }
                // Frames like the gesture master switch also set sub-features.
                self.mark_related(b, value);
            }
            None => return Err(Error::Unsupported(id.name().to_string())),
        }
        Ok(self.feature(id))
    }
}

/// How long after OBSCura changes a setting a matching camera-side change is
/// taken as its result rather than the camera's own.
const OWN_CHANGE: Duration = Duration::from_secs(10);

/// Camera-side state at the last status read (see `Device::observe`).
#[derive(Default)]
struct Observed {
    ai_mode: Option<u8>,
    power: Option<u8>,
    mic: Option<u8>,
    audio_source: Option<u8>,
    /// Zoom ratio at the last read, and the last one logged.
    zoom_seen: Option<u16>,
    zoom_logged: Option<u16>,
}

/// A camera event for the activity log. Payload: u32 source, u32 type,
/// optional u32 value (Tiny 3, from OBSBOT's SDK traffic).
fn describe_event(p: &[u8]) -> String {
    let word = |i: usize| {
        p.get(i..i + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    match (word(0), word(4), word(8)) {
        // SDK: kEvtInfoTargetLoss, 1 lost, 0 found again.
        (Some(0x71), Some(20), Some(1)) => "Camera: tracking target lost".to_string(),
        (Some(0x71), Some(20), Some(0)) => "Camera: tracking target found again".to_string(),
        (Some(source), Some(kind), value) => format!(
            "Camera event: source {source:#x}, type {kind}{}",
            value.map_or(String::new(), |v| format!(", value {v}"))
        ),
        _ => format!("Camera event: {}", log::hex(p)),
    }
}

/// Why the camera may have ignored a write, for the activity log.
fn ignored_hint(id: FeatureId, value: i64) -> &'static str {
    match id {
        // Measured on a Tiny 3: Human needs the camera to see someone, so it's
        // ignored without a stream; Group, Hand, Desk and Off are not.
        FeatureId::AiMode if value == 2 => {
            " Human tracking only starts while the camera is streaming video: open \
             the preview or start a call, then try again."
        }
        FeatureId::Zoom => " The camera ignores zoom while it sleeps.",
        _ => "",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_tiny_3_events() {
        // Event queue replies traced from OBSBOT's SDK on a Tiny 3.
        let lost = [0x71, 0, 0, 0, 0x14, 0, 0, 0, 1, 0, 0, 0];
        let found = [0x71, 0, 0, 0, 0x14, 0, 0, 0, 0, 0, 0, 0];
        let other = [0x71, 0, 0, 0, 0x10, 0, 0, 0];
        assert_eq!(describe_event(&lost), "Camera: tracking target lost");
        assert_eq!(
            describe_event(&found),
            "Camera: tracking target found again"
        );
        assert_eq!(describe_event(&other), "Camera event: source 0x71, type 16");
        assert_eq!(describe_event(&[1, 2]), "Camera event: 01 02");
    }
}
