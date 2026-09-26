//! Finds cameras through sysfs (`/sys/class/video4linux`).

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::profile::OBSBOT_VENDOR_IDS;
use crate::v4l2::VideoNode;

#[derive(Debug, Clone, Serialize)]
pub struct CameraInfo {
    /// Video capture node, e.g. `/dev/video0`.
    pub path: PathBuf,
    /// V4L2 card name.
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial: Option<String>,
    /// USB `bcdDevice`, formatted like `x.yz`; often tracks firmware.
    pub usb_version: Option<String>,
    /// Stable-ish sysfs USB path (e.g. `3-2`), used to tell devices apart.
    pub usb_path: String,
    pub is_obsbot: bool,
}

fn read(dir: &Path, file: &str) -> Option<String> {
    fs::read_to_string(dir.join(file)).ok().map(|s| s.trim().to_string())
}

fn read_hex(dir: &Path, file: &str) -> Option<u16> {
    u16::from_str_radix(&read(dir, file)?, 16).ok()
}

/// uvcvideo names nodes "<product>: <product, truncated>"; keep the first
/// part. This is what's shown when the USB product string can't be read
/// (e.g. under snap confinement, which only exposes the USB IDs).
fn card_name(raw: &str) -> String {
    raw.split(':').next().unwrap_or(raw).trim().to_string()
}

fn node_number(path: &Path) -> u32 {
    path.file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.trim_start_matches("video").parse().ok())
        .unwrap_or(u32::MAX)
}

/// Whether a node is the camera's video capture node (as opposed to the
/// metadata node uvcvideo also creates).
fn is_capture_node(sys: &Path, dev: &Path) -> bool {
    match VideoNode::open(dev).and_then(|n| n.is_video_capture()) {
        Ok(capture) => capture,
        // Can't open it (permissions); fall back to the sysfs index.
        Err(_) => read(sys, "index").as_deref() == Some("0"),
    }
}

/// Lists USB cameras, one entry per physical device. OBSBOT cameras only,
/// unless `include_all` is set (useful for testing with other webcams).
pub fn discover(include_all: bool) -> Vec<CameraInfo> {
    let Ok(entries) = fs::read_dir("/sys/class/video4linux") else {
        return Vec::new();
    };
    let mut nodes: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    nodes.sort_by_key(|p| node_number(p));

    let mut cameras: Vec<CameraInfo> = Vec::new();
    for sys in nodes {
        // `device` points at the USB interface; its parent is the USB device.
        let Ok(iface) = fs::canonicalize(sys.join("device")) else { continue };
        let Some(usb) = iface.parent() else { continue };
        let (Some(vendor_id), Some(product_id)) = (read_hex(usb, "idVendor"), read_hex(usb, "idProduct")) else {
            continue;
        };
        let usb_path = usb.file_name().unwrap_or_default().to_string_lossy().into_owned();
        if cameras.iter().any(|c| c.usb_path == usb_path) {
            continue;
        }
        let is_obsbot = OBSBOT_VENDOR_IDS.contains(&vendor_id);
        if !is_obsbot && !include_all {
            continue;
        }
        let dev = Path::new("/dev").join(sys.file_name().unwrap());
        if !is_capture_node(&sys, &dev) {
            continue;
        }
        cameras.push(CameraInfo {
            path: dev,
            name: card_name(&read(&sys, "name").unwrap_or_default()),
            vendor_id,
            product_id,
            manufacturer: read(usb, "manufacturer"),
            product: read(usb, "product"),
            serial: read(usb, "serial"),
            usb_version: read_hex(usb, "bcdDevice")
                .map(|b| format!("{}.{:02x}", b >> 8, b & 0xff)),
            usb_path,
            is_obsbot,
        });
    }
    cameras
}

#[cfg(test)]
mod tests {
    #[test]
    fn card_name_drops_uvc_suffix() {
        assert_eq!(super::card_name("OBSBOT Tiny SE: OBSBOT Tiny S"), "OBSBOT Tiny SE");
        assert_eq!(super::card_name("Integrated RGB Camera: Integrat"), "Integrated RGB Camera");
        assert_eq!(super::card_name("Plain"), "Plain");
    }
}
