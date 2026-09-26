//! Live MJPEG preview capture.
//!
//! Opens its own handle on the video node, so control access through
//! [`Device`](crate::Device) keeps working. Only one process can stream from
//! a camera at a time; if another app is using it, starting fails with
//! [`PreviewError::Busy`].

use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use v4l::buffer::Type;
use v4l::io::mmap::Stream;
use v4l::io::traits::CaptureStream;
use v4l::video::capture::Parameters;
use v4l::video::Capture;
use v4l::FourCC;

#[derive(Debug, Clone, Copy, serde::Deserialize)]
pub struct PreviewConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

/// The format the camera actually agreed to.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct PreviewFormat {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum PreviewError {
    #[error("the camera is in use by another application")]
    Busy,
    #[error("the camera doesn't offer MJPEG at {0}x{1}")]
    NoMjpeg(u32, u32),
    #[error("preview failed: {0}")]
    Io(#[from] io::Error),
}

fn map_busy(e: io::Error) -> PreviewError {
    if e.raw_os_error() == Some(nix::libc::EBUSY) {
        PreviewError::Busy
    } else {
        PreviewError::Io(e)
    }
}

/// Streams JPEG frames to `on_frame` until `stop` is set, `on_frame`
/// returns false, or the camera goes away. `on_start` is called once the
/// format has been negotiated.
pub fn run(
    path: &Path,
    cfg: PreviewConfig,
    stop: &AtomicBool,
    on_start: impl FnOnce(PreviewFormat),
    mut on_frame: impl FnMut(&[u8]) -> bool,
) -> Result<(), PreviewError> {
    let dev = v4l::Device::with_path(path)?;
    let mjpg = FourCC::new(b"MJPG");

    let mut fmt = dev.format()?;
    fmt.width = cfg.width;
    fmt.height = cfg.height;
    fmt.fourcc = mjpg;
    let fmt = dev.set_format(&fmt).map_err(map_busy)?;
    if fmt.fourcc != mjpg {
        return Err(PreviewError::NoMjpeg(cfg.width, cfg.height));
    }
    let params = dev.set_params(&Parameters::with_fps(cfg.fps)).map_err(map_busy)?;
    let fps = if params.interval.numerator > 0 {
        params.interval.denominator / params.interval.numerator
    } else {
        cfg.fps
    };

    let mut stream = Stream::with_buffers(&dev, Type::VideoCapture, 4).map_err(map_busy)?;
    on_start(PreviewFormat { width: fmt.width, height: fmt.height, fps });

    while !stop.load(Ordering::Relaxed) {
        let (buf, meta) = stream.next().map_err(map_busy)?;
        let used = (meta.bytesused as usize).min(buf.len());
        // Skip truncated frames (a JPEG starts with FFD8).
        if used < 4 || buf[0] != 0xff || buf[1] != 0xd8 {
            continue;
        }
        if !on_frame(&buf[..used]) {
            break;
        }
    }
    Ok(())
}
