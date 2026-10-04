//! OBSBOT's vendor protocol: how frames, the status block, camera events and
//! the wireless microphone info are laid out. Encoding and decoding only;
//! `device` does the I/O through `transport`.

pub mod events;
pub mod mics;
pub mod protocol;
pub mod status;
