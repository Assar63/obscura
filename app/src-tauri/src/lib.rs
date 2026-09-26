mod commands;
mod preview;

use std::sync::Mutex;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
            preview::start_preview,
            preview::preview_ready,
            preview::stop_preview,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
