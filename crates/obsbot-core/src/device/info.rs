//! Device information: firmware, update check, live status, wireless mics.

use super::*;

impl Device {
    /// Compares the camera's firmware with the latest on OBSBOT's download
    /// page. Needs network access; the error is a sentence for the user.
    #[cfg(feature = "update-check")]
    pub fn check_firmware_update(
        &self,
    ) -> std::result::Result<crate::firmware::UpdateCheck, String> {
        let source = self.profile.firmware.as_ref().ok_or_else(|| {
            format!(
                "No firmware download page is known for {}",
                self.profile.name
            )
        })?;
        let current = self
            .firmware_info()
            .ok_or("The camera didn't report its firmware version")?
            .version;
        crate::firmware::check(source, &current)
    }

    /// The camera's live state decoded from the status block, if it has one.
    pub fn live_status(&self) -> Option<crate::status::LiveStatus> {
        let block = self.status_block()?;
        self.observe(&block);
        let mut st = crate::status::decode(&block)?;
        st.ai_mode_label = match st.ai_mode {
            6 => "switching…".to_string(),
            v => self.declared_kind(FeatureId::AiMode).format(v as i64),
        };
        Some(st)
    }

    /// The wireless microphone slots (Vox SE), if the camera has a
    /// receiver: online state from the status block, battery and the rest
    /// from the mic info query.
    pub fn wireless_mics(&self) -> Option<Vec<crate::status::MicSlot>> {
        if !self.profile.wireless_mics {
            return None;
        }
        let status41 = *self.status_block()?.get(41)?;
        let info = self
            .query(protocol::DST_CAMERA, protocol::CMD_MIC_INFO)
            .unwrap_or_default();
        Some(crate::status::decode_mics(&info, status41))
    }

    /// Firmware version (e.g. "6.4.4.1") and serial number, for cameras
    /// whose profile says they answer the system module's queries.
    pub fn firmware_info(&self) -> Option<FirmwareInfo> {
        if !self.profile.system_info {
            return None;
        }
        let version = self
            .query(protocol::DST_SYSTEM, protocol::CMD_VERSION)
            .ok()?;
        let serial = self.query(protocol::DST_SYSTEM, protocol::CMD_SERIAL).ok();
        Some(FirmwareInfo {
            version: version
                .get(..4)?
                .iter()
                .rev()
                .map(|b| b.to_string())
                .collect::<Vec<_>>()
                .join("."),
            serial: serial.map(|s| {
                String::from_utf8_lossy(&s)
                    .trim_end_matches('\0')
                    .to_string()
            }),
        })
    }
}
