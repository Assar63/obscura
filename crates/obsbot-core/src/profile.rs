//! Per-model device profiles: which catalog features a camera supports and
//! how each one maps onto a transport. Profiles live in `profiles/*.toml` and
//! are compiled in; adding a model means adding a TOML file.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::features::FeatureId;
use crate::protocol::ValueEncoding;

/// USB vendor IDs used by OBSBOT (Remo Tech). To be confirmed against the
/// Tiny SE's descriptors from the captures.
pub const OBSBOT_VENDOR_IDS: &[u16] = &[0x3564, 0x6e30];

const BUILTIN: &[(&str, &str)] = &[
    ("generic-uvc.toml", include_str!("../../../profiles/generic-uvc.toml")),
    ("tiny-se.toml", include_str!("../../../profiles/tiny-se.toml")),
];

#[derive(Debug, Clone, Deserialize)]
pub struct UsbMatch {
    pub vendor_id: u16,
    pub product_id: Option<u16>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct V4l2Binding {
    /// V4L2 control ID (`V4L2_CID_*`).
    pub v4l2: u32,
    /// `[feature_value, device_value]` pairs for controls whose device values
    /// differ from the catalog's (e.g. exposure_auto: on = 3, off = 1).
    #[serde(default)]
    pub map: Vec<[i64; 2]>,
    /// Override the catalog's display scale for ranges.
    pub scale: Option<i64>,
    /// Override the catalog's unit; `""` removes it.
    pub unit: Option<String>,
}

impl V4l2Binding {
    pub fn to_device(&self, value: i64) -> i64 {
        self.map.iter().find(|[f, _]| *f == value).map_or(value, |[_, d]| *d)
    }

    pub fn to_feature(&self, value: i64) -> i64 {
        self.map.iter().find(|[_, d]| *d == value).map_or(value, |[f, _]| *f)
    }
}

fn hex_cmd<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<[u8; 2], D::Error> {
    let s = String::deserialize(d)?;
    let b = parse_hex(&s).map_err(serde::de::Error::custom)?;
    b.try_into().map_err(|_| serde::de::Error::custom("cmd must be 2 bytes, e.g. \"c430\""))
}

fn hex_bytes<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Vec<u8>, D::Error> {
    parse_hex(&String::deserialize(d)?).map_err(serde::de::Error::custom)
}

fn parse_hex(s: &str) -> std::result::Result<Vec<u8>, String> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if s.len() % 2 != 0 {
        return Err(format!("odd-length hex `{s}`"));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

fn one() -> i64 {
    1
}

fn u8_enc() -> ValueEncoding {
    ValueEncoding::U8
}

/// One framed command (selector 2) sent when the feature is written.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameSpec {
    pub dst: u8,
    #[serde(deserialize_with = "hex_cmd")]
    pub cmd: [u8; 2],
    /// Constant bytes before the value.
    #[serde(default, deserialize_with = "hex_bytes")]
    pub prefix: Vec<u8>,
    #[serde(default = "u8_enc")]
    pub value: ValueEncoding,
    /// For `f32`: the float sent is `value / divisor`.
    #[serde(default = "one")]
    pub divisor: i64,
}

/// A short setting (selector 6): `[id, len, value]`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShortSpec {
    pub id: u8,
    #[serde(default = "u8_enc")]
    pub value: ValueEncoding,
}

/// OBSBOT vendor feature over the UVC Extension Unit.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VendorBinding {
    /// Framed commands, sent in order.
    #[serde(default)]
    pub frames: Vec<FrameSpec>,
    pub short: Option<ShortSpec>,
    /// Byte offset in the selector 6 status block holding the current value.
    /// Without it the last written value is shown.
    pub status: Option<usize>,
}

/// How a feature reaches the device.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Binding {
    V4l2(V4l2Binding),
    Vendor(VendorBinding),
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceProfile {
    pub id: String,
    pub name: String,
    #[serde(default, rename = "match")]
    pub matches: Vec<UsbMatch>,
    /// Profile whose features this one starts from.
    pub inherits: Option<String>,
    #[serde(default)]
    pub features: BTreeMap<FeatureId, Binding>,
}

impl DeviceProfile {
    pub fn parse(name: &str, text: &str) -> Result<Self> {
        toml::from_str(text).map_err(|e| Error::Profile {
            name: name.to_string(),
            msg: e.to_string(),
        })
    }

    pub fn builtin() -> Vec<DeviceProfile> {
        let parsed: Vec<DeviceProfile> = BUILTIN
            .iter()
            .map(|(name, text)| Self::parse(name, text).expect("builtin profile must parse"))
            .collect();
        parsed
            .iter()
            .map(|p| {
                let mut p = p.clone();
                if let Some(parent) = p.inherits.as_ref().and_then(|id| parsed.iter().find(|q| &q.id == id)) {
                    for (id, b) in &parent.features {
                        p.features.entry(*id).or_insert_with(|| b.clone());
                    }
                }
                p
            })
            .collect()
    }

    /// The generic profile applies standard UVC controls to any camera.
    pub fn generic() -> DeviceProfile {
        Self::builtin()
            .into_iter()
            .find(|p| p.id == "generic-uvc")
            .expect("generic-uvc profile is builtin")
    }

    /// Best profile for a USB device: an exact product match wins over a
    /// vendor-only match, which wins over the generic fallback.
    pub fn for_usb(vendor_id: u16, product_id: u16) -> DeviceProfile {
        let mut best: Option<(u8, DeviceProfile)> = None;
        for p in Self::builtin() {
            for m in &p.matches {
                let score = match m.product_id {
                    Some(pid) if m.vendor_id == vendor_id && pid == product_id => 2,
                    None if m.vendor_id == vendor_id => 1,
                    _ => continue,
                };
                if best.as_ref().map_or(true, |(s, _)| score > *s) {
                    best = Some((score, p.clone()));
                }
            }
        }
        best.map_or_else(Self::generic, |(_, p)| p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_profiles_parse() {
        let generic = DeviceProfile::generic();
        assert!(generic.features.contains_key(&FeatureId::Contrast));
    }

    #[test]
    fn value_map_round_trips() {
        let Binding::V4l2(b) = &DeviceProfile::generic().features[&FeatureId::AutoExposure] else {
            panic!("auto_exposure is a V4L2 control")
        };
        assert_eq!(b.to_device(1), 3);
        assert_eq!(b.to_feature(1), 0);
        assert_eq!(b.to_feature(b.to_device(0)), 0);
    }

    #[test]
    fn tiny_se_inherits_standard_controls() {
        let p = DeviceProfile::for_usb(0x3564, 0xfeff);
        assert_eq!(p.id, "tiny-se");
        assert!(matches!(p.features[&FeatureId::Contrast], Binding::V4l2(_)));
        assert!(matches!(p.features[&FeatureId::MirrorImage], Binding::Vendor(_)));
    }

    #[test]
    fn unknown_device_falls_back_to_generic() {
        assert_eq!(DeviceProfile::for_usb(0x1234, 0x5678).id, "generic-uvc");
    }
}
