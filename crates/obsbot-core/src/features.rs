//! The catalog of user-facing features, independent of any device model.
//!
//! A feature describes *what* the UI can show (a toggle, a choice, a range).
//! Whether a given camera supports it, and how, is decided by its
//! [`DeviceProfile`](crate::profile::DeviceProfile) binding.
//!
//! Values are always `i64` on the wire; `Range::scale` turns them into
//! display units (e.g. zoom 150 with scale 100 is shown as `1.50x`).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChoiceOption {
    pub value: i64,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FeatureKind {
    Toggle,
    Choice {
        options: Vec<ChoiceOption>,
    },
    Range {
        min: i64,
        max: i64,
        step: i64,
        default: Option<i64>,
        /// Divide the raw value by this to get the display value.
        scale: i64,
        unit: Option<String>,
    },
    /// A one-shot command (e.g. gimbal reset) with no readable state.
    Action,
}

fn choice(opts: &[(i64, &str)]) -> FeatureKind {
    FeatureKind::Choice {
        options: opts
            .iter()
            .map(|&(value, label)| ChoiceOption {
                value,
                label: label.to_string(),
            })
            .collect(),
    }
}

fn range(min: i64, max: i64, scale: i64, unit: Option<&str>) -> FeatureKind {
    FeatureKind::Range {
        min,
        max,
        step: 1,
        default: None,
        scale,
        unit: unit.map(str::to_string),
    }
}

macro_rules! features {
    ($($variant:ident $name:literal => $group:literal, $label:literal, $kind:expr;)*) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub enum FeatureId { $(#[serde(rename = $name)] $variant),* }

        impl FeatureId {
            pub const ALL: &'static [FeatureId] = &[$(FeatureId::$variant),*];

            /// The snake_case name used by the CLI, profiles and the frontend.
            pub fn name(self) -> &'static str {
                match self { $(FeatureId::$variant => $name),* }
            }

            /// Default presentation of this feature. Profiles and live device
            /// queries may refine ranges/options.
            pub fn def(self) -> FeatureDef {
                match self {
                    $(FeatureId::$variant => FeatureDef {
                        id: self,
                        group: $group,
                        label: $label,
                        kind: $kind,
                    }),*
                }
            }
        }
    };
}

// Options marked "seen" come from the OBSBOT Center screenshots; lists with a
// single entry are only partially known and get completed from the captures.
features! {
    // Header quick row
    GestureControl        "gesture_control" => "gesture", "Gesture Control", FeatureKind::Toggle;
    GestureLockedTarget   "gesture_locked_target" => "gesture", "Locked Target", FeatureKind::Toggle;
    GestureZoom           "gesture_zoom" => "gesture", "Zoom", FeatureKind::Toggle;
    GestureZoomFactor     "gesture_zoom_factor" => "gesture", "Zoom Factor", range(100, 400, 100, Some("x"));
    GestureDynamicZoom    "gesture_dynamic_zoom" => "gesture", "Dynamic Zoom", FeatureKind::Toggle;
    GestureDirectionFlip  "gesture_direction_flip" => "gesture", "Direction Flip", FeatureKind::Toggle;
    MirrorImage           "mirror_image" => "header", "Mirror Image", FeatureKind::Toggle;
    Sleep                 "sleep" => "header", "Sleep", FeatureKind::Toggle;

    // AI tracking (bottom bar)
    AiMode                "ai_mode" => "ai", "AI Mode", choice(&[(0, "Off"), (2, "Human"), (1, "Group"), (3, "Hand Tracking")]);
    AiLock                "ai_lock" => "ai", "AI Lock", FeatureKind::Toggle;
    TrackingSpeed         "tracking_speed" => "ai", "Tracking Speed", choice(&[(0, "Very Slow"), (1, "Slow"), (2, "Normal"), (3, "Fast"), (4, "Very Fast")]);
    TrackingMotion        "tracking_motion" => "ai", "Motion Mode", FeatureKind::Toggle;

    // View and gimbal
    GimbalSpeed           "gimbal_speed" => "gimbal", "Gimbal Speed", choice(&[(0, "Slow"), (1, "Med."), (2, "Fast")]);
    Pan                   "pan" => "gimbal", "Pan", range(-100, 100, 1, None);
    Tilt                  "tilt" => "gimbal", "Tilt", range(-100, 100, 1, None);
    Zoom                  "zoom" => "gimbal", "Manual Zoom", range(100, 400, 100, Some("x"));
    FieldOfView           "field_of_view" => "gimbal", "Field of View", choice(&[(0, "86°"), (1, "78°"), (2, "65°")]);
    GimbalReset           "gimbal_reset" => "gimbal", "Reset View", FeatureKind::Action;
    GimbalReverse         "gimbal_reverse" => "gimbal", "View and Gimbal Reverse", FeatureKind::Toggle;

    // Image adjustment
    Hdr                   "hdr" => "image", "HDR", FeatureKind::Toggle;
    AutoFocus             "auto_focus" => "image", "Auto Focus", FeatureKind::Toggle;
    AutoFocusMode         "auto_focus_mode" => "image", "Auto Focus Mode", choice(&[(0, "Global"), (1, "Face")]);
    Focus                 "focus" => "image", "Focus", range(0, 100, 1, None);
    AutoExposure          "auto_exposure" => "image", "Auto Exposure", FeatureKind::Toggle;
    AutoExposureMode      "auto_exposure_mode" => "image", "Auto Exposure Mode", choice(&[(0, "Global"), (1, "Face")]);
    ExposureCompensation  "exposure_compensation" => "image", "Compensation", range(-3, 3, 1, None);
    Exposure              "exposure" => "image", "Exposure", range(1, 10000, 1, None);
    Gain                  "gain" => "image", "Gain", range(0, 100, 1, None);
    AntiFlicker           "anti_flicker" => "image", "Anti-Flicker", choice(&[(0, "Off"), (1, "50 Hz"), (2, "60 Hz")]);
    AutoWhiteBalance      "auto_white_balance" => "image", "Auto WB", FeatureKind::Toggle;
    WhiteBalanceTemperature "white_balance_temperature" => "image", "Temperature", range(2000, 10000, 1, Some("K"));
    Brightness            "brightness" => "image", "Brightness", range(0, 100, 1, None);
    Contrast              "contrast" => "image", "Contrast", range(0, 100, 1, None);
    Saturation            "saturation" => "image", "Saturation", range(0, 100, 1, None);
    Sharpness             "sharpness" => "image", "Sharpness", range(0, 100, 1, None);
    Hue                   "hue" => "image", "Hue", range(0, 100, 1, None);

    // Audio
    MicDuringSleep        "mic_during_sleep" => "audio", "Microphone Status During Sleep", FeatureKind::Toggle;
    NoiseReduction        "noise_reduction" => "audio", "Noise Reduction", choice(&[(0, "Off"), (1, "Low"), (2, "Medium"), (3, "High")]);
    AutoGain              "auto_gain" => "audio", "Auto Gain", FeatureKind::Toggle;
    DisableMicrophone     "disable_microphone" => "audio", "Disable Microphone", FeatureKind::Toggle;
    PickupDistance        "pickup_distance" => "audio", "Radio Distance", choice(&[(0, "Close"), (1, "Standard"), (2, "Far")]);
    MicCapture            "mic_capture" => "audio", "Microphone On", FeatureKind::Toggle;
    MicLevel              "mic_level" => "audio", "Level", range(-50, 0, 1, Some(" dB"));

    // Wireless microphones (Vox SE, Tiny 3 series): two slots, TX1 and TX2
    AudioSource           "audio_source" => "wireless", "Audio Source", choice(&[(0, "Built-in"), (3, "Wireless Mic")]);
    AudioAutoSelect       "audio_auto_select" => "wireless", "Auto-select Wireless Mic", FeatureKind::Toggle;
    MicPairTx1            "mic_pair_tx1" => "wireless", "Pair TX1", FeatureKind::Action;
    MicPairTx2            "mic_pair_tx2" => "wireless", "Pair TX2", FeatureKind::Action;
    MicPairStop           "mic_pair_stop" => "wireless", "Stop Pairing", FeatureKind::Action;
    MicForgetTx1          "mic_forget_tx1" => "wireless", "Forget TX1", FeatureKind::Action;
    MicForgetTx2          "mic_forget_tx2" => "wireless", "Forget TX2", FeatureKind::Action;
    MicTx1Mute            "mic_tx1_mute" => "wireless", "TX1 Mute", FeatureKind::Toggle;
    MicTx1Gain            "mic_tx1_gain" => "wireless", "TX1 Gain", range(-12, 12, 1, None);
    MicTx2Mute            "mic_tx2_mute" => "wireless", "TX2 Mute", FeatureKind::Toggle;
    MicTx2Gain            "mic_tx2_gain" => "wireless", "TX2 Gain", range(-12, 12, 1, None);

    // Voice control (Tiny 3 series)
    VoiceHiTiny           "voice_hi_tiny" => "voice", "\"Hi, Tiny\" (wake up)", FeatureKind::Toggle;
    VoiceSleepTiny        "voice_sleep_tiny" => "voice", "\"Sleep, Tiny\"", FeatureKind::Toggle;
    VoiceTrackMe          "voice_track_me" => "voice", "\"Track Me\"", FeatureKind::Toggle;
    VoiceUnlockMe         "voice_unlock_me" => "voice", "\"Unlock Me\"", FeatureKind::Toggle;
    VoiceZoomIn           "voice_zoom_in" => "voice", "\"Zoom in Closer\"", FeatureKind::Toggle;
    VoiceZoomOut          "voice_zoom_out" => "voice", "\"Zoom out Further\"", FeatureKind::Toggle;
    VoicePresets          "voice_presets" => "voice", "\"Position One/Two/Three\"", FeatureKind::Toggle;
    VoiceZoomFactor       "voice_zoom_factor" => "voice", "Voice Zoom", choice(&[(17, "1.5x"), (33, "2x"), (50, "2.5x"), (67, "3x"), (83, "3.5x"), (100, "4x")]);
    VoiceLanguage         "voice_language" => "voice", "Voice Language", choice(&[(1, "English"), (0, "Chinese")]);

    // Device sleep
    AutoSleep             "auto_sleep" => "sleep", "Auto Sleep", FeatureKind::Toggle;
    SleepTime             "sleep_time" => "sleep", "Sleep Time", choice(&[(30, "30s"), (120, "2min"), (600, "10min")]);
    SleepBackgroundMirror "sleep_background_mirror" => "sleep", "Sleep Background Mirror", FeatureKind::Toggle;

    // Indicator
    StatusLight           "status_light" => "indicator", "Status Light", FeatureKind::Toggle;
    StatusLightBrightness "status_light_brightness" => "indicator", "Brightness", range(1, 3, 1, None);

    // Maintenance
    FactoryReset          "factory_reset" => "maintenance", "Factory Reset", FeatureKind::Action;
}

#[derive(Debug, Clone, Serialize)]
pub struct FeatureDef {
    pub id: FeatureId,
    pub group: &'static str,
    pub label: &'static str,
    pub kind: FeatureKind,
}

impl FeatureId {
    pub fn from_name(name: &str) -> Option<FeatureId> {
        FeatureId::ALL.iter().copied().find(|f| f.name() == name)
    }
}

impl FeatureKind {
    /// Human-readable value, in display units.
    pub fn format(&self, value: i64) -> String {
        match self {
            FeatureKind::Toggle => if value != 0 { "on" } else { "off" }.to_string(),
            FeatureKind::Choice { options } => options
                .iter()
                .find(|o| o.value == value)
                .map_or_else(|| value.to_string(), |o| o.label.clone()),
            FeatureKind::Range { scale, unit, .. } => {
                let unit = unit.as_deref().unwrap_or("");
                if *scale > 1 {
                    let decimals = (*scale as f64).log10().ceil().min(2.0) as usize;
                    format!("{:.*}{unit}", decimals, value as f64 / *scale as f64)
                } else {
                    format!("{value}{unit}")
                }
            }
            FeatureKind::Action => String::new(),
        }
    }

    /// Parses user input: `on`/`off` for toggles, a label or number for
    /// choices, and display units for ranges (`1.5` zoom -> 150).
    pub fn parse(&self, input: &str) -> Option<i64> {
        let input = input.trim();
        match self {
            FeatureKind::Toggle => match input.to_ascii_lowercase().as_str() {
                "on" | "true" | "yes" | "1" => Some(1),
                "off" | "false" | "no" | "0" => Some(0),
                _ => None,
            },
            FeatureKind::Choice { options } => options
                .iter()
                .find(|o| o.label.eq_ignore_ascii_case(input))
                .map(|o| o.value)
                .or_else(|| input.parse().ok()),
            FeatureKind::Range { scale, unit, .. } => {
                let n = unit.as_deref().map_or(input, |u| input.trim_end_matches(u));
                let v: f64 = n.trim().parse().ok()?;
                Some((v * *scale as f64).round() as i64)
            }
            FeatureKind::Action => Some(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for &id in FeatureId::ALL {
            assert_eq!(FeatureId::from_name(id.name()), Some(id));
        }
        assert_eq!(
            FeatureId::WhiteBalanceTemperature.name(),
            "white_balance_temperature"
        );
    }

    #[test]
    fn parse_and_format() {
        let zoom = FeatureId::Zoom.def().kind;
        assert_eq!(zoom.parse("1.5"), Some(150));
        assert_eq!(zoom.parse("2x"), Some(200));
        assert_eq!(zoom.format(150), "1.50x");
        let flicker = FeatureId::AntiFlicker.def().kind;
        assert_eq!(flicker.parse("60 hz"), Some(2));
        assert_eq!(flicker.format(1), "50 Hz");
        let toggle = FeatureId::Hdr.def().kind;
        assert_eq!(toggle.parse("ON"), Some(1));
        assert_eq!(toggle.format(0), "off");
        assert_eq!(
            FeatureId::WhiteBalanceTemperature.def().kind.format(4800),
            "4800K"
        );
    }
}
