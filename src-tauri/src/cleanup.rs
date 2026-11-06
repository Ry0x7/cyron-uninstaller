//! Review-first cleanup engine.
//!
//! Callers never hand this module a path. Production code resolves ids through
//! `caches::CATALOG` (`plan_catalog`), the resulting plan is held by the command
//! layer, and every move is revalidated against its target root immediately
//! before it happens. Files are quarantined, never deleted outright, and there
//! is no recursive delete anywhere in this module.

use serde::{Deserialize, Serialize};
use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::caches;

/// How long a quarantine batch stays restorable before the user may purge it.
pub const RETENTION_DAYS: u64 = 7;
/// A preview is only executable for this long.
pub const PLAN_TTL_SECS: u64 = 600;
/// How many affected paths a preview lists; totals are still exact.
pub const MAX_PREVIEW_FILES: usize = 400;
/// Hard cap on files considered for one catalog location.
pub const MAX_FILES_PER_TARGET: usize = 100_000;
pub const CLEANUP_EVENT: &str = "cyron-cleanup-progress";

const MAX_DIRS: usize = 20_000;
const MAX_ERRORS: usize = 20;
const MAX_SKIPS: usize = 100;
const MAX_ITEMS: usize = 500;
const MAX_WARNINGS: usize = 12;
const MAX_IDS: usize = 64;
const MAX_EXCLUSIONS: usize = 128;
const MAX_DEPTH: u32 = 64;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const MANIFEST: &str = "manifest.json";
const LOG_FILE: &str = "cleanup-log.json";
const PROGRESS_INTERVAL: std::time::Duration = std::time::Duration::from_millis(120);

/* ------------------------------------------------------------------ types */

/// One location the planner may clean, already resolved to an absolute path.
#[derive(Debug, Clone)]
pub struct CleanupSpec {
    pub id: String,
    pub tool: String,
    pub label: String,
    pub risk: String,
    pub category: String,
    pub note: String,
    pub impact: String,
    pub path: PathBuf,
}

/// A dry run the backend is willing to execute.
#[derive(Debug, Clone)]
pub struct CleanupPlan {
    pub plan_id: String,
    pub specs: Vec<CleanupSpec>,
    pub expires_at: u64,
    pub read_only: bool,
    /// Absolute folders the user excluded in Settings. Nothing under them is
    /// counted or moved, and the plan keeps them so execution matches preview.
    pub exclusions: Vec<PathBuf>,
    pub expected_files: u64,
    pub expected_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupTarget {
    pub id: String,
    pub tool: String,
    pub label: String,
    pub path: String,
    pub note: String,
    pub impact: String,
    pub risk: String,
    pub category: String,
    pub exists: bool,
    pub size: u64,
    pub file_count: u64,
    pub dir_count: u64,
    pub includable: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupPreviewFile {
    pub path: String,
    pub size: u64,
    pub target_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupPreviewSkip {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupPreview {
    pub plan_id: String,
    pub generated_at: u64,
    pub expires_at: u64,
    pub targets: Vec<CleanupTarget>,
    pub files: Vec<CleanupPreviewFile>,
    pub skipped: Vec<CleanupPreviewSkip>,
    pub file_count: u64,
    pub bytes: u64,
    pub total_files: u64,
    pub read_only: bool,
    pub hidden_count: u64,
    pub warnings: Vec<String>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupItemResult {
    pub target_id: String,
    pub path: String,
    pub bytes: u64,
    pub status: String,
    pub reason: Option<String>,
    pub quarantine_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupReport {
    pub run_id: String,
    pub plan_id: String,
    pub bytes: u64,
    pub files: u64,
    pub dirs_removed: u64,
    pub quarantined: u64,
    pub skipped: u64,
    pub failed: u64,
    pub items: Vec<CleanupItemResult>,
    pub errors: Vec<String>,
    pub elapsed_ms: u64,
    pub cancelled: bool,
    pub quarantine_dir: Option<String>,
    pub purge_at: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupProgress {
    pub run_id: String,
    pub files: u64,
    pub total_files: u64,
    pub bytes: u64,
    pub current_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuarantineBatch {
    pub run_id: String,
    pub created_at: u64,
    pub expires_at: u64,
    pub bytes: u64,
    pub file_count: u64,
    pub dir: String,
    pub target_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreReport {
    pub run_id: String,
    pub restored: u64,
    pub skipped: u64,
    pub failed: u64,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PurgeReport {
    pub run_id: String,
    pub removed_files: u64,
    pub freed_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupLogEntry {
    pub id: String,
    pub at: u64,
    pub title: String,
    pub targets: Vec<String>,
    pub bytes: u64,
    pub files: u64,
    pub quarantined: u64,
    pub skipped: u64,
    pub failed: u64,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestItem {
    original_path: String,
    stored_path: String,
    size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BatchManifest {
    run_id: String,
    created_at: u64,
    expires_at: u64,
    target_ids: Vec<String>,
    items: Vec<ManifestItem>,
}

/* ------------------------------------------------------- plan construction */

/// Resolves catalog ids to specs. Unknown and protected ids are refused here,
/// before any path is touched.
pub fn catalog_specs(ids: &[String]) -> Result<Vec<CleanupSpec>, String> {
    if ids.is_empty() {
        return Err("Select at least one location to clean.".into());
    }
    if ids.len() > MAX_IDS {
        return Err(format!(
            "Select at most {MAX_IDS} locations at once."
        ));
    }

    let mut seen = std::collections::HashSet::new();
    let mut specs = Vec::new();
    for raw in ids {
        let id = raw.trim();
        if id.is_empty() {
            return Err("An empty cleanup target was provided.".into());
        }
        if !seen.insert(id.to_string()) {
            continue;
        }
        let definition =
            caches::find(id).ok_or_else(|| format!("Unknown cleanup target: {id}"))?;
        if definition.risk == "protected" {
            return Err(format!(
                "{} is protected data; Cyron never cleans it.",
                definition.label
            ));
        }
        let path = caches::resolve_template(definition.path)
            .map_err(|error| format!("{}: {error}", definition.label))?;
        specs.push(CleanupSpec {
            id: definition.id.to_string(),
            tool: definition.tool.to_string(),
            label: definition.label.to_string(),
            risk: definition.risk.to_string(),
            category: definition.category.to_string(),
            note: definition.note.to_string(),
            impact: caches::impact_for(definition),
            path: PathBuf::from(path),
        });
    }
    Ok(specs)
}

/// Resolves catalog ids and builds a dry run in one step.
pub fn plan_catalog(
    ids: &[String],
    read_only: bool,
) -> Result<(CleanupPlan, CleanupPreview), String> {
    plan_specs(catalog_specs(ids)?, read_only)
}

/// Resolves catalog ids with the caller's exclusion list.
pub fn plan_catalog_with_exclusions(
    ids: &[String],
    read_only: bool,
    exclusions: Vec<PathBuf>,
) -> Result<(CleanupPlan, CleanupPreview), String> {
    plan_specs_with_exclusions(catalog_specs(ids)?, read_only, exclusions)
}

/// Measures the given specs and returns a plan plus its preview. No file is
/// modified. A spec whose path fails the safety guard aborts the whole plan.
pub fn plan_specs(
    specs: Vec<CleanupSpec>,
    read_only: bool,
) -> Result<(CleanupPlan, CleanupPreview), String> {
    plan_specs_with_exclusions(specs, read_only, Vec::new())
}

/// Validates exclusion folders: each must be an absolute path. Relative or
/// empty entries are refused rather than guessed at, and a folder that happens
/// to hold a protected location keeps protecting it because exclusions can only
/// remove files from a plan, never add them.
pub fn validate_exclusions(paths: Vec<String>) -> Result<Vec<PathBuf>, String> {
    if paths.len() > MAX_EXCLUSIONS {
        return Err(format!("At most {MAX_EXCLUSIONS} exclusions are supported."));
    }
    let mut exclusions = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for raw in paths {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let path = PathBuf::from(trimmed);
        if !path.is_absolute() {
            return Err(format!(
                "Excluded folder must be an absolute path: {trimmed}"
            ));
        }
        if path.parent().is_none() {
            return Err(format!("Excluded folder cannot be a drive root: {trimmed}"));
        }
        if seen.insert(normalize_key(&path)) {
            exclusions.push(path);
        }
    }
    Ok(exclusions)
}

/// Measures the given specs and returns a plan plus its preview. No file is
/// modified. A spec whose path fails the safety guard aborts the whole plan.
pub fn plan_specs_with_exclusions(
    specs: Vec<CleanupSpec>,
    read_only: bool,
    exclusions: Vec<PathBuf>,
) -> Result<(CleanupPlan, CleanupPreview), String> {
    if specs.is_empty() {
        return Err("Select at least one location to preview.".into());
    }
    for spec in &specs {
        guard_path(&spec.path)?;
    }
    let exclusion_keys: Vec<String> = exclusions.iter().map(|path| normalize_key(path)).collect();

    let now = seconds_now();
    let plan_id = next_id("plan", now);
    let mut targets = Vec::with_capacity(specs.len());
    let mut files = Vec::new();
    let mut skipped = Vec::new();
    let mut warnings = Vec::new();
    let mut total_files = 0u64;
    let mut planned_files = 0u64;
    let mut planned_bytes = 0u64;
    let mut truncated = false;

    for spec in &specs {
        let remaining = MAX_PREVIEW_FILES.saturating_sub(files.len());
        let walk = walk_target(
            &spec.path,
            MAX_FILES_PER_TARGET,
            remaining,
            None,
            &exclusion_keys,
        );
        total_files += walk.file_count;
        planned_files += walk.file_count;
        planned_bytes += walk.bytes;

        for (path, reason) in walk.skipped.iter().take(MAX_SKIPS) {
            if skipped.len() < MAX_SKIPS {
                skipped.push(CleanupPreviewSkip {
                    path: path.to_string_lossy().to_string(),
                    reason: reason.clone(),
                });
            }
        }
        for error in &walk.errors {
            push_warning(&mut warnings, format!("{}: {error}", spec.label));
        }
        if walk.truncated {
            truncated = true;
            push_warning(
                &mut warnings,
                format!(
                    "{}: stopped after {} files, so the preview and totals are partial.",
                    spec.label, MAX_FILES_PER_TARGET
                ),
            );
        }
        if walk.cancelled {
            truncated = true;
            push_warning(
                &mut warnings,
                format!("{}: preview was interrupted and is partial.", spec.label),
            );
        }
        for (path, size) in &walk.files {
            if files.len() >= MAX_PREVIEW_FILES {
                break;
            }
            files.push(CleanupPreviewFile {
                path: path.to_string_lossy().to_string(),
                size: *size,
                target_id: spec.id.clone(),
            });
        }

        targets.push(CleanupTarget {
            id: spec.id.clone(),
            tool: spec.tool.clone(),
            label: spec.label.clone(),
            path: spec.path.to_string_lossy().to_string(),
            note: spec.note.clone(),
            impact: spec.impact.clone(),
            risk: spec.risk.clone(),
            category: spec.category.clone(),
            exists: walk.exists,
            size: walk.bytes,
            file_count: walk.file_count,
            dir_count: walk.dir_count,
            includable: spec.risk != "protected",
        });
    }

    if planned_files == 0 {
        push_warning(
            &mut warnings,
            "Nothing to move: the selected locations are empty or could not be read.".into(),
        );
    }
    let hidden_count = planned_files.saturating_sub(files.len() as u64);
    // Every planned file is still moved; only the listing is capped. Say so
    // instead of letting the preview look complete.
    if hidden_count > 0 {
        truncated = true;
        push_warning(
            &mut warnings,
            format!(
                "The preview lists the first {} of {} affected files; the rest are counted in the totals and will still be moved.",
                files.len(),
                planned_files
            ),
        );
    }
    let plan = CleanupPlan {
        plan_id: plan_id.clone(),
        specs,
        expires_at: now + PLAN_TTL_SECS,
        read_only,
        exclusions,
        expected_files: planned_files,
        expected_bytes: planned_bytes,
    };
    let preview = CleanupPreview {
        plan_id,
        generated_at: now,
        expires_at: now + PLAN_TTL_SECS,
        targets,
        files,
        skipped,
        file_count: planned_files,
        bytes: planned_bytes,
        total_files,
        read_only,
        hidden_count,
        warnings,
        truncated,
    };
    Ok((plan, preview))
}

/* --------------------------------------------------------------- execution */

enum MoveError {
    Skipped(String),
    Failed(String),
}

/// Moves every planned file into a quarantine batch. Never deletes content:
/// the only destructive primitives used are `rename`, `remove_file` (purge and
/// history's own temp file) and an empty-directory `remove_dir`.
pub fn execute_plan(
    plan: &CleanupPlan,
    quarantine_root: &Path,
    cancel: &AtomicBool,
    progress: &dyn Fn(CleanupProgress),
) -> Result<CleanupReport, String> {
    if plan.read_only {
        return Err("Read-only mode is on. Turn it off in Settings before cleaning.".into());
    }

    let started = Instant::now();
    let now = seconds_now();
    let run_id = next_id("run", now);
    let batch = quarantine_root.join(&run_id);
    let mut manifest = BatchManifest {
        run_id: run_id.clone(),
        created_at: now,
        expires_at: now + RETENTION_DAYS * 86_400,
        target_ids: plan.specs.iter().map(|spec| spec.id.clone()).collect(),
        items: Vec::new(),
    };

    let mut items = Vec::new();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut quarantined = 0u64;
    let mut moved_bytes = 0u64;
    let mut skipped = 0u64;
    let mut failed = 0u64;
    let mut dirs_removed = 0u64;
    let mut index = 0usize;
    let mut cancelled = false;
    let mut attempted = 0u64;
    let mut last_progress = Instant::now();
    let total_files = plan.expected_files;

    for spec in &plan.specs {
        if cancel.load(Ordering::Relaxed) {
            cancelled = true;
            break;
        }
        // Revalidate the target itself: environment variables and the catalog
        // may both have changed since the preview was built.
        guard_path(&spec.path)?;

        let exclusion_keys: Vec<String> =
            plan.exclusions.iter().map(|path| normalize_key(path)).collect();
        let walk = walk_target(
            &spec.path,
            MAX_FILES_PER_TARGET,
            MAX_FILES_PER_TARGET,
            Some(cancel),
            &exclusion_keys,
        );
        if walk.truncated {
            push_warning(
                &mut warnings,
                format!(
                    "{}: stopped after {} files; the rest of the location was left alone.",
                    spec.label, MAX_FILES_PER_TARGET
                ),
            );
        }
        if walk.cancelled {
            cancelled = true;
        }
        if !walk.files.is_empty() {
            fs::create_dir_all(&batch).map_err(|error| {
                format!("Could not create the quarantine folder: {error}")
            })?;
        }

        let target_root = fs::canonicalize(&spec.path).unwrap_or_else(|_| spec.path.clone());
        for (path, _size) in walk.files {
            if cancel.load(Ordering::Relaxed) {
                cancelled = true;
                break;
            }
            index += 1;
            attempted += 1;
            match quarantine_file(&path, &target_root, &batch, index) {
                Ok((stored, size)) => {
                    quarantined += 1;
                    moved_bytes += size;
                    manifest.items.push(ManifestItem {
                        original_path: path.to_string_lossy().to_string(),
                        stored_path: stored.to_string_lossy().to_string(),
                        size,
                    });
                    if items.len() < MAX_ITEMS {
                        items.push(CleanupItemResult {
                            target_id: spec.id.clone(),
                            path: path.to_string_lossy().to_string(),
                            bytes: size,
                            status: "quarantined".into(),
                            reason: None,
                            quarantine_path: Some(stored.to_string_lossy().to_string()),
                        });
                    }
                }
                Err(MoveError::Skipped(reason)) => {
                    skipped += 1;
                    if items.len() < MAX_ITEMS {
                        items.push(CleanupItemResult {
                            target_id: spec.id.clone(),
                            path: path.to_string_lossy().to_string(),
                            bytes: 0,
                            status: "skipped".into(),
                            reason: Some(reason),
                            quarantine_path: None,
                        });
                    }
                }
                Err(MoveError::Failed(reason)) => {
                    failed += 1;
                    if errors.len() < MAX_ERRORS {
                        errors.push(reason.clone());
                    }
                    if items.len() < MAX_ITEMS {
                        items.push(CleanupItemResult {
                            target_id: spec.id.clone(),
                            path: path.to_string_lossy().to_string(),
                            bytes: 0,
                            status: "failed".into(),
                            reason: Some(reason),
                            quarantine_path: None,
                        });
                    }
                }
            }

            // Flush periodically so an interrupted run still restores.
            if manifest.items.len() % 256 == 0 && !manifest.items.is_empty() {
                let _ = write_manifest(&batch, &manifest);
            }
            if last_progress.elapsed() >= PROGRESS_INTERVAL {
                last_progress = Instant::now();
                progress(CleanupProgress {
                    run_id: run_id.clone(),
                    files: attempted,
                    total_files,
                    bytes: moved_bytes,
                    current_path: path.to_string_lossy().to_string(),
                });
            }
        }

        remove_empty_dirs(&walk.dirs, &spec.path, &mut dirs_removed);
        if cancelled {
            break;
        }
    }

    if quarantined > 0 {
        write_manifest(&batch, &manifest)?;
    } else {
        // Nothing was moved, so there is nothing to restore or purge.
        let _ = fs::remove_dir(&batch);
    }

    progress(CleanupProgress {
        run_id: run_id.clone(),
        files: attempted,
        total_files,
        bytes: moved_bytes,
        current_path: String::new(),
    });

    for warning in warnings {
        if errors.len() < MAX_ERRORS {
            errors.push(warning);
        }
    }

    Ok(CleanupReport {
        run_id: run_id.clone(),
        plan_id: plan.plan_id.clone(),
        bytes: moved_bytes,
        files: quarantined,
        dirs_removed,
        quarantined,
        skipped,
        failed,
        items,
        errors,
        elapsed_ms: started.elapsed().as_millis() as u64,
        cancelled,
        quarantine_dir: if quarantined > 0 {
            Some(batch.to_string_lossy().to_string())
        } else {
            None
        },
        purge_at: now + RETENTION_DAYS * 86_400,
    })
}

fn quarantine_file(
    file: &Path,
    target_root: &Path,
    batch: &Path,
    index: usize,
) -> Result<(PathBuf, u64), MoveError> {
    let meta = fs::symlink_metadata(file)
        .map_err(|error| MoveError::Failed(describe(file, &error)))?;
    if !meta.file_type().is_file() {
        return Err(MoveError::Skipped("not a regular file".into()));
    }
    if has_reparse_attribute(meta.file_attributes()) {
        return Err(MoveError::Skipped("link or junction".into()));
    }

    // The parent is canonicalized right before the move so a reparse point or
    // renamed ancestor cannot redirect the destination outside the target.
    let parent = file
        .parent()
        .ok_or_else(|| MoveError::Skipped("no parent folder".into()))?;
    let parent_canonical = fs::canonicalize(parent)
        .map_err(|error| MoveError::Failed(describe(parent, &error)))?;
    if !is_within(&parent_canonical, target_root) {
        return Err(MoveError::Skipped(
            "path escaped its target folder".into(),
        ));
    }

    let name = file
        .file_name()
        .ok_or_else(|| MoveError::Skipped("no file name".into()))?;
    let destination = batch.join(format!("{index}-{}", name.to_string_lossy()));
    let size = meta.len();
    fs::rename(file, &destination).map_err(|error| MoveError::Failed(describe(file, &error)))?;
    Ok((destination, size))
}

fn remove_empty_dirs(dirs: &[PathBuf], root: &Path, removed: &mut u64) {
    let root_key = normalize_key(root);
    let mut ordered: Vec<&PathBuf> = dirs.iter().collect();
    ordered.sort_by_key(|dir| std::cmp::Reverse(normalize_key(dir.as_path()).len()));
    for dir in ordered {
        let key = normalize_key(dir);
        if key == root_key || !is_within_key(&key, &root_key) {
            continue;
        }
        match fs::symlink_metadata(dir) {
            Ok(meta) if has_reparse_attribute(meta.file_attributes()) => continue,
            Ok(meta) if !meta.is_dir() => continue,
            Ok(_) => {}
            Err(_) => continue,
        }
        // Removes only an already empty directory; never recursive.
        if fs::remove_dir(dir).is_ok() {
            *removed += 1;
        }
    }
}

/* -------------------------------------------------------------- quarantine */

/// Lists batches that still hold restorable files.
pub fn list_quarantine(root: &Path) -> Vec<QuarantineBatch> {
    let mut batches = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return batches;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let Some(manifest) = read_manifest(&dir) else {
            continue;
        };
        if manifest.items.is_empty() {
            continue;
        }
        batches.push(QuarantineBatch {
            run_id: manifest.run_id,
            created_at: manifest.created_at,
            expires_at: manifest.expires_at,
            bytes: manifest.items.iter().map(|item| item.size).sum(),
            file_count: manifest.items.len() as u64,
            dir: dir.to_string_lossy().to_string(),
            target_ids: manifest.target_ids,
        });
    }
    batches.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    batches
}

/// Puts quarantined files back where they came from. Existing files are never
/// overwritten; they are reported and stay quarantined.
pub fn restore_batch(root: &Path, run_id: &str) -> Result<RestoreReport, String> {
    let dir = batch_dir(root, run_id)?;
    let mut manifest = read_manifest(&dir)
        .ok_or_else(|| format!("No quarantine batch named {run_id} was found."))?;

    let mut report = RestoreReport {
        run_id: manifest.run_id.clone(),
        restored: 0,
        skipped: 0,
        failed: 0,
        errors: Vec::new(),
    };
    let mut remaining = Vec::new();

    for item in manifest.items.clone() {
        let original = PathBuf::from(&item.original_path);
        let stored = PathBuf::from(&item.stored_path);
        if original.exists() {
            report.skipped += 1;
            if report.errors.len() < MAX_ERRORS {
                report.errors.push(format!(
                    "{} already exists, so the quarantined copy was kept.",
                    original.display()
                ));
            }
            remaining.push(item);
            continue;
        }
        if !stored.exists() {
            report.failed += 1;
            if report.errors.len() < MAX_ERRORS {
                report
                    .errors
                    .push(format!("Missing quarantined file: {}", stored.display()));
            }
            remaining.push(item);
            continue;
        }
        if let Some(parent) = original.parent() {
            let _ = fs::create_dir_all(parent);
        }
        match move_back(&stored, &original) {
            Ok(()) => report.restored += 1,
            Err(error) => {
                report.failed += 1;
                if report.errors.len() < MAX_ERRORS {
                    report.errors.push(format!(
                        "{} could not be restored: {error}",
                        original.display()
                    ));
                }
                remaining.push(item);
            }
        }
    }

    manifest.items = remaining;
    if manifest.items.is_empty() {
        let _ = fs::remove_file(dir.join(MANIFEST));
        // `remove_dir` only succeeds on an empty folder, so nothing outside the
        // batch can be removed here.
        let _ = fs::remove_dir(&dir);
    } else {
        let _ = write_manifest(&dir, &manifest);
    }
    Ok(report)
}

/// Deletes the files of one batch, then the batch itself. Only paths recorded
/// in that batch's manifest and still inside the batch folder are removed.
pub fn purge_batch(root: &Path, run_id: &str) -> Result<PurgeReport, String> {
    let dir = batch_dir(root, run_id)?;
    let manifest = read_manifest(&dir)
        .ok_or_else(|| format!("No quarantine batch named {run_id} was found."))?;
    let canonical_dir = fs::canonicalize(&dir).unwrap_or_else(|_| dir.clone());

    let mut report = PurgeReport {
        run_id: manifest.run_id.clone(),
        removed_files: 0,
        freed_bytes: 0,
    };
    for item in &manifest.items {
        let stored = Path::new(&item.stored_path);
        let Ok(canonical) = fs::canonicalize(stored) else {
            continue;
        };
        if !is_within(&canonical, &canonical_dir) {
            continue;
        }
        if fs::remove_file(&canonical).is_ok() {
            report.removed_files += 1;
            report.freed_bytes += item.size;
        }
    }

    let _ = fs::remove_file(dir.join(MANIFEST));
    // `remove_dir` fails if anything unexpected is still inside, which is the
    // behaviour we want.
    let _ = fs::remove_dir(&dir);
    Ok(report)
}

fn batch_dir(root: &Path, run_id: &str) -> Result<PathBuf, String> {
    let trimmed = run_id.trim();
    if trimmed.is_empty()
        || trimmed.len() > 80
        || !trimmed
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err("The quarantine batch id is not valid.".into());
    }
    Ok(root.join(trimmed))
}

fn move_back(stored: &Path, original: &Path) -> Result<(), String> {
    if fs::rename(stored, original).is_ok() {
        return Ok(());
    }
    // Cross-volume moves need a copy; the quarantined file is only removed
    // once the copy succeeded.
    fs::copy(stored, original)
        .map_err(|error| error.to_string())
        .and_then(|_| fs::remove_file(stored).map_err(|error| error.to_string()))
}

fn read_manifest(dir: &Path) -> Option<BatchManifest> {
    let text = fs::read_to_string(dir.join(MANIFEST)).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_manifest(dir: &Path, manifest: &BatchManifest) -> Result<(), String> {
    fs::create_dir_all(dir)
        .map_err(|error| format!("Could not create the quarantine folder: {error}"))?;
    let json = serde_json::to_string_pretty(manifest)
        .map_err(|error| format!("Could not encode the quarantine manifest: {error}"))?;
    let path = dir.join(MANIFEST);
    let temp = dir.join("manifest.json.tmp");
    fs::write(&temp, json).map_err(|error| format!("Could not write the manifest: {error}"))?;
    fs::rename(&temp, &path).or_else(|_| {
        fs::copy(&temp, &path)
            .map(|_| ())
            .map_err(|error| error.to_string())
    })?;
    let _ = fs::remove_file(&temp);
    Ok(())
}

/* --------------------------------------------------------------------- log */

/// Reads the append-only operation log, newest first.
pub fn load_log(dir: &Path) -> Vec<CleanupLogEntry> {
    fs::read_to_string(dir.join(LOG_FILE))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Records one operation, keeping the newest 100 entries.
pub fn append_log(dir: &Path, entry: &CleanupLogEntry) -> Result<(), String> {
    if entry.id.trim().is_empty() {
        return Err("A cleanup log entry needs an id.".into());
    }
    let mut entries = load_log(dir);
    entries.retain(|existing| existing.id != entry.id);
    entries.insert(0, entry.clone());
    entries.truncate(100);

    fs::create_dir_all(dir).map_err(|error| format!("Could not create the log folder: {error}"))?;
    let json = serde_json::to_string_pretty(&entries)
        .map_err(|error| format!("Could not encode the cleanup log: {error}"))?;
    let path = dir.join(LOG_FILE);
    let temp = dir.join("cleanup-log.json.tmp");
    fs::write(&temp, json).map_err(|error| format!("Could not write the cleanup log: {error}"))?;
    if fs::rename(&temp, &path).is_err() {
        fs::copy(&temp, &path).map_err(|error| format!("Could not save the cleanup log: {error}"))?;
        let _ = fs::remove_file(&temp);
    }
    Ok(())
}

/* ------------------------------------------------------------------ safety */

/// Refuses roots, profile folders and any location that contains protected
/// catalog data. Every planned and executed target passes through here.
pub fn guard_path(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!(
            "Refusing to clean a relative path: {}",
            path.display()
        ));
    }
    let key = normalize_key(path);
    let parts: Vec<&str> = key.split('\\').filter(|part| !part.is_empty()).collect();
    if parts.len() < 3 {
        return Err(format!(
            "Refusing to clean a top-level location: {}",
            path.display()
        ));
    }
    for blocked in blocked_roots() {
        if key == blocked {
            return Err(format!(
                "Refusing to clean {}: Cyron only cleans its reviewed cache locations.",
                path.display()
            ));
        }
    }
    for protected in protected_paths() {
        if protected != key && is_within_key(&protected, &key) {
            return Err(format!(
                "Refusing to clean {}: it contains protected data.",
                path.display()
            ));
        }
    }
    Ok(())
}

/// True when the path itself is a reparse point (junction, symlink or mount
/// point). Never followed by the cleaner.
pub fn is_reparse(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|meta| has_reparse_attribute(meta.file_attributes()))
        .unwrap_or(false)
}

fn has_reparse_attribute(attributes: u32) -> bool {
    attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

fn blocked_roots() -> Vec<String> {
    let mut roots = Vec::new();
    for variable in [
        "USERPROFILE",
        "LOCALAPPDATA",
        "APPDATA",
        "WINDIR",
        "SystemDrive",
        "ProgramFiles",
        "ProgramFiles(x86)",
    ] {
        if let Ok(value) = std::env::var(variable) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                roots.push(normalize_key(Path::new(trimmed)));
            }
        }
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        for folder in ["Desktop", "Documents", "Downloads"] {
            roots.push(normalize_key(&Path::new(&profile).join(folder)));
        }
    }
    roots
}

fn protected_paths() -> Vec<String> {
    caches::CATALOG
        .iter()
        .filter(|definition| definition.risk == "protected")
        .filter_map(|definition| caches::resolve_template(definition.path).ok())
        .map(|path| normalize_key(Path::new(&path)))
        .collect()
}

fn is_within(child: &Path, parent: &Path) -> bool {
    is_within_key(&normalize_key(child), &normalize_key(parent))
}

fn is_within_key(child: &str, parent: &str) -> bool {
    child == parent || child.starts_with(&format!("{parent}\\"))
}

/// Lowercase, separator-normalized, prefix-free comparison key.
fn normalize_key(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let stripped = raw
        .strip_prefix("\\\\?\\")
        .or_else(|| raw.strip_prefix("\\??\\"))
        .unwrap_or(&raw);
    let unc = stripped.starts_with("\\\\");
    let mut parts: Vec<String> = Vec::new();
    for part in stripped.split(['\\', '/']) {
        match part {
            "" | "." => continue,
            ".." => {
                parts.pop();
            }
            _ => parts.push(part.to_ascii_lowercase()),
        }
    }
    let mut joined = parts.join("\\");
    if unc && !joined.starts_with("\\\\") {
        joined.insert_str(0, "\\\\");
    }
    joined
}

/* ----------------------------------------------------------------- walking */

struct WalkOutcome {
    exists: bool,
    file_count: u64,
    bytes: u64,
    dir_count: u64,
    /// Retained file paths with their sizes, capped by the caller.
    files: Vec<(PathBuf, u64)>,
    dirs: Vec<PathBuf>,
    skipped: Vec<(PathBuf, String)>,
    errors: Vec<String>,
    truncated: bool,
    cancelled: bool,
}

impl Default for WalkOutcome {
    fn default() -> Self {
        Self {
            exists: false,
            file_count: 0,
            bytes: 0,
            dir_count: 0,
            files: Vec::new(),
            dirs: Vec::new(),
            skipped: Vec::new(),
            errors: Vec::new(),
            truncated: false,
            cancelled: false,
        }
    }
}

/// Breadth-first-free depth-first walk that never follows a reparse point and
/// stops at the file cap. `retain` bounds the returned path list while counts
/// and bytes stay exact.
fn walk_target(
    root: &Path,
    file_cap: usize,
    retain: usize,
    cancel: Option<&AtomicBool>,
    exclusions: &[String],
) -> WalkOutcome {
    let excluded = |path: &Path| -> bool {
        if exclusions.is_empty() {
            return false;
        }
        let key = normalize_key(path);
        exclusions.iter().any(|prefix| is_within_key(&key, prefix))
    };
    let mut outcome = WalkOutcome::default();
    let meta = match fs::symlink_metadata(root) {
        Ok(meta) => meta,
        Err(_) => return outcome,
    };
    outcome.exists = true;
    if has_reparse_attribute(meta.file_attributes()) {
        outcome.skipped.push((
            root.to_path_buf(),
            "the target itself is a link or junction".into(),
        ));
        return outcome;
    }
    if !meta.is_dir() {
        outcome
            .errors
            .push(format!("{} is not a folder", root.display()));
        return outcome;
    }

    let mut stack: Vec<(PathBuf, u32)> = vec![(root.to_path_buf(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        if cancel.map(|flag| flag.load(Ordering::Relaxed)).unwrap_or(false) {
            outcome.cancelled = true;
            break;
        }
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) => {
                if outcome.errors.len() < MAX_ERRORS {
                    outcome.errors.push(describe(&dir, &error));
                }
                continue;
            }
        };
        for entry in entries {
            if cancel.map(|flag| flag.load(Ordering::Relaxed)).unwrap_or(false) {
                outcome.cancelled = true;
                break;
            }
            let Ok(entry) = entry else {
                continue;
            };
            let path = entry.path();
            let Ok(entry_meta) = entry.metadata() else {
                if outcome.skipped.len() < MAX_SKIPS {
                    outcome
                        .skipped
                        .push((path, "metadata is not readable".into()));
                }
                continue;
            };
            if has_reparse_attribute(entry_meta.file_attributes()) {
                if outcome.skipped.len() < MAX_SKIPS {
                    outcome
                        .skipped
                        .push((path, "link or junction, never followed".into()));
                }
                continue;
            }
            if excluded(&path) {
                if outcome.skipped.len() < MAX_SKIPS {
                    outcome
                        .skipped
                        .push((path, "excluded in Settings".into()));
                }
                continue;
            }
            if entry_meta.is_dir() {
                outcome.dir_count += 1;
                if outcome.dirs.len() < MAX_DIRS {
                    outcome.dirs.push(path.clone());
                } else {
                    outcome.truncated = true;
                }
                if depth + 1 >= MAX_DEPTH {
                    if outcome.errors.len() < MAX_ERRORS {
                        outcome.errors.push(format!(
                            "{}: nested deeper than {MAX_DEPTH} levels",
                            path.display()
                        ));
                    }
                    continue;
                }
                stack.push((path, depth + 1));
            } else if entry_meta.is_file() {
                outcome.file_count += 1;
                outcome.bytes += entry_meta.len();
                if outcome.files.len() < retain {
                    outcome.files.push((path, entry_meta.len()));
                }
                if outcome.file_count >= file_cap as u64 {
                    outcome.truncated = true;
                    return outcome;
                }
            }
        }
        if outcome.cancelled {
            break;
        }
    }
    outcome
}

/* ----------------------------------------------------------------- helpers */

fn describe(path: &Path, error: &std::io::Error) -> String {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => {
            format!("{}: in use or access denied", path.display())
        }
        std::io::ErrorKind::NotFound => format!("{}: no longer exists", path.display()),
        _ if error.raw_os_error() == Some(32) => {
            format!("{}: in use by another program", path.display())
        }
        _ => format!("{}: {error}", path.display()),
    }
}

fn push_warning(warnings: &mut Vec<String>, message: String) {
    if warnings.len() < MAX_WARNINGS && !warnings.iter().any(|existing| existing == &message) {
        warnings.push(message);
    }
}

fn seconds_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn next_id(prefix: &str, at: u64) -> String {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.subsec_nanos())
        .unwrap_or(0);
    format!("{prefix}-{at}-{nanos}-{}-{sequence}", std::process::id())
}

/* ------------------------------------------------------------------- tests */

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new(tag: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "cyron-cleanup-{tag}-{}-{nanos}",
                std::process::id()
            ));
            fs::create_dir_all(root.join("cache").join("nested")).expect("fixture dirs");
            fs::create_dir_all(root.join("cache").join("empty")).expect("fixture dirs");
            fs::create_dir_all(root.join("outside")).expect("fixture dirs");
            fs::create_dir_all(root.join("quarantine")).expect("fixture dirs");
            fs::write(root.join("cache").join("a.txt"), b"hello").expect("fixture file");
            fs::write(root.join("cache").join("nested").join("b.bin"), b"0123456789")
                .expect("fixture file");
            fs::write(root.join("outside").join("keep.txt"), b"keep me").expect("fixture file");
            Self { root }
        }

        fn cache(&self) -> PathBuf {
            self.root.join("cache")
        }

        fn nested(&self) -> PathBuf {
            self.root.join("cache").join("nested")
        }

        fn quarantine(&self) -> PathBuf {
            self.root.join("quarantine")
        }

        fn outside(&self) -> PathBuf {
            self.root.join("outside").join("keep.txt")
        }

        fn spec(&self) -> CleanupSpec {
            CleanupSpec {
                id: "fixture-cache".into(),
                tool: "Fixture".into(),
                label: "Fixture cache".into(),
                risk: "safe".into(),
                category: "packages".into(),
                note: "Test fixture cache folder.".into(),
                impact: "Recreated by the fixture tool.".into(),
                path: self.cache(),
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn run(fixture: &Fixture, plan: &CleanupPlan) -> CleanupReport {
        execute_plan(plan, &fixture.quarantine(), &AtomicBool::new(false), &|_| {})
            .expect("execute fixture plan")
    }

    #[test]
    fn protected_catalog_entries_are_refused() {
        let protected = caches::CATALOG
            .iter()
            .find(|definition| definition.risk == "protected")
            .expect("a protected catalog entry");
        let error = catalog_specs(&[protected.id.to_string()]).expect_err("must refuse");
        assert!(error.contains("protected"), "{error}");
    }

    #[test]
    fn unknown_and_traversal_ids_are_refused() {
        for id in ["not-a-target", "..\\Windows", "C:\\Windows\\System32", ""] {
            let error = catalog_specs(&[id.to_string()]).expect_err("must refuse");
            assert!(
                error.contains("Unknown") || error.contains("empty"),
                "{id}: {error}"
            );
        }
    }

    #[test]
    fn catalog_selection_is_deduplicated_and_bounded() {
        let ids = vec!["npm-cache".to_string(), " npm-cache ".to_string()];
        let specs = catalog_specs(&ids).expect("known target");
        assert_eq!(specs.len(), 1);
        let too_many: Vec<String> = (0..=MAX_IDS).map(|_| "npm-cache".to_string()).collect();
        assert!(catalog_specs(&too_many).is_err());
    }

    #[test]
    fn top_level_and_user_roots_are_refused() {
        for raw in ["C:\\", "C:"] {
            assert!(guard_path(Path::new(raw)).is_err(), "{raw} must be refused");
        }
        for variable in ["USERPROFILE", "LOCALAPPDATA", "APPDATA", "WINDIR", "SystemDrive"] {
            if let Ok(value) = std::env::var(variable) {
                assert!(
                    guard_path(Path::new(&value)).is_err(),
                    "{variable} must be refused"
                );
            }
        }
    }

    #[test]
    fn ancestors_of_protected_data_are_refused() {
        let profile = std::env::var("USERPROFILE").expect("USERPROFILE");
        let claude = Path::new(&profile).join(".claude");
        let error = guard_path(&claude).expect_err("parent of protected history");
        assert!(error.contains("protected"), "{error}");
    }

    #[test]
    fn catalog_cleanup_paths_pass_the_guard() {
        for definition in caches::CATALOG
            .iter()
            .filter(|definition| definition.risk != "protected")
        {
            let Ok(path) = caches::resolve_template(definition.path) else {
                continue;
            };
            assert!(
                guard_path(Path::new(&path)).is_ok(),
                "{} at {path} must be cleanable",
                definition.id
            );
        }
    }

    #[test]
    fn preview_counts_every_file_in_the_target() {
        let fixture = Fixture::new("preview");
        let (_, preview) = plan_specs(vec![fixture.spec()], false).expect("plan");
        assert_eq!(preview.file_count, 2);
        assert_eq!(preview.bytes, 15);
        assert_eq!(preview.files.len(), 2);
        assert_eq!(preview.hidden_count, 0);
        assert_eq!(preview.targets.len(), 1);
        assert!(preview.targets[0].exists);
        assert!(preview.targets[0].includable);
        assert_eq!(preview.targets[0].dir_count, 2);
        assert!(preview.skipped.is_empty());
        assert!(!preview.truncated);
    }

    #[test]
    fn large_targets_report_a_truncated_listing_but_full_totals() {
        let fixture = Fixture::new("listing");
        let bulk = fixture.cache().join("bulk");
        fs::create_dir_all(&bulk).expect("bulk dir");
        let extra = MAX_PREVIEW_FILES + 25;
        for index in 0..extra {
            fs::write(bulk.join(format!("f{index}.tmp")), b"x").expect("bulk file");
        }
        let (_, preview) = plan_specs(vec![fixture.spec()], false).expect("plan");
        // Two fixture files plus the bulk set are all counted and all moved.
        assert_eq!(preview.file_count as usize, extra + 2);
        assert_eq!(preview.bytes as usize, extra + 15);
        assert_eq!(preview.files.len(), MAX_PREVIEW_FILES);
        assert_eq!(preview.hidden_count as usize, extra + 2 - MAX_PREVIEW_FILES);
        assert!(preview.truncated, "a capped listing must say it is truncated");
        assert!(preview
            .warnings
            .iter()
            .any(|warning| warning.contains("lists the first")));
    }

    #[test]
    fn exclusions_are_never_counted_or_moved() {
        let fixture = Fixture::new("excluded");
        let kept = fixture.cache().join("nested").join("b.bin");
        let (plan, preview) = plan_specs_with_exclusions(
            vec![fixture.spec()],
            false,
            vec![fixture.nested()],
        )
        .expect("plan");
        assert_eq!(preview.file_count, 1, "only the non-excluded file counts");
        assert_eq!(preview.bytes, 5);
        assert!(preview
            .skipped
            .iter()
            .any(|skip| skip.reason.contains("excluded in Settings")));

        let cancel = AtomicBool::new(false);
        let report = execute_plan(&plan, &fixture.quarantine(), &cancel, &|_| {})
            .expect("execute excluded plan");
        assert_eq!(report.quarantined, 1);
        assert!(kept.exists(), "an excluded file must stay where it is");
        assert!(!fixture.cache().join("a.txt").exists());
    }

    #[test]
    fn exclusion_paths_must_be_absolute_and_inside_a_drive() {
        assert!(validate_exclusions(vec!["relative\\folder".into()]).is_err());
        assert!(validate_exclusions(vec!["C:\\".into()]).is_err());
        let accepted = validate_exclusions(vec![
            "C:\\Users\\me\\keep".into(),
            " C:\\Users\\me\\keep ".into(),
        ])
        .expect("absolute folder");
        assert_eq!(accepted.len(), 1, "duplicates collapse to one exclusion");
    }

    #[test]
    fn missing_target_previews_as_empty() {
        let fixture = Fixture::new("missing");
        let mut spec = fixture.spec();
        spec.path = fixture.root.join("does-not-exist");
        let (_, preview) = plan_specs(vec![spec], false).expect("plan");
        assert!(!preview.targets[0].exists);
        assert_eq!(preview.file_count, 0);
        assert!(preview
            .warnings
            .iter()
            .any(|warning| warning.contains("Nothing to move")));
    }

    #[test]
    fn execute_moves_files_and_leaves_everything_else() {
        let fixture = Fixture::new("execute");
        let (plan, _) = plan_specs(vec![fixture.spec()], false).expect("plan");
        let report = run(&fixture, &plan);

        assert_eq!(report.quarantined, 2);
        assert_eq!(report.failed, 0);
        assert_eq!(report.bytes, 15);
        assert!(!report.cancelled);
        assert!(report.dirs_removed >= 2, "empty folders are cleaned");
        assert!(fixture.outside().exists());
        assert!(!fixture.cache().join("a.txt").exists());
        assert!(!fixture.nested().exists());
        assert!(report.quarantine_dir.is_some());
        let batch = PathBuf::from(report.quarantine_dir.clone().expect("batch"));
        assert!(batch.join(MANIFEST).exists());
        assert!(report.items.iter().all(|item| item.status == "quarantined"));
    }

    #[test]
    fn restore_round_trips_content() {
        let fixture = Fixture::new("restore");
        let (plan, _) = plan_specs(vec![fixture.spec()], false).expect("plan");
        let report = run(&fixture, &plan);
        let restore = restore_batch(&fixture.quarantine(), &report.run_id).expect("restore");

        assert_eq!(restore.restored, 2);
        assert_eq!(restore.failed, 0);
        assert_eq!(
            fs::read(fixture.cache().join("a.txt")).expect("restored file"),
            b"hello"
        );
        assert_eq!(
            fs::read(fixture.nested().join("b.bin")).expect("restored file"),
            b"0123456789"
        );
        assert!(list_quarantine(&fixture.quarantine()).is_empty());
    }

    #[test]
    fn restore_never_overwrites_an_existing_file() {
        let fixture = Fixture::new("restore-conflict");
        let (plan, _) = plan_specs(vec![fixture.spec()], false).expect("plan");
        let report = run(&fixture, &plan);
        fs::write(fixture.cache().join("a.txt"), b"newer").expect("recreate original");

        let restore = restore_batch(&fixture.quarantine(), &report.run_id).expect("restore");
        assert!(restore.skipped >= 1);
        assert_eq!(
            fs::read(fixture.cache().join("a.txt")).expect("kept file"),
            b"newer"
        );
        assert_eq!(restore.restored, 1);
    }

    #[test]
    fn purge_removes_only_batch_files() {
        let fixture = Fixture::new("purge");
        let (plan, _) = plan_specs(vec![fixture.spec()], false).expect("plan");
        let report = run(&fixture, &plan);
        let batch = PathBuf::from(report.quarantine_dir.clone().expect("batch"));

        let purge = purge_batch(&fixture.quarantine(), &report.run_id).expect("purge");
        assert_eq!(purge.removed_files, 2);
        assert_eq!(purge.freed_bytes, 15);
        assert!(!batch.exists());
        assert!(fixture.outside().exists());
    }

    #[test]
    fn quarantine_ids_cannot_escape_their_folder() {
        let fixture = Fixture::new("escape");
        for id in ["..", "..\\Windows", "C:\\", "run/one", ""] {
            assert!(
                restore_batch(&fixture.quarantine(), id).is_err(),
                "{id} must be refused"
            );
            assert!(
                purge_batch(&fixture.quarantine(), id).is_err(),
                "{id} must be refused"
            );
        }
    }

    #[test]
    fn read_only_plans_cannot_execute() {
        let fixture = Fixture::new("read-only");
        let (plan, preview) = plan_specs(vec![fixture.spec()], true).expect("plan");
        assert!(preview.read_only);
        let error = execute_plan(&plan, &fixture.quarantine(), &AtomicBool::new(false), &|_| {})
            .expect_err("read-only must refuse");
        assert!(error.contains("Read-only"), "{error}");
        assert!(fixture.cache().join("a.txt").exists());
    }

    #[test]
    fn cancellation_stops_before_moving() {
        let fixture = Fixture::new("cancel");
        let (plan, _) = plan_specs(vec![fixture.spec()], false).expect("plan");
        let cancel = AtomicBool::new(true);
        let report = execute_plan(&plan, &fixture.quarantine(), &cancel, &|_| {}).expect("run");
        assert!(report.cancelled);
        assert_eq!(report.quarantined, 0);
        assert!(fixture.cache().join("a.txt").exists());
    }

    #[test]
    fn reparse_attributes_are_recognized() {
        assert!(has_reparse_attribute(0x400));
        assert!(has_reparse_attribute(0x400 | 0x10));
        assert!(!has_reparse_attribute(0x10));
        let fixture = Fixture::new("reparse");
        assert!(!is_reparse(&fixture.cache().join("a.txt")));
    }

    #[test]
    fn plan_ids_are_unique() {
        let fixture = Fixture::new("ids");
        let (first, _) = plan_specs(vec![fixture.spec()], false).expect("plan");
        let (second, _) = plan_specs(vec![fixture.spec()], false).expect("plan");
        assert_ne!(first.plan_id, second.plan_id);
    }

    #[test]
    fn audit_log_is_newest_first_and_capped() {
        let fixture = Fixture::new("log");
        for index in 0..3 {
            append_log(
                &fixture.root,
                &CleanupLogEntry {
                    id: format!("entry-{index}"),
                    at: 1_700_000_000 + index,
                    title: "Fixture".into(),
                    targets: vec!["fixture-cache".into()],
                    bytes: 15,
                    files: 2,
                    quarantined: 2,
                    skipped: 0,
                    failed: 0,
                    status: "completed".into(),
                    detail: "Fixture run".into(),
                },
            )
            .expect("append log");
        }
        let log = load_log(&fixture.root);
        assert_eq!(log.len(), 3);
        assert_eq!(log[0].id, "entry-2");
    }

    #[test]
    fn empty_preview_cannot_execute() {
        let fixture = Fixture::new("empty-plan");
        assert!(plan_specs(Vec::new(), false).is_err());
        let (plan, _) = plan_specs(vec![fixture.spec()], false).expect("plan");
        fs::remove_dir_all(fixture.cache()).expect("clear fixture");
        let report = run(&fixture, &plan);
        assert_eq!(report.quarantined, 0);
        assert!(report.quarantine_dir.is_none());
    }
}
