//! Persistent installed/removed application lifecycle records.
//!
//! The inventory is authoritative for what is currently installed. This store
//! keeps a bounded history so a removal survives restarts and so later remnant
//! review can cite when an application disappeared. It is descriptive only: no
//! cleanup decision may treat a history match as proof of ownership.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Manager;

const HISTORY_LIMIT: usize = 2000;
const FILE_NAME: &str = "app-history.json";

/// Serialises read-modify-write access so concurrent inventory refreshes cannot
/// drop each other's lifecycle updates.
static HISTORY_LOCK: Mutex<()> = Mutex::new(());

fn lock_history() -> MutexGuard<'static, ()> {
    HISTORY_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppHistoryRecord {
    pub id: String,
    pub name: String,
    pub publisher: Option<String>,
    pub version: Option<String>,
    pub source: String,
    pub install_location: Option<String>,
    pub install_date: Option<String>,
    pub first_seen_at: u64,
    pub last_seen_at: u64,
    pub removed_at: Option<u64>,
    pub removal_method: Option<String>,
}

fn history_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|dir| dir.join(FILE_NAME))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Loads the persisted records. A missing or corrupt file reads as empty; a
/// malformed history must never stop the application from starting.
pub fn load(app: &tauri::AppHandle) -> Vec<AppHistoryRecord> {
    let _guard = lock_history();
    let Some(path) = history_path(app) else {
        return Vec::new();
    };
    load_from(&path)
}

/// Reconciles a complete inventory observation with the persisted history.
/// Incomplete inventories may add/update current applications but never mark a
/// missing application as removed.
pub fn sync_inventory(
    app: &tauri::AppHandle,
    inventory: &crate::apps::AppInventory,
) -> Vec<AppHistoryRecord> {
    let _guard = lock_history();
    let Some(path) = history_path(app) else {
        return Vec::new();
    };
    sync_at(&path, inventory, now_secs())
}

/// Records a verified removal for an already-known application. Unknown ids are
/// ignored rather than inventing a record with no install evidence.
pub fn record_removal(
    app: &tauri::AppHandle,
    app_id: &str,
    method: &str,
) -> Vec<AppHistoryRecord> {
    let _guard = lock_history();
    let Some(path) = history_path(app) else {
        return Vec::new();
    };
    record_removal_at(&path, app_id, method, now_secs())
}

fn load_from(path: &Path) -> Vec<AppHistoryRecord> {
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(mut records) = serde_json::from_str::<Vec<AppHistoryRecord>>(&raw) else {
        return Vec::new();
    };
    normalise(&mut records);
    records
}

fn sync_at(
    path: &Path,
    inventory: &crate::apps::AppInventory,
    now: u64,
) -> Vec<AppHistoryRecord> {
    let mut records = load_from(path);
    let before = serde_json::to_string(&records).unwrap_or_default();

    for app in &inventory.apps {
        match records.iter_mut().find(|record| record.id == app.id) {
            Some(record) => {
                record.name = app.name.clone();
                record.publisher = app.publisher.clone();
                record.version = app.version.clone();
                record.source = app.source.clone();
                record.install_location = app.install_location.clone();
                record.install_date = app.install_date.clone();
                record.last_seen_at = now;
                if record.removed_at.is_some() {
                    // The application is present in a live inventory again, so
                    // this is a reinstall rather than a removal.
                    record.removed_at = None;
                    record.removal_method = None;
                }
            }
            None => records.push(AppHistoryRecord {
                id: app.id.clone(),
                name: app.name.clone(),
                publisher: app.publisher.clone(),
                version: app.version.clone(),
                source: app.source.clone(),
                install_location: app.install_location.clone(),
                install_date: app.install_date.clone(),
                first_seen_at: now,
                last_seen_at: now,
                removed_at: None,
                removal_method: None,
            }),
        }
    }

    // A partial inventory is not evidence of absence. Current applications are
    // still refreshed above, but no stored record is retired.
    if inventory.warnings.is_empty() {
        for record in records.iter_mut() {
            if record.removed_at.is_none()
                && !inventory.apps.iter().any(|app| app.id == record.id)
            {
                record.removed_at = Some(now);
                record.last_seen_at = now;
                record.removal_method = Some("observed".into());
            }
        }
    }

    normalise(&mut records);
    let after = serde_json::to_string(&records).unwrap_or_default();
    if before != after {
        let _ = write_records(path, &records);
    }
    records
}

fn record_removal_at(
    path: &Path,
    app_id: &str,
    method: &str,
    now: u64,
) -> Vec<AppHistoryRecord> {
    let mut records = load_from(path);
    let before = serde_json::to_string(&records).unwrap_or_default();
    if let Some(record) = records.iter_mut().find(|record| record.id == app_id) {
        record.removed_at = Some(now);
        record.last_seen_at = now;
        record.removal_method = Some(method.to_string());
    }
    normalise(&mut records);
    let after = serde_json::to_string(&records).unwrap_or_default();
    if before != after {
        let _ = write_records(path, &records);
    }
    records
}

/// Newest activity first, capped to the persisted limit.
fn normalise(records: &mut Vec<AppHistoryRecord>) {
    records.sort_by(|left, right| {
        right
            .last_seen_at
            .cmp(&left.last_seen_at)
            .then_with(|| right.removed_at.cmp(&left.removed_at))
            .then_with(|| left.id.cmp(&right.id))
    });
    records.truncate(HISTORY_LIMIT);
}

/// Writes through a same-directory temporary file with a durable flush, then
/// publishes with a rename. A filesystem that refuses replacement falls back to
/// a direct copy and removes the temporary file.
fn write_records(path: &Path, records: &[AppHistoryRecord]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create the app history folder: {error}"))?;
    }
    let json = serde_json::to_string_pretty(records)
        .map_err(|error| format!("Could not encode app history: {error}"))?;
    let temp = path.with_extension("json.tmp");
    {
        let mut file = fs::File::create(&temp)
            .map_err(|error| format!("Could not create the app history temp file: {error}"))?;
        file.write_all(json.as_bytes())
            .map_err(|error| format!("Could not write app history: {error}"))?;
        file.flush()
            .map_err(|error| format!("Could not flush app history: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("Could not sync app history: {error}"))?;
    }
    if fs::rename(&temp, path).is_err() {
        fs::copy(&temp, path)
            .map_err(|error| format!("Could not publish app history: {error}"))?;
        let _ = fs::remove_file(&temp);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apps::{AppInventory, InstalledApp};

    fn fixture_path(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "cyron-history-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("temp directory");
        root.join(FILE_NAME)
    }

    fn app(id: &str, name: &str) -> InstalledApp {
        InstalledApp {
            id: id.into(),
            name: name.into(),
            version: Some("1.0".into()),
            publisher: Some("Fixture Publisher".into()),
            install_location: Some(format!(r"C:\Program Files\{name}")),
            install_date: None,
            uninstall_string: None,
            quiet_uninstall_string: None,
            display_icon: None,
            estimated_size: None,
            hive: "machine-64".into(),
            scope: "machine".into(),
            architecture: "x64".into(),
            is_protected: false,
            source: "win32".into(),
            uninstall_support: "standard".into(),
            confidence: "confirmed".into(),
            detection_reason: None,
            registry_key_name: Some(id.into()),
        }
    }

    fn inventory(apps: Vec<InstalledApp>, warnings: Vec<String>) -> AppInventory {
        AppInventory { apps, warnings }
    }

    #[test]
    fn phase2_history_survives_reload() {
        let path = fixture_path("reload");
        let first = sync_at(&path, &inventory(vec![app("a", "Alpha")], vec![]), 100);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].first_seen_at, 100);
        assert_eq!(first[0].last_seen_at, 100);

        let reloaded = load_from(&path);
        assert_eq!(reloaded.len(), 1);
        assert_eq!(reloaded[0].id, "a");
        assert_eq!(reloaded[0].name, "Alpha");
        assert!(reloaded[0].removed_at.is_none());

        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn phase2_history_marks_removed_only_on_complete_inventory() {
        let path = fixture_path("removed");
        sync_at(&path, &inventory(vec![app("a", "Alpha")], vec![]), 100);

        // Partial inventory: the app is missing, but absence is not provable.
        let partial = sync_at(
            &path,
            &inventory(vec![], vec!["Could not read a hive".into()]),
            200,
        );
        assert!(partial[0].removed_at.is_none());

        let complete = sync_at(&path, &inventory(vec![], vec![]), 300);
        assert_eq!(complete[0].removed_at, Some(300));
        assert_eq!(complete[0].removal_method.as_deref(), Some("observed"));
        assert_eq!(complete[0].last_seen_at, 300);

        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn phase2_history_reinstall_clears_removal() {
        let path = fixture_path("reinstall");
        sync_at(&path, &inventory(vec![app("a", "Alpha")], vec![]), 100);
        sync_at(&path, &inventory(vec![], vec![]), 200);
        let removed = load_from(&path);
        assert_eq!(removed[0].removed_at, Some(200));
        let first_seen = removed[0].first_seen_at;

        let reinstalled = sync_at(&path, &inventory(vec![app("a", "Alpha")], vec![]), 300);
        assert_eq!(reinstalled[0].first_seen_at, first_seen);
        assert_eq!(reinstalled[0].last_seen_at, 300);
        assert!(reinstalled[0].removed_at.is_none());
        assert!(reinstalled[0].removal_method.is_none());

        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}
