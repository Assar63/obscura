//! Control library for OBSBOT webcams on Linux.
//!
//! Features are described once in [`features`] (what the UI shows), bound to
//! transport-level controls per model in [`profile`] (how the device does it),
//! and executed by [`device::Device`] over V4L2 standard controls or UVC
//! Extension Unit queries.

pub mod companion;
pub mod device;
pub mod discovery;
pub mod error;
pub mod features;
pub mod firmware;
pub mod log;
pub mod preview;
pub mod profile;
pub mod protocol;
pub mod status;
pub mod transport;
pub mod v4l2;

pub use device::{Device, FeatureState, FirmwareInfo};
pub use discovery::{discover, CameraInfo};
pub use error::{Error, Result};
pub use features::{FeatureId, FeatureKind};
pub use profile::DeviceProfile;
pub use status::{LiveStatus, MicSlot};
