//! What the camera does on its own: queued events, and AI mode, power, mic and zoom changes OBSCura didn't make.

use super::*;
use crate::profile::StatusLayout;
use crate::vendor::events;
use crate::vendor::mics;
use crate::vendor::status::{StatusBlock, AI_MODE_SWITCHING};

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

impl Device {
    /// Logs what the camera did on its own since the last status read:
    /// queued events (target lost/found, …), and AI mode, power and zoom
    /// changes that weren't OBSCura's (a gesture, a voice command,
    /// auto-sleep). Status layout: docs/protocol.md.
    pub(super) fn observe(&self, block: &[u8]) {
        if self.profile.status_layout != Some(StatusLayout::Tiny) {
            return;
        }
        let st = StatusBlock(block);
        if self.profile.event_queue {
            self.drain_events(st.event_count().unwrap_or(0));
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
        if let Some(mode) = st.ai_mode().filter(|&m| m != AI_MODE_SWITCHING) {
            if seen.ai_mode.is_some_and(|m| m != mode) && !ours(FeatureId::AiMode) {
                log::info(format!(
                    "Camera: AI mode is now {} (gesture, voice or camera)",
                    self.declared_kind(FeatureId::AiMode).format(mode as i64)
                ));
            }
            seen.ai_mode = Some(mode);
        }

        if let Some(power) = st.power() {
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
        if let (Some(mic), Some(audio)) = (st.mic(), st.audio_source()) {
            if seen.mic.is_some_and(|m| m != mic) {
                log::info(format!(
                    "Camera: wireless mic: {} (status[41] = {mic:02x})",
                    mics::describe_mic(mic)
                ));
            }
            if seen.audio_source.is_some_and(|a| a != audio) {
                log::info(format!("Camera: audio source is now {audio}"));
            }
            seen.mic = Some(mic);
            seen.audio_source = Some(audio);
        }

        // Zoom ramps; log it once two reads agree.
        if let Some(ratio) = st.zoom_ratio() {
            if seen.zoom_seen == Some(ratio) && seen.zoom_logged != Some(ratio) {
                if seen.zoom_logged.is_some() && !ours(FeatureId::Zoom) {
                    log::info(format!(
                        "Camera: zoom is now {:.2}x (gesture, voice or tracking)",
                        st.zoom().unwrap_or(1.0)
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
                Ok(Some(payload)) => log::info(events::describe(&payload)),
                _ => break,
            }
        }
    }
}
