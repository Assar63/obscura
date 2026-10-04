mod commands;
mod preview;

use std::sync::Mutex;

use commands::AppState;
use obsbot_core::companion::{self, Role};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // The panel indicator runs alongside the app, whether the app was
    // started from the indicator or directly.
    companion::apply_first_run_defaults();
    if let Err(e) = companion::register(Role::Gui) {
        eprintln!("obscura: couldn't record instance: {e}");
    }
    if companion::running(Role::Indicator).is_none() {
        if let Err(e) = companion::launch(Role::Indicator) {
            eprintln!("obscura: couldn't start the panel indicator: {e}");
        }
    }

    tauri::Builder::default()
        .manage(AppState(Mutex::new(None)))
        .manage(preview::PreviewState::default())
        .invoke_handler(tauri::generate_handler![
            commands::app_options,
            commands::feature_catalog,
            commands::list_cameras,
            commands::open_camera,
            commands::get_features,
            commands::set_feature,
            commands::get_log,
            commands::live_status,
            commands::check_firmware_update,
            commands::toggle_diagnostics,
            commands::gimbal_move,
            commands::save_preset,
            commands::rename_preset,
            commands::recall_preset,
            commands::get_autostart,
            commands::set_autostart,
            preview::start_preview,
            preview::preview_ready,
            preview::stop_preview,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            if let tauri::RunEvent::Exit = event {
                companion::unregister(Role::Gui);
                // With autostart on, the indicator is meant to live in the
                // panel; otherwise it's only a companion to the open app.
                if !companion::autostart_enabled() {
                    companion::terminate(Role::Indicator);
                }
            }
        });
}
