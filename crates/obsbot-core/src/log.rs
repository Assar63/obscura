//! Activity log shared by the app, `obsbotctl` and the indicator: settings
//! written, changes the camera ignored, and errors. With `OBSCURA_TRACE=1`
//! it also records every raw Extension Unit frame and prints everything to
//! stderr, so running any of them from a terminal shows what happens.

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

/// Entries kept in memory; older ones are dropped.
const CAPACITY: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    /// Raw frames, only with `OBSCURA_TRACE=1`.
    Trace,
    /// Settings written.
    Info,
    /// A change the camera ignored, or an error.
    Warn,
}

#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    /// Increasing number, for fetching only newer entries.
    pub seq: u64,
    /// Milliseconds since the Unix epoch.
    pub time_ms: u64,
    pub level: Level,
    pub message: String,
}

struct Log {
    next: u64,
    entries: VecDeque<Entry>,
}

static LOG: Mutex<Log> = Mutex::new(Log {
    next: 1,
    entries: VecDeque::new(),
});

/// Whether `OBSCURA_TRACE` is set (to anything but `0`).
pub fn trace_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("OBSCURA_TRACE").is_some_and(|v| v != "0"))
}

pub fn push(level: Level, message: impl Into<String>) {
    if level == Level::Trace && !trace_enabled() {
        return;
    }
    let message = message.into();
    let time_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    if trace_enabled() {
        let secs = time_ms / 1000;
        let (h, m, s) = ((secs / 3600) % 24, (secs / 60) % 60, secs % 60);
        eprintln!(
            "[{h:02}:{m:02}:{s:02}.{:03} UTC {level:?}] {message}",
            time_ms % 1000
        );
    }
    let mut log = LOG.lock().unwrap();
    let seq = log.next;
    log.next += 1;
    if log.entries.len() == CAPACITY {
        log.entries.pop_front();
    }
    log.entries.push_back(Entry {
        seq,
        time_ms,
        level,
        message,
    });
}

pub fn info(message: impl Into<String>) {
    push(Level::Info, message);
}

pub fn warn(message: impl Into<String>) {
    push(Level::Warn, message);
}

pub fn trace(message: impl Into<String>) {
    push(Level::Trace, message);
}

/// Entries newer than `seq` (0 for all), oldest first.
pub fn since(seq: u64) -> Vec<Entry> {
    let log = LOG.lock().unwrap();
    log.entries
        .iter()
        .filter(|e| e.seq > seq)
        .cloned()
        .collect()
}

/// Hex bytes with trailing zero padding trimmed, for frame traces.
pub fn hex(data: &[u8]) -> String {
    let end = data.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
    data[..end]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_order_and_skips_trace_when_disabled() {
        let before = since(0).last().map_or(0, |e| e.seq);
        info("first");
        trace("raw");
        warn("second");
        let new: Vec<_> = since(before).into_iter().map(|e| e.message).collect();
        if trace_enabled() {
            assert_eq!(new, ["first", "raw", "second"]);
        } else {
            assert_eq!(new, ["first", "second"]);
        }
    }

    #[test]
    fn hex_trims_padding() {
        assert_eq!(hex(&[0xaa, 0x25, 0, 0x01, 0, 0]), "aa 25 00 01");
        assert_eq!(hex(&[0, 0]), "");
    }
}
