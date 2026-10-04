//! Minimal raw V4L2 / uvcvideo ioctl bindings.
//!
//! Only what a control tool needs: capability query, control enumeration and
//! get/set, and the uvcvideo Extension Unit query. Structs mirror
//! `<linux/videodev2.h>` and `<linux/uvcvideo.h>`.

use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use crate::error::{Error, Result};

pub const CAP_VIDEO_CAPTURE: u32 = 0x0000_0001;
pub const CAP_DEVICE_CAPS: u32 = 0x8000_0000;

pub const CTRL_FLAG_DISABLED: u32 = 0x0001;
pub const CTRL_FLAG_READ_ONLY: u32 = 0x0004;
pub const CTRL_FLAG_INACTIVE: u32 = 0x0010;
pub const CTRL_FLAG_NEXT_CTRL: u32 = 0x8000_0000;

pub const CTRL_TYPE_INTEGER: u32 = 1;
pub const CTRL_TYPE_BOOLEAN: u32 = 2;
pub const CTRL_TYPE_MENU: u32 = 3;
pub const CTRL_TYPE_BUTTON: u32 = 4;
pub const CTRL_TYPE_CTRL_CLASS: u32 = 6;
pub const CTRL_TYPE_INTEGER_MENU: u32 = 9;

/// Standard V4L2 control IDs used by UVC cameras.
pub mod cid {
    pub const BRIGHTNESS: u32 = 0x0098_0900;
    pub const CONTRAST: u32 = 0x0098_0901;
    pub const SATURATION: u32 = 0x0098_0902;
    pub const HUE: u32 = 0x0098_0903;
    pub const AUTO_WHITE_BALANCE: u32 = 0x0098_090c;
    pub const GAIN: u32 = 0x0098_0913;
    pub const POWER_LINE_FREQUENCY: u32 = 0x0098_0918;
    pub const WHITE_BALANCE_TEMPERATURE: u32 = 0x0098_091a;
    pub const SHARPNESS: u32 = 0x0098_091b;
    pub const EXPOSURE_AUTO: u32 = 0x009a_0901;
    pub const EXPOSURE_ABSOLUTE: u32 = 0x009a_0902;
    pub const PAN_ABSOLUTE: u32 = 0x009a_0908;
    pub const TILT_ABSOLUTE: u32 = 0x009a_0909;
    pub const FOCUS_ABSOLUTE: u32 = 0x009a_090a;
    pub const FOCUS_AUTO: u32 = 0x009a_090c;
    pub const ZOOM_ABSOLUTE: u32 = 0x009a_090d;
}

#[repr(C)]
pub struct Capability {
    pub driver: [u8; 16],
    pub card: [u8; 32],
    pub bus_info: [u8; 32],
    pub version: u32,
    pub capabilities: u32,
    pub device_caps: u32,
    pub reserved: [u32; 3],
}

#[repr(C)]
#[derive(Clone)]
pub struct QueryCtrl {
    pub id: u32,
    pub type_: u32,
    pub name: [u8; 32],
    pub minimum: i32,
    pub maximum: i32,
    pub step: i32,
    pub default_value: i32,
    pub flags: u32,
    pub reserved: [u32; 2],
}

#[repr(C, packed)]
pub struct QueryMenu {
    pub id: u32,
    pub index: u32,
    /// Union of `name[32]` and `__s64 value`.
    pub name: [u8; 32],
    pub reserved: u32,
}

#[repr(C)]
pub struct Control {
    pub id: u32,
    pub value: i32,
}

/// `struct uvc_xu_control_query`
#[repr(C)]
pub struct UvcXuControlQuery {
    pub unit: u8,
    pub selector: u8,
    pub query: u8,
    pub size: u16,
    pub data: *mut u8,
}

/// UVC request codes for [`UvcXuControlQuery::query`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UvcQuery {
    SetCur = 0x01,
    GetCur = 0x81,
    GetMin = 0x82,
    GetMax = 0x83,
    GetRes = 0x84,
    GetLen = 0x85,
    GetInfo = 0x86,
    GetDef = 0x87,
}

mod ioctls {
    use super::*;
    nix::ioctl_read!(querycap, b'V', 0, Capability);
    nix::ioctl_readwrite!(g_ctrl, b'V', 27, Control);
    nix::ioctl_readwrite!(s_ctrl, b'V', 28, Control);
    nix::ioctl_readwrite!(queryctrl, b'V', 36, QueryCtrl);
    nix::ioctl_readwrite!(querymenu, b'V', 37, QueryMenu);
    nix::ioctl_readwrite!(uvc_ctrl_query, b'u', 0x21, UvcXuControlQuery);
}

pub fn cstr(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// Description of a V4L2 control as reported by the driver.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ControlInfo {
    pub id: u32,
    pub name: String,
    pub type_: u32,
    pub min: i64,
    pub max: i64,
    pub step: i64,
    pub default: i64,
    pub flags: u32,
    /// (value, label) for menu controls.
    pub menu: Vec<(i64, String)>,
}

impl ControlInfo {
    pub fn is_disabled(&self) -> bool {
        self.flags & CTRL_FLAG_DISABLED != 0
    }
    pub fn is_inactive(&self) -> bool {
        self.flags & CTRL_FLAG_INACTIVE != 0
    }
}

/// An open `/dev/videoN` node.
pub struct VideoNode {
    file: File,
}

impl VideoNode {
    /// Opens non-blocking and read/write. Control ioctls work while another
    /// process is streaming from the same node.
    pub fn open(path: &Path) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(nix::libc::O_NONBLOCK)
            .open(path)
            .map_err(|source| Error::Io {
                path: path.to_path_buf(),
                source,
            })?;
        Ok(Self { file })
    }

    fn fd(&self) -> i32 {
        self.file.as_raw_fd()
    }

    pub fn capability(&self) -> Result<Capability> {
        let mut cap: Capability = unsafe { std::mem::zeroed() };
        unsafe { ioctls::querycap(self.fd(), &mut cap) }.map_err(|source| Error::Ioctl {
            op: "VIDIOC_QUERYCAP",
            source,
        })?;
        Ok(cap)
    }

    pub fn is_video_capture(&self) -> Result<bool> {
        let cap = self.capability()?;
        let caps = if cap.capabilities & CAP_DEVICE_CAPS != 0 {
            cap.device_caps
        } else {
            cap.capabilities
        };
        Ok(caps & CAP_VIDEO_CAPTURE != 0)
    }

    pub fn query_control(&self, id: u32) -> Result<Option<ControlInfo>> {
        let mut q: QueryCtrl = unsafe { std::mem::zeroed() };
        q.id = id;
        match unsafe { ioctls::queryctrl(self.fd(), &mut q) } {
            Ok(_) => Ok(Some(self.control_info(&q))),
            Err(nix::Error::EINVAL) => Ok(None),
            Err(source) => Err(Error::Ioctl {
                op: "VIDIOC_QUERYCTRL",
                source,
            }),
        }
    }

    /// Enumerates every control the driver exposes (excluding class headers).
    pub fn list_controls(&self) -> Result<Vec<ControlInfo>> {
        let mut out = Vec::new();
        let mut q: QueryCtrl = unsafe { std::mem::zeroed() };
        q.id = CTRL_FLAG_NEXT_CTRL;
        loop {
            match unsafe { ioctls::queryctrl(self.fd(), &mut q) } {
                Ok(_) => {
                    if q.type_ != CTRL_TYPE_CTRL_CLASS {
                        out.push(self.control_info(&q));
                    }
                    q.id |= CTRL_FLAG_NEXT_CTRL;
                }
                Err(nix::Error::EINVAL) => break,
                Err(source) => {
                    return Err(Error::Ioctl {
                        op: "VIDIOC_QUERYCTRL",
                        source,
                    })
                }
            }
        }
        Ok(out)
    }

    fn control_info(&self, q: &QueryCtrl) -> ControlInfo {
        let mut menu = Vec::new();
        if q.type_ == CTRL_TYPE_MENU || q.type_ == CTRL_TYPE_INTEGER_MENU {
            for index in q.minimum..=q.maximum {
                let mut m: QueryMenu = unsafe { std::mem::zeroed() };
                m.id = q.id;
                m.index = index as u32;
                if unsafe { ioctls::querymenu(self.fd(), &mut m) }.is_ok() {
                    let label = if q.type_ == CTRL_TYPE_MENU {
                        cstr(&{ m.name })
                    } else {
                        let raw = { m.name };
                        i64::from_ne_bytes(raw[..8].try_into().unwrap()).to_string()
                    };
                    menu.push((index as i64, label));
                }
            }
        }
        ControlInfo {
            id: q.id,
            name: cstr(&q.name),
            type_: q.type_,
            min: q.minimum as i64,
            max: q.maximum as i64,
            step: q.step as i64,
            default: q.default_value as i64,
            flags: q.flags,
            menu,
        }
    }

    pub fn get_control(&self, id: u32) -> Result<i64> {
        let mut c = Control { id, value: 0 };
        unsafe { ioctls::g_ctrl(self.fd(), &mut c) }.map_err(|source| Error::Ioctl {
            op: "VIDIOC_G_CTRL",
            source,
        })?;
        Ok(c.value as i64)
    }

    pub fn set_control(&self, id: u32, value: i64) -> Result<()> {
        let mut c = Control {
            id,
            value: value as i32,
        };
        unsafe { ioctls::s_ctrl(self.fd(), &mut c) }.map_err(|source| Error::Ioctl {
            op: "VIDIOC_S_CTRL",
            source,
        })?;
        Ok(())
    }

    /// Raw UVC Extension Unit query. `data` must be exactly the control's
    /// length (use [`UvcQuery::GetLen`] with a 2-byte buffer to find it).
    pub fn xu_query(&self, unit: u8, selector: u8, query: UvcQuery, data: &mut [u8]) -> Result<()> {
        let mut q = UvcXuControlQuery {
            unit,
            selector,
            query: query as u8,
            size: data.len() as u16,
            data: data.as_mut_ptr(),
        };
        unsafe { ioctls::uvc_ctrl_query(self.fd(), &mut q) }.map_err(|source| Error::Ioctl {
            op: "UVCIOC_CTRL_QUERY",
            source,
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn struct_layouts_match_kernel_abi() {
        assert_eq!(std::mem::size_of::<Capability>(), 104);
        assert_eq!(std::mem::size_of::<QueryCtrl>(), 68);
        assert_eq!(std::mem::size_of::<QueryMenu>(), 44);
        assert_eq!(std::mem::size_of::<Control>(), 8);
        #[cfg(target_pointer_width = "64")]
        assert_eq!(std::mem::size_of::<UvcXuControlQuery>(), 16);
    }
}
