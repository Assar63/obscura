//! OBSCura tray indicator.
//!
//! A small StatusNotifierItem (AppIndicator) that stays running in the panel
//! so the heavy GUI doesn't have to. It offers quick camera toggles that are
//! handy during video calls, and starts/stops the `obscura` GUI on demand:
//! "Hide" really quits the GUI, freeing its memory. The GUI and indicator
//! find each other through `obsbot_core::companion`, so Show/Hide also work
//! on a GUI that was started directly.
//!
//! No GTK or webview here: the tray menu is rendered by the desktop shell
//! over D-Bus, and the camera is driven directly through `obsbot-core`.

use std::time::Duration;

use ksni::blocking::TrayMethods;
use ksni::menu::{CheckmarkItem, RadioGroup, RadioItem, StandardItem, SubMenu};
use ksni::MenuItem;
use obsbot_core::companion::{self, Role};
use obsbot_core::virtualcam;
use obsbot_core::{discover, Device, FeatureId, FeatureKind};

/// Background refresh; the menu also refreshes whenever it's opened.
const POLL: Duration = Duration::from_secs(5);

/// On/off features worth having one click away during a call.
const TOGGLES: &[(FeatureId, &str)] = &[
    (FeatureId::GestureControl, "Gesture control"),
    (FeatureId::MirrorImage, "Mirror image"),
];

/// The application icon, embedded so it works without an installed theme.
const ICONS: &[&[u8]] = &[
    include_bytes!("../../../app/src-tauri/icons/32x32.png"),
    include_bytes!("../../../app/src-tauri/icons/64x64.png"),
];

/// Decodes an 8-bit RGBA PNG into the ARGB32 (network byte order) pixmap
/// that StatusNotifierItem expects.
fn load_icon(png_data: &[u8]) -> Option<ksni::Icon> {
    let mut reader = png::Decoder::new(std::io::Cursor::new(png_data))
        .read_info()
        .ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    let data = buf[..info.buffer_size()]
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|&[r, g, b, a]| [a, r, g, b])
        .collect();
    Some(ksni::Icon {
        width: info.width as i32,
        height: info.height as i32,
        data,
    })
}

struct Indicator {
    device: Option<Device>,
    camera_name: String,
    /// Current AI mode, or None if the camera doesn't support it.
    ai_mode: Option<i64>,
    /// The camera's AI modes in menu order, with their device values.
    ai_modes: Vec<(i64, String)>,
    /// Supported toggles and their state (None when the camera can't report it).
    toggles: Vec<(FeatureId, &'static str, Option<bool>)>,
    can_center: bool,
    /// The camera has a vendor re-center command (used by `center`).
    can_reset: bool,
    /// Filled camera presets: slot and name.
    presets: Vec<(u32, String)>,
    /// Connected wireless mics, e.g. "TX1: 89%, charging".
    mics: Vec<String>,
    gui_running: bool,
    /// The camera is being shared as the virtual camera (`obsbotctl share`).
    sharing: bool,
    icons: Vec<ksni::Icon>,
    error: Option<String>,
}

impl Indicator {
    fn new() -> Self {
        let mut this = Self {
            device: None,
            camera_name: String::new(),
            ai_mode: None,
            ai_modes: Vec::new(),
            toggles: Vec::new(),
            can_center: false,
            can_reset: false,
            presets: Vec::new(),
            mics: Vec::new(),
            gui_running: false,
            sharing: false,
            icons: ICONS.iter().filter_map(|png| load_icon(png)).collect(),
            error: None,
        };
        this.refresh();
        this
    }

    /// Re-reads GUI and camera state. One status-block read covers all the
    /// vendor features shown in the menu.
    fn refresh(&mut self) {
        self.gui_running = companion::running(Role::Gui).is_some();
        self.sharing = companion::running(Role::Share).is_some();

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
            self.presets.clear();
            self.mics.clear();
            return;
        };
        self.camera_name = dev
            .info
            .product
            .clone()
            .unwrap_or_else(|| dev.info.name.clone());

        let mut ids = vec![
            FeatureId::AiMode,
            FeatureId::Pan,
            FeatureId::Tilt,
            FeatureId::GimbalReset,
        ];
        ids.extend(TOGGLES.iter().map(|(id, _)| *id));
        let states = dev.read(&ids);
        self.ai_mode = states[0].supported.then(|| states[0].value.unwrap_or(0));
        self.ai_modes = match &states[0].kind {
            FeatureKind::Choice { options } => {
                options.iter().map(|o| (o.value, o.label.clone())).collect()
            }
            _ => Vec::new(),
        };
        self.can_reset = states[3].supported;
        self.can_center = self.can_reset || (states[1].supported && states[2].supported);
        self.toggles = TOGGLES
            .iter()
            .zip(&states[4..])
            .filter(|(_, s)| s.supported)
            .map(|((id, label), s)| (*id, *label, s.value.map(|v| v != 0)))
            .collect();
        self.presets = dev
            .presets()
            .unwrap_or_default()
            .into_iter()
            .zip(0..)
            .filter_map(|(name, slot)| Some((slot, name?)))
            .collect();
        self.mics = dev
            .wireless_mics()
            .unwrap_or_default()
            .into_iter()
            .filter(|m| m.connected)
            .map(|m| {
                let mut label = format!("TX{}", m.slot);
                if let Some(b) = m.battery {
                    label += &format!(": {b}%");
                }
                if m.charging {
                    label += ", charging";
                }
                if m.muted {
                    label += ", muted";
                }
                label
            })
            .collect();
    }

    fn recall_preset(&mut self, slot: u32) {
        if let Some(dev) = &self.device {
            self.error = dev.recall_preset(slot).err().map(|e| e.to_string());
        }
    }

    fn set(&mut self, id: FeatureId, value: i64) {
        if let Some(dev) = &self.device {
            self.error = dev.set(id, value).err().map(|e| e.to_string());
        }
        self.refresh();
    }

    fn center(&mut self) {
        if self.can_reset {
            self.set(FeatureId::GimbalReset, 1);
        } else {
            self.set(FeatureId::Pan, 0);
            self.set(FeatureId::Tilt, 0);
        }
    }

    fn show_gui(&mut self) {
        if self.gui_running {
            return;
        }
        match companion::launch(Role::Gui) {
            Ok(()) => {
                self.gui_running = true;
                self.error = None;
            }
            Err(e) => self.error = Some(format!("couldn't start OBSCura: {e}")),
        }
    }

    /// Quits the GUI (not just hides it) so it stops using memory.
    fn hide_gui(&mut self) {
        companion::terminate(Role::Gui);
        self.gui_running = false;
    }

    fn toggle_gui(&mut self) {
        if self.gui_running {
            self.hide_gui();
        } else {
            self.show_gui();
        }
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

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        self.icons.clone()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let description = match (&self.device, self.ai_mode) {
            (None, _) => "No OBSBOT camera connected".to_string(),
            (Some(_), Some(mode)) => {
                let label = self
                    .ai_modes
                    .iter()
                    .find(|(v, _)| *v == mode)
                    .map_or("?", |(_, l)| l.as_str());
                format!("{} · AI tracking: {label}", self.camera_name)
            }
            (Some(_), None) => self.camera_name.clone(),
        };
        let description = if self.mics.is_empty() {
            description
        } else {
            format!("{description}\nMics: {}", self.mics.join(" · "))
        };
        let description = if self.sharing {
            format!("{description}\nSharing as \"{}\"", virtualcam::LABEL)
        } else {
            description
        };
        ksni::ToolTip {
            title: "OBSCura".into(),
            description,
            icon_pixmap: self.icons.clone(),
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
        for mic in &self.mics {
            items.push(
                StandardItem {
                    label: format!("🎙 {mic}"),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
        }
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
                label: if self.gui_running {
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
                    selected: self
                        .ai_modes
                        .iter()
                        .position(|(v, _)| *v == mode)
                        .unwrap_or(0),
                    select: Box::new(|this: &mut Self, index| {
                        if let Some(&(value, _)) = this.ai_modes.get(index) {
                            this.set(FeatureId::AiMode, value);
                        }
                    }),
                    options: self
                        .ai_modes
                        .iter()
                        .map(|(_, label)| RadioItem {
                            label: label.clone(),
                            ..Default::default()
                        })
                        .collect(),
                }
                .into(),
            );
        }

        if !self.toggles.is_empty() || self.can_center || !self.presets.is_empty() {
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
        if self.sharing {
            items.push(
                StandardItem {
                    label: format!("Stop sharing \"{}\"", virtualcam::LABEL),
                    activate: Box::new(|this: &mut Self| {
                        companion::terminate(Role::Share);
                        this.sharing = false;
                    }),
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
        if !self.presets.is_empty() {
            items.push(
                SubMenu {
                    label: "Presets".into(),
                    submenu: self
                        .presets
                        .iter()
                        .map(|(slot, name)| {
                            let slot = *slot;
                            StandardItem {
                                label: name.clone(),
                                activate: Box::new(move |this: &mut Self| this.recall_preset(slot)),
                                ..Default::default()
                            }
                            .into()
                        })
                        .collect(),
                    ..Default::default()
                }
                .into(),
            );
        }

        items.push(MenuItem::Separator);
        items.push(
            CheckmarkItem {
                label: "Start at login".into(),
                checked: companion::autostart_enabled(),
                activate: Box::new(|this: &mut Self| {
                    this.error = companion::set_autostart(!companion::autostart_enabled())
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
                activate: Box::new(|_: &mut Self| {
                    companion::unregister(Role::Indicator);
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
    companion::apply_first_run_defaults();
    // One indicator per session: the GUI starts one, and so may autostart.
    if companion::running(Role::Indicator).is_some() {
        return;
    }
    if let Err(e) = companion::register(Role::Indicator) {
        eprintln!("obscura-indicator: couldn't record instance: {e}");
    }
    // At login the indicator can start before the desktop shell's AppIndicator
    // extension has registered its StatusNotifierWatcher; without this, ksni
    // treats that as fatal and exits instead of waiting for it to appear.
    let handle = match Indicator::new().assume_sni_available(true).spawn() {
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
