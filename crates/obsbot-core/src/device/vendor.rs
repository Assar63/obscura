//! Vendor frames and queries over the UVC Extension Unit.

use super::*;

fn hex(cmd: [u8; 2]) -> String {
    format!("{:02x}{:02x}", cmd[0], cmd[1])
}

impl Device {
    /// Reads the vendor status block (selector 6), if the device has one.
    pub fn status_block(&self) -> Option<Vec<u8>> {
        let has_vendor = self
            .profile
            .features
            .values()
            .any(|b| matches!(b, Binding::Vendor(_)));
        if !has_vendor {
            return None;
        }
        uvc_xu::get(
            &self.node,
            protocol::XU_UNIT,
            protocol::SEL_STATUS,
            crate::transport::v4l2::UvcQuery::GetCur,
        )
        .ok()
    }

    /// Sends a query (selector 2) and returns the response payload. Only use
    /// queries seen in OBSBOT Center captures.
    pub fn query(&self, dst: u8, cmd: [u8; 2]) -> Result<Vec<u8>> {
        self.query_with(dst, cmd, protocol::FLAGS_QUERY, &[])?
            .ok_or_else(|| Error::Unsupported(format!("query {} answered empty", hex(cmd))))
    }

    /// Sends a query with `flags` and `payload`; `None` if the camera
    /// answers that there is nothing there (e.g. an empty preset slot).
    pub fn query_with(
        &self,
        dst: u8,
        cmd: [u8; 2],
        flags: u8,
        payload: &[u8],
    ) -> Result<Option<Vec<u8>>> {
        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        uvc_xu::set(
            &self.node,
            protocol::XU_UNIT,
            protocol::SEL_COMMAND,
            &encode_query(seq, dst, cmd, flags, payload),
        )?;
        // OBSBOT Center reads the response about 40 ms later.
        for _ in 0..5 {
            std::thread::sleep(Duration::from_millis(30));
            let frame = uvc_xu::get(
                &self.node,
                protocol::XU_UNIT,
                protocol::SEL_COMMAND,
                crate::transport::v4l2::UvcQuery::GetCur,
            )?;
            match protocol::decode_response(&frame, seq, cmd) {
                Some((protocol::FLAGS_RESPONSE, p)) => return Ok(Some(p.to_vec())),
                Some((protocol::FLAGS_RESPONSE_EMPTY, _)) => return Ok(None),
                _ => {}
            }
        }
        Err(Error::Unsupported(format!(
            "no response to query {}",
            hex(cmd)
        )))
    }

    pub(super) fn command(&self, dst: u8, cmd: [u8; 2], payload: &[u8]) -> Result<()> {
        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        uvc_xu::set(
            &self.node,
            protocol::XU_UNIT,
            protocol::SEL_COMMAND,
            &encode_command(seq, dst, cmd, payload),
        )
    }

    pub(super) fn send_vendor(&self, b: &VendorBinding, value: i64) -> Result<()> {
        for f in &b.frames {
            let payload = f.payload.clone().unwrap_or_else(|| {
                let mut p = f.prefix.clone();
                p.extend(encode_value(f.value, value, f.divisor));
                p
            });
            let seq = self.seq.fetch_add(1, Ordering::Relaxed);
            let packet = match f.flags {
                Some(flags) => encode_query(seq, f.dst, f.cmd, flags, &payload),
                None => encode_command(seq, f.dst, f.cmd, &payload),
            };
            uvc_xu::set(
                &self.node,
                protocol::XU_UNIT,
                protocol::SEL_COMMAND,
                &packet,
            )?;
        }
        if let Some(s) = &b.short {
            let mut payload = s.prefix.clone();
            payload.extend(encode_value(s.value, value, 1));
            let packet = encode_short(s.id, &payload);
            uvc_xu::set(&self.node, protocol::XU_UNIT, protocol::SEL_STATUS, &packet)?;
        }
        Ok(())
    }
}
