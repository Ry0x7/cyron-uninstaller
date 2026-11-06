use crate::scan::{self, ScanProgress, ScanResult};
use crate::{apps, caches, duplicates, history, mft, volumes, AppState};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Emitter, State};

const SCAN_EVENT: &str = "cyron-scan-progress";

/* --------------------------------------------------------------- validation */

fn validate_job_id(job_id: &str) -> Result<String, String> {
    let trimmed = job_id.trim();
    if trimmed.is_empty() {
        return Err("A scan job id is required.".into());
    }
    if trimmed.len() > 64 {
        return Err("The scan job id is too long.".into());
    }
    if !trimmed
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err("The scan job id contains unsupported characters.".into());
    }
    Ok(trimmed.to_string())
}

fn validate_existing(path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("No path was provided.".into());
    }
    let raw = PathBuf::from(trimmed);
    if !raw.is_absolute() {
        return Err(format!("The path must be absolute: {trimmed}"));
    }
    if !raw.exists() {
        return Err(format!("That path does not exist: {trimmed}"));
    }
    raw.canonicalize()
        .map_err(|error| format!("Windows refused access to {trimmed}: {error}"))
}

fn validate_directory(path: &str) -> Result<PathBuf, String> {
    let canonical = validate_existing(path)?;
    if !canonical.is_dir() {
        return Err(format!("That path is not a folder: {path}"));
    }
    Ok(canonical)
}

/// Keep concurrent jobs addressable by an unambiguous cancellation id. Without
/// this check a repeated id replaces the first job's flag and its eventual
/// cleanup can also remove the newer job from the registry.
fn register_scan(
    registry: &std::sync::Mutex<std::collections::HashMap<String, Arc<AtomicBool>>>,
    job_id: &str,
    cancel: Arc<AtomicBool>,
    label: &str,
) -> Result<(), String> {
    let mut scans = registry
        .lock()
        .map_err(|_| format!("The {label} scan registry is unavailable."))?;
    if scans.contains_key(job_id) {
        return Err(format!("A {label} scan with this job id is already running."));
    }
    scans.insert(job_id.to_string(), cancel);
    Ok(())
}

/* -------------------------------------------------------------- background */

async fn run_blocking<T, F>(label: &str, work: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| format!("{label} failed: {error}"))
}

/* ---------------------------------------------------------------- commands */

#[tauri::command]
pub async fn list_installed_apps() -> Result<apps::AppInventory, String> {
    run_blocking("Application discovery", apps::discover_apps).await
}

#[tauri::command]
pub fn list_volumes() -> Vec<volumes::VolumeInfo> {
    volumes::list_volumes()
}

#[tauri::command]
pub async fn list_caches() -> Result<caches::CacheInventory, String> {
    run_blocking("Cache discovery", caches::discover_caches).await
}

#[tauri::command]
pub async fn start_scan(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    job_id: String,
    path: String,
    options: scan::ScanOptions,
) -> Result<ScanResult, String> {
    let job_id = validate_job_id(&job_id)?;
    let root = validate_directory(&path)?;

    let mut options = options;
    options.max_depth = options.max_depth.clamp(1, 64);
    options.exclusions = options
        .exclusions
        .into_iter()
        .map(|entry| entry.trim().to_string())
        .filter(|entry| !entry.is_empty())
        .take(200)
        .collect();
    // Auto uses the MFT only for a whole volume, where reading the table pays
    // for itself. A subfolder scan is faster through the directory walk because
    // the MFT path must parse every record on the volume first.
    let method = match options.method.trim().to_ascii_lowercase().as_str() {
        "mft" => "mft",
        "directory" => "directory",
        _ if root.parent().is_none() => "mft",
        _ => "directory",
    }
    .to_string();
    options.method = method.clone();

    let cancel = Arc::new(AtomicBool::new(false));
    register_scan(&state.scans, &job_id, Arc::clone(&cancel), "filesystem")?;

    let scans = Arc::clone(&state.scans);
    let cleanup_job = job_id.clone();
    let task = tauri::async_runtime::spawn_blocking(move || {
        let emit_target = app.clone();
        let progress = move |payload: ScanProgress| {
            // A closed window only means nobody is listening.
            let _ = emit_target.emit(SCAN_EVENT, payload);
        };
        let started = SystemTime::now();
        let mut note: Option<String> = None;
        let mut result = if method == "directory" {
            scan::scan_directory(&job_id, &root, &options, cancel, &progress)
        } else {
            let mft_attempt = mft::support(&root).and_then(|()| {
                mft::scan(&job_id, &root, &options, Arc::clone(&cancel), &progress)
            });
            match mft_attempt {
                Ok(outcome) => mft_scan_result(&job_id, &root, outcome, &started),
                Err(reason) => {
                    // An explicit MFT request still falls back rather than
                    // failing the scan; the note explains why.
                    note = Some(reason);
                    scan::scan_directory(&job_id, &root, &options, cancel, &progress)
                }
            }
        };
        result.method_note = note;
        let elapsed = started.elapsed().unwrap_or_default().as_millis() as u64;

        let status = if result.cancelled {
            "cancelled"
        } else if result.file_count == 0 && !result.errors.is_empty() {
            "failed"
        } else {
            "completed"
        };
        history::push_history(
            &app,
            scan_history_entry(&root, &result, status, elapsed.max(result.elapsed_ms)),
        );
        result
    });

    let outcome = task.await;
    if let Ok(mut scans) = scans.lock() {
        scans.remove(&cleanup_job);
    }
    outcome.map_err(|error| format!("The scan task stopped unexpectedly: {error}"))
}

#[tauri::command]
pub fn cancel_scan(state: State<'_, AppState>, job_id: String) {
    let Ok(scans) = state.scans.lock() else {
        return;
    };
    if let Some(flag) = scans.get(job_id.trim()) {
        flag.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

#[tauri::command]
pub async fn start_duplicate_scan(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    job_id: String,
    path: String,
    options: duplicates::DuplicateOptions,
) -> Result<duplicates::DuplicateReport, String> {
    let job_id = validate_job_id(&job_id)?;
    let root = validate_directory(&path)?;

    let mut options = options;
    options.exclusions = options
        .exclusions
        .into_iter()
        .map(|entry| entry.trim().to_string())
        .filter(|entry| !entry.is_empty())
        .take(200)
        .collect();

    let cancel = Arc::new(AtomicBool::new(false));
    register_scan(
        &state.duplicate_scans,
        &job_id,
        Arc::clone(&cancel),
        "duplicate",
    )?;

    let scans = Arc::clone(&state.duplicate_scans);
    let cleanup_job = job_id.clone();
    let task = tauri::async_runtime::spawn_blocking(move || {
        let emit_target = app.clone();
        let progress = move |payload: duplicates::DuplicateProgress| {
            let _ = emit_target.emit(duplicates::DUPLICATE_EVENT, payload);
        };
        duplicates::find_duplicates(&job_id, &root, &options, cancel, &progress)
    });

    let outcome = task.await;
    if let Ok(mut scans) = scans.lock() {
        scans.remove(&cleanup_job);
    }
    outcome.map_err(|error| format!("The duplicate scan task stopped unexpectedly: {error}"))
}

#[tauri::command]
pub fn cancel_duplicate_scan(state: State<'_, AppState>, job_id: String) {
    let Ok(scans) = state.duplicate_scans.lock() else {
        return;
    };
    if let Some(flag) = scans.get(job_id.trim()) {
        flag.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

#[tauri::command]
pub async fn measure_path(path: String) -> Result<scan::Measured, String> {
    let root = validate_directory(&path)?;
    run_blocking("Folder measurement", move || scan::measure_path(&root)).await
}

#[tauri::command]
pub fn reveal_path(path: String) -> Result<(), String> {
    let target = validate_existing(&path)?;
    let mut command = std::process::Command::new("explorer.exe");
    if target.is_dir() {
        command.arg(&target);
    } else {
        command.arg(format!("/select,{}", target.display()));
    }
    command
        .spawn()
        .map_err(|error| format!("Could not open Explorer: {error}"))?;
    Ok(())
}

#[tauri::command]
pub fn get_history(app: tauri::AppHandle) -> Vec<history::HistoryEntry> {
    history::load_history(&app)
}

#[tauri::command]
pub fn clear_history(app: tauri::AppHandle) -> Result<(), String> {
    history::clear(&app)
}

/* ----------------------------------------------------------------- helpers */

/// Adapts an MFT outcome to the same contract the directory walk returns, so
/// the UI does not have to branch on the scanner.
fn mft_scan_result(
    job_id: &str,
    root: &std::path::Path,
    outcome: mft::MftOutcome,
    started: &SystemTime,
) -> ScanResult {
    ScanResult {
        job_id: job_id.to_string(),
        root_path: scan::display_path(&root.to_string_lossy()),
        root: outcome.root,
        total_size: outcome.total_size,
        total_allocated: outcome.total_allocated,
        file_count: outcome.file_count,
        dir_count: outcome.dir_count,
        deduped_files: outcome.deduped_files,
        skipped_reparse: outcome.skipped_reparse,
        errors: outcome.errors,
        extension_stats: outcome.extension_stats,
        largest_files: outcome.largest_files,
        elapsed_ms: started.elapsed().unwrap_or_default().as_millis() as u64,
        cancelled: outcome.cancelled,
        truncated: outcome.truncated,
        method: "mft".into(),
        method_note: None,
        categories: outcome.categories,
        age_buckets: outcome.age_buckets,
        insights: outcome.insights,
    }
}

/// Two scans can finish in the same second with the same file count, so the
/// id also carries sub-second precision and a process-local sequence.
fn scan_history_id(at: u64, nanos: u32) -> String {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("scan-{at}-{nanos}-{sequence}")
}

fn scan_history_entry(
    root: &std::path::Path,
    result: &ScanResult,
    status: &str,
    elapsed_ms: u64,
) -> history::HistoryEntry {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let at = since_epoch.as_secs();
    let detail = format!(
        "{} files in {} folders · {} · {}",
        result.file_count,
        result.dir_count,
        human_bytes(result.total_allocated),
        human_duration(elapsed_ms),
    );
    let title = root.display().to_string();
    history::HistoryEntry {
        id: scan_history_id(at, since_epoch.subsec_nanos()),
        kind: "scan".into(),
        title,
        detail,
        at,
        bytes: result.total_allocated,
        items: result.file_count,
        status: status.into(),
    }
}

fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else if value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else if value >= 10.0 {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.2} {}", UNITS[unit])
    }
}

fn human_duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms} ms")
    } else if ms < 60_000 {
        format!("{:.1} s", ms as f64 / 1000.0)
    } else {
        format!("{} m {} s", ms / 60_000, (ms % 60_000) / 1000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cyron-commands-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|value| value.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).expect("temp directory");
        dir
    }

    #[test]
    fn rejects_empty_and_relative_paths() {
        assert!(validate_existing("   ").is_err());
        assert!(validate_existing("relative\\folder").is_err());
    }

    #[test]
    fn rejects_missing_paths() {
        let missing = std::env::temp_dir().join("cyron-definitely-missing-folder-xyz");
        assert!(validate_existing(&missing.to_string_lossy()).is_err());
    }

    #[test]
    fn accepts_an_existing_folder() {
        let dir = temp_dir();
        let validated = validate_directory(&dir.to_string_lossy()).expect("existing folder");
        assert!(validated.is_dir());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_a_file_as_scan_root() {
        let dir = temp_dir();
        let file = dir.join("note.txt");
        std::fs::write(&file, "hello").expect("write file");
        assert!(validate_directory(&file.to_string_lossy()).is_err());
        assert!(validate_existing(&file.to_string_lossy()).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn job_ids_are_restricted() {
        assert_eq!(
            validate_job_id(" 6f1c2b7e-3d5a-4b6e-9f00-123456789abc ")
                .unwrap()
                .len(),
            36
        );
        assert!(validate_job_id("").is_err());
        assert!(validate_job_id("scan/one").is_err());
        assert!(validate_job_id(&"a".repeat(65)).is_err());
    }

    #[test]
    fn duplicate_job_ids_do_not_replace_the_cancellable_scan() {
        let registry = std::sync::Mutex::new(std::collections::HashMap::new());
        let original = Arc::new(AtomicBool::new(false));
        let replacement = Arc::new(AtomicBool::new(false));

        register_scan(&registry, "same-job", Arc::clone(&original), "duplicate")
            .expect("first job registers");
        let error = register_scan(&registry, "same-job", Arc::clone(&replacement), "duplicate")
            .expect_err("duplicate job id must be rejected");

        assert!(error.contains("already running"));
        let scans = registry.lock().expect("registry lock");
        assert!(Arc::ptr_eq(scans.get("same-job").expect("original remains"), &original));
        assert!(!replacement.load(Ordering::Relaxed));
    }

    #[test]
    fn scan_history_ids_never_collide_within_a_process() {
        let first = scan_history_id(1_700_000_000, 0);
        let second = scan_history_id(1_700_000_000, 0);
        assert_ne!(first, second);
        assert!(first.starts_with("scan-1700000000-0-"));
    }

    #[test]
    fn byte_formatting_is_readable() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(999), "999 B");
        assert_eq!(human_bytes(1024), "1.00 KB");
        assert_eq!(human_bytes(12_345_678), "11.8 MB");
    }
}
