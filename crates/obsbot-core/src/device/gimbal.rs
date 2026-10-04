//! Joystick: gimbal velocity moves.

use super::*;

impl Device {
    /// Whether the camera supports velocity-based gimbal moves.
    pub fn has_gimbal_velocity(&self) -> bool {
        self.profile.gimbal_velocity.is_some()
    }

    /// Moves the gimbal at a velocity. `right` and `up` are -1..1 fractions
    /// of the profile's `max_speed`; send (0, 0) to stop. The camera expects
    /// the command to be repeated (~10 Hz) while moving.
    pub fn gimbal_velocity(&self, right: f32, up: f32) -> Result<()> {
        let g = self
            .profile
            .gimbal_velocity
            .as_ref()
            .ok_or_else(|| Error::Unsupported("gimbal velocity".into()))?;
        let yaw = right.clamp(-1.0, 1.0) * g.max_speed * g.yaw_sign;
        let pitch = up.clamp(-1.0, 1.0) * g.max_speed * g.pitch_sign;
        let mut payload = Vec::with_capacity(12);
        for v in [0.0f32, pitch, yaw] {
            payload.extend(v.to_le_bytes());
        }
        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        let packet = encode_command(seq, g.dst, g.cmd, &payload);
        uvc_xu::set(
            &self.node,
            protocol::XU_UNIT,
            protocol::SEL_COMMAND,
            &packet,
        )
    }
}
