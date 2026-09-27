//! OBSCura tray indicator.
//!
//! A small StatusNotifierItem (AppIndicator) that stays running in the panel
//! so the heavy GUI doesn't have to. It offers quick camera toggles that are
//! handy during video calls, and starts/stops the `obscura` GUI on demand:
//! "Hide" really quits the GUI, freeing its memory.
//!
//! No GTK or webview here: the tray menu is rendered by the desktop shell
//! over D-Bus, and the camera is driven directly through `obsbot-core`.

use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::Duration;

use ksni::blocking::TrayMethods;
use ksni::menu::{CheckmarkItem, RadioGroup, RadioItem, StandardItem};
use ksni::MenuItem;
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use obsbot_core::{discover, Device, FeatureId};

/// Background refresh; the menu also refreshes whenever it's opened.
const POLL: Duration = Duration::from_secs(5);

/// AI tracking modes in menu order, with their device values.
const AI_MODES: &[(i64, &str)] = &[(0, "Off"), (2, "Human"), (1, "Group"), (3, "Hand tracking")];

/// On/off features worth having one click away during a call.
const TOGGLES: &[(FeatureId, &str)] = &[
    (FeatureId::GestureControl, "Gesture control"),
    (FeatureId::MirrorImage, "Mirror image"),
];

const AUTOSTART_FILE: &str = "obscura-indicator.desktop";

struct Indicator {
    device: Option<Device>,
    camera_name: String,
    /// Current AI mode, or None if the camera doesn't support it.
    ai_mode: Option<i64>,
    /// Supported toggles and their state (None when the camera can't report it).
    toggles: Vec<(FeatureId, &'static str, Option<bool>)>,
    can_center: bool,
    gui: Option<Child>,
    error: Option<String>,
}

impl Indicator {
    fn new() -> Self {
        let mut this = Self {
            device: None,
            camera_name: String::new(),
            ai_mode: None,
            toggles: Vec::new(),
            can_center: false,
            gui: None,
            error: None,
        };
        this.refresh();
        this
    }

    /// Re-reads GUI and camera state. One status-block read covers all the
    /// vendor features shown in the menu.
    fn refresh(&mut self) {
        if let Some(child) = &mut self.gui {
            if !matches!(child.try_wait(), Ok(None)) {
                self.gui = None;
            }
        }

        if self.device.as_ref().is_some_and(|d| !d.is_connected()) {
            self.device = None;
        }
        if self.device.is_none() {
            self.device = discover(false)
                .into_iter()
                .next()
                .and_then(|info| Device::open_auto(info).ok());
        }

        let Some(dev) = &self.device else {
            self.camera_name.clear();
            self.ai_mode = None;
            self.toggles.clear();
            self.can_center = false;
            return;
        };
        self.camera_name = dev
            .info
            .product
            .clone()
            .unwrap_or_else(|| dev.info.name.clone());

        let mut ids = vec![FeatureId::AiMode, FeatureId::Pan, FeatureId::Tilt];
        ids.extend(TOGGLES.iter().map(|(id, _)| *id));
        let states = dev.read(&ids);
        self.ai_mode = states[0].supported.then(|| states[0].value.unwrap_or(0));
        self.can_center = states[1].supported && states[2].supported;
        self.toggles = TOGGLES
            .iter()
            .zip(&states[3..])
            .filter(|(_, s)| s.supported)
            .map(|((id, label), s)| (*id, *label, s.value.map(|v| v != 0)))
            .collect();
    }

    fn set(&mut self, id: FeatureId, value: i64) {
        if let Some(dev) = &self.device {
            self.error = dev.set(id, value).err().map(|e| e.to_string());
        }
        self.refresh();
    }

    fn center(&mut self) {
        self.set(FeatureId::Pan, 0);
        self.set(FeatureId::Tilt, 0);
    }

    fn gui_running(&self) -> bool {
        self.gui.is_some()
    }

    fn show_gui(&mut self) {
        if self.gui_running() {
            return;
        }
        match Command::new(gui_binary()).spawn() {
            Ok(child) => {
                self.gui = Some(child);
                self.error = None;
            }
            Err(e) => self.error = Some(format!("couldn't start OBSCura: {e}")),
        }
    }

    /// Quits the GUI (not just hides it) so it stops using memory.
    fn hide_gui(&mut self) {
        if let Some(mut child) = self.gui.take() {
            let _ = kill(Pid::from_raw(child.id() as i32), Signal::SIGTERM);
            // Reap it off the D-Bus thread.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
    }

    fn toggle_gui(&mut self) {
        if self.gui_running() {
            self.hide_gui();
        } else {
            self.show_gui();
        }
    }
}

/// The GUI binary: next to this one (dev builds, the snap), else on PATH.
fn gui_binary() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("obscura")))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("obscura"))
}

/// `~/.config/autostart/obscura-indicator.desktop`. Inside the snap HOME is
/// `$SNAP_USER_DATA`, which is where snapd looks for an app's autostart file.
fn autostart_path() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(config.join("autostart").join(AUTOSTART_FILE))
}

fn autostart_enabled() -> bool {
    autostart_path().is_some_and(|p| p.exists())
}

fn set_autostart(enable: bool) -> std::io::Result<()> {
    let Some(path) = autostart_path() else {
        return Ok(());
    };
    if !enable {
        return match fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    let exec = if std::env::var_os("SNAP").is_some() {
        "obscura.indicator".to_string()
    } else {
        std::env::current_exe()?.display().to_string()
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

/// In the snap, start at login by default: the first time the indicator
/// runs it enables autostart (snapd launches apps whose autostart file is in
/// `$SNAP_USER_DATA/.config/autostart`). A marker in the unversioned
/// `$SNAP_USER_COMMON` records that this happened, so turning "Start at
/// login" off afterwards sticks across restarts and refreshes.
fn snap_first_run_autostart() {
    let Some(common) = std::env::var_os("SNAP_USER_COMMON").map(PathBuf::from) else {
        return;
    };
    let marker = common.join("autostart-initialized");
    if marker.exists() {
        return;
    }
    match set_autostart(true) {
        Ok(()) => {
            let _ = fs::write(&marker, "");
        }
        Err(e) => eprintln!("obscura-indicator: couldn't enable autostart: {e}"),
    }
}

impl ksni::Tray for Indicator {
    fn id(&self) -> String {
        "obscura".into()
    }

    fn title(&self) -> String {
        "OBSCura".into()
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::Hardware
    }

    fn icon_name(&self) -> String {
        "camera-web-symbolic".into()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let description = match (&self.device, self.ai_mode) {
            (None, _) => "No OBSBOT camera connected".to_string(),
            (Some(_), Some(mode)) => {
                let label = AI_MODES
                    .iter()
                    .find(|(v, _)| *v == mode)
                    .map_or("?", |(_, l)| l);
                format!("{} · AI tracking: {label}", self.camera_name)
            }
            (Some(_), None) => self.camera_name.clone(),
        };
        ksni::ToolTip {
            title: "OBSCura".into(),
            description,
            icon_name: "camera-web".into(),
            ..Default::default()
        }
    }

    /// Left click toggles the GUI (on hosts that don't just open the menu).
    fn activate(&mut self, _x: i32, _y: i32) {
        self.toggle_gui();
    }

    fn menu_about_to_show(&mut self) {
        self.refresh();
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let mut items: Vec<MenuItem<Self>> = Vec::new();

        let header = if self.device.is_some() {
            self.camera_name.clone()
        } else {
            "No OBSBOT camera connected".into()
        };
        items.push(
            StandardItem {
                label: header,
                enabled: false,
                ..Default::default()
            }
            .into(),
        );
        if let Some(err) = &self.error {
            items.push(
                StandardItem {
                    label: format!("⚠ {err}"),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
        }
        items.push(MenuItem::Separator);

        items.push(
            StandardItem {
                label: if self.gui_running() {
                    "Hide OBSCura"
                } else {
                    "Show OBSCura"
                }
                .into(),
                activate: Box::new(|this: &mut Self| this.toggle_gui()),
                ..Default::default()
            }
            .into(),
        );

        if let Some(mode) = self.ai_mode {
            items.push(MenuItem::Separator);
            items.push(
                StandardItem {
                    label: "AI tracking".into(),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
            items.push(
                RadioGroup {
                    selected: AI_MODES.iter().position(|(v, _)| *v == mode).unwrap_or(0),
                    select: Box::new(|this: &mut Self, index| {
                        this.set(FeatureId::AiMode, AI_MODES[index].0);
                    }),
                    options: AI_MODES
                        .iter()
                        .map(|(_, label)| RadioItem {
                            label: (*label).into(),
                            ..Default::default()
                        })
                        .collect(),
                }
                .into(),
            );
        }

        if !self.toggles.is_empty() || self.can_center {
            items.push(MenuItem::Separator);
        }
        for &(id, label, state) in &self.toggles {
            let checked = state.unwrap_or(false);
            items.push(
                CheckmarkItem {
                    label: label.into(),
                    checked,
                    activate: Box::new(move |this: &mut Self| this.set(id, (!checked) as i64)),
                    ..Default::default()
                }
                .into(),
            );
        }
        if self.can_center {
            items.push(
                StandardItem {
                    label: "Re-center camera".into(),
                    activate: Box::new(|this: &mut Self| this.center()),
                    ..Default::default()
                }
                .into(),
            );
        }

        items.push(MenuItem::Separator);
        items.push(
            CheckmarkItem {
                label: "Start at login".into(),
                checked: autostart_enabled(),
                activate: Box::new(|this: &mut Self| {
                    this.error = set_autostart(!autostart_enabled())
                        .err()
                        .map(|e| format!("autostart: {e}"));
                }),
                ..Default::default()
            }
            .into(),
        );
        items.push(
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|this: &mut Self| {
                    this.hide_gui();
                    std::process::exit(0);
                }),
                ..Default::default()
            }
            .into(),
        );
        items
    }
}

fn main() {
    snap_first_run_autostart();
    let handle = match Indicator::new().spawn() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("obscura-indicator: couldn't register the tray icon: {e}");
            eprintln!(
                "On GNOME this needs the AppIndicator extension (enabled by default on Ubuntu)."
            );
            std::process::exit(1);
        }
    };
    loop {
        std::thread::sleep(POLL);
        if handle.update(Indicator::refresh).is_none() {
            break; // tray service shut down
        }
    }
}
