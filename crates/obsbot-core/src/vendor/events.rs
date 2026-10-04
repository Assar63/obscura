//! Camera events. The camera queues them; status[43] counts them and query
//! dst 02 `021d` pops one (see docs/protocol.md, "Camera events").

use crate::log;

/// A camera event for the activity log. Payload: u32 source, u32 type,
/// optional u32 value (Tiny 3, from OBSBOT's SDK traffic).
pub fn describe(p: &[u8]) -> String {
    let word = |i: usize| {
        p.get(i..i + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    match (word(0), word(4), word(8)) {
        // SDK: kEvtInfoTargetLoss, 1 lost, 0 found again.
        (Some(0x71), Some(20), Some(1)) => "Camera: tracking target lost".to_string(),
        (Some(0x71), Some(20), Some(0)) => "Camera: tracking target found again".to_string(),
        (Some(source), Some(kind), value) => format!(
            "Camera event: source {source:#x}, type {kind}{}",
            value.map_or(String::new(), |v| format!(", value {v}"))
        ),
        _ => format!("Camera event: {}", log::hex(p)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_tiny_3_events() {
        // Event queue replies traced from OBSBOT's SDK on a Tiny 3.
        let lost = [0x71, 0, 0, 0, 0x14, 0, 0, 0, 1, 0, 0, 0];
        let found = [0x71, 0, 0, 0, 0x14, 0, 0, 0, 0, 0, 0, 0];
        let other = [0x71, 0, 0, 0, 0x10, 0, 0, 0];
        assert_eq!(describe(&lost), "Camera: tracking target lost");
        assert_eq!(describe(&found), "Camera: tracking target found again");
        assert_eq!(describe(&other), "Camera event: source 0x71, type 16");
        assert_eq!(describe(&[1, 2]), "Camera event: 01 02");
    }
}
