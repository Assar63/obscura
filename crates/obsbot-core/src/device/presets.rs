//! Camera-side gimbal presets.

use super::*;
use crate::profile::PresetFormat;

impl Device {
    /// Number of camera-side gimbal presets (0 if unsupported).
    pub fn preset_slots(&self) -> u32 {
        self.profile.presets.as_ref().map_or(0, |p| p.slots)
    }

    fn preset_format(&self) -> PresetFormat {
        self.profile
            .presets
            .as_ref()
            .map_or(PresetFormat::F32, |p| p.format)
    }

    /// Whether preset names can be read back (and so renamed usefully).
    pub fn preset_names_readable(&self) -> bool {
        self.preset_format() == PresetFormat::F32
    }

    /// Whether preset slots can be emptied.
    pub fn preset_delete_supported(&self) -> bool {
        self.preset_format() == PresetFormat::I16
    }

    pub(super) fn check_slot(&self, slot: u32) -> Result<()> {
        if slot < self.preset_slots() {
            Ok(())
        } else {
            Err(Error::OutOfRange {
                value: slot as i64,
                min: 0,
                max: self.preset_slots() as i64 - 1,
            })
        }
    }

    /// Preset names by slot; `None` for empty slots.
    pub fn presets(&self) -> Result<Vec<Option<String>>> {
        if self.preset_format() == PresetFormat::I16 {
            // An empty list comes back as "nothing there".
            let stored = self
                .query_with(
                    protocol::DST_GIMBAL,
                    protocol::CMD_PRESET_LIST,
                    protocol::FLAGS_QUERY,
                    &[],
                )?
                .unwrap_or_default();
            return Ok((0..self.preset_slots())
                .map(|slot| {
                    stored
                        .contains(&(slot as u8))
                        .then(|| format!("Preset {}", slot + 1))
                })
                .collect());
        }
        (0..self.preset_slots())
            .map(|slot| {
                let name = self.query_with(
                    protocol::DST_GIMBAL,
                    protocol::CMD_PRESET_NAME_QUERY,
                    protocol::FLAGS_QUERY_SLOT,
                    &slot.to_le_bytes(),
                )?;
                Ok(name.map(|n| {
                    String::from_utf8_lossy(&n)
                        .trim_end_matches('\0')
                        .to_string()
                }))
            })
            .collect()
    }

    /// Stores the gimbal's current position and zoom in `slot`, as OBSBOT
    /// Center's "Add" does, and names it.
    pub fn save_preset(&self, slot: u32, name: &str) -> Result<()> {
        self.check_slot(slot)?;
        let format = self.preset_format();
        // The Tiny 3 doesn't answer the position query with OBSBOT
        // Center's payload byte; the SDK sends none.
        let pos_payload: &[u8] = match format {
            PresetFormat::F32 => &[1],
            PresetFormat::I16 => &[],
        };
        let pos = self
            .query_with(
                protocol::DST_GIMBAL,
                protocol::CMD_GIMBAL_POSITION,
                protocol::FLAGS_QUERY,
                pos_payload,
            )?
            .ok_or_else(|| Error::Unsupported("gimbal position unavailable".into()))?;
        let angle = |o: usize| -> Result<f32> {
            let b = pos
                .get(o..o + 2)
                .ok_or_else(|| Error::Unsupported("short gimbal position".into()))?;
            // x 0.1 in f32, as OBSBOT Center does (matches its bytes exactly).
            Ok(i16::from_le_bytes([b[0], b[1]]) as f32 * 0.1)
        };
        let zoom = self
            .query(protocol::DST_GIMBAL, protocol::CMD_ZOOM)?
            .get(..4)
            .map_or(1.0, |b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        let mut payload = slot.to_le_bytes().to_vec();
        match format {
            PresetFormat::F32 => {
                for v in [angle(10)?, angle(8)?, 0.0, zoom, -1000.0] {
                    payload.extend(v.to_le_bytes());
                }
            }
            PresetFormat::I16 => {
                // Yaw, pitch, roll and zoom x 100, as OBSBOT's SDK sends
                // them; the camera fills in the last 12 bytes itself.
                for v in [angle(10)?, angle(8)?, 0.0, zoom] {
                    payload.extend(((v * 100.0).round() as i16).to_le_bytes());
                }
                payload.extend([0; 12]);
            }
        }
        self.command(protocol::DST_GIMBAL, protocol::CMD_PRESET_SAVE, &payload)?;
        self.rename_preset(slot, name)
    }

    /// Longest preset name sent to the camera, in bytes.
    pub const PRESET_NAME_MAX: usize = 16;

    pub fn rename_preset(&self, slot: u32, name: &str) -> Result<()> {
        self.check_slot(slot)?;
        let mut end = name.len().min(Self::PRESET_NAME_MAX);
        while !name.is_char_boundary(end) {
            end -= 1;
        }
        let mut payload = slot.to_le_bytes().to_vec();
        payload.extend(&name.as_bytes()[..end]);
        self.command(protocol::DST_GIMBAL, protocol::CMD_PRESET_NAME, &payload)
    }

    /// Empties a preset slot (Tiny 3; the Tiny SE's delete is unknown).
    pub fn delete_preset(&self, slot: u32) -> Result<()> {
        self.check_slot(slot)?;
        if !self.preset_delete_supported() {
            return Err(Error::Unsupported("preset delete".into()));
        }
        self.command(
            protocol::DST_GIMBAL,
            protocol::CMD_PRESET_DELETE,
            &slot.to_le_bytes(),
        )
    }

    /// Moves the gimbal (and zoom) to a stored preset.
    pub fn recall_preset(&self, slot: u32) -> Result<()> {
        self.check_slot(slot)?;
        let mut payload = slot.to_le_bytes().to_vec();
        for _ in 0..4 {
            payload.extend(1.0f32.to_le_bytes());
        }
        self.command(protocol::DST_GIMBAL, protocol::CMD_PRESET_RECALL, &payload)
    }
}
