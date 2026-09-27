//! Raw UVC Extension Unit access.
//!
//! OBSBOT's vendor features are expected to be XU controls. Until the
//! protocol is decoded from captures this module offers raw get/set and a
//! read-only probe used by `obsbotctl raw` to map a camera's XU layout.

use crate::error::Result;
use crate::v4l2::{UvcQuery, VideoNode};

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
    Ok(buf)
}

pub fn set(node: &VideoNode, unit: u8, selector: u8, data: &[u8]) -> Result<()> {
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
