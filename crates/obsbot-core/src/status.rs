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
    /// Number of queued camera events (status[43]); see `Device::observe`.
    pub event_count: u8,
    /// Wireless microphone state (status[41]) in words.
    pub mic: String,
    /// Audio source (status[40] bits 0-2): 0 built-in, 3 wireless mic.
    pub audio_source: u8,
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
        mic: describe_mic(b(41)?),
        audio_source: b(40)? & 0x07,
        raw: crate::log::hex(block),
    })
}

/// One wireless microphone slot (TX1 or TX2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MicSlot {
    /// 1 or 2.
    pub slot: u8,
    /// Online, from status[41].
    pub connected: bool,
    /// Battery percent, while connected.
    pub battery: Option<u8>,
    pub charging: bool,
    pub muted: bool,
}

/// Decodes the mic info reply (query `02c0`, SDK `DevTWSInfo`) together
/// with status[41]: [2] bit 0/1 mic 1/2 muted, [7]/[8] battery percent,
/// [9]/[10] charging. Measured on a Tiny 3 with one Vox SE.
pub fn decode_mics(info: &[u8], status41: u8) -> Vec<MicSlot> {
    let online = (status41 >> 1) & 3;
    (1..=2u8)
        .map(|slot| {
            let i = slot as usize - 1;
            let connected = online & slot != 0;
            MicSlot {
                slot,
                connected,
                battery: info.get(7 + i).copied().filter(|_| connected),
                charging: connected && info.get(9 + i).is_some_and(|&c| c != 0),
                muted: info.get(2).is_some_and(|&m| m & (1 << i) != 0),
            }
        })
        .collect()
}

/// The wireless microphone byte (status[41], SDK `wireless_mic`): bit 0
/// TWS (Bluetooth) mode, bits 1-2 which mics are online, bit 3 Bluetooth
/// connected, bit 4 scanning.
pub fn describe_mic(b: u8) -> String {
    let mut parts = vec![match (b >> 1) & 3 {
        0 => "no microphone connected",
        1 => "TX1 connected",
        2 => "TX2 connected",
        _ => "TX1 and TX2 connected",
    }
    .to_string()];
    if b & 0x08 != 0 {
        parts.push("Bluetooth connected".into());
    }
    if b & 0x10 != 0 {
        parts.push("scanning".into());
    }
    if b & 0x01 != 0 {
        parts.push("TWS mode".into());
    }
    parts.join(", ")
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
    fn describes_the_mic_byte() {
        assert_eq!(describe_mic(0), "no microphone connected");
        assert_eq!(describe_mic(0x02), "TX1 connected");
        assert_eq!(describe_mic(0x14), "TX2 connected, scanning");
        assert_eq!(describe_mic(0x07), "TX1 and TX2 connected, TWS mode");
    }

    #[test]
    fn decodes_mic_info() {
        // Tiny 3 with one Vox SE on TX1 at 80 %, status[41] = 02.
        let info = [
            0x00, 0x27, 0x00, 0x02, 0x02, 0, 0, 0x50, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        let mics = decode_mics(&info, 0x02);
        assert_eq!(
            mics[0],
            MicSlot {
                slot: 1,
                connected: true,
                battery: Some(80),
                charging: false,
                muted: false
            }
        );
        assert_eq!(
            mics[1],
            MicSlot {
                slot: 2,
                connected: false,
                battery: None,
                charging: false,
                muted: false
            }
        );
    }

    #[test]
    fn short_block_is_none() {
        assert_eq!(decode(&[0; 10]), None);
    }
}
