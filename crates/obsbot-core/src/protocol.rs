//! OBSBOT vendor protocol over the UVC Extension Unit (unit 2).
//!
//! Two channels, both 60-byte XU controls:
//!
//! * **Selector 2, framed commands** (`FrmPacketV3`):
//!   ```text
//!   0      aa           magic
//!   1      flags        0x25 = command, 0x01 = query (response readable on selector 2)
//!   2..4   seq u16le
//!   4..6   0x000c       bytes covered by the header token
//!   6..8   token u16le  CRC-16/USB of bytes 0..12 with the token zeroed
//!   8      0x0a         sender (host)
//!   9      dst          receiver (0x04 = AI/gimbal, 0x02 = ISP)
//!   10..12 cmd          command ID
//!   12..14 len u16le    payload length
//!   14..16 crc u16le    CRC-16/USB of bytes 12..16+len with this field zeroed
//!   16..   payload
//!   ```
//!   The camera silently drops frames with a wrong token or payload CRC.
//!
//! * **Selector 6, short settings**: `[id, len, value (len bytes, LE)]`,
//!   zero-padded. Reading selector 6 returns a status block that reflects
//!   many settings.
//!
//! Decoded from USB captures of OBSBOT Center (see `docs/protocol.md`).

pub const XU_UNIT: u8 = 2;
pub const SEL_COMMAND: u8 = 2;
pub const SEL_STATUS: u8 = 6;
pub const PACKET_LEN: usize = 60;

const MAGIC: u8 = 0xaa;
const FLAGS_COMMAND: u8 = 0x25;
const HEADER_LEN: u16 = 12;
const SENDER_HOST: u8 = 0x0a;

/// CRC-16/USB: poly 0x8005 reflected (0xA001), init 0xFFFF, xorout 0xFFFF.
pub fn crc16_usb(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xffff;
    for &b in data {
        crc ^= b as u16;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xa001 } else { crc >> 1 };
        }
    }
    crc ^ 0xffff
}

/// Encodes a framed command for selector 2.
pub fn encode_command(seq: u16, dst: u8, cmd: [u8; 2], payload: &[u8]) -> [u8; PACKET_LEN] {
    assert!(payload.len() <= PACKET_LEN - 16, "payload too large");
    let mut p = [0u8; PACKET_LEN];
    p[0] = MAGIC;
    p[1] = FLAGS_COMMAND;
    p[2..4].copy_from_slice(&seq.to_le_bytes());
    p[4..6].copy_from_slice(&HEADER_LEN.to_le_bytes());
    p[8] = SENDER_HOST;
    p[9] = dst;
    p[10..12].copy_from_slice(&cmd);
    p[12..14].copy_from_slice(&(payload.len() as u16).to_le_bytes());
    p[16..16 + payload.len()].copy_from_slice(payload);
    let seg_crc = crc16_usb(&p[12..16 + payload.len()]);
    p[14..16].copy_from_slice(&seg_crc.to_le_bytes());
    let token = crc16_usb(&p[..12]);
    p[6..8].copy_from_slice(&token.to_le_bytes());
    p
}

/// Encodes a short setting for selector 6.
pub fn encode_short(id: u8, value: &[u8]) -> [u8; PACKET_LEN] {
    let mut p = [0u8; PACKET_LEN];
    p[0] = id;
    p[1] = value.len() as u8;
    p[2..2 + value.len()].copy_from_slice(value);
    p
}

/// How a feature value is laid out in a payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValueEncoding {
    U8,
    U16,
    U32,
    /// float32 LE of `value / divisor`.
    F32,
}

pub fn encode_value(enc: ValueEncoding, value: i64, divisor: i64) -> Vec<u8> {
    match enc {
        ValueEncoding::U8 => vec![value as u8],
        ValueEncoding::U16 => (value as u16).to_le_bytes().to_vec(),
        ValueEncoding::U32 => (value as u32).to_le_bytes().to_vec(),
        ValueEncoding::F32 => ((value as f64 / divisor.max(1) as f64) as f32).to_le_bytes().to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        let s: String = s.split_whitespace().collect();
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    fn padded(s: &str) -> [u8; PACKET_LEN] {
        let mut p = [0u8; PACKET_LEN];
        let b = hex(s);
        p[..b.len()].copy_from_slice(&b);
        p
    }

    // Byte-exact packets from the OBSBOT Center captures.
    #[test]
    fn locked_target_matches_capture() {
        // 11-gesture-locked-target: value 0 then 1
        assert_eq!(
            encode_command(0x20, 0x04, [0xc4, 0x30], &[0]),
            padded("aa 25 20 00 0c 00 ab da 0a 04 c4 30 01 00 e6 3f 00")
        );
        assert_eq!(
            encode_command(0x21, 0x04, [0xc4, 0x30], &[1]),
            padded("aa 25 21 00 0c 00 fa 1f 0a 04 c4 30 01 00 27 ff 01")
        );
    }

    #[test]
    fn zoom_factor_matches_capture() {
        // 13-gesture-zoom-factor: 2.1x is sent to the ISP (x100, u32) and to
        // the AI module (float32). Bytes past the payload in captures are
        // stale buffer contents, so only the meaningful prefix is compared.
        let mut isp = vec![2, 0, 0, 0];
        isp.extend(encode_value(ValueEncoding::U32, 210, 1));
        assert_eq!(
            encode_command(0x42, 0x02, [0x42, 0x19], &isp),
            padded("aa 25 42 00 0c 00 4a 11 0a 02 42 19 08 00 c2 83 02 00 00 00 d2 00 00 00")
        );
        let ai = encode_command(0x43, 0x04, [0x44, 0x32], &encode_value(ValueEncoding::F32, 210, 100));
        assert_eq!(ai[..20], padded("aa 25 43 00 0c 00 b8 6a 0a 04 44 32 04 00 42 c0 66 66 06 40")[..20]);
    }

    #[test]
    fn crc16_usb_check_value() {
        assert_eq!(crc16_usb(b"123456789"), 0xb4c8);
    }

    #[test]
    fn short_setting_layout() {
        assert_eq!(&encode_short(0x14, &[1])[..4], &[0x14, 0x01, 0x01, 0x00]);
        assert_eq!(&encode_short(0x16, &[2, 0])[..4], &[0x16, 0x02, 0x02, 0x00]);
    }
}
