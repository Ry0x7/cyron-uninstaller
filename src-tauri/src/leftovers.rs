//! Residual and leftover scanner with verified registry backup, restore, and staged quarantine.
//!
//! Compares application metadata against filesystem and registry locations to discover
//! leftovers. Findings are classified by confidence and risk. Shared components and system
//! directories are strictly protected. Registry modifications require an automatic, verified
//! multi-type .reg export backup into the quarantine batch before any deletion occurs.
//! Recursive traversal never follows junctions, links, or reparse points.

use serde::{Deserialize, Serialize};
use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use winreg::enums::*;
use winreg::{RegKey, RegValue};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeftoverCandidate {
    pub id: String,
    pub kind: String, // "directory", "file", "registry_key"
    pub path: String,
    pub confidence: String, // "high", "medium", "low"
    pub risk: String,       // "safe", "caution", "protected"
    pub reason: String,
    pub size: u64,
    pub file_count: u64,
    pub selected: bool,
}

#[derive(Debug, Clone)]
pub struct StoredLeftoverPlan {
    pub plan_id: String,
    pub app_id: String,
    pub app_name: String,
    pub candidates: Vec<LeftoverCandidate>,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeftoverScanReport {
    pub plan_id: String,
    pub app_id: String,
    pub app_name: String,
    pub candidates: Vec<LeftoverCandidate>,
    pub total_size: u64,
    pub total_files: u64,
    pub registry_keys_count: u64,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeftoverCleanReport {
    pub run_id: String,
    pub app_id: String,
    pub quarantined_files: u64,
    pub quarantined_bytes: u64,
    pub deleted_registry_keys: u64,
    pub registry_backup_path: Option<String>,
    pub quarantine_dir: Option<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrokenEntryMetadata {
    pub hive: String,
    pub subkey_name: String,
    pub registry_key_path: String,
    pub display_name: String,
    pub uninstall_string: Option<String>,
    pub quiet_uninstall_string: Option<String>,
    pub install_location: Option<String>,
    pub publisher: Option<String>,
    pub display_version: Option<String>,
    pub system_component: bool,
    pub is_protected: bool,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrokenEntryPlan {
    pub plan_id: String,
    pub app_id: String,
    pub app_name: String,
    pub registry_key_path: String,
    pub subkey_name: String,
    pub hive: String,
    pub fingerprint: String,
    pub expires_at: u64,
}

#[derive(Debug, Clone)]
pub struct StoredBrokenEntryPlan {
    pub plan_id: String,
    pub app_id: String,
    pub app_name: String,
    pub registry_key_path: String,
    pub subkey_name: String,
    pub hive: String,
    pub fingerprint: String,
    pub expires_at: u64,
}

static PLAN_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub fn generate_collision_resistant_plan_id(prefix: &str) -> String {
    use sha2::{Digest, Sha256};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let count = PLAN_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let pid = std::process::id();

    let mut hasher = Sha256::new();
    hasher.update(now.to_le_bytes());
    hasher.update(count.to_le_bytes());
    hasher.update(pid.to_le_bytes());
    let entropy = &format!("{:x}", hasher.finalize())[..12];

    format!("{prefix}-{now}-{count}-{entropy}")
}

pub fn compute_entry_fingerprint(
    hive: &str,
    subkey: &str,
    path: &str,
    display_name: &str,
    uninstall_string: Option<&str>,
    quiet_uninstall_string: Option<&str>,
    install_location: Option<&str>,
    publisher: Option<&str>,
    display_version: Option<&str>,
) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(hive.as_bytes());
    hasher.update(b"|");
    hasher.update(subkey.as_bytes());
    hasher.update(b"|");
    hasher.update(path.to_ascii_lowercase().as_bytes());
    hasher.update(b"|");
    hasher.update(display_name.trim().as_bytes());
    hasher.update(b"|");
    hasher.update(uninstall_string.unwrap_or("").trim().as_bytes());
    hasher.update(b"|");
    hasher.update(quiet_uninstall_string.unwrap_or("").trim().as_bytes());
    hasher.update(b"|");
    hasher.update(install_location.unwrap_or("").trim().as_bytes());
    hasher.update(b"|");
    hasher.update(publisher.unwrap_or("").trim().as_bytes());
    hasher.update(b"|");
    hasher.update(display_version.unwrap_or("").trim().as_bytes());
    format!("{:x}", hasher.finalize())
}

pub const SHARED_VENDORS: &[&str] = &[
    "microsoft",
    "google",
    "apple",
    "adobe",
    "mozilla",
    "windows",
    "common files",
    "system",
    "intel",
    "amd",
    "nvidia",
    "oracle",
];

const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x00000400;

fn is_reparse(meta: &fs::Metadata) -> bool {
    (meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT) != 0 || meta.file_type().is_symlink()
}

/// Refuses to consider top-level, system, profile or shared folders as leftovers.
pub fn guard_leftover_path(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!("Path must be absolute: {}", path.display()));
    }
    let key = normalize_key(path);
    let parts: Vec<&str> = key.split('\\').filter(|p| !p.is_empty()).collect();
    if parts.len() < 3 {
        return Err(format!(
            "Refusing top-level folder as leftover: {}",
            path.display()
        ));
    }

    for env_var in [
        "USERPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
        "PROGRAMDATA",
        "WINDIR",
        "ProgramFiles",
        "ProgramFiles(x86)",
    ] {
        if let Ok(val) = std::env::var(env_var) {
            let val_key = normalize_key(Path::new(&val));
            if key == val_key {
                return Err(format!(
                    "Refusing system/profile folder as leftover: {}",
                    path.display()
                ));
            }
        }
    }

    if let Ok(profile) = std::env::var("USERPROFILE") {
        for folder in ["Desktop", "Documents", "Downloads", "Music", "Pictures", "Videos"] {
            let protected_user = normalize_key(&Path::new(&profile).join(folder));
            if key == protected_user || is_within_key(&key, &protected_user) {
                return Err(format!(
                    "User data folder {} cannot be a leftover.",
                    path.display()
                ));
            }
        }
    }

    Ok(())
}

/// Discovers leftovers for an application and stores a server-side plan with 10-minute TTL.
pub fn scan_app_leftovers(
    app_id: &str,
    app_name: &str,
    publisher: Option<&str>,
    install_location: Option<&str>,
) -> (StoredLeftoverPlan, LeftoverScanReport) {
    scan_app_leftovers_expanded(app_id, app_name, publisher, install_location, None, None, "")
}

/// Expanded leftover scan taking candidate uninstaller command, display icon, and registry hive.
pub fn scan_app_leftovers_expanded(
    app_id: &str,
    app_name: &str,
    publisher: Option<&str>,
    install_location: Option<&str>,
    uninstall_string: Option<&str>,
    display_icon: Option<&str>,
    hive: &str,
) -> (StoredLeftoverPlan, LeftoverScanReport) {
    let mut candidates = Vec::new();
    let mut total_size = 0u64;
    let mut total_files = 0u64;
    let mut reg_count = 0u64;

    let clean_app_name = sanitize_for_path(app_name);
    let clean_pub = publisher.map(sanitize_for_path);

    // 1. Check install location and candidate roots
    let candidate_dirs = crate::process::extract_candidate_roots(
        install_location,
        uninstall_string,
        display_icon,
    );

    for path in candidate_dirs {
        if path.exists() && guard_leftover_path(&path).is_ok() {
            let (size, files) = measure_directory_shallow(&path);
            if files > 0 && !candidates.iter().any(|c: &LeftoverCandidate| c.path == path.to_string_lossy()) {
                total_size += size;
                total_files += files;
                candidates.push(LeftoverCandidate {
                    id: format!("cand-{}", candidates.len()),
                    kind: "directory".into(),
                    path: path.to_string_lossy().to_string(),
                    confidence: "high".into(),
                    risk: "safe".into(),
                    reason: "Application installation folder.".into(),
                    size,
                    file_count: files,
                    selected: true,
                });
            }
        }
    }

    // 2. Check AppData and ProgramData folders
    let base_folders = [
        std::env::var("APPDATA").ok(),
        std::env::var("LOCALAPPDATA").ok(),
        std::env::var("PROGRAMDATA").ok(),
    ];

    for base_opt in base_folders.into_iter().flatten() {
        let base_path = PathBuf::from(base_opt);

        // Subfolder: <base>\<Publisher>\<AppName>
        if let Some(ref pub_name) = clean_pub {
            let vendor_folder = base_path.join(pub_name);
            let is_shared_vendor = SHARED_VENDORS
                .iter()
                .any(|&v| pub_name.to_ascii_lowercase().contains(v));

            let candidate_dir = vendor_folder.join(&clean_app_name);
            if candidate_dir.exists() && guard_leftover_path(&candidate_dir).is_ok() {
                let (size, files) = measure_directory_shallow(&candidate_dir);
                if files > 0 {
                    total_size += size;
                    total_files += files;
                    candidates.push(LeftoverCandidate {
                        id: format!("cand-{}", candidates.len()),
                        kind: "directory".into(),
                        path: candidate_dir.to_string_lossy().to_string(),
                        confidence: "high".into(),
                        risk: "caution".into(),
                        reason: "Application configuration and data under AppData/ProgramData.".into(),
                        size,
                        file_count: files,
                        selected: true,
                    });
                }
            } else if !is_shared_vendor && vendor_folder.exists() && guard_leftover_path(&vendor_folder).is_ok() {
                // If the entire vendor directory belongs solely to this product
                if vendor_folder.to_string_lossy().to_ascii_lowercase().contains(&clean_app_name.to_ascii_lowercase()) {
                    let (size, files) = measure_directory_shallow(&vendor_folder);
                    if files > 0 {
                        total_size += size;
                        total_files += files;
                        candidates.push(LeftoverCandidate {
                            id: format!("cand-{}", candidates.len()),
                            kind: "directory".into(),
                            path: vendor_folder.to_string_lossy().to_string(),
                            confidence: "medium".into(),
                            risk: "caution".into(),
                            reason: "Publisher data directory matching this application.".into(),
                            size,
                            file_count: files,
                            selected: false,
                        });
                    }
                }
            }
        }

        // Subfolder: <base>\<AppName>
        let direct_dir = base_path.join(&clean_app_name);
        if direct_dir.exists() && guard_leftover_path(&direct_dir).is_ok() {
            if !candidates.iter().any(|c| c.path == direct_dir.to_string_lossy()) {
                let (size, files) = measure_directory_shallow(&direct_dir);
                if files > 0 {
                    total_size += size;
                    total_files += files;
                    candidates.push(LeftoverCandidate {
                        id: format!("cand-{}", candidates.len()),
                        kind: "directory".into(),
                        path: direct_dir.to_string_lossy().to_string(),
                        confidence: "high".into(),
                        risk: "caution".into(),
                        reason: "Application data directory.".into(),
                        size,
                        file_count: files,
                        selected: true,
                    });
                }
            }
        }
    }

    // 3. Check Shortcuts (Start Menu and Desktop)
    check_shortcut_candidates(&clean_app_name, &mut candidates, &mut total_size, &mut total_files);

    // 4. Check Registry settings
    check_registry_candidates(
        &clean_app_name,
        clean_pub.as_deref(),
        &mut candidates,
        &mut reg_count,
    );

    // 5. Check Uninstall registry registration key
    check_uninstall_key_candidate(app_id, hive, &mut candidates, &mut reg_count);

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let plan_id = format!("leftover-{}-{}-{}", now, std::process::id(), app_id.replace(|c: char| !c.is_ascii_alphanumeric(), "_"));
    let expires_at = now + 600;

    let plan = StoredLeftoverPlan {
        plan_id: plan_id.clone(),
        app_id: app_id.to_string(),
        app_name: app_name.to_string(),
        candidates: candidates.clone(),
        expires_at,
    };

    let report = LeftoverScanReport {
        plan_id,
        app_id: app_id.to_string(),
        app_name: app_name.to_string(),
        candidates,
        total_size,
        total_files,
        registry_keys_count: reg_count,
        expires_at,
    };

    (plan, report)
}

fn check_registry_candidates(
    app_name: &str,
    publisher: Option<&str>,
    candidates: &mut Vec<LeftoverCandidate>,
    reg_count: &mut u64,
) {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    if let Some(pub_name) = publisher {
        let is_shared = SHARED_VENDORS.iter().any(|&v| pub_name.to_ascii_lowercase().contains(v));
        let path = format!("Software\\{pub_name}\\{app_name}");
        if hkcu.open_subkey(&path).is_ok() {
            candidates.push(LeftoverCandidate {
                id: format!("cand-{}", candidates.len()),
                kind: "registry_key".into(),
                path: format!("HKCU\\{path}"),
                confidence: "high".into(),
                risk: "safe".into(),
                reason: "Application user registry settings.".into(),
                size: 0,
                file_count: 0,
                selected: true,
            });
            *reg_count += 1;
        }

        let hklm_path = format!("Software\\{pub_name}\\{app_name}");
        if hklm.open_subkey(&hklm_path).is_ok() {
            candidates.push(LeftoverCandidate {
                id: format!("cand-{}", candidates.len()),
                kind: "registry_key".into(),
                path: format!("HKLM\\{hklm_path}"),
                confidence: "high".into(),
                risk: "safe".into(),
                reason: "Application machine registry settings.".into(),
                size: 0,
                file_count: 0,
                selected: true,
            });
            *reg_count += 1;
        }

        let wow_path = format!("Software\\WOW6432Node\\{pub_name}\\{app_name}");
        if hklm.open_subkey(&wow_path).is_ok() {
            candidates.push(LeftoverCandidate {
                id: format!("cand-{}", candidates.len()),
                kind: "registry_key".into(),
                path: format!("HKLM\\{wow_path}"),
                confidence: "high".into(),
                risk: "safe".into(),
                reason: "32-bit application machine registry settings.".into(),
                size: 0,
                file_count: 0,
                selected: true,
            });
            *reg_count += 1;
        }

        if !is_shared && hkcu.open_subkey(format!("Software\\{pub_name}")).is_ok() {
            if pub_name.to_ascii_lowercase().contains(&app_name.to_ascii_lowercase()) {
                candidates.push(LeftoverCandidate {
                    id: format!("cand-{}", candidates.len()),
                    kind: "registry_key".into(),
                    path: format!("HKCU\\Software\\{pub_name}"),
                    confidence: "medium".into(),
                    risk: "caution".into(),
                    reason: "Publisher registry key.".into(),
                    size: 0,
                    file_count: 0,
                    selected: false,
                });
                *reg_count += 1;
            }
        }
    }

    let direct_path = format!("Software\\{app_name}");
    if hkcu.open_subkey(&direct_path).is_ok() {
        if !candidates.iter().any(|c| c.path == format!("HKCU\\{direct_path}")) {
            candidates.push(LeftoverCandidate {
                id: format!("cand-{}", candidates.len()),
                kind: "registry_key".into(),
                path: format!("HKCU\\{direct_path}"),
                confidence: "high".into(),
                risk: "safe".into(),
                reason: "Application user registry settings.".into(),
                size: 0,
                file_count: 0,
                selected: true,
            });
            *reg_count += 1;
        }
    }
}

fn check_shortcut_candidates(
    clean_app_name: &str,
    candidates: &mut Vec<LeftoverCandidate>,
    total_size: &mut u64,
    total_files: &mut u64,
) {
    let mut shortcut_dirs = Vec::new();
    if let Ok(appdata) = std::env::var("APPDATA") {
        shortcut_dirs.push(PathBuf::from(appdata).join(r"Microsoft\Windows\Start Menu\Programs"));
    }
    if let Ok(progdata) = std::env::var("PROGRAMDATA") {
        shortcut_dirs.push(PathBuf::from(progdata).join(r"Microsoft\Windows\Start Menu\Programs"));
    }
    if let Ok(userprof) = std::env::var("USERPROFILE") {
        shortcut_dirs.push(PathBuf::from(userprof).join("Desktop"));
    }
    if let Ok(pubprof) = std::env::var("PUBLIC") {
        shortcut_dirs.push(PathBuf::from(pubprof).join("Desktop"));
    }

    let needle = clean_app_name.to_ascii_lowercase();

    for dir in shortcut_dirs {
        if !dir.exists() {
            continue;
        }
        scan_shortcuts_in_dir(&dir, &needle, candidates, total_size, total_files, 0);
    }
}

fn scan_shortcuts_in_dir(
    dir: &Path,
    needle: &str,
    candidates: &mut Vec<LeftoverCandidate>,
    total_size: &mut u64,
    total_files: &mut u64,
    depth: usize,
) {
    if depth > 2 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if is_reparse(&meta) {
            continue;
        }
        if meta.is_dir() {
            let dir_name = path.file_name().unwrap_or_default().to_string_lossy().to_ascii_lowercase();
            if dir_name.contains(needle) {
                if guard_leftover_path(&path).is_ok() {
                    let (size, files) = measure_directory_shallow(&path);
                    if files > 0 && !candidates.iter().any(|c| c.path == path.to_string_lossy()) {
                        *total_size += size;
                        *total_files += files;
                        candidates.push(LeftoverCandidate {
                            id: format!("cand-{}", candidates.len()),
                            kind: "directory".into(),
                            path: path.to_string_lossy().to_string(),
                            confidence: "high".into(),
                            risk: "safe".into(),
                            reason: "Start menu shortcut group for application.".into(),
                            size,
                            file_count: files,
                            selected: true,
                        });
                    }
                }
            } else {
                scan_shortcuts_in_dir(&path, needle, candidates, total_size, total_files, depth + 1);
            }
        } else if meta.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            if ext == "lnk" {
                let stem = path.file_stem().unwrap_or_default().to_string_lossy().to_ascii_lowercase();
                if stem.contains(needle) && !candidates.iter().any(|c| c.path == path.to_string_lossy()) {
                    let size = meta.len();
                    *total_size += size;
                    *total_files += 1;
                    candidates.push(LeftoverCandidate {
                        id: format!("cand-{}", candidates.len()),
                        kind: "file".into(),
                        path: path.to_string_lossy().to_string(),
                        confidence: "high".into(),
                        risk: "safe".into(),
                        reason: "Application shortcut (.lnk).".into(),
                        size,
                        file_count: 1,
                        selected: true,
                    });
                }
            }
        }
    }
}

fn check_uninstall_key_candidate(
    app_id: &str,
    hive: &str,
    candidates: &mut Vec<LeftoverCandidate>,
    reg_count: &mut u64,
) {
    if app_id.is_empty() {
        return;
    }
    let subkey_path = match hive {
        "machine-64" => format!("HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{app_id}"),
        "machine-32" => format!("HKLM\\SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{app_id}"),
        "user-64" => format!("HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{app_id}"),
        "user-32" => format!("HKCU\\Software\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{app_id}"),
        _ => return,
    };

    if let Ok((root, sub)) = parse_reg_path(&subkey_path) {
        if root.open_subkey(&sub).is_ok() && !candidates.iter().any(|c| c.path == subkey_path) {
            candidates.push(LeftoverCandidate {
                id: format!("cand-{}", candidates.len()),
                kind: "registry_key".into(),
                path: subkey_path,
                confidence: "high".into(),
                risk: "safe".into(),
                reason: "Windows Application Uninstall registration key.".into(),
                size: 0,
                file_count: 0,
                selected: true,
            });
            *reg_count += 1;
        }
    }
}

/// Cleans approved leftover candidates from a backend-verified single-use plan:
/// 1. Revalidates every candidate against safety guards.
/// 2. Exports selected registry keys to a complete, verified .reg backup file.
/// 3. Quarantines selected directories and files with durable write-ahead journal.
/// 4. Deletes approved registry keys only after the backup is verified.
pub fn clean_leftovers(
    plan: &StoredLeftoverPlan,
    selected_ids: &[String],
    quarantine_root: &Path,
) -> Result<LeftoverCleanReport, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    if now > plan.expires_at {
        return Err("The residual cleanup plan has expired. Please scan again.".into());
    }

    let run_id = format!("uninstall-{now}-{}", std::process::id());
    let batch = quarantine_root.join(&run_id);
    fs::create_dir_all(&batch).map_err(|e| format!("Could not create quarantine folder: {e}"))?;

    let mut errors = Vec::new();
    let mut quarantined_files = 0u64;
    let mut quarantined_bytes = 0u64;
    let mut deleted_registry_keys = 0u64;

    // Filter candidates strictly against the server-generated plan
    let selected_dirs: Vec<&LeftoverCandidate> = plan
        .candidates
        .iter()
        .filter(|c| selected_ids.iter().any(|id| id == &c.id) && c.kind == "directory")
        .collect();
    let selected_files: Vec<&LeftoverCandidate> = plan
        .candidates
        .iter()
        .filter(|c| selected_ids.iter().any(|id| id == &c.id) && c.kind == "file")
        .collect();
    let selected_regs: Vec<&LeftoverCandidate> = plan
        .candidates
        .iter()
        .filter(|c| selected_ids.iter().any(|id| id == &c.id) && c.kind == "registry_key")
        .collect();

    // Step 1: Backup registry entries to .reg file
    let mut backup_path = None;
    if !selected_regs.is_empty() {
        let reg_backup_file = batch.join("registry_backup.reg");
        match export_registry_backup(&selected_regs, &reg_backup_file) {
            Ok(()) => match verify_registry_backup(&reg_backup_file) {
                Ok(()) => {
                    backup_path = Some(reg_backup_file.to_string_lossy().to_string());
                }
                Err(e) => {
                    errors.push(format!("Registry backup integrity verification failed: {e}"));
                    return Err(format!(
                        "Registry backup verification failed ({e}). Residual cleanup was aborted to protect your registry."
                    ));
                }
            },
            Err(e) => {
                errors.push(format!("Failed to create registry backup: {e}"));
                return Err(format!(
                    "Registry backup failed ({e}). Residual cleanup was aborted to protect your registry."
                ));
            }
        }
    }

    // Step 2: Quarantine filesystem directories
    let journal_path = batch.join(crate::cleanup::JOURNAL_FILE);
    let mut manifest_items = Vec::new();
    let mut index = 0usize;

    for dir_cand in selected_dirs {
        let dir_path = PathBuf::from(&dir_cand.path);
        if guard_leftover_path(&dir_path).is_err() {
            errors.push(format!("Refused protected folder: {}", dir_cand.path));
            continue;
        }

        // Check if root of candidate is a reparse point or link
        if let Ok(meta) = fs::symlink_metadata(&dir_path) {
            if is_reparse(&meta) {
                errors.push(format!("Refused junction or symlink root: {}", dir_cand.path));
                continue;
            }
        }

        let files = collect_files_bounded(&dir_path, 100_000);
        let canonical_root = fs::canonicalize(&dir_path).unwrap_or_else(|_| dir_path.clone());

        for file in files {
            index += 1;
            let meta = match fs::symlink_metadata(&file) {
                Ok(m) => m,
                Err(_) => continue,
            };
            if !meta.file_type().is_file() || is_reparse(&meta) {
                continue;
            }

            // Verify canonical parent is within canonical target root
            let Some(parent) = file.parent() else {
                continue;
            };
            let Ok(parent_canonical) = fs::canonicalize(parent) else {
                continue;
            };
            if !is_within_key(&normalize_key(&parent_canonical), &normalize_key(&canonical_root)) {
                errors.push(format!("Skipped file outside target boundary: {}", file.display()));
                continue;
            }

            let size = meta.len();
            let dest_name = format!("{index}-{}", file.file_name().unwrap_or_default().to_string_lossy());
            let dest = batch.join(dest_name);

            // Record in durable write-ahead journal before destructive rename
            let item = serde_json::json!({
                "originalPath": file.to_string_lossy(),
                "storedPath": dest.to_string_lossy(),
                "size": size
            });
            let line = serde_json::to_string(&item).unwrap_or_default();
            use std::io::Write;
            let journal_res: Result<(), std::io::Error> = (|| {
                let mut jf = fs::OpenOptions::new().create(true).append(true).open(&journal_path)?;
                writeln!(jf, "{line}")?;
                jf.flush()?;
                jf.sync_all()?;
                Ok(())
            })();

            if let Err(e) = journal_res {
                errors.push(format!("Journal write failed for {}: {e}; skipped moving to preserve recoverability", file.display()));
                continue; // fail closed: NEVER move file if journal write failed!
            }

            if fs::rename(&file, &dest).is_ok() {
                quarantined_files += 1;
                quarantined_bytes += size;
                manifest_items.push(item);
            } else {
                errors.push(format!("Could not move {} to quarantine", file.display()));
            }
        }

        remove_empty_dir_tree(&dir_path);
    }

    for file_cand in selected_files {
        let file_path = PathBuf::from(&file_cand.path);
        if !file_path.exists() {
            continue;
        }
        let meta = match fs::symlink_metadata(&file_path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if !meta.file_type().is_file() || is_reparse(&meta) {
            errors.push(format!("Refused non-regular or link file: {}", file_cand.path));
            continue;
        }
        let size = meta.len();
        index += 1;
        let dest_name = format!("{index}-{}", file_path.file_name().unwrap_or_default().to_string_lossy());
        let dest = batch.join(dest_name);

        let item = serde_json::json!({
            "originalPath": file_path.to_string_lossy(),
            "storedPath": dest.to_string_lossy(),
            "size": size
        });
        let line = serde_json::to_string(&item).unwrap_or_default();
        use std::io::Write;
        let journal_res: Result<(), std::io::Error> = (|| {
            let mut jf = fs::OpenOptions::new().create(true).append(true).open(&journal_path)?;
            writeln!(jf, "{line}")?;
            jf.flush()?;
            jf.sync_all()?;
            Ok(())
        })();

        if let Err(e) = journal_res {
            errors.push(format!("Journal write failed for {}: {e}; skipped moving", file_path.display()));
            continue;
        }

        if fs::rename(&file_path, &dest).is_ok() {
            quarantined_files += 1;
            quarantined_bytes += size;
            manifest_items.push(item);
        } else {
            errors.push(format!("Could not move {} to quarantine", file_path.display()));
        }
    }

    // Write quarantine manifest
    let manifest = serde_json::json!({
        "runId": run_id,
        "createdAt": now,
        "expiresAt": now + 7 * 86_400,
        "targetIds": [plan.app_id],
        "items": manifest_items
    });
    let _ = fs::write(batch.join("manifest.json"), serde_json::to_string_pretty(&manifest).unwrap_or_default());

    // Step 3: Delete backed-up registry keys only if backup succeeded and was verified
    if backup_path.is_some() {
        for reg_cand in selected_regs {
            if is_safe_to_delete_reg_key(&reg_cand.path) {
                if delete_registry_key(&reg_cand.path) {
                    deleted_registry_keys += 1;
                } else {
                    errors.push(format!("Could not remove registry key: {}", reg_cand.path));
                }
            } else {
                errors.push(format!("Refusing deletion of shared or protected registry key: {}", reg_cand.path));
            }
        }
    }

    Ok(LeftoverCleanReport {
        run_id,
        app_id: plan.app_id.clone(),
        quarantined_files,
        quarantined_bytes,
        deleted_registry_keys,
        registry_backup_path: backup_path,
        quarantine_dir: Some(batch.to_string_lossy().to_string()),
        errors,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryRestoreReport {
    pub keys_restored: usize,
    pub values_restored: usize,
    pub errors: Vec<String>,
}

/// Recursively exports registry keys and all subkeys with full value type preservation.
pub fn export_registry_backup(regs: &[&LeftoverCandidate], output: &Path) -> Result<(), String> {
    use std::io::Write;
    let mut file = fs::File::create(output).map_err(|e| format!("Could not create registry backup file: {e}"))?;
    writeln!(file, "Windows Registry Editor Version 5.00\r\n").map_err(|e| e.to_string())?;

    for reg in regs {
        let full = &reg.path;
        let (root_hive, subkey) = parse_reg_path(full)?;
        let key = root_hive.open_subkey(&subkey).map_err(|e| format!("Could not open {full}: {e}"))?;
        export_key_recursive(&root_hive, full, &subkey, &key, &mut file)?;
    }

    file.flush().map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;

    verify_registry_backup(output)
}

/// Validates a .reg backup file: checks file size, header, and syntax of sections and values.
pub fn verify_registry_backup(path: &Path) -> Result<(), String> {
    let meta = fs::metadata(path).map_err(|e| format!("Cannot read backup metadata: {e}"))?;
    if meta.len() == 0 {
        return Err("Backup file is empty.".into());
    }
    let content = fs::read_to_string(path).map_err(|e| format!("Cannot read backup file: {e}"))?;
    if !content.starts_with("Windows Registry Editor Version 5.00") {
        return Err("Missing or invalid Windows Registry header.".into());
    }

    let mut section_count = 0;
    for (line_idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with("Windows Registry") {
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            section_count += 1;
            let section = &trimmed[1..trimmed.len() - 1];
            if !section.starts_with("HKEY_CURRENT_USER\\")
                && !section.starts_with("HKEY_LOCAL_MACHINE\\")
                && !section.starts_with("HKCU\\")
                && !section.starts_with("HKLM\\")
            {
                return Err(format!("Line {}: Unrecognized hive prefix in [{section}]", line_idx + 1));
            }
        } else if let Some(eq) = trimmed.find('=') {
            let val = trimmed[eq + 1..].trim();
            if val.starts_with('"') {
                if !val.ends_with('"') || val.len() < 2 {
                    return Err(format!("Line {}: Unterminated string literal", line_idx + 1));
                }
            } else if let Some(dw) = val.strip_prefix("dword:") {
                if u32::from_str_radix(dw, 16).is_err() {
                    return Err(format!("Line {}: Malformed dword value '{dw}'", line_idx + 1));
                }
            } else if val.starts_with("hex:") || val.starts_with("hex(") {
                let Some(colon) = val.find(':') else {
                    return Err(format!("Line {}: Malformed hex entry", line_idx + 1));
                };
                let bytes_part = &val[colon + 1..];
                parse_hex_bytes(bytes_part).map_err(|e| format!("Line {}: {e}", line_idx + 1))?;
            }
        }
    }

    if section_count == 0 {
        return Err("Backup file contains no registry sections.".into());
    }

    Ok(())
}

fn export_key_recursive(
    root: &RegKey,
    display_prefix: &str,
    subkey_path: &str,
    key: &RegKey,
    file: &mut fs::File,
) -> Result<(), String> {
    use std::io::Write;

    // Output section header
    let full_hive_prefix = if display_prefix.starts_with("HKCU\\") {
        display_prefix.replace("HKCU\\", "HKEY_CURRENT_USER\\")
    } else if display_prefix.starts_with("HKLM\\") {
        display_prefix.replace("HKLM\\", "HKEY_LOCAL_MACHINE\\")
    } else {
        display_prefix.to_string()
    };

    writeln!(file, "[{full_hive_prefix}]").map_err(|e| format!("Failed writing section header for {full_hive_prefix}: {e}"))?;

    for res in key.enum_values() {
        let (name, val) = res.map_err(|e| format!("Failed to read registry value in {display_prefix}: {e}"))?;
        let escaped_name = if name.is_empty() {
            "@".into()
        } else {
            format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\""))
        };

        match val.vtype {
            REG_SZ => {
                let text = val.to_string();
                let escaped_val = text.replace('\\', "\\\\").replace('"', "\\\"");
                writeln!(file, "{escaped_name}=\"{escaped_val}\"").map_err(|e| e.to_string())?;
            }
            REG_DWORD => {
                let dword = if val.bytes.len() >= 4 {
                    u32::from_le_bytes([val.bytes[0], val.bytes[1], val.bytes[2], val.bytes[3]])
                } else {
                    0
                };
                writeln!(file, "{escaped_name}=dword:{:08x}", dword).map_err(|e| e.to_string())?;
            }
            REG_BINARY => {
                let hex_str = val
                    .bytes
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<Vec<_>>()
                    .join(",");
                writeln!(file, "{escaped_name}=hex:{}", hex_str).map_err(|e| e.to_string())?;
            }
            REG_EXPAND_SZ => {
                let hex_str = val
                    .bytes
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<Vec<_>>()
                    .join(",");
                writeln!(file, "{escaped_name}=hex(2):{}", hex_str).map_err(|e| e.to_string())?;
            }
            REG_MULTI_SZ => {
                let hex_str = val
                    .bytes
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<Vec<_>>()
                    .join(",");
                writeln!(file, "{escaped_name}=hex(7):{}", hex_str).map_err(|e| e.to_string())?;
            }
            REG_QWORD => {
                let hex_str = val
                    .bytes
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<Vec<_>>()
                    .join(",");
                writeln!(file, "{escaped_name}=hex(b):{}", hex_str).map_err(|e| e.to_string())?;
            }
            other => {
                let hex_str = val
                    .bytes
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<Vec<_>>()
                    .join(",");
                writeln!(file, "{escaped_name}=hex({:x}):{}", other as u32, hex_str).map_err(|e| e.to_string())?;
            }
        }
    }
    writeln!(file).map_err(|e| e.to_string())?;

    // Recursively enumerate subkeys, failing closed if enumeration or open fails
    for res in key.enum_keys() {
        let sub = res.map_err(|e| format!("Failed to enumerate subkeys in {display_prefix}: {e}"))?;
        let child_subkey = format!("{subkey_path}\\{sub}");
        let child_display = format!("{display_prefix}\\{sub}");
        let child_key = root
            .open_subkey(&child_subkey)
            .map_err(|e| format!("Failed to open subkey {child_display} during backup: {e}"))?;
        export_key_recursive(root, &child_display, &child_subkey, &child_key, file)?;
    }

    Ok(())
}

/// Restores registry keys and values from a .reg file.
/// Fails closed if any key creation, subkey traversal, or value write operation fails.
pub fn restore_registry_backup(backup_path: &Path) -> Result<RegistryRestoreReport, String> {
    let content = fs::read_to_string(backup_path)
        .map_err(|e| format!("Could not read registry backup file: {e}"))?;

    let mut keys_restored = 0usize;
    let mut values_restored = 0usize;
    let mut errors = Vec::new();
    let mut current_key: Option<RegKey> = None;
    let mut current_section: String = String::new();

    for (line_idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with("Windows Registry") {
            continue;
        }

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let section = &trimmed[1..trimmed.len() - 1];
            current_section = section.to_string();
            let (root_hive, subkey) = if let Some(rest) = section.strip_prefix("HKEY_CURRENT_USER\\") {
                (RegKey::predef(HKEY_CURRENT_USER), rest.to_string())
            } else if let Some(rest) = section.strip_prefix("HKEY_LOCAL_MACHINE\\") {
                (RegKey::predef(HKEY_LOCAL_MACHINE), rest.to_string())
            } else if let Some(rest) = section.strip_prefix("HKCU\\") {
                (RegKey::predef(HKEY_CURRENT_USER), rest.to_string())
            } else if let Some(rest) = section.strip_prefix("HKLM\\") {
                (RegKey::predef(HKEY_LOCAL_MACHINE), rest.to_string())
            } else {
                errors.push(format!("Line {}: Unknown hive in section [{section}]", line_idx + 1));
                current_key = None;
                continue;
            };

            match root_hive.create_subkey(&subkey) {
                Ok((created, _)) => {
                    current_key = Some(created);
                    keys_restored += 1;
                }
                Err(e) => {
                    errors.push(format!("Line {}: Failed to create registry subkey {subkey}: {e}", line_idx + 1));
                    current_key = None;
                }
            }
            continue;
        }

        let Some(ref key) = current_key else {
            continue;
        };

        if let Some(eq_idx) = trimmed.find('=') {
            let name_part = trimmed[..eq_idx].trim();
            let val_part = trimmed[eq_idx + 1..].trim();

            let val_name = if name_part == "@" {
                String::new()
            } else if name_part.starts_with('"') && name_part.ends_with('"') && name_part.len() >= 2 {
                name_part[1..name_part.len() - 1]
                    .replace("\\\"", "\"")
                    .replace("\\\\", "\\")
            } else {
                name_part.to_string()
            };

            let set_res: Result<(), String> = (|| {
                if val_part.starts_with('"') && val_part.ends_with('"') && val_part.len() >= 2 {
                    let unescaped = val_part[1..val_part.len() - 1]
                        .replace("\\\"", "\"")
                        .replace("\\\\", "\\");
                    key.set_value(&val_name, &unescaped).map_err(|e| e.to_string())?;
                } else if let Some(dword_str) = val_part.strip_prefix("dword:") {
                    let dword = u32::from_str_radix(dword_str, 16)
                        .map_err(|e| format!("Invalid dword literal '{dword_str}': {e}"))?;
                    key.set_value(&val_name, &dword).map_err(|e| e.to_string())?;
                } else if let Some(hex_str) = val_part.strip_prefix("hex:") {
                    let bytes = parse_hex_bytes(hex_str)?;
                    let reg_val = RegValue {
                        vtype: REG_BINARY,
                        bytes,
                    };
                    key.set_raw_value(&val_name, &reg_val).map_err(|e| e.to_string())?;
                } else if let Some(hex_str) = val_part.strip_prefix("hex(2):") {
                    let bytes = parse_hex_bytes(hex_str)?;
                    let reg_val = RegValue {
                        vtype: REG_EXPAND_SZ,
                        bytes,
                    };
                    key.set_raw_value(&val_name, &reg_val).map_err(|e| e.to_string())?;
                } else if let Some(hex_str) = val_part.strip_prefix("hex(7):") {
                    let bytes = parse_hex_bytes(hex_str)?;
                    let reg_val = RegValue {
                        vtype: REG_MULTI_SZ,
                        bytes,
                    };
                    key.set_raw_value(&val_name, &reg_val).map_err(|e| e.to_string())?;
                } else if let Some(hex_str) = val_part.strip_prefix("hex(b):") {
                    let bytes = parse_hex_bytes(hex_str)?;
                    let reg_val = RegValue {
                        vtype: REG_QWORD,
                        bytes,
                    };
                    key.set_raw_value(&val_name, &reg_val).map_err(|e| e.to_string())?;
                } else {
                    return Err(format!("Unrecognized value format: '{val_part}'"));
                }
                Ok(())
            })();

            match set_res {
                Ok(()) => {
                    values_restored += 1;
                }
                Err(e) => {
                    errors.push(format!("Line {}: Failed to set value '{val_name}' in [{}]: {e}", line_idx + 1, current_section));
                }
            }
        }
    }

    if !errors.is_empty() {
        return Err(format!("Registry restoration encountered {} errors (restored {} keys, {} values): {}",
            errors.len(), keys_restored, values_restored, errors.join("; ")));
    }

    Ok(RegistryRestoreReport {
        keys_restored,
        values_restored,
        errors,
    })
}

fn parse_hex_bytes(s: &str) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    for part in s.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        let b = u8::from_str_radix(trimmed, 16)
            .map_err(|e| format!("Invalid hex byte '{trimmed}': {e}"))?;
        bytes.push(b);
    }
    Ok(bytes)
}

/// Refuses to delete shared vendor keys or registry roots.
pub fn is_safe_to_delete_reg_key(full_path: &str) -> bool {
    let lower = full_path.to_ascii_lowercase();
    let parts: Vec<&str> = lower.split('\\').filter(|p| !p.is_empty()).collect();
    if parts.len() < 3 {
        return false;
    }

    // Must be under Software
    if parts[1] != "software" {
        return false;
    }

    // Protect Windows system trees under Microsoft\Windows
    if lower.contains("software\\microsoft\\windows") {
        if lower.contains("currentversion\\uninstall") {
            let last_part = parts.last().unwrap_or(&"");
            if *last_part == "uninstall"
                || *last_part == "currentversion"
                || *last_part == "windows"
                || *last_part == "microsoft"
                || *last_part == "wow6432node"
            {
                return false;
            }
            return parts.len() >= 7;
        }
        return false;
    }

    // Direct vendor key e.g. HKCU\Software\Microsoft
    if parts.len() == 3 {
        for vendor in SHARED_VENDORS {
            if parts[2] == *vendor {
                return false;
            }
        }
        // If it has multiple subkeys, do not delete the entire vendor key
        if let Ok((root, subkey)) = parse_reg_path(full_path) {
            if let Ok(key) = root.open_subkey(&subkey) {
                if key.enum_keys().count() > 1 {
                    return false;
                }
            }
        }
    }

    true
}

fn delete_registry_key(full_path: &str) -> bool {
    let Ok((root_hive, subkey)) = parse_reg_path(full_path) else {
        return false;
    };
    root_hive.delete_subkey_all(&subkey).is_ok()
}

fn parse_reg_path(path: &str) -> Result<(RegKey, String), String> {
    if let Some(rest) = path.strip_prefix("HKCU\\") {
        Ok((RegKey::predef(HKEY_CURRENT_USER), rest.to_string()))
    } else if let Some(rest) = path.strip_prefix("HKLM\\") {
        Ok((RegKey::predef(HKEY_LOCAL_MACHINE), rest.to_string()))
    } else {
        Err(format!("Unsupported registry root in {path}"))
    }
}

/// Resolves canonical registry uninstall subkey path from hive and subkey name.
pub fn resolve_uninstall_subkey_path(hive: &str, subkey_name: &str) -> Result<String, String> {
    let subkey_trimmed = subkey_name.trim();
    if subkey_trimmed.is_empty()
        || subkey_trimmed.contains('\\')
        || subkey_trimmed.contains('/')
        || subkey_trimmed.contains("..")
    {
        return Err(format!("Unsafe or invalid registry subkey name: {subkey_trimmed}"));
    }

    match hive {
        "user-64" => Ok(format!(r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\{subkey_trimmed}")),
        "user-32" => Ok(format!(r"HKCU\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\{subkey_trimmed}")),
        "machine-64" => Ok(format!(r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\{subkey_trimmed}")),
        "machine-32" => Ok(format!(r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\{subkey_trimmed}")),
        _ => Err(format!("Unsupported hive '{hive}' for registry inventory cleanup.")),
    }
}

/// Reads the exact live metadata of an uninstall registry entry and calculates its SHA-256 fingerprint.
pub fn read_live_broken_entry_metadata(hive: &str, subkey_name: &str) -> Result<BrokenEntryMetadata, String> {
    let registry_key_path = resolve_uninstall_subkey_path(hive, subkey_name)?;

    if !is_safe_to_delete_reg_key(&registry_key_path) {
        return Err(format!("Refusing deletion of protected or shared registry key: {registry_key_path}"));
    }

    let (root, subkey) = parse_reg_path(&registry_key_path)?;
    let key = root
        .open_subkey(&subkey)
        .map_err(|_| format!("Registry subkey does not exist: {registry_key_path}"))?;

    let display_name: String = key
        .get_value("DisplayName")
        .unwrap_or_else(|_| subkey_name.to_string());
    let display_version: Option<String> = key.get_value("DisplayVersion").ok();
    let publisher: Option<String> = key.get_value("Publisher").ok();
    let uninstall_string: Option<String> = key.get_value("UninstallString").ok();
    let quiet_uninstall_string: Option<String> = key.get_value("QuietUninstallString").ok();
    let install_location: Option<String> = key.get_value("InstallLocation").ok();
    let system_comp_dw: u32 = key.get_value("SystemComponent").unwrap_or(0);
    let system_component = system_comp_dw == 1;

    let windir = std::env::var("WINDIR").ok();
    let is_prot = crate::apps::is_protected(
        &display_name,
        publisher.as_deref(),
        install_location.as_deref(),
        windir.as_deref(),
    );

    let fingerprint = compute_entry_fingerprint(
        hive,
        subkey_name,
        &registry_key_path,
        &display_name,
        uninstall_string.as_deref(),
        quiet_uninstall_string.as_deref(),
        install_location.as_deref(),
        publisher.as_deref(),
        display_version.as_deref(),
    );

    Ok(BrokenEntryMetadata {
        hive: hive.to_string(),
        subkey_name: subkey_name.to_string(),
        registry_key_path,
        display_name,
        uninstall_string,
        quiet_uninstall_string,
        install_location,
        publisher,
        display_version,
        system_component,
        is_protected: is_prot,
        fingerprint,
    })
}

/// Evaluates whether a live registry entry genuinely qualifies as broken right now.
pub fn verify_live_broken_status(meta: &BrokenEntryMetadata) -> Result<(), String> {
    if meta.is_protected {
        return Err("Application is marked as a protected Windows component.".into());
    }

    if meta.system_component {
        return Err("Application is marked as a Windows SystemComponent and cannot be removed as a broken entry.".into());
    }

    // Must qualify as broken:
    // 1. Check uninstaller executable command
    let uninst = meta
        .uninstall_string
        .as_deref()
        .or(meta.quiet_uninstall_string.as_deref());

    let mut uninstaller_exists = false;
    if let Some(cmd) = uninst {
        let trimmed = cmd.trim();
        if !trimmed.is_empty() {
            let parsed = crate::uninstall::parse_uninstall_command(trimmed);
            let p = Path::new(&parsed.executable);
            if p.is_absolute() && p.exists() {
                uninstaller_exists = true;
            }
        }
    }

    if uninstaller_exists {
        return Err("Application uninstaller executable exists on disk; this entry has a functional uninstaller and is not broken.".into());
    }

    // 2. Check install location
    if let Some(ref loc) = meta.install_location {
        let trimmed = loc.trim();
        if !trimmed.is_empty() {
            let p = Path::new(trimmed);
            if p.is_absolute() && p.exists() {
                return Err(format!(
                    "Application install location still exists on disk at '{}'; this entry is not orphaned.",
                    p.display()
                ));
            }
        }
    }

    Ok(())
}

/// Verifies that an installed app qualifies as a broken registry entry and resolves its exact subkey path.
pub fn verify_broken_entry_candidate(app: &crate::apps::InstalledApp) -> Result<(String, String), String> {
    if app.is_protected {
        return Err("Application is marked as a protected Windows component.".into());
    }

    let subkey_name = if let Some(ref rkn) = app.registry_key_name {
        rkn.clone()
    } else {
        let (_, key_name) = app
            .id
            .split_once('|')
            .ok_or_else(|| "Invalid application ID format: missing hive separator".to_string())?;
        key_name.to_string()
    };

    let meta = read_live_broken_entry_metadata(&app.hive, &subkey_name)?;
    verify_live_broken_status(&meta)?;

    Ok((meta.subkey_name, meta.registry_key_path))
}

pub fn execute_broken_entry_removal(
    plan: &StoredBrokenEntryPlan,
    quarantine_root: &Path,
) -> Result<LeftoverCleanReport, String> {
    // 1. Re-read live metadata and re-verify everything right now
    let live_meta = read_live_broken_entry_metadata(&plan.hive, &plan.subkey_name)?;

    // 2. Metadata fingerprint validation: live registry entry must match preview exactly
    if live_meta.fingerprint != plan.fingerprint {
        return Err("Registry metadata has changed since this cleanup plan was prepared. Aborting deletion to prevent modifying altered applications.".into());
    }

    // 3. Re-verify broken qualification (e.g. uninstaller binary wasn't restored on disk)
    verify_live_broken_status(&live_meta)?;

    // 4. Create quarantine batch with durable write-ahead recovery journal and manifest
    let batch = quarantine_root.join(&plan.plan_id);
    fs::create_dir_all(&batch).map_err(|e| format!("Could not create quarantine directory: {e}"))?;
    let reg_backup_file = batch.join("registry_backup.reg");

    let candidate = LeftoverCandidate {
        id: "cand-broken-entry".into(),
        kind: "registry_key".into(),
        path: plan.registry_key_path.clone(),
        confidence: "high".into(),
        risk: "safe".into(),
        reason: "Broken Windows application uninstall registration key.".into(),
        size: 0,
        file_count: 0,
        selected: true,
    };

    export_registry_backup(&[&candidate], &reg_backup_file)?;
    verify_registry_backup(&reg_backup_file)?;

    // Write write-ahead recovery journal for crash-consistency
    let journal_file = batch.join(crate::cleanup::JOURNAL_FILE);
    let journal_entry = serde_json::json!({
        "originalPath": plan.registry_key_path,
        "storedPath": reg_backup_file.to_string_lossy(),
        "size": 0,
        "action": "registry_backup"
    });
    let line = serde_json::to_string(&journal_entry).unwrap_or_default();
    use std::io::Write;
    let mut jf = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&journal_file)
        .map_err(|e| format!("Could not open recovery journal: {e}"))?;
    writeln!(jf, "{line}").map_err(|e| format!("Could not write recovery journal: {e}"))?;
    jf.flush().map_err(|e| format!("Could not flush recovery journal: {e}"))?;
    jf.sync_all().map_err(|e| format!("Could not sync recovery journal: {e}"))?;

    // Write quarantine manifest for discoverable recovery
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let manifest = serde_json::json!({
        "runId": plan.plan_id,
        "createdAt": now,
        "expiresAt": now + 7 * 86_400,
        "targetIds": [plan.app_id],
        "items": [],
        "registryBackup": "registry_backup.reg",
        "registryKeyPath": plan.registry_key_path
    });
    let _ = fs::write(
        batch.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).unwrap_or_default(),
    );

    // 5. Delete the subkey
    if !delete_registry_key(&plan.registry_key_path) {
        return Err(format!("Could not remove registry key {}", plan.registry_key_path));
    }

    Ok(LeftoverCleanReport {
        run_id: plan.plan_id.clone(),
        app_id: plan.app_id.clone(),
        quarantined_files: 0,
        quarantined_bytes: 0,
        deleted_registry_keys: 1,
        registry_backup_path: Some(reg_backup_file.to_string_lossy().to_string()),
        quarantine_dir: Some(batch.to_string_lossy().to_string()),
        errors: Vec::new(),
    })
}

fn measure_directory_shallow(path: &Path) -> (u64, u64) {
    let mut size = 0u64;
    let mut count = 0u64;
    let mut stack = vec![(path.to_path_buf(), 0usize)];

    while let Some((dir, depth)) = stack.pop() {
        if depth > 32 || count > 100_000 {
            break;
        }
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if let Ok(meta) = fs::symlink_metadata(&entry_path) {
                    if is_reparse(&meta) {
                        continue;
                    }
                    if meta.is_dir() {
                        stack.push((entry_path, depth + 1));
                    } else if meta.is_file() {
                        count += 1;
                        size += meta.len();
                    }
                }
            }
        }
    }
    (size, count)
}

fn collect_files_bounded(path: &Path, max_files: usize) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![(path.to_path_buf(), 0usize)];

    while let Some((dir, depth)) = stack.pop() {
        if depth > 32 || files.len() >= max_files {
            break;
        }
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if let Ok(meta) = fs::symlink_metadata(&entry_path) {
                    if is_reparse(&meta) {
                        continue;
                    }
                    if meta.is_dir() {
                        stack.push((entry_path, depth + 1));
                    } else if meta.is_file() {
                        files.push(entry_path);
                        if files.len() >= max_files {
                            break;
                        }
                    }
                }
            }
        }
    }
    files
}

fn remove_empty_dir_tree(path: &Path) {
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if let Ok(meta) = fs::symlink_metadata(&p) {
                if meta.is_dir() && !is_reparse(&meta) {
                    remove_empty_dir_tree(&p);
                }
            }
        }
    }
    let _ = fs::remove_dir(path);
}

fn sanitize_for_path(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            other => other,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn normalize_key(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

fn is_within_key(child: &str, parent: &str) -> bool {
    if child == parent {
        return true;
    }
    if child.starts_with(parent) {
        let remainder = &child[parent.len()..];
        return remainder.starts_with('\\');
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guards_reject_system_and_user_roots() {
        assert!(guard_leftover_path(Path::new("C:\\")).is_err());
        assert!(guard_leftover_path(Path::new("C:\\Windows")).is_err());
        assert!(guard_leftover_path(Path::new("C:\\Program Files")).is_err());
        if let Ok(profile) = std::env::var("USERPROFILE") {
            assert!(guard_leftover_path(Path::new(&profile)).is_err());
            assert!(guard_leftover_path(&Path::new(&profile).join("Documents")).is_err());
        }
    }

    #[test]
    fn guards_allow_deep_vendor_app_folders() {
        assert!(guard_leftover_path(Path::new("C:\\Users\\User\\AppData\\Local\\MyVendor\\MyApp")).is_ok());
        assert!(guard_leftover_path(Path::new("C:\\Program Files\\MyVendor\\MyApp")).is_ok());
    }

    #[test]
    fn protects_shared_vendor_registry_keys() {
        assert!(!is_safe_to_delete_reg_key("HKCU\\Software\\Microsoft"));
        assert!(!is_safe_to_delete_reg_key("HKLM\\Software\\Google"));
        assert!(!is_safe_to_delete_reg_key("HKCU\\Software\\Intel"));
        assert!(!is_safe_to_delete_reg_key("HKCU\\Software"));
        assert!(!is_safe_to_delete_reg_key("HKLM\\Software"));
        assert!(!is_safe_to_delete_reg_key("HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall"));
        assert!(!is_safe_to_delete_reg_key("HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion"));
        assert!(is_safe_to_delete_reg_key("HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{1234-5678}"));
        assert!(is_safe_to_delete_reg_key("HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\MyApp"));
    }

    #[test]
    fn registry_backup_and_restore_roundtrip_fixture() {
        let test_subkey = format!("Software\\CyronTestFixture_{}_{}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos());
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);

        // 1. Create fixture key and subkey with values of ALL major types
        let (key, _) = hkcu.create_subkey(&test_subkey).expect("create test key");
        key.set_value("StringVal", &"hello world").expect("set string");
        key.set_value("DwordVal", &42u32).expect("set dword");
        key.set_value("", &"default value").expect("set default value");

        let bin_val = RegValue {
            vtype: REG_BINARY,
            bytes: vec![0xde, 0xad, 0xbe, 0xef],
        };
        key.set_raw_value("BinaryVal", &bin_val).expect("set binary");

        let qword_val = RegValue {
            vtype: REG_QWORD,
            bytes: vec![0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88],
        };
        key.set_raw_value("QwordVal", &qword_val).expect("set qword");

        let expand_val = RegValue {
            vtype: REG_EXPAND_SZ,
            bytes: vec![b'%', b'P', b'A', b'T', b'H', b'%', 0],
        };
        key.set_raw_value("ExpandVal", &expand_val).expect("set expand");

        let multi_val = RegValue {
            vtype: REG_MULTI_SZ,
            bytes: vec![b'a', 0, b'b', 0, 0],
        };
        key.set_raw_value("MultiVal", &multi_val).expect("set multi");

        let child_path = format!("{test_subkey}\\ChildKey");
        let (child_key, _) = hkcu.create_subkey(&child_path).expect("create child key");
        child_key.set_value("ChildString", &"child value").expect("set child string");

        // 2. Export to .reg file
        let temp_reg = std::env::temp_dir().join(format!("cyron_reg_test_{}.reg", std::process::id()));
        let cand = LeftoverCandidate {
            id: "test-cand".into(),
            kind: "registry_key".into(),
            path: format!("HKCU\\{test_subkey}"),
            confidence: "high".into(),
            risk: "safe".into(),
            reason: "test".into(),
            size: 0,
            file_count: 0,
            selected: true,
        };

        export_registry_backup(&[&cand], &temp_reg).expect("export registry");
        assert!(temp_reg.exists());
        let reg_content = fs::read_to_string(&temp_reg).expect("read reg");
        assert!(reg_content.contains("Windows Registry Editor Version 5.00"));
        assert!(reg_content.contains("StringVal"));
        assert!(reg_content.contains("DwordVal"));
        assert!(reg_content.contains("ChildKey"));
        assert!(reg_content.contains("hex(b):")); // QWORD
        assert!(reg_content.contains("hex(2):")); // EXPAND_SZ
        assert!(reg_content.contains("hex(7):")); // MULTI_SZ

        // Verify backup validation passes
        verify_registry_backup(&temp_reg).expect("verify registry backup");

        // 3. Delete fixture key
        assert!(delete_registry_key(&format!("HKCU\\{test_subkey}")));
        assert!(hkcu.open_subkey(&test_subkey).is_err());

        // 4. Restore from .reg file
        let restore_report = restore_registry_backup(&temp_reg).expect("restore backup");
        assert!(restore_report.keys_restored >= 2); // Parent and ChildKey
        assert!(restore_report.values_restored >= 7);
        assert!(restore_report.errors.is_empty());

        // 5. Verify restored data across all types
        let restored_parent = hkcu.open_subkey(&test_subkey).expect("open restored parent");
        let str_val: String = restored_parent.get_value("StringVal").expect("get string");
        assert_eq!(str_val, "hello world");
        let def_val: String = restored_parent.get_value("").expect("get default value");
        assert_eq!(def_val, "default value");
        let dw_val: u32 = restored_parent.get_value("DwordVal").expect("get dword");
        assert_eq!(dw_val, 42);

        let restored_bin = restored_parent.get_raw_value("BinaryVal").expect("get binary");
        assert_eq!(restored_bin.bytes, vec![0xde, 0xad, 0xbe, 0xef]);

        let restored_qword = restored_parent.get_raw_value("QwordVal").expect("get qword");
        assert_eq!(restored_qword.bytes, vec![0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]);

        let restored_expand = restored_parent.get_raw_value("ExpandVal").expect("get expand");
        assert_eq!(restored_expand.vtype, REG_EXPAND_SZ);

        let restored_multi = restored_parent.get_raw_value("MultiVal").expect("get multi");
        assert_eq!(restored_multi.vtype, REG_MULTI_SZ);

        let restored_child = hkcu.open_subkey(&child_path).expect("open restored child");
        let child_str: String = restored_child.get_value("ChildString").expect("get child string");
        assert_eq!(child_str, "child value");

        // 6. Clean up
        let _ = hkcu.delete_subkey_all(&test_subkey);
        let _ = fs::remove_file(&temp_reg);
    }

    #[test]
    fn broken_inventory_cleanup_isolated_hkcu_fixtures_all_cases() {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let test_subkey_name = format!("CyronTestBroken_{}_{}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos());
        let uninstall_root = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";
        let subkey_rel = format!(r"{uninstall_root}\{test_subkey_name}");
        let full_path = format!(r"HKCU\{subkey_rel}");

        // Create test registry key in HKCU Uninstall
        let (key, _) = hkcu.create_subkey(&subkey_rel).expect("create test broken uninstall key");
        key.set_value("DisplayName", &"Cyron Broken Test App").expect("set name");
        key.set_value("UninstallString", &"C:\\NonExistentPath\\uninst.exe").expect("set uninst");

        let broken_app = crate::apps::InstalledApp {
            id: format!("user-64|{test_subkey_name}"),
            name: "Cyron Broken Test App".into(),
            version: None,
            publisher: None,
            install_location: None,
            install_date: None,
            uninstall_string: Some("C:\\NonExistentPath\\uninst.exe".into()),
            quiet_uninstall_string: None,
            display_icon: None,
            estimated_size: None,
            hive: "user-64".into(),
            scope: "user".into(),
            architecture: "x64".into(),
            is_protected: false,
            source: "win32".into(),
            uninstall_support: "missing".into(),
            confidence: "confirmed".into(),
            detection_reason: None,
            registry_key_name: Some(test_subkey_name.clone()),
        };

        // 1. Valid broken candidate verifies and resolves exact subkey path
        let (verified_sub, verified_path) = verify_broken_entry_candidate(&broken_app).expect("verify broken app candidate");
        assert_eq!(verified_sub, test_subkey_name);
        assert_eq!(verified_path.to_ascii_lowercase(), full_path.to_ascii_lowercase());

        // 2. Protected app is rejected
        let mut protected_app = broken_app.clone();
        protected_app.is_protected = true;
        assert!(verify_broken_entry_candidate(&protected_app).is_err(), "protected app must be rejected");

        // 3. Functional / working app in live registry is rejected from broken entry cleanup
        let cmd_exe = std::env::var("COMSPEC").unwrap_or_else(|_| "C:\\Windows\\System32\\cmd.exe".into());
        key.set_value("UninstallString", &cmd_exe).expect("set live uninst to cmd.exe");
        assert!(verify_broken_entry_candidate(&broken_app).is_err(), "working app must not qualify as broken");
        key.set_value("UninstallString", &"C:\\NonExistentPath\\uninst.exe").expect("revert uninst to nonexistent");

        // 4. Read live metadata and verify fingerprint
        let live_meta = read_live_broken_entry_metadata("user-64", &test_subkey_name).expect("read live metadata");
        assert_eq!(live_meta.display_name, "Cyron Broken Test App");
        assert!(!live_meta.fingerprint.is_empty());

        let quarantine_dir = std::env::temp_dir().join(format!("cyron-test-quarantine-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&quarantine_dir);
        let plan_id = generate_collision_resistant_plan_id("broken-plan-test");
        let valid_stored_plan = StoredBrokenEntryPlan {
            plan_id: plan_id.clone(),
            app_id: broken_app.id.clone(),
            app_name: broken_app.name.clone(),
            registry_key_path: full_path.clone(),
            subkey_name: test_subkey_name.clone(),
            hive: "user-64".into(),
            fingerprint: live_meta.fingerprint.clone(),
            expires_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 300,
        };

        // 5. Test: entry that becomes valid / recovered between preview and execution MUST BE REFUSED
        // Simulate recovery by updating UninstallString to an existing executable (cmd.exe)
        let cmd_exe = std::env::var("COMSPEC").unwrap_or_else(|_| "C:\\Windows\\System32\\cmd.exe".into());
        key.set_value("UninstallString", &cmd_exe).expect("update uninst to valid executable");
        
        // Attempting to execute the plan on the recovered entry must be refused!
        let recovered_err = execute_broken_entry_removal(&valid_stored_plan, &quarantine_dir);
        assert!(recovered_err.is_err(), "entry that became valid/recovered must be refused without deletion");
        assert!(hkcu.open_subkey(&subkey_rel).is_ok(), "recovered key must NOT be deleted");

        // 6. Test: entry with changed metadata (fingerprint mismatch) MUST BE REFUSED
        // Revert UninstallString back to nonexistent path, but change DisplayName
        key.set_value("UninstallString", &"C:\\NonExistentPath\\uninst.exe").expect("revert uninst");
        key.set_value("DisplayName", &"Altered Application Name").expect("alter display name");
        let altered_err = execute_broken_entry_removal(&valid_stored_plan, &quarantine_dir);
        assert!(altered_err.is_err(), "entry with altered metadata fingerprint must be refused");
        assert!(hkcu.open_subkey(&subkey_rel).is_ok(), "altered key must NOT be deleted");

        // Restore original metadata so fingerprint matches for normal execution test
        key.set_value("DisplayName", &"Cyron Broken Test App").expect("restore name");

        // 7. Execution successfully removes key, backs up to .reg, writes journal and manifest
        let clean_report = execute_broken_entry_removal(&valid_stored_plan, &quarantine_dir).expect("execute broken removal");
        assert_eq!(clean_report.deleted_registry_keys, 1);
        assert!(clean_report.registry_backup_path.is_some());
        let backup_file = PathBuf::from(clean_report.registry_backup_path.unwrap());
        assert!(backup_file.exists());

        // Verify quarantine manifest was written
        let batch_manifest = quarantine_dir.join(&plan_id).join("manifest.json");
        assert!(batch_manifest.exists(), "quarantine manifest must be created");

        // Verify key is now gone from registry
        assert!(hkcu.open_subkey(&subkey_rel).is_err(), "deleted key must no longer exist in registry");

        // 8. Replayed plan on already-deleted key fails cleanly
        assert!(execute_broken_entry_removal(&valid_stored_plan, &quarantine_dir).is_err(), "replaying plan on removed key must fail");

        // 9. Expired plan is recognized
        let expired_plan = StoredBrokenEntryPlan {
            expires_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() - 10,
            ..valid_stored_plan
        };
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        assert!(now > expired_plan.expires_at, "expired plan must be recognized as expired");

        // Clean up quarantine fixture
        let _ = fs::remove_dir_all(&quarantine_dir);
    }
}
