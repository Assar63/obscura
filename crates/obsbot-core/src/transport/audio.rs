//! The camera's USB audio capture level and switch, through the ALSA mixer
//! of its sound card. This is a hardware control in the camera, so every app
//! (Teams, OBS, browsers) gets what is set here. PipeWire/PulseAudio keep
//! their own input volume on top of it; the two don't follow each other.
//!
//! Needs the `alsa` cargo feature; in the snap, the `alsa` interface
//! (`sudo snap connect obscura:alsa`).

use std::path::Path;

use crate::error::{Error, Result};

/// The ALSA card of the camera with this USB bus path (e.g. `3-2.1.1.2`):
/// the card whose sysfs device lives under that USB device.
pub fn find_card(usb_path: &str) -> Option<u32> {
    let needle = format!("/{usb_path}:");
    std::fs::read_dir("/sys/class/sound")
        .ok()?
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let card: u32 = name.strip_prefix("card")?.parse().ok()?;
            let dev =
                std::fs::canonicalize(Path::new("/sys/class/sound").join(&name).join("device"))
                    .ok()?;
            dev.to_string_lossy().contains(&needle).then_some(card)
        })
        .min()
}

#[cfg(feature = "alsa")]
mod mixer {
    use alsa::mixer::{MilliBel, Mixer, Selem, SelemChannelId};
    use alsa::Round;

    use super::*;

    fn audio_err(e: alsa::Error) -> Error {
        Error::Audio(e.to_string())
    }

    /// Runs `f` on the card's first simple element with a capture control.
    fn with_capture<T>(card: u32, f: impl FnOnce(&Selem) -> Result<T>) -> Result<T> {
        let mixer = Mixer::new(&format!("hw:{card}"), false).map_err(audio_err)?;
        for elem in mixer.iter() {
            if let Some(selem) = Selem::new(elem) {
                if selem.has_capture_volume() || selem.has_capture_switch() {
                    return f(&selem);
                }
            }
        }
        Err(Error::Audio(
            "the camera's sound card has no capture control".into(),
        ))
    }

    /// Capture level in whole dB (0 = full level on the Tiny 3, whose
    /// control runs 0..100 = -100..0 dB).
    pub fn volume(card: u32) -> Result<i64> {
        with_capture(card, |s| {
            if !s.has_capture_volume() {
                return Err(Error::Audio("no capture volume".into()));
            }
            let mb = s
                .get_capture_vol_db(SelemChannelId::mono())
                .map_err(audio_err)?;
            Ok((mb.0 as f64 / 100.0).round() as i64)
        })
    }

    pub fn set_volume(card: u32, db: i64) -> Result<()> {
        with_capture(card, |s| {
            s.set_capture_db_all(MilliBel(db * 100), Round::Floor)
                .map_err(audio_err)
        })
    }

    /// Whether capture is switched on (not muted).
    pub fn switch(card: u32) -> Result<bool> {
        with_capture(card, |s| {
            if !s.has_capture_switch() {
                return Err(Error::Audio("no capture switch".into()));
            }
            Ok(s.get_capture_switch(SelemChannelId::mono())
                .map_err(audio_err)?
                != 0)
        })
    }

    pub fn set_switch(card: u32, on: bool) -> Result<()> {
        with_capture(card, |s| {
            s.set_capture_switch_all(on as i32).map_err(audio_err)
        })
    }
}

#[cfg(feature = "alsa")]
pub use mixer::{set_switch, set_volume, switch, volume};

#[cfg(not(feature = "alsa"))]
mod disabled {
    use super::*;

    fn unsupported<T>() -> Result<T> {
        Err(Error::Audio("built without ALSA support".into()))
    }
    pub fn volume(_: u32) -> Result<i64> {
        unsupported()
    }
    pub fn set_volume(_: u32, _: i64) -> Result<()> {
        unsupported()
    }
    pub fn switch(_: u32) -> Result<bool> {
        unsupported()
    }
    pub fn set_switch(_: u32, _: bool) -> Result<()> {
        unsupported()
    }
}

#[cfg(not(feature = "alsa"))]
pub use disabled::{set_switch, set_volume, switch, volume};
