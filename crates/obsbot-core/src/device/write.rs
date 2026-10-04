//! Writing features: validation, switch/level settings, locks, and checking that the camera took a write.

use super::*;

impl Device {
    /// Validates `value` against the feature's kind, writes it, and returns
    /// the re-read state (other controls may change as a side effect, e.g.
    /// toggling Auto WB activates Temperature).
    pub fn set(&self, id: FeatureId, value: i64) -> Result<FeatureState> {
        let current = self.feature(id);
        let wanted = current.kind.format(value);
        let result = self.set_checked(id, value, &current);
        match &result {
            Ok(_) => log::info(format!("{}: set to {wanted}", current.label)),
            Err(e) => log::warn(format!("{}: setting {wanted} failed: {e}", current.label)),
        }
        result
    }

    pub(super) fn set_checked(
        &self,
        id: FeatureId,
        value: i64,
        current: &FeatureState,
    ) -> Result<FeatureState> {
        if !current.supported {
            return Err(Error::Unsupported(id.name().to_string()));
        }
        validate(&current.kind, value)?;
        match self.profile.features.get(&id) {
            Some(Binding::V4l2(b)) => {
                v4l2_ctrl::set(&self.node, b, value)?;
                if id == FeatureId::AutoWhiteBalance && value == 0 {
                    self.reapply_white_balance_temperature();
                }
            }
            Some(Binding::Lock(l)) => self.set_lock(&l.lock, value != 0)?,
            Some(Binding::Alsa(a)) => {
                let card = crate::transport::audio::find_card(&self.info.usb_path)
                    .ok_or_else(|| Error::Audio("no USB audio found for this camera".into()))?;
                match a.alsa {
                    AlsaControl::CaptureVolume => crate::transport::audio::set_volume(card, value)?,
                    AlsaControl::CaptureSwitch => {
                        crate::transport::audio::set_switch(card, value != 0)?
                    }
                }
            }
            Some(Binding::Vendor(b)) => {
                self.send_vendor(b, self.feature_to_setting(b, value))?;
                if b.level == Some(Level::Magnitude) {
                    self.remember_level(b, value);
                }
                self.written
                    .lock()
                    .unwrap()
                    .insert(id, (value, Instant::now()));
                if b.status.is_some() || b.query.is_some() {
                    self.unverified
                        .lock()
                        .unwrap()
                        .insert(id, (value, Instant::now()));
                }
                // Frames like the gesture master switch also set sub-features.
                self.mark_related(b, value);
            }
            None => return Err(Error::Unsupported(id.name().to_string())),
        }
        Ok(self.feature(id))
    }

    /// Raw setting to write for a feature value.
    pub(super) fn feature_to_setting(&self, b: &VendorBinding, value: i64) -> i64 {
        let Some(level) = b.level else {
            return if b.invert { (value == 0) as i64 } else { value };
        };
        let block = self.status_block();
        let current = block
            .as_deref()
            .and_then(|block| Self::status_value(b, block));
        if let Some(raw) = current {
            self.remember_level(b, raw);
        }
        if let Some(o) = b.level_status {
            if let Some(&l) = block.as_deref().and_then(|block| block.get(o)) {
                if l != 0 {
                    self.levels.lock().unwrap().insert(o, l as i64);
                }
            }
        }
        match level {
            Level::Switch if value != 0 => self.level(b),
            Level::Switch => 0,
            Level::SignSwitch if value != 0 => self.level(b),
            Level::SignSwitch => -self.level(b),
            Level::Magnitude if current.is_some_and(|c| c < 0) => -value,
            Level::Magnitude => value,
        }
    }

    pub(super) fn set_lock(&self, spec: &LockSpec, lock: bool) -> Result<()> {
        if lock {
            if self.locked.lock().unwrap().is_some() {
                return Ok(());
            }
            let saved = spec
                .restore
                .iter()
                .filter_map(|&id| self.feature(id).value.map(|v| (id, v)))
                .collect();
            for &id in &spec.off {
                self.set(id, 0)?;
            }
            *self.locked.lock().unwrap() = Some(saved);
        } else {
            let saved = self.locked.lock().unwrap().take();
            for (id, v) in saved.unwrap_or_default() {
                self.set(id, v)?;
            }
        }
        Ok(())
    }

    /// Records `value` for other vendor features whose frames are a subset of
    /// `b`'s (e.g. the gesture master switch covers the four sub-switches).
    pub(super) fn mark_related(&self, b: &VendorBinding, value: i64) {
        if b.frames.len() < 2 {
            return;
        }
        let now = Instant::now();
        let mut written = self.written.lock().unwrap();
        for (id, other) in &self.profile.features {
            if let Binding::Vendor(o) = other {
                let covered = !o.frames.is_empty()
                    && o.frames.iter().all(|f| {
                        b.frames.iter().any(|g| {
                            g.dst == f.dst
                                && g.cmd == f.cmd
                                && f.prefix.is_empty()
                                && g.prefix.is_empty()
                        })
                    });
                if covered {
                    written.insert(*id, (value, now));
                }
            }
        }
    }

    /// Re-writes the white balance temperature after Auto WB is switched
    /// off. Tiny-series firmware uses whichever white balance control was
    /// written last (joshualambert/obsbot-tiny3-linux), so this makes the
    /// shown temperature the one in effect.
    pub(super) fn reapply_white_balance_temperature(&self) {
        let Some(Binding::V4l2(t)) = self
            .profile
            .features
            .get(&FeatureId::WhiteBalanceTemperature)
        else {
            return;
        };
        if let Ok(v) = v4l2_ctrl::get(&self.node, t) {
            if v4l2_ctrl::set(&self.node, t, v).is_ok() {
                log::info(format!(
                    "Temperature: re-applied {v}K for manual white balance"
                ));
            }
        }
    }

    /// Once the status lag has passed after a write, logs a warning if the
    /// camera reports something else than what was written.
    pub(super) fn verify_write(&self, id: FeatureId, read: Option<i64>, state: &FeatureState) {
        let mut unverified = self.unverified.lock().unwrap();
        let Some(&(wanted, at)) = unverified.get(&id) else {
            return;
        };
        if at.elapsed() < STATUS_LAG {
            return;
        }
        unverified.remove(&id);
        match read {
            Some(got) if got != wanted => log::warn(format!(
                "{}: asked for {}, but the camera reports {}.{}",
                state.label,
                state.kind.format(wanted),
                state.kind.format(got),
                ignored_hint(id, wanted),
            )),
            _ => {}
        }
    }
}
/// Why the camera may have ignored a write, for the activity log.
fn ignored_hint(id: FeatureId, value: i64) -> &'static str {
    match id {
        // Measured on a Tiny 3: Human needs the camera to see someone, so it's
        // ignored without a stream; Group, Hand, Desk and Off are not.
        FeatureId::AiMode if value == 2 => {
            " Human tracking only starts while the camera is streaming video: open \
             the preview or start a call, then try again."
        }
        FeatureId::Zoom => " The camera ignores zoom while it sleeps.",
        _ => "",
    }
}

pub(super) fn validate(kind: &FeatureKind, value: i64) -> Result<()> {
    let (min, max) = match kind {
        FeatureKind::Toggle => (0, 1),
        FeatureKind::Range { min, max, .. } => (*min, *max),
        FeatureKind::Choice { options } => {
            if options.iter().any(|o| o.value == value) {
                return Ok(());
            }
            let min = options.iter().map(|o| o.value).min().unwrap_or(0);
            let max = options.iter().map(|o| o.value).max().unwrap_or(0);
            return Err(Error::OutOfRange { value, min, max });
        }
        FeatureKind::Action => return Ok(()),
    };
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(Error::OutOfRange { value, min, max })
    }
}
