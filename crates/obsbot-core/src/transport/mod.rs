//! How OBSCura talks to the camera's hardware (no OBSBOT knowledge here;
//! that's in `vendor`):
//!
//! * [`v4l2`]: the video device node and its ioctls.
//! * [`v4l2_ctrl`]: standard UVC controls exposed by `uvcvideo` as V4L2
//!   controls (contrast, white balance, zoom, ...).
//! * [`uvc_xu`]: raw UVC Extension Unit queries, which carry OBSBOT's vendor
//!   frames and status block.
//! * [`audio`]: the capture level and switch of the camera's USB audio,
//!   through ALSA.

pub mod audio;
pub mod uvc_xu;
pub mod v4l2;
pub mod v4l2_ctrl;
