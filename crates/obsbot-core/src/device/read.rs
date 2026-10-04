//! Reading features: one status read and the queries they need, then each feature's value.

use super::*;

/// A query's destination, command and payload.
pub(super) type QueryKey = (u8, [u8; 2], Vec<u8>);

/// Status block and query responses shared by the features being read.
pub(super) struct Readback {
    pub(super) status: Option<Vec<u8>>,
    pub(super) queries: HashMap<QueryKey, Option<Vec<u8>>>,
}

impl Device {
    /// A feature's kind as the profile declares it (catalog, or the
    /// profile's own `options`), without reading the camera.
    pub fn declared_kind(&self, id: FeatureId) -> FeatureKind {
        match self.profile.features.get(&id) {
            Some(Binding::Vendor(b)) if !b.options.is_empty() => FeatureKind::Choice {
                options: b
                    .options
                    .iter()
                    .map(|(value, label)| ChoiceOption {
                        value: *value,
                        label: label.clone(),
                    })
                    .collect(),
            },
            _ => id.def().kind,
        }
    }

    pub fn feature(&self, id: FeatureId) -> FeatureState {
        let readback = self.readback(&[id]);
        self.feature_with_status(id, &readback)
    }

    /// Reads the status block and query responses needed by `ids`, once each.
    pub(super) fn readback(&self, ids: &[FeatureId]) -> Readback {
        let vendor = || {
            ids.iter()
                .filter_map(|id| match self.profile.features.get(id) {
                    Some(Binding::Vendor(b)) => Some(b),
                    _ => None,
                })
        };
        let mut rb = Readback {
            status: None,
            queries: HashMap::new(),
        };
        if vendor().any(|b| b.status.is_some() && b.query.is_none()) {
            rb.status = self.status_block();
            if let Some(block) = &rb.status {
                self.observe(block);
            }
        }
        for q in vendor().filter_map(|b| b.query.as_ref()) {
            if let std::collections::hash_map::Entry::Vacant(e) =
                rb.queries.entry((q.dst, q.cmd, q.payload.clone()))
            {
                let flags = q.flags.unwrap_or(protocol::FLAGS_QUERY);
                e.insert(
                    self.query_with(q.dst, q.cmd, flags, &q.payload)
                        .ok()
                        .flatten(),
                );
            }
        }
        rb
    }

    pub(super) fn query_value(q: &QueryRead, rb: &Readback) -> Option<i64> {
        let payload = rb
            .queries
            .get(&(q.dst, q.cmd, q.payload.clone()))?
            .as_ref()?;
        if let (ValueEncoding::F32, [o]) = (q.value, q.offsets.as_slice()) {
            let v = f32::from_le_bytes(payload.get(*o..*o + 4)?.try_into().ok()?);
            return Some((v as f64 * q.scale as f64).round() as i64);
        }
        let bytes: Option<Vec<u8>> = q.offsets.iter().map(|&o| payload.get(o).copied()).collect();
        match bytes?.as_slice() {
            [b] => {
                let b = q.mask.map_or(*b, |m| (b & m) >> m.trailing_zeros());
                let v = match q.value {
                    ValueEncoding::I8 => b as i8 as i64,
                    _ => b as i64,
                };
                Some(v * q.scale)
            }
            bs => Some(bs.iter().any(|&b| b != 0) as i64),
        }
    }

    pub(super) fn feature_with_status(&self, id: FeatureId, rb: &Readback) -> FeatureState {
        let status = rb.status.as_deref();
        let def = id.def();
        let mut state = FeatureState {
            id,
            group: def.group,
            label: def.label,
            kind: def.kind.clone(),
            supported: false,
            active: true,
            value: None,
            reason: None,
        };
        match self.profile.features.get(&id) {
            None => state.reason = Some(format!("not in profile `{}`", self.profile.id)),
            Some(Binding::V4l2(b)) => match self.node.query_control(b.v4l2) {
                Ok(Some(info)) if !info.is_disabled() => {
                    state.supported = true;
                    state.active = !info.is_inactive();
                    state.kind = v4l2_ctrl::describe(&info, b, &def.kind);
                    match v4l2_ctrl::get(&self.node, b) {
                        Ok(v) => state.value = Some(v),
                        Err(e) => state.reason = Some(e.to_string()),
                    }
                }
                Ok(_) => state.reason = Some("control not exposed by the camera".into()),
                Err(e) => state.reason = Some(e.to_string()),
            },
            Some(Binding::Vendor(b)) => {
                state.supported = true;
                if !b.options.is_empty() {
                    state.kind = self.declared_kind(id);
                }
                let written = self.written.lock().unwrap().get(&id).copied();
                let raw = status.and_then(|block| Self::status_value(b, block));
                let read = match &b.query {
                    Some(q) => Self::query_value(q, rb),
                    None if raw.is_some_and(|r| b.transient.contains(&r)) => {
                        self.settled.lock().unwrap().get(&id).copied()
                    }
                    None => {
                        let v = raw.map(|r| self.setting_to_feature(b, r));
                        if let Some(v) = v {
                            self.settled.lock().unwrap().insert(id, v);
                        }
                        v
                    }
                };
                state.value = match written {
                    Some((v, at)) if at.elapsed() < STATUS_LAG => Some(v),
                    w => read.or(w.map(|(v, _)| v)),
                };
                let transient = raw.is_some_and(|r| b.transient.contains(&r));
                if !transient {
                    self.verify_write(id, read, &state);
                }
                // A level is inactive while its switch is off.
                if b.level == Some(Level::Magnitude) {
                    state.active = raw.is_none_or(|r| r > 0);
                }
            }
            Some(Binding::Lock(_)) => {
                state.supported = true;
                state.value = Some(self.locked.lock().unwrap().is_some() as i64);
            }
            Some(Binding::Alsa(a)) => match crate::audio::find_card(&self.info.usb_path) {
                None => state.reason = Some("no USB audio found for this camera".into()),
                Some(card) => {
                    let read = match a.alsa {
                        AlsaControl::CaptureVolume => crate::audio::volume(card),
                        AlsaControl::CaptureSwitch => crate::audio::switch(card).map(i64::from),
                    };
                    match read {
                        Ok(v) => {
                            state.supported = true;
                            state.value = Some(v);
                        }
                        Err(e) => state.reason = Some(e.to_string()),
                    }
                }
            },
        }
        state
    }

    /// Reads a vendor setting's raw value from the status block.
    pub(super) fn status_value(b: &VendorBinding, block: &[u8]) -> Option<i64> {
        let v = protocol::decode_status(block, b.status?, b.status_value)?;
        Some(match b.status_mask {
            Some(m) => ((v as u8 & m) >> m.trailing_zeros()) as i64,
            None => v,
        })
    }

    /// The last level seen for a switch/level setting.
    pub(super) fn level(&self, b: &VendorBinding) -> i64 {
        b.level_status
            .or(b.status)
            .and_then(|o| self.levels.lock().unwrap().get(&o).copied())
            .or(b.default_level)
            .unwrap_or(1)
    }

    pub(super) fn remember_level(&self, b: &VendorBinding, raw: i64) {
        if b.level_status.is_some() {
            return; // `raw` is the switch, not the level
        }
        if let (Some(o), Some(_), true) = (b.status, b.level, raw != 0) {
            self.levels.lock().unwrap().insert(o, raw.abs());
        }
    }

    /// Feature value from a raw setting.
    pub(super) fn setting_to_feature(&self, b: &VendorBinding, raw: i64) -> i64 {
        self.remember_level(b, raw);
        match b.level {
            None if b.invert => (raw == 0) as i64,
            None => raw,
            Some(Level::Switch) => (raw != 0) as i64,
            Some(Level::SignSwitch) => (raw > 0) as i64,
            Some(Level::Magnitude) if raw == 0 => self.level(b),
            Some(Level::Magnitude) => raw.abs(),
        }
    }

    pub fn read_all(&self) -> Vec<FeatureState> {
        self.read(FeatureId::ALL)
    }

    /// Reads several features with a single status-block read.
    pub fn read(&self, ids: &[FeatureId]) -> Vec<FeatureState> {
        let readback = self.readback(ids);
        ids.iter()
            .map(|&id| self.feature_with_status(id, &readback))
            .collect()
    }
}
