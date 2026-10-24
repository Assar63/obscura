//! Raw UVC Extension Unit access.
//!
//! OBSBOT's vendor features are expected to be XU controls. Until the
//! protocol is decoded from captures this module offers raw get/set and a
//! read-only probe used by `obsbotctl raw` to map a camera's XU layout.

use crate::error::Result;
use crate::log;
use crate::transport::v4l2::{UvcQuery, VideoNode};

/// Length in bytes of an XU control (UVC GET_LEN).
pub fn len(node: &VideoNode, unit: u8, selector: u8) -> Result<u16> {
    let mut buf = [0u8; 2];
    node.xu_query(unit, selector, UvcQuery::GetLen, &mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

/// Capability bitmap (UVC GET_INFO): bit0 = GET supported, bit1 = SET supported.
pub fn info(node: &VideoNode, unit: u8, selector: u8) -> Result<u8> {
    let mut buf = [0u8; 1];
    node.xu_query(unit, selector, UvcQuery::GetInfo, &mut buf)?;
    Ok(buf[0])
}

/// Runs a GET_* query, sizing the buffer with GET_LEN first.
pub fn get(node: &VideoNode, unit: u8, selector: u8, query: UvcQuery) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; len(node, unit, selector)? as usize];
    node.xu_query(unit, selector, query, &mut buf)?;
    log::trace(format!(
        "XU GET  unit {unit} sel {selector}: {}",
        trace_data(unit, selector, &buf)
    ));
    Ok(buf)
}

fn trace_data(unit: u8, selector: u8, data: &[u8]) -> String {
    let is_serial_reply = unit == crate::vendor::protocol::XU_UNIT
        && selector == crate::vendor::protocol::SEL_COMMAND
        && data.first() == Some(&0xaa)
        && data.get(1) == Some(&crate::vendor::protocol::FLAGS_RESPONSE)
        && data.get(10..12) == Some(&crate::vendor::protocol::CMD_SERIAL);
    if is_serial_reply {
        format!(
            "{} [serial payload redacted]",
            log::hex(data.get(..16).unwrap_or(data))
        )
    } else {
        log::hex(data)
    }
}

pub fn set(node: &VideoNode, unit: u8, selector: u8, data: &[u8]) -> Result<()> {
    log::trace(format!(
        "XU SET  unit {unit} sel {selector}: {}",
        log::hex(data)
    ));
    let mut buf = data.to_vec();
    node.xu_query(unit, selector, UvcQuery::SetCur, &mut buf)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct XuControl {
    pub unit: u8,
    pub selector: u8,
    pub len: u16,
    pub info: u8,
}

/// Read-only scan for XU controls (GET_LEN + GET_INFO only; never SET).
/// The kernel only accepts queries for units declared in the descriptors,
/// so unknown units simply fail and are skipped.
pub fn probe(node: &VideoNode) -> Vec<XuControl> {
    let mut found = Vec::new();
    for unit in 1..=31u8 {
        for selector in 1..=31u8 {
            if let (Ok(len), Ok(info)) = (len(node, unit, selector), info(node, unit, selector)) {
                found.push(XuControl {
                    unit,
                    selector,
                    len,
                    info,
                });
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_redacts_serial_query_responses_only() {
        let mut response = [0u8; crate::vendor::protocol::PACKET_LEN];
        response[0] = 0xaa;
        response[1] = crate::vendor::protocol::FLAGS_RESPONSE;
        response[10..12].copy_from_slice(&crate::vendor::protocol::CMD_SERIAL);
        response[12..14].copy_from_slice(&8u16.to_le_bytes());
        response[16..24].copy_from_slice(b"12345678");

        let traced = trace_data(
            crate::vendor::protocol::XU_UNIT,
            crate::vendor::protocol::SEL_COMMAND,
            &response,
        );
        assert!(!traced.contains("31 32 33 34 35 36 37 38"));
        assert!(traced.contains("[serial payload redacted]"));

        let other = trace_data(2, 2, &[0xaa, 0x29, 0x31]);
        assert_eq!(other, "aa 29 31");
    }
}
