pub mod apps;
pub mod caches;
pub mod commands;
pub mod duplicates;
pub mod fs;
pub mod history;
pub mod mft;
pub mod scan;
pub mod storage;
pub mod volumes;

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

/// Tracks in-flight scans so the UI can cancel them.
#[derive(Default)]
pub struct AppState {
    pub scans: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    pub duplicate_scans: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::list_installed_apps,
            commands::list_volumes,
            commands::list_caches,
            commands::start_scan,
            commands::cancel_scan,
            commands::start_duplicate_scan,
            commands::cancel_duplicate_scan,
            commands::measure_path,
            commands::reveal_path,
            commands::get_history,
            commands::clear_history,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Cyron");
}
