//! Live camera state decoded from the selector 6 status block, using the
//! layout OBSBOT's SDK documents as `CameraStatus::tiny` (Tiny, Tiny 4K,
//! Tiny 2 series, Tiny SE, Tiny 3, Meet 2, Meet SE). See the "Status block
//! layout" table in docs/protocol.md.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LiveStatus {
    /// "awake", "asleep", "privacy", or the raw value.
    pub power: String,
    /// AI mode value (status[24]); 6 means a mode change is in progress.
    pub ai_mode: u8,
    /// The AI mode's name from the camera's profile ("switching…" for 6);
    /// filled in by `Device::live_status`.
    pub ai_mode_label: String,
    /// AI sub-mode (status[28]): framing within human tracking.
    pub ai_sub_mode: u8,
    /// Zoom factor from the zoom ratio (status[4..6], 0-100 = 1x-4x).
    pub zoom: f32,
    /// Field of view (status[17]), or `None` when the camera reports none.
    pub fov: Option<&'static str>,
    /// Frame rate of the current or last stream (status[31]).
    pub fps: u8,
    /// Status light brightness 0-3 (status[33]).
    pub light_level: u8,
    /// Camera event counter (status[43]); not every recognised gesture
    /// counts.
    pub event_count: u8,
    /// The whole block in hex, trailing zeros trimmed.
    pub raw: String,
}

pub fn decode(block: &[u8]) -> Option<LiveStatus> {
    let b = |i: usize| block.get(i).copied();
    let power = match b(9)? {
        1 => "awake".to_string(),
        3 => "asleep".to_string(),
        4 => "privacy".to_string(),
        v => format!("unknown ({v})"),
    };
    let ratio = u16::from_le_bytes([b(4)?, b(5)?]);
    Some(LiveStatus {
        power,
        ai_mode: b(24)?,
        ai_mode_label: String::new(),
        ai_sub_mode: b(28)?,
        zoom: 1.0 + ratio.min(100) as f32 * 3.0 / 100.0,
        fov: match b(17)? {
            0 => Some("86°"),
            1 => Some("78°"),
            2 => Some("65°"),
            _ => None,
        },
        fps: b(31)?,
        light_level: b(33)?,
        event_count: b(43)?,
        raw: crate::log::hex(block),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(hex: &str) -> Vec<u8> {
        let mut v: Vec<u8> = hex
            .split_whitespace()
            .map(|b| u8::from_str_radix(b, 16).unwrap())
            .collect();
        v.resize(60, 0);
        v
    }

    #[test]
    fn decodes_a_tiny_3_block() {
        // Tiny 3, firmware 6.6.8.3: awake, human tracking, zoomed to 2x.
        let s = decode(&block(
            "2e 00 00 00 21 00 00 01 03 01 78 00 00 01 01 00 01 00 00 00 \
             01 00 21 00 02 01 43 00 00 00 00 1e 00 03 10 00 00 00 00 00 \
             08 00 00 0a 03 01 01",
        ))
        .unwrap();
        assert_eq!(s.power, "awake");
        assert_eq!(s.ai_mode, 2);
        assert!((s.zoom - 1.99).abs() < 0.01);
        assert_eq!(s.fov, Some("86°"));
        assert_eq!((s.fps, s.light_level, s.event_count), (30, 3, 10));
    }

    #[test]
    fn short_block_is_none() {
        assert_eq!(decode(&[0; 10]), None);
    }
}
