//! OBSBOT's wireless microphones (Vox SE): the mic byte of the status
//! block and the mic info query (`02c0`, SDK `DevTWSInfo`). See the Vox SE
//! section of docs/protocol.md.

use serde::Serialize;

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
}
