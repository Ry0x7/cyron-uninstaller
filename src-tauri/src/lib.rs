pub mod apps;
pub mod caches;
pub mod cleanup;
pub mod commands;
pub mod duplicates;
pub mod fs;
pub mod history;
pub mod leftovers;
pub mod mft;
pub mod process;
pub mod scan;
pub mod storage;
pub mod uninstall;
pub mod volumes;

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

/// Tracks in-flight scans so the UI can cancel them, plus the single-use
/// cleanup and uninstallation plans a preview hands back and the backend read-only switch.
pub struct AppState {
    pub scans: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    pub duplicate_scans: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    pub plans: Arc<Mutex<HashMap<String, cleanup::CleanupPlan>>>,
    pub cleanup_jobs: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    pub uninstall_plans: Arc<Mutex<HashMap<String, uninstall::StoredUninstallPlan>>>,
    pub process_snapshots: Arc<Mutex<HashMap<String, process::StoredProcessSnapshot>>>,
    pub leftover_plans: Arc<Mutex<HashMap<String, leftovers::StoredLeftoverPlan>>>,
    /// True until the user deliberately enables cleaning.
    pub read_only: Arc<AtomicBool>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            scans: Arc::default(),
            duplicate_scans: Arc::default(),
            plans: Arc::default(),
            cleanup_jobs: Arc::default(),
            uninstall_plans: Arc::default(),
            process_snapshots: Arc::default(),
            leftover_plans: Arc::default(),
            read_only: Arc::new(AtomicBool::new(true)),
        }
    }
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
            commands::get_cleanup_mode,
            commands::set_cleanup_mode,
            commands::preview_cleanup,
            commands::start_cleanup,
            commands::list_quarantine,
            commands::restore_quarantine,
            commands::purge_quarantine,
            commands::get_cleanup_log,
            commands::prepare_uninstall,
            commands::discover_app_processes,
            commands::terminate_app_processes,
            commands::parse_uninstall_command,
            commands::execute_uninstall,
            commands::scan_app_leftovers,
            commands::clean_app_leftovers,
            commands::execute_batch_uninstall,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Cyron");
}
