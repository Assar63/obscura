use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("ioctl {op} failed: {source}")]
    Ioctl {
        op: &'static str,
        source: nix::Error,
    },
    #[error("feature `{0}` is not supported by this device")]
    Unsupported(String),
    #[error("unknown feature `{0}`")]
    UnknownFeature(String),
    #[error("value {value} out of range {min}..={max}")]
    OutOfRange { value: i64, min: i64, max: i64 },
    #[error("invalid profile {name}: {msg}")]
    Profile { name: String, msg: String },
    #[error("no camera found")]
    NoDevice,
    #[error("audio: {0}")]
    Audio(String),
}

pub type Result<T> = std::result::Result<T, Error>;
