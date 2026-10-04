//! What the camera does on its own: queued events, and AI mode, power, mic and zoom changes OBSCura didn't make.

use super::*;

/// How long after OBSCura changes a setting a matching camera-side change is
/// taken as its result rather than the camera's own.
pub(super) const OWN_CHANGE: Duration = Duration::from_secs(10);

/// Camera-side state at the last status read (see `Device::observe`).
#[derive(Default)]
pub(super) struct Observed {
    ai_mode: Option<u8>,
    power: Option<u8>,
    mic: Option<u8>,
    audio_source: Option<u8>,
    /// Zoom ratio at the last read, and the last one logged.
    zoom_seen: Option<u16>,
    zoom_logged: Option<u16>,
}

/// A camera event for the activity log. Payload: u32 source, u32 type,
/// optional u32 value (Tiny 3, from OBSBOT's SDK traffic).
pub(super) fn describe_event(p: &[u8]) -> String {
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

impl Device {
    /// Logs what the camera did on its own since the last status read:
    /// queued events (target lost/found, …), and AI mode, power and zoom
    /// changes that weren't OBSCura's (a gesture, a voice command,
    /// auto-sleep). Status layout: docs/protocol.md.
    pub(super) fn observe(&self, block: &[u8]) {
        if self.profile.event_queue {
            self.drain_events(block.get(43).copied().unwrap_or(0));
        }
        let ours = |id: FeatureId| {
            self.written
                .lock()
                .unwrap()
                .get(&id)
                .is_some_and(|(_, at)| at.elapsed() < OWN_CHANGE)
        };
        let mut seen = self.observed.lock().unwrap();

        // AI mode; 6 means a change is in progress.
        if let Some(&mode) = block.get(24).filter(|&&m| m != 6) {
            if seen.ai_mode.is_some_and(|m| m != mode) && !ours(FeatureId::AiMode) {
                log::info(format!(
                    "Camera: AI mode is now {} (gesture, voice or camera)",
                    self.declared_kind(FeatureId::AiMode).format(mode as i64)
                ));
            }
            seen.ai_mode = Some(mode);
        }

        if let Some(&power) = block.get(9) {
            if seen.power.is_some_and(|p| p != power) && !ours(FeatureId::Sleep) {
                log::info(match power {
                    1 => "Camera: woke up".to_string(),
                    3 => "Camera: went to sleep".to_string(),
                    4 => "Camera: privacy mode".to_string(),
                    v => format!("Camera: power state {v}"),
                });
            }
            seen.power = Some(power);
        }

        // Wireless microphones and the audio source; logged whoever changed
        // them, since that's what pairing experiments need to see.
        if let (Some(&mic), Some(&audio)) = (block.get(41), block.get(40)) {
            if seen.mic.is_some_and(|m| m != mic) {
                log::info(format!(
                    "Camera: wireless mic: {} (status[41] = {mic:02x})",
                    crate::status::describe_mic(mic)
                ));
            }
            if seen.audio_source.is_some_and(|a| a != audio & 7) {
                log::info(format!("Camera: audio source is now {}", audio & 7));
            }
            seen.mic = Some(mic);
            seen.audio_source = Some(audio & 7);
        }

        // Zoom ramps; log it once two reads agree.
        if let (Some(&lo), Some(&hi)) = (block.get(4), block.get(5)) {
            let ratio = u16::from_le_bytes([lo, hi]);
            if seen.zoom_seen == Some(ratio) && seen.zoom_logged != Some(ratio) {
                if seen.zoom_logged.is_some() && !ours(FeatureId::Zoom) {
                    log::info(format!(
                        "Camera: zoom is now {:.2}x (gesture, voice or tracking)",
                        1.0 + ratio.min(100) as f32 * 3.0 / 100.0
                    ));
                }
                seen.zoom_logged = Some(ratio);
            }
            seen.zoom_seen = Some(ratio);
        }
    }

    /// Pops up to `pending` queued camera events and logs them.
    pub(super) fn drain_events(&self, pending: u8) {
        for _ in 0..pending.min(16) {
            match self.query_with(
                protocol::DST_CAMERA,
                protocol::CMD_EVENT,
                protocol::FLAGS_QUERY,
                &[],
            ) {
                Ok(Some(payload)) => log::info(describe_event(&payload)),
                _ => break,
            }
        }
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
        assert_eq!(describe_event(&lost), "Camera: tracking target lost");
        assert_eq!(
            describe_event(&found),
            "Camera: tracking target found again"
        );
        assert_eq!(describe_event(&other), "Camera event: source 0x71, type 16");
        assert_eq!(describe_event(&[1, 2]), "Camera event: 01 02");
    }
}
