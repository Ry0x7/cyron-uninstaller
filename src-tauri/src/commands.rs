use crate::scan::{self, ScanProgress, ScanResult};
use crate::{apps, caches, cleanup, duplicates, history, leftovers, mft, process, uninstall, volumes, AppState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager, State};

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

/* ------------------------------------------------------------------ cleanup */

const READ_ONLY_MESSAGE: &str =
    "Read-only mode is on. Enable cleaning in Settings before Cyron can move any file.";
const MAX_CLEANUP_TARGETS: usize = 64;

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn app_data_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|error| format!("Cyron cannot locate its application data folder: {error}"))
}

fn cleanup_quarantine_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let root = app_data_dir(app)?.join("quarantine");
    std::fs::create_dir_all(&root)
        .map_err(|error| format!("Cyron cannot prepare its quarantine folder: {error}"))?;
    Ok(root)
}

/// Catalog ids are the only thing the frontend may name. Anything that is not
/// a short identifier is rejected before it can reach the planner.
fn validate_target_ids(target_ids: Vec<String>) -> Result<Vec<String>, String> {
    let mut result: Vec<String> = Vec::new();
    for raw in target_ids {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err("A cleanup location id is empty.".into());
        }
        if trimmed.len() > 64 {
            return Err(format!("A cleanup location id is too long: {trimmed}"));
        }
        if !trimmed
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
        {
            return Err(format!(
                "A cleanup location id contains unsupported characters: {trimmed}"
            ));
        }
        if !result.iter().any(|existing| existing == trimmed) {
            result.push(trimmed.to_string());
        }
    }
    if result.is_empty() {
        return Err("Select at least one cleanup location first.".into());
    }
    if result.len() > MAX_CLEANUP_TARGETS {
        return Err(format!(
            "Select at most {MAX_CLEANUP_TARGETS} cleanup locations at once."
        ));
    }
    Ok(result)
}

fn validate_cleanup_record_id(value: &str) -> Result<String, String> {
    validate_job_id(value).map_err(|_| "That cleanup record id is not recognized.".to_string())
}

fn ensure_cleanup_enabled(read_only: bool) -> Result<(), String> {
    if read_only {
        return Err(READ_ONLY_MESSAGE.into());
    }
    Ok(())
}

/// Plans are single use: taking one removes it, so a stale confirmation cannot
/// be replayed against files that changed in between.
fn take_plan(
    plans: &std::sync::Mutex<HashMap<String, cleanup::CleanupPlan>>,
    plan_id: &str,
    now: u64,
) -> Result<cleanup::CleanupPlan, String> {
    let mut plans = plans
        .lock()
        .map_err(|_| "The cleanup plan registry is unavailable.".to_string())?;
    plans.retain(|_, plan| plan.expires_at > now);
    plans.remove(plan_id).ok_or_else(|| {
        "That cleanup preview expired or was already used. Preview the selection again."
            .to_string()
    })
}

fn check_plan_selection(
    plan: &cleanup::CleanupPlan,
    target_ids: &[String],
) -> Result<(), String> {
    let mut planned: Vec<&str> = plan.specs.iter().map(|spec| spec.id.as_str()).collect();
    planned.sort_unstable();
    let mut requested: Vec<&str> = target_ids.iter().map(String::as_str).collect();
    requested.sort_unstable();
    if planned != requested {
        return Err(
            "The selection changed after the preview. Preview the selection again.".to_string(),
        );
    }
    Ok(())
}

fn cleanup_run_id(at: u64, nanos: u32) -> String {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("cleanup-{at}-{nanos}-{sequence}")
}

fn cleanup_status(report: &cleanup::CleanupReport) -> &'static str {
    if report.cancelled {
        "cancelled"
    } else if report.quarantined == 0 && report.failed > 0 {
        "failed"
    } else {
        "completed"
    }
}

fn cleanup_log_entry(report: &cleanup::CleanupReport, at: u64) -> cleanup::CleanupLogEntry {
    let targets: Vec<String> = report
        .items
        .iter()
        .map(|item| item.target_id.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    cleanup::CleanupLogEntry {
        id: format!("cleanup-{}-{at}", report.run_id),
        at,
        title: format!("Cleanup · {} locations", targets.len()),
        targets,
        bytes: report.bytes,
        files: report.files,
        quarantined: report.quarantined,
        skipped: report.skipped,
        failed: report.failed,
        status: cleanup_status(report).into(),
        detail: format!(
            "{} moved to quarantine · {} skipped · {} failed · {}{}",
            report.quarantined,
            report.skipped,
            report.failed,
            human_bytes(report.bytes),
            if report.cancelled { " · cancelled" } else { "" }
        ),
    }
}

fn failed_cleanup_log(
    run_id: &str,
    targets: Vec<String>,
    message: &str,
    at: u64,
) -> cleanup::CleanupLogEntry {
    cleanup::CleanupLogEntry {
        id: format!("cleanup-{run_id}-{at}"),
        at,
        title: format!("Cleanup did not run · {} locations", targets.len()),
        targets,
        bytes: 0,
        files: 0,
        quarantined: 0,
        skipped: 0,
        failed: 0,
        status: "failed".into(),
        detail: message.to_string(),
    }
}

fn cleanup_history_entry(log: &cleanup::CleanupLogEntry) -> history::HistoryEntry {
    history::HistoryEntry {
        id: log.id.clone(),
        kind: "cleanup".into(),
        title: log.title.clone(),
        detail: log.detail.clone(),
        at: log.at,
        bytes: log.bytes,
        items: log.files,
        status: match log.status.as_str() {
            "cancelled" => "cancelled",
            "failed" => "failed",
            _ => "completed",
        }
        .into(),
    }
}

#[tauri::command]
pub fn get_cleanup_mode(state: State<'_, AppState>) -> bool {
    state.read_only.load(Ordering::Relaxed)
}

#[tauri::command]
pub fn set_cleanup_mode(state: State<'_, AppState>, read_only: bool) -> bool {
    state.read_only.store(read_only, Ordering::Relaxed);
    read_only
}

#[tauri::command]
pub async fn preview_cleanup(
    state: State<'_, AppState>,
    target_ids: Vec<String>,
    exclusions: Option<Vec<String>>,
) -> Result<cleanup::CleanupPreview, String> {
    let target_ids = validate_target_ids(target_ids)?;
    // Exclusions are validated here, never trusted as opaque prefixes.
    let exclusions = cleanup::validate_exclusions(exclusions.unwrap_or_default())?;
    // Previewing is a dry run: it reads sizes and paths only, so it stays
    // available in read-only mode. Execution enforces the switch separately.

    let (plan, preview) = tauri::async_runtime::spawn_blocking(move || {
        cleanup::plan_catalog_with_exclusions(&target_ids, false, exclusions)
    })
    .await
    .map_err(|error| format!("The cleanup preview stopped unexpectedly: {error}"))??;

    let mut plans = state
        .plans
        .lock()
        .map_err(|_| "The cleanup plan registry is unavailable.".to_string())?;
    let now = now_secs();
    plans.retain(|_, existing| existing.expires_at > now);
    plans.insert(plan.plan_id.clone(), plan);
    Ok(preview)
}

#[tauri::command]
pub async fn start_cleanup(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    plan_id: String,
    target_ids: Vec<String>,
) -> Result<cleanup::CleanupReport, String> {
    let plan_id = validate_job_id(&plan_id)?;
    let target_ids = validate_target_ids(target_ids)?;
    ensure_cleanup_enabled(state.read_only.load(Ordering::Relaxed))?;

    let plan = take_plan(&state.plans, &plan_id, now_secs())?;
    check_plan_selection(&plan, &target_ids)?;

    let quarantine_root = cleanup_quarantine_root(&app)?;
    let data_dir = app_data_dir(&app)?;
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let run_id = cleanup_run_id(since_epoch.as_secs(), since_epoch.subsec_nanos());

    let cancel = Arc::new(AtomicBool::new(false));
    register_scan(
        &state.cleanup_jobs,
        &run_id,
        Arc::clone(&cancel),
        "cleanup",
    )?;

    let jobs = Arc::clone(&state.cleanup_jobs);
    let registered_run = run_id.clone();
    let app_for_task = app.clone();
    let plan_for_task = plan;
    let task = tauri::async_runtime::spawn_blocking(move || {
        let emitter = app_for_task.clone();
        let progress = move |payload: cleanup::CleanupProgress| {
            let _ = emitter.emit(cleanup::CLEANUP_EVENT, payload);
        };
        let report = cleanup::execute_plan(&plan_for_task, &quarantine_root, &cancel, &progress);
        let at = now_secs();
        let log = match &report {
            Ok(report) => cleanup_log_entry(report, at),
            Err(error) => failed_cleanup_log(
                &run_id,
                plan_for_task
                    .specs
                    .iter()
                    .map(|spec| spec.id.clone())
                    .collect(),
                error,
                at,
            ),
        };
        // A cancelled or partly failed run is still a real operation; record it.
        let _ = cleanup::append_log(&data_dir, &log);
        history::push_history(&app_for_task, cleanup_history_entry(&log));
        report
    });

    let outcome = task.await;
    if let Ok(mut jobs) = jobs.lock() {
        jobs.remove(&registered_run);
    }
    outcome.map_err(|error| format!("The cleanup task stopped unexpectedly: {error}"))?
}

#[tauri::command]
pub fn list_quarantine(app: tauri::AppHandle) -> Vec<cleanup::QuarantineBatch> {
    cleanup_quarantine_root(&app)
        .map(|root| cleanup::list_quarantine(&root))
        .unwrap_or_default()
}

#[tauri::command]
pub async fn restore_quarantine(
    app: tauri::AppHandle,
    run_id: String,
) -> Result<cleanup::RestoreReport, String> {
    let run_id = validate_cleanup_record_id(&run_id)?;
    let root = cleanup_quarantine_root(&app)?;
    let data_dir = app_data_dir(&app)?;
    let report = run_blocking("Quarantine restore", move || {
        cleanup::restore_batch(&root, &run_id)
    })
    .await??;
    let at = now_secs();
    let log = cleanup::CleanupLogEntry {
        id: format!("restore-{}-{at}", report.run_id),
        at,
        title: "Restored quarantined files".into(),
        targets: Vec::new(),
        bytes: 0,
        files: report.restored,
        quarantined: 0,
        skipped: report.skipped,
        failed: report.failed,
        status: if report.failed == 0 { "completed" } else { "failed" }.into(),
        detail: format!(
            "{} restored · {} skipped · {} failed",
            report.restored, report.skipped, report.failed
        ),
    };
    let _ = cleanup::append_log(&data_dir, &log);
    Ok(report)
}

#[tauri::command]
pub async fn purge_quarantine(
    app: tauri::AppHandle,
    run_id: String,
) -> Result<cleanup::PurgeReport, String> {
    let run_id = validate_cleanup_record_id(&run_id)?;
    let root = cleanup_quarantine_root(&app)?;
    let data_dir = app_data_dir(&app)?;
    let report =
        run_blocking("Quarantine purge", move || cleanup::purge_batch(&root, &run_id)).await??;
    let at = now_secs();
    let log = cleanup::CleanupLogEntry {
        id: format!("purge-{}-{at}", report.run_id),
        at,
        title: "Deleted a quarantine batch".into(),
        targets: Vec::new(),
        bytes: report.freed_bytes,
        files: report.removed_files,
        quarantined: 0,
        skipped: 0,
        failed: 0,
        status: "completed".into(),
        detail: format!(
            "{} files permanently removed · {} freed",
            report.removed_files,
            human_bytes(report.freed_bytes)
        ),
    };
    let _ = cleanup::append_log(&data_dir, &log);
    Ok(report)
}

#[tauri::command]
pub fn get_cleanup_log(app: tauri::AppHandle) -> Vec<cleanup::CleanupLogEntry> {
    app_data_dir(&app)
        .map(|dir| cleanup::load_log(&dir))
        .unwrap_or_default()
}

/* ----------------------------------------------------------------- phase 4 */

#[tauri::command]
pub async fn discover_app_processes(
    state: State<'_, AppState>,
    app_id: String,
) -> Result<Vec<process::AppProcess>, String> {
    let app_id_trimmed = app_id.trim().to_string();
    let inventory = run_blocking("Application lookup for process discovery", apps::discover_apps).await?;
    let app = inventory.apps.iter().find(|a| a.id == app_id_trimmed);
    let install_location = app.and_then(|a| a.install_location.clone());

    let (snapshot, procs) = run_blocking("Process discovery", move || {
        process::discover_app_processes(&app_id_trimmed, install_location.as_deref())
    })
    .await?;

    let mut snapshots = state
        .process_snapshots
        .lock()
        .map_err(|_| "Process snapshot registry is unavailable.".to_string())?;
    snapshots.insert(snapshot.app_id.clone(), snapshot);

    Ok(procs)
}

#[tauri::command]
pub async fn terminate_app_processes(
    state: State<'_, AppState>,
    app_id: String,
    pids: Vec<u32>,
    force: bool,
) -> Result<Vec<process::ProcessTerminateResult>, String> {
    if pids.is_empty() {
        return Ok(Vec::new());
    }
    if pids.len() > 64 {
        return Err("Cannot terminate more than 64 processes at once.".into());
    }

    let snapshot = {
        let snapshots = state
            .process_snapshots
            .lock()
            .map_err(|_| "Process snapshot registry is unavailable.".to_string())?;
        snapshots
            .get(&app_id)
            .cloned()
            .ok_or_else(|| "No verified process snapshot found for this application. Please refresh processes.".to_string())?
    };

    run_blocking("Process termination", move || {
        process::terminate_processes_verified(&app_id, &snapshot, &pids, force)
    })
    .await
}

#[tauri::command]
pub fn parse_uninstall_command(raw_command: String) -> uninstall::ParsedUninstallCommand {
    uninstall::parse_uninstall_command(&raw_command)
}

#[tauri::command]
pub async fn prepare_uninstall(
    state: State<'_, AppState>,
    app_id: String,
    quiet: bool,
) -> Result<uninstall::UninstallPlan, String> {
    let app_id_trimmed = app_id.trim().to_string();
    if app_id_trimmed.is_empty() {
        return Err("Application ID is required.".into());
    }

    let inventory = run_blocking("Application lookup", apps::discover_apps).await?;
    let app = inventory
        .apps
        .iter()
        .find(|a| a.id == app_id_trimmed)
        .ok_or_else(|| "Application is not registered in uninstall inventory.".to_string())?;

    let plan = uninstall::create_uninstall_plan(app, quiet)?;
    let public_plan = uninstall::to_public_plan(&plan);

    let mut plans = state
        .uninstall_plans
        .lock()
        .map_err(|_| "Uninstall plan registry is unavailable.".to_string())?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    plans.retain(|_, p| p.expires_at > now);
    plans.insert(plan.plan_id.clone(), plan);

    Ok(public_plan)
}

#[tauri::command]
pub async fn execute_uninstall(
    state: State<'_, AppState>,
    app_id: String,
    plan_id: String,
    timeout_secs: Option<u64>,
) -> Result<uninstall::UninstallExecutionResult, String> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

    // Consume single-use plan from backend registry
    let plan = {
        let mut plans = state
            .uninstall_plans
            .lock()
            .map_err(|_| "Uninstall plan registry is unavailable.".to_string())?;
        plans
            .remove(&plan_id)
            .ok_or_else(|| "Uninstall plan expired or not found. Please review and confirm again.".to_string())?
    };

    if now > plan.expires_at {
        return Err("Uninstall plan has expired. Please review and confirm again.".into());
    }

    if plan.app_id != app_id {
        return Err("Uninstall plan application ID mismatch.".into());
    }

    // Re-verify against live system inventory to detect uninstaller tampering or uninstalled state
    let inventory = run_blocking("Live application verification", apps::discover_apps).await?;
    let live_app = inventory
        .apps
        .iter()
        .find(|a| a.id == app_id)
        .ok_or_else(|| "Application is no longer registered in uninstall inventory.".to_string())?;

    let live_cmd = if plan.command.arguments.iter().any(|a| a == "/qn" || a == "/quiet" || a == "/s" || a == "/S") && live_app.quiet_uninstall_string.is_some() {
        live_app.quiet_uninstall_string.as_deref().unwrap_or(&plan.raw_command)
    } else {
        live_app.uninstall_string.as_deref().unwrap_or(&plan.raw_command)
    };

    if live_cmd != plan.raw_command {
        return Err("The registered uninstaller command was modified after confirmation. Execution aborted.".into());
    }

    let cancel = Arc::new(AtomicBool::new(false));
    let timeout = timeout_secs.unwrap_or(600);
    let plan_command = plan.command;
    let plan_app_id = plan.app_id;

    run_blocking("Standard uninstall", move || {
        uninstall::execute_uninstaller(&plan_app_id, &plan_command, &cancel, timeout)
    })
    .await?
}

#[tauri::command]
pub async fn scan_app_leftovers(
    state: State<'_, AppState>,
    app_id: String,
) -> Result<leftovers::LeftoverScanReport, String> {
    let app_id_trimmed = app_id.trim().to_string();
    let inventory = run_blocking("Application lookup for leftover scan", apps::discover_apps).await?;
    let app = inventory
        .apps
        .iter()
        .find(|a| a.id == app_id_trimmed)
        .ok_or_else(|| "Application not found in uninstall inventory.".to_string())?;

    let name = app.name.clone();
    let publisher = app.publisher.clone();
    let install_location = app.install_location.clone();

    let (plan, report) = run_blocking("Leftover scan", move || {
        leftovers::scan_app_leftovers(&app_id_trimmed, &name, publisher.as_deref(), install_location.as_deref())
    })
    .await?;

    let mut plans = state
        .leftover_plans
        .lock()
        .map_err(|_| "Leftover plan registry is unavailable.".to_string())?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    plans.retain(|_, p| p.expires_at > now);
    plans.insert(plan.plan_id.clone(), plan);

    Ok(report)
}

#[tauri::command]
pub async fn clean_app_leftovers(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    app_id: String,
    plan_id: String,
    candidate_ids: Vec<String>,
) -> Result<leftovers::LeftoverCleanReport, String> {
    ensure_cleanup_enabled(state.read_only.load(Ordering::Relaxed))?;
    let quarantine_root = cleanup_quarantine_root(&app)?;

    let plan = {
        let mut plans = state
            .leftover_plans
            .lock()
            .map_err(|_| "Leftover plan registry is unavailable.".to_string())?;
        plans
            .remove(&plan_id)
            .ok_or_else(|| "Residual cleanup plan expired or not found. Please scan again.".to_string())?
    };

    if plan.app_id != app_id {
        return Err("Residual cleanup plan application ID mismatch.".into());
    }

    run_blocking("Leftover cleanup", move || {
        leftovers::clean_leftovers(&plan, &candidate_ids, &quarantine_root)
    })
    .await?
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchUninstallItemResult {
    pub app_id: String,
    pub name: String,
    pub status: String,
    pub exit_code: Option<i32>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchUninstallReport {
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub restart_required: usize,
    pub results: Vec<BatchUninstallItemResult>,
}

#[tauri::command]
pub async fn execute_batch_uninstall(
    app_ids: Vec<String>,
    quiet: bool,
) -> Result<BatchUninstallReport, String> {
    if app_ids.is_empty() {
        return Err("Select at least one application to uninstall.".into());
    }
    if app_ids.len() > 32 {
        return Err("Select at most 32 applications for batch uninstall.".into());
    }
    run_blocking("Batch uninstallation", move || {
        let inventory = apps::discover_apps();
        let mut results = Vec::new();
        let mut completed = 0;
        let mut failed = 0;
        let mut restart_required = 0;

        for id in app_ids {
            let app = inventory.apps.iter().find(|a| a.id == id);
            let Some(app) = app else {
                results.push(BatchUninstallItemResult {
                    app_id: id,
                    name: "Unknown application".into(),
                    status: "failed".into(),
                    exit_code: None,
                    message: "Application is no longer registered in uninstall inventory.".into(),
                });
                failed += 1;
                continue;
            };

            let raw_cmd = if quiet && app.quiet_uninstall_string.is_some() {
                app.quiet_uninstall_string.as_ref().unwrap()
            } else if app.uninstall_string.is_some() {
                app.uninstall_string.as_ref().unwrap()
            } else {
                results.push(BatchUninstallItemResult {
                    app_id: id,
                    name: app.name.clone(),
                    status: "failed".into(),
                    exit_code: None,
                    message: "No registered uninstaller command found.".into(),
                });
                failed += 1;
                continue;
            };

            let parsed = uninstall::parse_uninstall_command(raw_cmd);
            if !parsed.is_valid {
                results.push(BatchUninstallItemResult {
                    app_id: id,
                    name: app.name.clone(),
                    status: "failed".into(),
                    exit_code: None,
                    message: parsed.rejection_reason.unwrap_or_else(|| "Invalid uninstaller command".into()),
                });
                failed += 1;
                continue;
            }

            let cancel = Arc::new(AtomicBool::new(false));
            match uninstall::execute_uninstaller(&id, &parsed, &cancel, 600) {
                Ok(res) => {
                    if res.status == "completed" {
                        completed += 1;
                    } else if res.status == "restart_required" {
                        restart_required += 1;
                    } else {
                        failed += 1;
                    }
                    results.push(BatchUninstallItemResult {
                        app_id: id,
                        name: app.name.clone(),
                        status: res.status,
                        exit_code: res.exit_code,
                        message: res.message,
                    });
                }
                Err(err) => {
                    failed += 1;
                    results.push(BatchUninstallItemResult {
                        app_id: id,
                        name: app.name.clone(),
                        status: "failed".into(),
                        exit_code: None,
                        message: err,
                    });
                }
            }
        }

        let total = results.len();
        BatchUninstallReport {
            total,
            completed,
            failed,
            restart_required,
            results,
        }
    })
    .await
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

    /* ------------------------------------------------------- cleanup layer */

    fn spec(id: &str) -> cleanup::CleanupSpec {
        cleanup::CleanupSpec {
            id: id.into(),
            tool: "Tool".into(),
            label: "Cache".into(),
            risk: "safe".into(),
            category: "ide".into(),
            note: "A disposable cache used only by tests.".into(),
            impact: "Regenerated automatically.".into(),
            path: std::env::temp_dir(),
        }
    }

    fn plan(specs: Vec<cleanup::CleanupSpec>, expires_at: u64) -> cleanup::CleanupPlan {
        cleanup::CleanupPlan {
            plan_id: "plan-1".into(),
            expected_files: 1,
            expected_bytes: 1,
            specs,
            expires_at,
            read_only: false,
            exclusions: Vec::new(),
            snapshot_paths: HashMap::new(),
        }
    }

    fn report() -> cleanup::CleanupReport {
        cleanup::CleanupReport {
            run_id: "cleanup-1".into(),
            plan_id: "plan-1".into(),
            bytes: 2048,
            space_moved: 2048,
            space_freed: 0,
            files: 2,
            dirs_removed: 1,
            quarantined: 2,
            skipped: 1,
            failed: 0,
            items: vec![cleanup::CleanupItemResult {
                target_id: "npm-cache".into(),
                path: "C:\\cache\\file".into(),
                bytes: 2048,
                status: "quarantined".into(),
                reason: None,
                quarantine_path: Some("C:\\quarantine\\1-file".into()),
            }],
            errors: Vec::new(),
            elapsed_ms: 12,
            cancelled: false,
            quarantine_dir: Some("C:\\quarantine\\cleanup-1".into()),
            purge_at: 0,
        }
    }

    #[test]
    fn cleanup_target_ids_are_validated_and_deduped() {
        let ids = validate_target_ids(vec![
            " npm-cache ".into(),
            "npm-cache".into(),
            "user-temp".into(),
        ])
        .expect("valid ids");
        assert_eq!(ids, ["npm-cache", "user-temp"]);
    }

    #[test]
    fn cleanup_target_ids_reject_traversal_and_empty_lists() {
        assert!(validate_target_ids(Vec::new()).is_err());
        assert!(validate_target_ids(vec![" ".into()]).is_err());
        assert!(validate_target_ids(vec!["..\\Windows".into()]).is_err());
        assert!(validate_target_ids(vec!["C:\\Windows".into()]).is_err());
        assert!(validate_target_ids(vec!["a".repeat(65)]).is_err());
        let too_many: Vec<String> = (0..65).map(|index| format!("target-{index}")).collect();
        assert!(validate_target_ids(too_many).is_err());
    }

    #[test]
    fn read_only_mode_blocks_cleaning() {
        let error = ensure_cleanup_enabled(true).expect_err("read-only must refuse");
        assert!(error.contains("Read-only mode"), "{error}");
        assert!(ensure_cleanup_enabled(false).is_ok());
    }

    #[test]
    fn expired_plans_are_swept_and_cannot_be_reused() {
        let registry = std::sync::Mutex::new(HashMap::new());
        registry
            .lock()
            .expect("registry")
            .insert("plan-1".into(), plan(vec![spec("npm-cache")], 100));

        let error = take_plan(&registry, "plan-1", 101).expect_err("expired plan");
        assert!(error.contains("expired"), "{error}");
        assert!(registry.lock().expect("registry").is_empty());

        registry
            .lock()
            .expect("registry")
            .insert("plan-2".into(), plan(vec![spec("npm-cache")], 500));
        assert!(take_plan(&registry, "plan-2", 200).is_ok());
        assert!(
            take_plan(&registry, "plan-2", 200).is_err(),
            "a plan is single use"
        );
    }

    #[test]
    fn selection_must_match_the_previewed_plan() {
        let planned = plan(vec![spec("npm-cache"), spec("user-temp")], 100);
        assert!(check_plan_selection(&planned, &["user-temp".into(), "npm-cache".into()]).is_ok());
        let error = check_plan_selection(&planned, &["npm-cache".into()])
            .expect_err("a shortened selection must be rejected");
        assert!(error.contains("selection changed"), "{error}");
        assert!(check_plan_selection(&planned, &["npm-cache".into(), "wsl".into()]).is_err());
    }

    #[test]
    fn a_cleanup_run_id_cannot_be_reused_while_running() {
        let registry = std::sync::Mutex::new(HashMap::new());
        let original = Arc::new(AtomicBool::new(false));
        let replacement = Arc::new(AtomicBool::new(false));
        register_scan(&registry, "cleanup-run", Arc::clone(&original), "cleanup")
            .expect("first run registers");
        let error = register_scan(
            &registry,
            "cleanup-run",
            Arc::clone(&replacement),
            "cleanup",
        )
        .expect_err("duplicate run id");
        assert!(error.contains("already running"), "{error}");
        assert!(!replacement.load(Ordering::Relaxed));
    }

    #[test]
    fn cleanup_log_and_history_keep_the_report_numbers() {
        let report = report();
        assert_eq!(cleanup_status(&report), "completed");

        let log = cleanup_log_entry(&report, 1_700_000_000);
        assert_eq!(log.targets, ["npm-cache"]);
        assert_eq!(log.bytes, 2048);
        assert_eq!(log.files, 2);
        assert_eq!(log.quarantined, 2);
        assert_eq!(log.skipped, 1);
        assert_eq!(log.status, "completed");

        let history = cleanup_history_entry(&log);
        assert_eq!(history.kind, "cleanup");
        assert_eq!(history.status, "completed");
        assert_eq!(history.bytes, 2048);
        assert_eq!(history.items, 2);
    }

    #[test]
    fn cancelled_and_failed_cleanups_get_honest_statuses() {
        let mut cancelled = report();
        cancelled.cancelled = true;
        assert_eq!(cleanup_status(&cancelled), "cancelled");

        let mut failed = report();
        failed.quarantined = 0;
        failed.failed = 3;
        assert_eq!(cleanup_status(&failed), "failed");
        assert_eq!(cleanup_history_entry(&cleanup_log_entry(&failed, 1)).status, "failed");
    }

    #[test]
    fn failed_cleanup_logs_stay_visible() {
        let log = failed_cleanup_log("cleanup-9", vec!["npm-cache".into()], "disk full", 5);
        assert_eq!(log.status, "failed");
        assert!(log.detail.contains("disk full"));
        assert_eq!(cleanup_history_entry(&log).status, "failed");
    }

    #[test]
    fn cleanup_record_ids_are_restricted() {
        assert!(validate_cleanup_record_id("cleanup-1700000000-1-0").is_ok());
        assert!(validate_cleanup_record_id("..\\secrets").is_err());
        assert!(validate_cleanup_record_id("").is_err());
    }
}
