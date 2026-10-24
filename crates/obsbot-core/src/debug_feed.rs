//! Debug-only, local live feed for activity-log entries.
//!
//! The Unix socket is opt-in (`OBSCURA_DEBUG_FEED=1`), user-readable only,
//! and carries JSONL log entries and user-supplied markers, never media or
//! camera-control requests.

use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::log::{self, Entry};

const SOCKET_NAME: &str = "debug-feed.sock";
const MAX_MARKER_BYTES: usize = 512;
const POLL_INTERVAL: Duration = Duration::from_millis(25);

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    Entry { entry: Entry },
    Marker { text: String },
}

/// Handle that keeps the opt-in diagnostics listener alive.
pub struct FeedServer {
    stop: Arc<AtomicBool>,
    path: PathBuf,
}

impl FeedServer {
    /// Starts the listener at `path`. Existing active sockets are never
    /// replaced; stale socket files are removed before binding.
    pub fn bind(path: &Path) -> io::Result<Self> {
        prepare_socket_path(path)?;
        let listener = UnixListener::bind(path)?;
        if let Err(error) = fs::set_permissions(path, fs::Permissions::from_mode(0o600)) {
            let _ = fs::remove_file(path);
            return Err(error);
        }
        if let Err(error) = listener.set_nonblocking(true) {
            remove_socket(path);
            return Err(error);
        }

        let stop = Arc::new(AtomicBool::new(false));
        let clients = Arc::new(Mutex::new(Vec::<UnixStream>::new()));
        let initial_seq = log::since(0).last().map_or(0, |entry| entry.seq);
        let listener_stop = Arc::clone(&stop);
        let listener_clients = Arc::clone(&clients);
        if let Err(error) = thread::Builder::new()
            .name("debug-feed-listener".into())
            .spawn(move || accept_clients(listener, listener_stop, listener_clients))
        {
            remove_socket(path);
            return Err(error);
        }

        let publisher_stop = Arc::clone(&stop);
        if let Err(error) = thread::Builder::new()
            .name("debug-feed-publisher".into())
            .spawn(move || publish_entries(publisher_stop, clients, initial_seq))
        {
            stop.store(true, Ordering::Release);
            let _ = UnixStream::connect(path);
            remove_socket(path);
            return Err(error);
        }

        Ok(Self {
            stop,
            path: path.to_path_buf(),
        })
    }

    /// Starts the user-scoped listener only when explicitly enabled.
    pub fn start_if_enabled() -> io::Result<Option<Self>> {
        if !enabled(std::env::var_os("OBSCURA_DEBUG_FEED").as_deref()) {
            return Ok(None);
        }
        Self::bind(&socket_path()).map(Some)
    }
}

impl Drop for FeedServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = UnixStream::connect(&self.path);
        remove_socket(&self.path);
    }
}

/// Whether the setting explicitly opts into the feed.
pub fn enabled(value: Option<&std::ffi::OsStr>) -> bool {
    value.is_some_and(|v| v == "1")
}

/// The per-user diagnostics socket.
pub fn socket_path() -> PathBuf {
    crate::companion::runtime_dir().join(SOCKET_NAME)
}

/// Connects a client to the opt-in GUI feed.
pub fn connect() -> io::Result<UnixStream> {
    UnixStream::connect(socket_path())
}

/// Sends a physical-action annotation. The server only accepts markers;
/// this connection cannot issue camera-control requests.
pub fn send_marker(text: &str) -> io::Result<()> {
    send_marker_at(&socket_path(), text)
}

fn send_marker_at(path: &Path, text: &str) -> io::Result<()> {
    validate_marker(text)?;
    let mut stream = UnixStream::connect(path)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let message = Message::Marker {
        text: text.to_owned(),
    };
    serde_json::to_writer(&mut stream, &message).map_err(io::Error::other)?;
    stream.write_all(b"\n")?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "diagnostics feed disconnected before confirming the marker",
            ));
        }
        match serde_json::from_str::<Message>(&line).map_err(io::Error::other)? {
            Message::Entry { entry } if entry.message == format!("User marker: {text}") => {
                return Ok(());
            }
            Message::Entry { .. } => {}
            Message::Marker { .. } => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "diagnostics feed sent a marker request instead of a log entry",
                ));
            }
        }
    }
}

fn validate_marker(text: &str) -> io::Result<()> {
    if text.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "marker text cannot be empty",
        ));
    }
    if text.len() > MAX_MARKER_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("marker text must be at most {MAX_MARKER_BYTES} bytes"),
        ));
    }
    if text.chars().any(char::is_control) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "marker text cannot contain control characters",
        ));
    }
    Ok(())
}

fn prepare_socket_path(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.file_type().is_socket() => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("refusing to replace non-socket path {}", path.display()),
            ));
        }
        Ok(_) => match UnixStream::connect(path) {
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AddrInUse,
                    format!(
                        "a diagnostics feed is already listening at {}",
                        path.display()
                    ),
                ));
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
                ) =>
            {
                fs::remove_file(path)?;
            }
            Err(error) => return Err(error),
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    Ok(())
}

fn remove_socket(path: &Path) {
    if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_socket()) {
        let _ = fs::remove_file(path);
    }
}

fn accept_clients(
    listener: UnixListener,
    stop: Arc<AtomicBool>,
    clients: Arc<Mutex<Vec<UnixStream>>>,
) {
    while !stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, _)) => {
                let reader = match stream.try_clone() {
                    Ok(reader) => reader,
                    Err(error) => {
                        log::warn(format!("Debug feed could not accept a client: {error}"));
                        continue;
                    }
                };
                if let Err(error) = stream.set_write_timeout(Some(Duration::from_millis(100))) {
                    log::warn(format!("Debug feed could not configure a client: {error}"));
                    continue;
                }
                clients.lock().unwrap().push(stream);
                let marker_stop = Arc::clone(&stop);
                if let Err(error) = thread::Builder::new()
                    .name("debug-feed-client".into())
                    .spawn(move || read_markers(reader, marker_stop))
                {
                    log::warn(format!("Debug feed could not read client markers: {error}"));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(POLL_INTERVAL);
            }
            Err(error) => {
                log::warn(format!("Debug feed listener stopped: {error}"));
                break;
            }
        }
    }
}

fn read_markers(stream: UnixStream, stop: Arc<AtomicBool>) {
    let mut reader = BufReader::new(stream);
    let mut line = Vec::new();
    while !stop.load(Ordering::Acquire) {
        line.clear();
        match reader
            .by_ref()
            .take((MAX_MARKER_BYTES + 128) as u64)
            .read_until(b'\n', &mut line)
        {
            Ok(0) => break,
            Ok(_) if line.len() > MAX_MARKER_BYTES + 127 => break,
            Ok(_) => {
                let message = match serde_json::from_slice::<Message>(&line) {
                    Ok(message) => message,
                    Err(_) => break,
                };
                match message {
                    Message::Marker { text } if validate_marker(&text).is_ok() => {
                        log::info(format!("User marker: {text}"));
                    }
                    Message::Marker { .. } | Message::Entry { .. } => break,
                }
            }
            Err(_) => break,
        }
    }
}

fn publish_entries(stop: Arc<AtomicBool>, clients: Arc<Mutex<Vec<UnixStream>>>, mut last: u64) {
    while !stop.load(Ordering::Acquire) {
        for entry in log::since(last) {
            last = entry.seq;
            let message = match serde_json::to_vec(&Message::Entry { entry }) {
                Ok(message) => message,
                Err(error) => {
                    log::warn(format!(
                        "Debug feed could not encode an activity entry: {error}"
                    ));
                    continue;
                }
            };
            let mut line = message;
            line.push(b'\n');
            clients
                .lock()
                .unwrap()
                .retain_mut(|client| client.write_all(&line).is_ok());
        }
        thread::sleep(POLL_INTERVAL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;
    use std::os::unix::fs::MetadataExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_socket() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir()
            .join(format!(
                "obscura-debug-feed-{}-{unique}",
                std::process::id()
            ))
            .join(SOCKET_NAME)
    }

    #[test]
    fn opt_in_requires_explicit_one() {
        assert!(enabled(Some(std::ffi::OsStr::new("1"))));
        assert!(!enabled(None));
        assert!(!enabled(Some(std::ffi::OsStr::new("0"))));
        assert!(!enabled(Some(std::ffi::OsStr::new("true"))));
    }

    #[test]
    fn markers_are_bounded_and_safe() {
        assert!(validate_marker("gesture").is_ok());
        assert!(validate_marker(" ").is_err());
        assert!(validate_marker("line\nbreak").is_err());
        assert!(validate_marker(&"x".repeat(MAX_MARKER_BYTES + 1)).is_err());
    }

    #[test]
    fn socket_is_private_and_existing_files_are_not_replaced() {
        let path = temp_socket();
        let server = FeedServer::bind(&path).unwrap();
        let mode = fs::metadata(&path).unwrap().mode() & 0o777;
        assert_eq!(mode, 0o600);
        drop(server);

        let parent = path.parent().unwrap();
        fs::create_dir_all(parent).unwrap();
        fs::write(&path, b"keep").unwrap();
        let error = match FeedServer::bind(&path) {
            Ok(_) => panic!("refused path should not bind"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&path).unwrap(), b"keep");
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn clients_receive_ordered_entries_and_disconnect_cleanly() {
        let path = temp_socket();
        let server = FeedServer::bind(&path).unwrap();
        let first = UnixStream::connect(&path).unwrap();
        let second = UnixStream::connect(&path).unwrap();
        first
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        second
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        thread::sleep(Duration::from_millis(50));

        let mut first_reader = BufReader::new(first);
        let mut second_reader = BufReader::new(second);
        let first_marker = format!("first marker {}", path.display());
        send_marker_at(&path, &first_marker).unwrap();
        let first_seq = read_until_marker(&mut first_reader, &first_marker, 0);
        let second_seq = read_until_marker(&mut second_reader, &first_marker, 0);
        assert_eq!(first_seq, second_seq);
        drop(first_reader);

        let second_marker = format!("second marker {}", path.display());
        send_marker_at(&path, &second_marker).unwrap();
        assert!(read_until_marker(&mut second_reader, &second_marker, second_seq) > second_seq);
        drop(server);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    fn read_until_marker(reader: &mut BufReader<UnixStream>, text: &str, mut last_seq: u64) -> u64 {
        let mut line = String::new();
        loop {
            line.clear();
            reader.read_line(&mut line).unwrap();
            let message: Message = serde_json::from_str(&line).unwrap();
            let Message::Entry { entry } = message else {
                panic!("expected a log entry");
            };
            assert!(entry.seq > last_seq, "feed entries must be ordered");
            last_seq = entry.seq;
            if entry.message == format!("User marker: {text}") {
                return last_seq;
            }
        }
    }
}
