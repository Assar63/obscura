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
    (
        "generic-uvc.toml",
        include_str!("../../../profiles/generic-uvc.toml"),
    ),
    (
        "tiny-se.toml",
        include_str!("../../../profiles/tiny-se.toml"),
    ),
    ("tiny-2.toml", include_str!("../../../profiles/tiny-2.toml")),
    (
        "tiny-2-lite.toml",
        include_str!("../../../profiles/tiny-2-lite.toml"),
    ),
    (
        "tiny-4k.toml",
        include_str!("../../../profiles/tiny-4k.toml"),
    ),
    ("meet-2.toml", include_str!("../../../profiles/meet-2.toml")),
    (
        "meet-se.toml",
        include_str!("../../../profiles/meet-se.toml"),
    ),
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
    /// Subtracted from the device value (after `map`), for controls whose
    /// zero is mid-range (e.g. exposure compensation 0..18 -> -9..9).
    #[serde(default)]
    pub offset: i64,
}

impl V4l2Binding {
    pub fn to_device(&self, value: i64) -> i64 {
        self.map
            .iter()
            .find(|[f, _]| *f == value)
            .map_or(value, |[_, d]| *d)
            + self.offset
    }

    pub fn to_feature(&self, value: i64) -> i64 {
        let value = value - self.offset;
        self.map
            .iter()
            .find(|[_, d]| *d == value)
            .map_or(value, |[f, _]| *f)
    }
}

fn hex_cmd<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<[u8; 2], D::Error> {
    let s = String::deserialize(d)?;
    let b = parse_hex(&s).map_err(serde::de::Error::custom)?;
    b.try_into()
        .map_err(|_| serde::de::Error::custom("cmd must be 2 bytes, e.g. \"c430\""))
}

fn hex_bytes<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Vec<u8>, D::Error> {
    parse_hex(&String::deserialize(d)?).map_err(serde::de::Error::custom)
}

fn parse_hex(s: &str) -> std::result::Result<Vec<u8>, String> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if !s.len().is_multiple_of(2) {
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
    /// Frame flags other than a plain command (0x25), e.g. 0x01 for a query
    /// or 0x05 as used by the gimbal reset.
    pub flags: Option<u8>,
    /// A fixed payload sent instead of the value (actions).
    #[serde(default, deserialize_with = "hex_opt")]
    pub payload: Option<Vec<u8>>,
}

fn hex_opt<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<Vec<u8>>, D::Error> {
    hex_bytes(d).map(Some)
}

/// A short setting (selector 6): `[id, len, value]`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShortSpec {
    pub id: u8,
    /// Constant bytes before the value (counted in `len`).
    #[serde(default, deserialize_with = "hex_bytes")]
    pub prefix: Vec<u8>,
    #[serde(default = "u8_enc")]
    pub value: ValueEncoding,
}

/// How a feature reads a setting that holds both a switch and a level,
/// e.g. the status light (0 = off, 1..3 = brightness) or auto sleep
/// (seconds, negative = off).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Level {
    /// Toggle: on when the setting is non-zero. Turning it on restores the
    /// last level.
    Switch,
    /// Toggle: on when the setting is positive. Turning it off negates the
    /// level instead of zeroing it.
    SignSwitch,
    /// The level itself, ignoring the switch: the magnitude of the setting
    /// (the last level while switched off). Writing keeps the switch state.
    Magnitude,
}

/// Readback through a framed query (selector 2). Responses are shared by
/// every feature reading the same query.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryRead {
    pub dst: u8,
    #[serde(deserialize_with = "hex_cmd")]
    pub cmd: [u8; 2],
    /// Response offset holding the value. With several, the feature is a
    /// toggle that is on when any of the bytes is non-zero.
    pub offsets: Vec<usize>,
    /// Layout of a single value: `u8` (default) or `f32`.
    #[serde(default = "u8_enc")]
    pub value: ValueEncoding,
    /// Feature value = value x scale (e.g. zoom factor x10 -> x100).
    #[serde(default = "one")]
    pub scale: i64,
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
    /// Without it (or `query`) the last written value is shown.
    pub status: Option<usize>,
    /// Readback through a query; takes precedence over `status`.
    pub query: Option<QueryRead>,
    /// Layout of the status value: `u8` (default), `u16` or `i16`.
    #[serde(default = "u8_enc")]
    pub status_value: ValueEncoding,
    /// Bits of the status byte holding the value (shifted down).
    pub status_mask: Option<u8>,
    /// For toggles whose camera value is the opposite of the label (e.g.
    /// "Disable Microphone" is 1 while the microphone is enabled).
    #[serde(default)]
    pub invert: bool,
    /// Switch/level handling for settings that combine both.
    pub level: Option<Level>,
    /// The level assumed before one has been seen (see `level`).
    pub default_level: Option<i64>,
}

/// A host-side lock (OBSBOT Center's AI lock): locking saves `restore`
/// features, then sets every `off` feature to 0; unlocking writes the saved
/// values back. The camera has no lock state of its own.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockSpec {
    pub off: Vec<FeatureId>,
    #[serde(default)]
    pub restore: Vec<FeatureId>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockBinding {
    pub lock: LockSpec,
}

/// Gimbal velocity command: payload is three float32 `[roll, pitch, yaw]`
/// in degrees/second, resent while moving and zeroed to stop.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GimbalVelocity {
    pub dst: u8,
    #[serde(deserialize_with = "hex_cmd")]
    pub cmd: [u8; 2],
    /// Multipliers mapping "right"/"up" to the device's sign convention.
    #[serde(default = "one_f")]
    pub yaw_sign: f32,
    #[serde(default = "one_f")]
    pub pitch_sign: f32,
    /// Largest speed sent, in the device's units.
    pub max_speed: f32,
}

/// Camera-side gimbal presets (position and zoom), stored in numbered slots.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Presets {
    pub slots: u32,
}

fn one_f() -> f32 {
    1.0
}

/// How a feature reaches the device.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Binding {
    V4l2(V4l2Binding),
    Vendor(VendorBinding),
    Lock(LockBinding),
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceProfile {
    pub id: String,
    pub name: String,
    #[serde(default, rename = "match")]
    pub matches: Vec<UsbMatch>,
    /// Profile whose features this one starts from.
    pub inherits: Option<String>,
    /// Vendor gimbal velocity command, used for joystick control.
    pub gimbal_velocity: Option<GimbalVelocity>,
    /// Whether the camera answers the system module's firmware version and
    /// serial number queries.
    #[serde(default)]
    pub system_info: bool,
    /// Camera-side gimbal presets.
    pub presets: Option<Presets>,
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
                if let Some(parent) = p
                    .inherits
                    .as_ref()
                    .and_then(|id| parsed.iter().find(|q| &q.id == id))
                {
                    p.system_info |= parent.system_info;
                    if p.presets.is_none() {
                        p.presets = parent.presets.clone();
                    }
                    if p.gimbal_velocity.is_none() {
                        p.gimbal_velocity = parent.gimbal_velocity.clone();
                    }
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
                if best.as_ref().is_none_or(|(s, _)| score > *s) {
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
        assert!(matches!(
            p.features[&FeatureId::MirrorImage],
            Binding::Vendor(_)
        ));
    }

    #[test]
    fn tiny_se_bindings_match_captures() {
        use crate::protocol::{encode_short, encode_value};
        let p = DeviceProfile::for_usb(0x3564, 0xfeff);
        assert!(p.system_info);
        let short = |id: FeatureId, value: i64| {
            let Binding::Vendor(VendorBinding { short: Some(s), .. }) = &p.features[&id] else {
                panic!("{id:?} is a short setting")
            };
            let mut payload = s.prefix.clone();
            payload.extend(encode_value(s.value, value, 1));
            encode_short(s.id, &payload)[..4].to_vec()
        };
        // 51-device-sleep, 53-status-light, 44-hdr
        assert_eq!(short(FeatureId::AutoSleep, -30), [0x0b, 0x02, 0xe2, 0xff]);
        assert_eq!(short(FeatureId::SleepTime, 600), [0x0b, 0x02, 0x58, 0x02]);
        assert_eq!(
            short(FeatureId::SleepBackgroundMirror, 1),
            [0x0e, 0x02, 0x02, 0x01]
        );
        assert_eq!(
            short(FeatureId::StatusLightBrightness, 3),
            [0x1a, 0x01, 0x03, 0x00]
        );
        assert_eq!(short(FeatureId::Hdr, 1), [0x01, 0x01, 0x01, 0x00]);
        assert!(matches!(p.features[&FeatureId::AiLock], Binding::Lock(_)));
    }

    #[test]
    fn v4l2_offset_round_trips() {
        let p = DeviceProfile::for_usb(0x3564, 0xfeff);
        let Binding::V4l2(b) = &p.features[&FeatureId::ExposureCompensation] else {
            panic!("exposure_compensation is a V4L2 control")
        };
        assert_eq!(b.to_device(0), 9);
        assert_eq!(b.to_feature(18), 9);
        assert_eq!(b.to_feature(0), -9);
    }

    #[test]
    fn unknown_device_falls_back_to_generic() {
        assert_eq!(DeviceProfile::for_usb(0x1234, 0x5678).id, "generic-uvc");
    }

    #[test]
    fn tiny_2_lite_assumes_tiny_se_vendor_features() {
        let p = DeviceProfile::for_usb(0x3564, 0xfef9);
        assert_eq!(p.id, "tiny-2-lite");
        assert!(matches!(p.features[&FeatureId::Contrast], Binding::V4l2(_)));
        assert!(matches!(
            p.features[&FeatureId::MirrorImage],
            Binding::Vendor(_)
        ));
    }

    #[test]
    fn tiny_2_matches_by_product_id() {
        let p = DeviceProfile::for_usb(0x3564, 0xfef8);
        assert_eq!(p.id, "tiny-2");
        assert!(p.presets.is_some());
        assert!(matches!(
            p.features[&FeatureId::MirrorImage],
            Binding::Vendor(_)
        ));
    }

    #[test]
    fn tiny_4k_matches_by_product_id() {
        let p = DeviceProfile::for_usb(0x3564, 0xfef4);
        assert_eq!(p.id, "tiny-4k");
        assert!(p.presets.is_some());
        assert!(matches!(
            p.features[&FeatureId::MirrorImage],
            Binding::Vendor(_)
        ));
    }

    #[test]
    fn meet_2_has_no_gimbal_features() {
        let p = DeviceProfile::for_usb(0x3564, 0xfefb);
        assert_eq!(p.id, "meet-2");
        assert!(p.presets.is_none());
        assert!(p.gimbal_velocity.is_none());
        assert!(!p.features.contains_key(&FeatureId::GimbalReset));
        assert!(!p.features.contains_key(&FeatureId::GimbalReverse));
        assert!(matches!(
            p.features[&FeatureId::MirrorImage],
            Binding::Vendor(_)
        ));
    }

    #[test]
    fn meet_se_has_no_gimbal_features() {
        let p = DeviceProfile::for_usb(0x3564, 0xfefe);
        assert_eq!(p.id, "meet-se");
        assert!(p.presets.is_none());
        assert!(p.gimbal_velocity.is_none());
        assert!(!p.features.contains_key(&FeatureId::GimbalReset));
        assert!(!p.features.contains_key(&FeatureId::GimbalReverse));
    }
}
