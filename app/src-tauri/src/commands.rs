//! Tauri commands exposed to the Svelte frontend.

use std::path::PathBuf;
use std::sync::Mutex;

use obsbot_core::{discover, CameraInfo, Device, FeatureId, FeatureState, FirmwareInfo};
use serde::Serialize;
use tauri::State;

pub struct AppState(pub Mutex<Option<Device>>);

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[derive(Serialize)]
pub struct Snapshot {
    info: CameraInfo,
    profile_id: String,
    profile_name: String,
    /// Joystick can drive the gimbal by velocity (vendor command).
    gimbal_velocity: bool,
    /// Firmware version and serial number reported by the camera.
    firmware: Option<FirmwareInfo>,
    features: Vec<FeatureState>,
}

/// Every catalog feature, unsupported, so the UI can lay out before (or
/// without) a device.
#[tauri::command]
pub fn feature_catalog() -> Vec<FeatureState> {
    FeatureId::ALL
        .iter()
        .map(|&id| {
            let def = id.def();
            FeatureState {
                id,
                group: def.group,
                label: def.label,
                kind: def.kind,
                supported: false,
                active: true,
                value: None,
                reason: Some("no camera connected".into()),
            }
        })
        .collect()
}

#[derive(Serialize)]
pub struct AppOptions {
    /// Auto-open non-OBSBOT cameras too (`OBSBOT_ALL_CAMERAS=1`); handy for
    /// development without an OBSBOT attached.
    include_all: bool,
}

#[tauri::command]
pub fn app_options() -> AppOptions {
    AppOptions {
        include_all: std::env::var_os("OBSBOT_ALL_CAMERAS").is_some_and(|v| v != "0"),
    }
}

/// All USB cameras; OBSBOT ones first. Non-OBSBOT cameras get the generic
/// UVC profile when opened.
#[tauri::command]
pub fn list_cameras() -> Vec<CameraInfo> {
    let mut cameras = discover(true);
    cameras.sort_by_key(|c| !c.is_obsbot);
    cameras
}

#[tauri::command]
pub fn open_camera(state: State<AppState>, path: PathBuf) -> CmdResult<Snapshot> {
    let info = discover(true)
        .into_iter()
        .find(|c| c.path == path)
        .ok_or_else(|| format!("{} is no longer connected", path.display()))?;
    let device = Device::open_auto(info).map_err(err)?;
    let snapshot = Snapshot {
        info: device.info.clone(),
        profile_id: device.profile.id.clone(),
        profile_name: device.profile.name.clone(),
        gimbal_velocity: device.has_gimbal_velocity(),
        firmware: device.firmware_info(),
        features: device.read_all(),
    };
    *state.0.lock().unwrap() = Some(device);
    Ok(snapshot)
}

#[tauri::command]
pub fn get_features(state: State<AppState>) -> CmdResult<Vec<FeatureState>> {
    let guard = state.0.lock().unwrap();
    let device = guard.as_ref().ok_or("no camera open")?;
    // Per-feature read errors are reported in each FeatureState; this catches
    // the camera disappearing altogether (ENODEV after unplug or reboot).
    device.node().capability().map_err(err)?;
    Ok(device.read_all())
}

/// Writes one feature and returns every feature, since a write can change
/// others (e.g. turning off Auto WB activates Temperature).
#[tauri::command]
pub fn set_feature(
    state: State<AppState>,
    id: FeatureId,
    value: i64,
) -> CmdResult<Vec<FeatureState>> {
    let guard = state.0.lock().unwrap();
    let device = guard.as_ref().ok_or("no camera open")?;
    device.set(id, value).map_err(err)?;
    Ok(device.read_all())
}

/// Joystick velocity: `right`/`up` in -1..1; (0, 0) stops.
#[tauri::command]
pub fn gimbal_move(state: State<AppState>, right: f32, up: f32) -> CmdResult<()> {
    let guard = state.0.lock().unwrap();
    let device = guard.as_ref().ok_or("no camera open")?;
    device.gimbal_velocity(right, up).map_err(err)
}

/// Whether the panel indicator starts at login (and stays running after the
/// app closes).
#[tauri::command]
pub fn get_autostart() -> bool {
    obsbot_core::companion::autostart_enabled()
}

#[tauri::command]
pub fn set_autostart(enabled: bool) -> CmdResult<bool> {
    obsbot_core::companion::set_autostart(enabled).map_err(err)?;
    Ok(obsbot_core::companion::autostart_enabled())
}
