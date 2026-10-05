//! A virtual camera ("OBSCura Camera") that other apps can open while
//! OBSCura uses the real one. Linux lets only one program stream from a
//! camera, so OBSCura reads it and passes the same JPEG frames to a
//! v4l2loopback device; any number of apps, including OBSCura's own
//! preview, can then read from that.
//!
//! The device is created by the v4l2loopback kernel module, which needs
//! root to load; see [`SETUP`].
//!
//! v4l2loopback lets only one reader stream at a time, so OBSCura's own
//! preview doesn't read the virtual camera: the share process also sends
//! the frames over a local socket ([`FrameServer`], [`read_shared`]),
//! leaving the virtual camera to OBS, Teams or a browser.

use std::io::{self, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::preview::PreviewFormat;

use v4l::video::Output;
use v4l::{Format, FourCC};

/// The name the virtual camera has in other apps (`card_label`).
pub const LABEL: &str = "OBSCura Camera";

/// Video node number used by the setup commands (`/dev/video42`).
pub const VIDEO_NR: u32 = 42;

/// The module options, shared by the commands below.
pub const MODULE_OPTIONS: &str =
    "devices=1 video_nr=42 card_label=\"OBSCura Camera\" exclusive_caps=1";

/// Commands that create the virtual camera: the first loads it until the
/// next reboot, the other two make it load at every boot.
pub const SETUP: [&str; 3] = [
    "sudo modprobe v4l2loopback devices=1 video_nr=42 card_label=\"OBSCura Camera\" exclusive_caps=1",
    "echo v4l2loopback | sudo tee /etc/modules-load.d/obscura-camera.conf",
    "echo 'options v4l2loopback devices=1 video_nr=42 card_label=\"OBSCura Camera\" exclusive_caps=1' | sudo tee /etc/modprobe.d/obscura-camera.conf",
];

/// The virtual camera's video node, if the module is loaded with our label.
pub fn find() -> Option<PathBuf> {
    find_in(Path::new("/sys/class/video4linux"))
}

fn find_in(sysfs: &Path) -> Option<PathBuf> {
    let mut nodes: Vec<_> = std::fs::read_dir(sysfs)
        .ok()?
        .flatten()
        .filter(|e| std::fs::read_to_string(e.path().join("name")).is_ok_and(|n| n.trim() == LABEL))
        .map(|e| PathBuf::from("/dev").join(e.file_name()))
        .collect();
    nodes.sort();
    nodes.into_iter().next()
}

/// Feeds JPEG frames to the virtual camera.
pub struct Writer {
    /// v4l2loopback only takes frames on the handle that set the format.
    dev: v4l::Device,
}

impl Writer {
    /// Opens `path` and announces MJPEG frames of `width` x `height`.
    pub fn open(path: &Path, width: u32, height: u32) -> io::Result<Self> {
        let dev = v4l::Device::with_path(path)?;
        let mut fmt = Format::new(width, height, FourCC::new(b"MJPG"));
        // Room for the largest frame: MJPEG never exceeds raw YUYV.
        fmt.size = width * height * 2;
        Output::set_format(&dev, &fmt)?;
        Ok(Self { dev })
    }

    /// Writes one frame; readers get it as one buffer.
    pub fn write(&mut self, jpeg: &[u8]) -> io::Result<()> {
        self.dev.write_all(jpeg)
    }
}

/// The socket the share process sends its frames to OBSCura's preview on.
pub fn socket_path() -> PathBuf {
    crate::companion::runtime_dir().join("share.sock")
}

/// Sends frames to every connected preview: first the format (width,
/// height, fps as u32 LE), then each frame as a u32 LE length and the JPEG.
/// A client that can't keep up within 100 ms is dropped.
pub struct FrameServer {
    clients: Arc<Mutex<Vec<UnixStream>>>,
    format: Arc<Mutex<Option<[u8; 12]>>>,
}

impl FrameServer {
    pub fn bind(path: &Path) -> io::Result<Self> {
        let _ = std::fs::remove_file(path);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let listener = UnixListener::bind(path)?;
        let clients: Arc<Mutex<Vec<UnixStream>>> = Arc::default();
        let format: Arc<Mutex<Option<[u8; 12]>>> = Arc::default();
        {
            let (clients, format) = (clients.clone(), format.clone());
            std::thread::Builder::new()
                .name("share-socket".into())
                .spawn(move || {
                    for mut c in listener.incoming().flatten() {
                        let _ = c.set_write_timeout(Some(Duration::from_millis(100)));
                        let header = *format.lock().unwrap();
                        if header.is_none_or(|h| c.write_all(&h).is_ok()) {
                            clients.lock().unwrap().push(c);
                        }
                    }
                })?;
        }
        Ok(Self { clients, format })
    }

    /// Announces the format to new and connected clients.
    pub fn set_format(&self, fmt: PreviewFormat) {
        let mut h = [0u8; 12];
        h[..4].copy_from_slice(&fmt.width.to_le_bytes());
        h[4..8].copy_from_slice(&fmt.height.to_le_bytes());
        h[8..].copy_from_slice(&fmt.fps.to_le_bytes());
        *self.format.lock().unwrap() = Some(h);
        self.clients
            .lock()
            .unwrap()
            .retain_mut(|c| c.write_all(&h).is_ok());
    }

    pub fn send(&self, jpeg: &[u8]) {
        let len = (jpeg.len() as u32).to_le_bytes();
        self.clients
            .lock()
            .unwrap()
            .retain_mut(|c| c.write_all(&len).and_then(|_| c.write_all(jpeg)).is_ok());
    }
}

/// Reads the share process's frames, like `preview::run` reads a camera.
pub fn read_shared(
    path: &Path,
    stop: &AtomicBool,
    on_start: impl FnOnce(PreviewFormat),
    mut on_frame: impl FnMut(&[u8]) -> bool,
) -> io::Result<()> {
    let mut s = UnixStream::connect(path)?;
    // Wake up now and then to notice `stop` even when no frames come.
    s.set_read_timeout(Some(Duration::from_millis(500)))?;
    let mut word = [0u8; 4];
    let mut read_word = |s: &mut UnixStream| -> io::Result<Option<u32>> {
        loop {
            match s.read_exact(&mut word) {
                Ok(()) => return Ok(Some(u32::from_le_bytes(word))),
                Err(e)
                    if e.kind() == io::ErrorKind::WouldBlock
                        || e.kind() == io::ErrorKind::TimedOut =>
                {
                    if stop.load(Ordering::Relaxed) {
                        return Ok(None);
                    }
                }
                Err(e) => return Err(e),
            }
        }
    };
    let (Some(width), Some(height), Some(fps)) =
        (read_word(&mut s)?, read_word(&mut s)?, read_word(&mut s)?)
    else {
        return Ok(());
    };
    on_start(PreviewFormat { width, height, fps });
    let mut frame = Vec::new();
    while let Some(len) = read_word(&mut s)? {
        frame.resize(len as usize, 0);
        s.read_exact(&mut frame)?;
        if !on_frame(&frame) || stop.load(Ordering::Relaxed) {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_labelled_node() {
        let dir = std::env::temp_dir().join(format!("obscura-vcam-{}", std::process::id()));
        for (node, name) in [("video5", "OBSBOT Tiny 3"), ("video42", LABEL)] {
            std::fs::create_dir_all(dir.join(node)).unwrap();
            std::fs::write(dir.join(node).join("name"), format!("{name}\n")).unwrap();
        }
        assert_eq!(find_in(&dir), Some(PathBuf::from("/dev/video42")));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn setup_commands_use_the_module_options() {
        assert!(SETUP[0].ends_with(MODULE_OPTIONS));
        assert!(SETUP[2].contains(MODULE_OPTIONS));
        assert!(MODULE_OPTIONS.contains(&format!("video_nr={VIDEO_NR}")));
    }
}
