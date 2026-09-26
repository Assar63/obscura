//! An opened camera: resolves catalog features through the profile bindings.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::discovery::CameraInfo;
use crate::error::{Error, Result};
use crate::features::{FeatureId, FeatureKind};
use crate::profile::{Binding, DeviceProfile, VendorBinding};
use crate::protocol::{self, encode_command, encode_short, encode_value};
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

pub struct Device {
    pub info: CameraInfo,
    pub profile: DeviceProfile,
    node: VideoNode,
    /// Sequence number for framed commands.
    seq: AtomicU16,
    /// Last written value of each vendor feature, and when. Used when there
    /// is no status readback, and to mask the status block's update lag.
    written: Mutex<HashMap<FeatureId, (i64, Instant)>>,
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
        })
    }

    /// Reads the vendor status block (selector 6), if the device has one.
    pub fn status_block(&self) -> Option<Vec<u8>> {
        let has_vendor = self.profile.features.values().any(|b| matches!(b, Binding::Vendor(_)));
        if !has_vendor {
            return None;
        }
        uvc_xu::get(&self.node, protocol::XU_UNIT, protocol::SEL_STATUS, crate::v4l2::UvcQuery::GetCur).ok()
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
                    && o.frames.iter().all(|f| b.frames.iter().any(|g| g.dst == f.dst && g.cmd == f.cmd && f.prefix.is_empty() && g.prefix.is_empty()));
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
        uvc_xu::set(&self.node, protocol::XU_UNIT, protocol::SEL_COMMAND, &packet)
    }

    fn send_vendor(&self, b: &VendorBinding, value: i64) -> Result<()> {
        for f in &b.frames {
            let mut payload = f.prefix.clone();
            payload.extend(encode_value(f.value, value, f.divisor));
            let seq = self.seq.fetch_add(1, Ordering::Relaxed);
            let packet = encode_command(seq, f.dst, f.cmd, &payload);
            uvc_xu::set(&self.node, protocol::XU_UNIT, protocol::SEL_COMMAND, &packet)?;
        }
        if let Some(s) = &b.short {
            let packet = encode_short(s.id, &encode_value(s.value, value, 1));
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
        let status = match self.profile.features.get(&id) {
            Some(Binding::Vendor(b)) if b.status.is_some() => self.status_block(),
            _ => None,
        };
        self.feature_with_status(id, status.as_deref())
    }

    fn feature_with_status(&self, id: FeatureId, status: Option<&[u8]>) -> FeatureState {
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
                state.value = match (b.status, status, written) {
                    (_, _, Some((v, at))) if at.elapsed() < STATUS_LAG => Some(v),
                    (Some(offset), Some(block), _) => block.get(offset).map(|&v| v as i64),
                    (_, _, w) => w.map(|(v, _)| v),
                };
            }
        }
        state
    }

    pub fn read_all(&self) -> Vec<FeatureState> {
        let status = self.status_block();
        FeatureId::ALL
            .iter()
            .map(|&id| self.feature_with_status(id, status.as_deref()))
            .collect()
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
            Some(Binding::Vendor(b)) => {
                self.send_vendor(b, value)?;
                self.written.lock().unwrap().insert(id, (value, Instant::now()));
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
