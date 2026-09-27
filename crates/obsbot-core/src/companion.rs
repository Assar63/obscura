//! Coordination between the OBSCura GUI and its panel indicator.
//!
//! * **Instances**: each program records its PID in the user's runtime
//!   directory, so the other can tell whether it's running and ask it to
//!   quit (SIGTERM). Inside the snap, `$XDG_RUNTIME_DIR` is shared by all of
//!   the snap's apps.
//! * **Autostart**: the indicator's XDG autostart entry, enabled by default
//!   the first time either program runs. Inside the snap, `$HOME/.config`
//!   is `$SNAP_USER_DATA/.config`, which is where snapd looks for autostart
//!   files.

use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::{Child, Command};

use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;

const AUTOSTART_FILE: &str = "obscura-indicator.desktop";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Gui,
    Indicator,
}

impl Role {
    fn binary(self) -> &'static str {
        match self {
            Role::Gui => "obscura",
            Role::Indicator => "obscura-indicator",
        }
    }

    fn pid_file(self) -> PathBuf {
        runtime_dir().join(format!("{}.pid", self.binary()))
    }
}

fn runtime_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join(format!("obscura-{}", nix::unistd::getuid())))
        .join("obscura")
}

/// Records this process as the running instance of `role`.
pub fn register(role: Role) -> io::Result<()> {
    let path = role.pid_file();
    fs::create_dir_all(path.parent().unwrap())?;
    fs::write(path, std::process::id().to_string())
}

/// Removes this process's record, if it's still ours.
pub fn unregister(role: Role) {
    let path = role.pid_file();
    if read_pid(role) == Some(std::process::id() as i32) {
        let _ = fs::remove_file(path);
    }
}

fn read_pid(role: Role) -> Option<i32> {
    fs::read_to_string(role.pid_file())
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// PID of the running instance of `role`, if any (stale files are ignored).
pub fn running(role: Role) -> Option<i32> {
    let pid = read_pid(role)?;
    if pid as u32 == std::process::id() {
        return None;
    }
    kill(Pid::from_raw(pid), None).ok()?;
    // Guard against PID reuse. comm is truncated to 15 bytes; if it can't be
    // read (confinement), trust the signal check.
    if let Ok(comm) = fs::read_to_string(format!("/proc/{pid}/comm")) {
        let name = role.binary();
        if !name.starts_with(comm.trim()) {
            return None;
        }
    }
    Some(pid)
}

/// Asks the running instance of `role` to quit. Returns whether one was found.
pub fn terminate(role: Role) -> bool {
    let Some(pid) = running(role) else {
        return false;
    };
    let sent = kill(Pid::from_raw(pid), Signal::SIGTERM).is_ok();
    // SIGTERM ends it without a chance to clean up; drop its record here.
    if sent && read_pid(role) == Some(pid) {
        let _ = fs::remove_file(role.pid_file());
    }
    sent
}

/// Path to a sibling binary (dev builds and the snap install both binaries
/// side by side), falling back to `$PATH`.
fn sibling(role: Role) -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(role.binary())))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from(role.binary()))
}

/// Starts `role` in its own process group, so it outlives this process and
/// isn't hit by a Ctrl-C aimed at it. The child is reaped in the background.
pub fn launch(role: Role) -> io::Result<()> {
    use std::os::unix::process::CommandExt;
    let child: Child = Command::new(sibling(role)).process_group(0).spawn()?;
    std::thread::spawn(move || {
        let mut child = child;
        let _ = child.wait();
    });
    Ok(())
}

fn autostart_path() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(config.join("autostart").join(AUTOSTART_FILE))
}

/// Whether the indicator starts at login.
pub fn autostart_enabled() -> bool {
    autostart_path().is_some_and(|p| p.exists())
}

pub fn set_autostart(enable: bool) -> io::Result<()> {
    let Some(path) = autostart_path() else {
        return Ok(());
    };
    if !enable {
        return match fs::remove_file(&path) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    // snapd launches the app named by the file; elsewhere use the real path.
    let exec = if std::env::var_os("SNAP").is_some() {
        "obscura.indicator".to_string()
    } else {
        sibling(Role::Indicator).display().to_string()
    };
    fs::create_dir_all(path.parent().unwrap())?;
    fs::write(
        &path,
        format!(
            "[Desktop Entry]\nType=Application\nName=OBSCura Indicator\n\
             Comment=Quick OBSBOT camera controls in the panel\n\
             Exec={exec}\nIcon=camera-web\nTerminal=false\n\
             X-GNOME-Autostart-enabled=true\n"
        ),
    )
}

/// Where the "defaults applied" marker lives: `$SNAP_USER_COMMON` in the
/// snap (unversioned, survives refreshes), else `$XDG_STATE_HOME/obscura`.
fn state_dir() -> Option<PathBuf> {
    if let Some(common) = std::env::var_os("SNAP_USER_COMMON") {
        return Some(PathBuf::from(common));
    }
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    Some(state.join("obscura"))
}

/// Enables autostart the first time OBSCura (GUI or indicator) runs. A
/// marker records that this happened, so turning it off later sticks.
pub fn apply_first_run_defaults() {
    let Some(dir) = state_dir() else { return };
    let marker = dir.join("autostart-initialized");
    if marker.exists() {
        return;
    }
    if set_autostart(true).is_ok() {
        let _ = fs::create_dir_all(&dir);
        let _ = fs::write(marker, "");
    }
}
