//! An opened camera: resolves catalog features through the profile bindings.
//!
//! `Device` is split by concern: reading features (`read`), writing them
//! (`write`), vendor frames and queries (`vendor`), what the camera does on
//! its own (`observe`), presets, gimbal, and device info.

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

mod gimbal;
mod info;
mod observe;
mod presets;
mod read;
mod vendor;
mod write;

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
    observed: Mutex<observe::Observed>,
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
            observed: Mutex::new(observe::Observed::default()),
            locked: Mutex::new(None),
        })
    }

    /// Opens a camera with the best matching builtin profile.
    pub fn open_auto(info: CameraInfo) -> Result<Self> {
        let profile = DeviceProfile::for_usb(info.vendor_id, info.product_id);
        Self::open(info, profile)
    }

    pub fn node(&self) -> &VideoNode {
        &self.node
    }

    /// Whether the camera is still there (false after unplug or reboot).
    pub fn is_connected(&self) -> bool {
        self.node.capability().is_ok()
    }
}
