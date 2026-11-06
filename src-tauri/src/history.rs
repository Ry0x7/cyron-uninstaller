use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use tauri::Manager;

const HISTORY_LIMIT: usize = 100;

/// Serialises read-modify-write access so two scans finishing together cannot
/// drop each other's history entry.
static HISTORY_LOCK: Mutex<()> = Mutex::new(());

fn lock_history() -> MutexGuard<'static, ()> {
    HISTORY_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    /// scan, cache-scan or app-scan.
    pub kind: String,
    pub title: String,
    pub detail: String,
    /// Seconds since the Unix epoch.
    pub at: u64,
    pub bytes: u64,
    pub items: u64,
    /// completed, cancelled or failed.
    pub status: String,
}

fn history_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|dir| dir.join("history.json"))
}

pub fn load_history(app: &tauri::AppHandle) -> Vec<HistoryEntry> {
    let _guard = lock_history();
    let Some(path) = history_path(app) else {
        return Vec::new();
    };
    load_from(&path)
}

fn load_from(path: &std::path::Path) -> Vec<HistoryEntry> {
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(mut entries) = serde_json::from_str::<Vec<HistoryEntry>>(&raw) else {
        // A corrupt log must never stop the application from starting.
        return Vec::new();
    };
    normalise(&mut entries, HISTORY_LIMIT);
    entries
}

pub fn push_history(app: &tauri::AppHandle, entry: HistoryEntry) {
    let _guard = lock_history();
    let Some(path) = history_path(app) else {
        return;
    };
    if let Some(dir) = path.parent() {
        if fs::create_dir_all(dir).is_err() {
            return;
        }
    }
    let mut entries = load_from(&path);
    normalise(&mut entries, HISTORY_LIMIT.saturating_sub(1));
    // Newest first, including entries that share the same whole second.
    entries.insert(0, entry);
    normalise(&mut entries, HISTORY_LIMIT);

    let Ok(raw) = serde_json::to_string_pretty(&entries) else {
        return;
    };
    let temp = path.with_extension("json.tmp");
    if fs::write(&temp, &raw).is_err() {
        return;
    }
    if fs::rename(&temp, &path).is_err() {
        // Some filesystems refuse to replace on rename; fall back to a direct write.
        let _ = fs::write(&path, &raw);
        let _ = fs::remove_file(&temp);
    }
}

pub fn clear(app: &tauri::AppHandle) -> Result<(), String> {
    let _guard = lock_history();
    let Some(path) = history_path(app) else {
        return Ok(());
    };
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Could not clear the activity log: {error}")),
    }
}

/// Newest first, capped to `limit`.
fn normalise(entries: &mut Vec<HistoryEntry>, limit: usize) {
    entries.sort_by(|left, right| right.at.cmp(&left.at));
    entries.truncate(limit);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, at: u64) -> HistoryEntry {
        HistoryEntry {
            id: id.into(),
            kind: "scan".into(),
            title: format!("scan {id}"),
            detail: String::new(),
            at,
            bytes: 0,
            items: 0,
            status: "completed".into(),
        }
    }

    #[test]
    fn newest_entry_comes_first() {
        let mut entries = vec![entry("a", 10), entry("b", 30), entry("c", 20)];
        normalise(&mut entries, 10);
        let ids: Vec<_> = entries.iter().map(|item| item.id.as_str()).collect();
        assert_eq!(ids, ["b", "c", "a"]);
    }

    #[test]
    fn entries_sharing_a_timestamp_keep_newest_first() {
        let mut entries = vec![entry("newer", 10), entry("older", 10)];
        normalise(&mut entries, 10);
        let ids: Vec<_> = entries.iter().map(|item| item.id.as_str()).collect();
        assert_eq!(ids, ["newer", "older"]);
    }

    #[test]
    fn history_is_capped() {
        let mut entries = (0..12)
            .map(|index| entry(&index.to_string(), index))
            .collect();
        normalise(&mut entries, 5);
        assert_eq!(entries.len(), 5);
        assert_eq!(entries.first().map(|item| item.id.as_str()), Some("11"));
    }

    #[test]
    fn corrupt_log_reads_as_empty() {
        let dir = std::env::temp_dir().join(format!(
            "cyron-history-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|value| value.as_nanos())
                .unwrap_or_default()
        ));
        fs::create_dir_all(&dir).expect("temp directory");
        let path = dir.join("history.json");
        fs::write(&path, "{ not json").expect("write corrupt log");
        assert!(load_from(&path).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
