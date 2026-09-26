//! Transports that carry feature values to the camera.
//!
//! * [`v4l2_ctrl`]: standard UVC controls exposed by `uvcvideo` as V4L2
//!   controls (contrast, white balance, zoom, ...).
//! * [`uvc_xu`]: raw UVC Extension Unit queries, which is where OBSBOT's
//!   vendor features (AI tracking, gestures, ...) are expected to live.
//!   Their encodings get filled in from the USB captures (see
//!   `docs/CAPTURING.md`).

pub mod uvc_xu;
pub mod v4l2_ctrl;
