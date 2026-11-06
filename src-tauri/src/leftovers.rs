//! Residual and leftover scanner with registry backup and staged quarantine.
//!
//! Compares application metadata against filesystem and registry locations to discover
//! leftovers. Findings are classified by confidence and risk. Shared components and system
//! directories are strictly protected. Registry modifications require an automatic .reg
//! export backup into the quarantine batch before any deletion occurs.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use winreg::enums::*;
use winreg::RegKey;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeftoverScanReport {
    pub app_id: String,
    pub app_name: String,
    pub candidates: Vec<LeftoverCandidate>,
    pub total_size: u64,
    pub total_files: u64,
    pub registry_keys_count: u64,
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

const SHARED_VENDORS: &[&str] = &[
    "microsoft",
    "google",
    "apple",
    "adobe",
    "mozilla",
    "windows",
    "common files",
    "system",
];

/// Refuses to consider top-level, system or shared folders as leftovers.
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
            if key == protected_user || is_within_key(&protected_user, &key) {
                return Err(format!(
                    "User data folder {} cannot be a leftover.",
                    path.display()
                ));
            }
        }
    }

    Ok(())
}

/// Discovers leftovers for an installed or uninstalled application.
pub fn scan_app_leftovers(
    app_id: &str,
    app_name: &str,
    publisher: Option<&str>,
    install_location: Option<&str>,
) -> LeftoverScanReport {
    let mut candidates = Vec::new();
    let mut total_size = 0u64;
    let mut total_files = 0u64;
    let mut reg_count = 0u64;

    let clean_app_name = sanitize_for_path(app_name);
    let clean_pub = publisher.map(sanitize_for_path);

    // 1. Check install location if it exists
    if let Some(loc) = install_location {
        let path = PathBuf::from(loc.trim());
        if path.exists() && guard_leftover_path(&path).is_ok() {
            let (size, files) = measure_directory_shallow(&path);
            if files > 0 {
                total_size += size;
                total_files += files;
                candidates.push(LeftoverCandidate {
                    id: format!("dir-install-{}", candidates.len()),
                    kind: "directory".into(),
                    path: path.to_string_lossy().to_string(),
                    confidence: "high".into(),
                    risk: "safe".into(),
                    reason: "Remaining installation folder after uninstall.".into(),
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
                        id: format!("dir-appdata-{}", candidates.len()),
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
                            id: format!("dir-vendor-{}", candidates.len()),
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
            // Avoid duplicate
            if !candidates.iter().any(|c| c.path == direct_dir.to_string_lossy()) {
                let (size, files) = measure_directory_shallow(&direct_dir);
                if files > 0 {
                    total_size += size;
                    total_files += files;
                    candidates.push(LeftoverCandidate {
                        id: format!("dir-direct-{}", candidates.len()),
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

    // 3. Check Registry
    check_registry_candidates(
        &clean_app_name,
        clean_pub.as_deref(),
        &mut candidates,
        &mut reg_count,
    );

    LeftoverScanReport {
        app_id: app_id.to_string(),
        app_name: app_name.to_string(),
        candidates,
        total_size,
        total_files,
        registry_keys_count: reg_count,
    }
}

fn check_registry_candidates(
    app_name: &str,
    publisher: Option<&str>,
    candidates: &mut Vec<LeftoverCandidate>,
    reg_count: &mut u64,
) {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    // HKCU\Software\<Publisher>\<AppName>
    if let Some(pub_name) = publisher {
        let is_shared = SHARED_VENDORS.iter().any(|&v| pub_name.to_ascii_lowercase().contains(v));
        let path = format!("Software\\{pub_name}\\{app_name}");
        if hkcu.open_subkey(&path).is_ok() {
            candidates.push(LeftoverCandidate {
                id: format!("reg-hkcu-{}", candidates.len()),
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
                id: format!("reg-hklm-{}", candidates.len()),
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
                id: format!("reg-wow-{}", candidates.len()),
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
            // Only suggest vendor key if app name is contained in publisher name
            if pub_name.to_ascii_lowercase().contains(&app_name.to_ascii_lowercase()) {
                candidates.push(LeftoverCandidate {
                    id: format!("reg-hkcu-vendor-{}", candidates.len()),
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

    // Direct HKCU\Software\<AppName>
    let direct_path = format!("Software\\{app_name}");
    if hkcu.open_subkey(&direct_path).is_ok() {
        if !candidates.iter().any(|c| c.path == format!("HKCU\\{direct_path}")) {
            candidates.push(LeftoverCandidate {
                id: format!("reg-hkcu-dir-{}", candidates.len()),
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

/// Cleans approved leftover candidates:
/// 1. Exports selected registry keys to a `.reg` backup file in the quarantine batch.
/// 2. Quarantines selected directories and files into `<quarantine_root>/<run_id>/`.
/// 3. Deletes approved registry keys only after the backup is confirmed.
pub fn clean_leftovers(
    app_id: &str,
    candidates: &[LeftoverCandidate],
    quarantine_root: &Path,
) -> Result<LeftoverCleanReport, String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let run_id = format!("uninstall-{now}-{}", std::process::id());
    let batch = quarantine_root.join(&run_id);
    fs::create_dir_all(&batch).map_err(|e| format!("Could not create quarantine folder: {e}"))?;

    let mut errors = Vec::new();
    let mut quarantined_files = 0u64;
    let mut quarantined_bytes = 0u64;
    let mut deleted_registry_keys = 0u64;

    // Filter selected candidates
    let selected_dirs: Vec<&LeftoverCandidate> = candidates
        .iter()
        .filter(|c| c.selected && c.kind == "directory")
        .collect();
    let selected_regs: Vec<&LeftoverCandidate> = candidates
        .iter()
        .filter(|c| c.selected && c.kind == "registry_key")
        .collect();

    // Step 1: Backup registry entries to .reg file
    let mut backup_path = None;
    if !selected_regs.is_empty() {
        let reg_backup_file = batch.join("registry_backup.reg");
        match export_registry_backup(&selected_regs, &reg_backup_file) {
            Ok(()) => {
                backup_path = Some(reg_backup_file.to_string_lossy().to_string());
            }
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

        // Collect all files in the directory
        let files = collect_files_recursive(&dir_path);
        let _canonical_root = fs::canonicalize(&dir_path).unwrap_or_else(|_| dir_path.clone());

        for file in files {
            index += 1;
            let meta = match fs::symlink_metadata(&file) {
                Ok(m) => m,
                Err(_) => continue,
            };
            if !meta.file_type().is_file() {
                continue;
            }
            let size = meta.len();
            let dest_name = format!("{index}-{}", file.file_name().unwrap_or_default().to_string_lossy());
            let dest = batch.join(dest_name);

            // Record in journal
            let item = serde_json::json!({
                "originalPath": file.to_string_lossy(),
                "storedPath": dest.to_string_lossy(),
                "size": size
            });
            let line = serde_json::to_string(&item).unwrap_or_default();
            use std::io::Write;
            if let Ok(mut jf) = fs::OpenOptions::new().create(true).append(true).open(&journal_path) {
                let _ = writeln!(jf, "{line}");
                let _ = jf.flush();
            }

            if fs::rename(&file, &dest).is_ok() {
                quarantined_files += 1;
                quarantined_bytes += size;
                manifest_items.push(item);
            }
        }

        // Remove empty residual directories
        remove_empty_dir_tree(&dir_path);
    }

    // Write quarantine manifest
    let manifest = serde_json::json!({
        "runId": run_id,
        "createdAt": now,
        "expiresAt": now + 7 * 86_400,
        "targetIds": [app_id],
        "items": manifest_items
    });
    let _ = fs::write(batch.join("manifest.json"), serde_json::to_string_pretty(&manifest).unwrap_or_default());

    // Step 3: Delete backed-up registry keys
    if backup_path.is_some() {
        for reg_cand in selected_regs {
            if delete_registry_key(&reg_cand.path) {
                deleted_registry_keys += 1;
            } else {
                errors.push(format!("Could not remove registry key: {}", reg_cand.path));
            }
        }
    }

    Ok(LeftoverCleanReport {
        run_id,
        app_id: app_id.to_string(),
        quarantined_files,
        quarantined_bytes,
        deleted_registry_keys,
        registry_backup_path: backup_path,
        quarantine_dir: Some(batch.to_string_lossy().to_string()),
        errors,
    })
}

fn export_registry_backup(regs: &[&LeftoverCandidate], output: &Path) -> Result<(), String> {
    use std::io::Write;
    let mut file = fs::File::create(output).map_err(|e| e.to_string())?;
    writeln!(file, "Windows Registry Editor Version 5.00\r\n").map_err(|e| e.to_string())?;

    for reg in regs {
        let full = &reg.path;
        let (root_hive, subkey) = parse_reg_path(full)?;
        let key = root_hive.open_subkey(&subkey).map_err(|e| e.to_string())?;

        writeln!(file, "[{full}]").map_err(|e| e.to_string())?;
        for (name, val) in key.enum_values().flatten() {
            let escaped_name = if name.is_empty() { "@".into() } else { format!("\"{name}\"") };
            writeln!(file, "{escaped_name}=\"{}\"", val.to_string().replace('\\', "\\\\")).map_err(|e| e.to_string())?;
        }
        writeln!(file).map_err(|e| e.to_string())?;
    }
    file.flush().map_err(|e| e.to_string())?;
    Ok(())
}

fn delete_registry_key(full_path: &str) -> bool {
    let Ok((root_hive, subkey)) = parse_reg_path(full_path) else {
        return false;
    };
    // Winreg delete_subkey_all removes the key and any subkeys
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

fn measure_directory_shallow(path: &Path) -> (u64, u64) {
    let mut size = 0u64;
    let mut count = 0u64;
    let mut stack = vec![path.to_path_buf()];

    while let Some(dir) = stack.pop() {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if meta.is_dir() {
                        stack.push(entry.path());
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

fn collect_files_recursive(path: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![path.to_path_buf()];

    while let Some(dir) = stack.pop() {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if let Ok(meta) = entry.metadata() {
                    if meta.is_dir() {
                        stack.push(p);
                    } else if meta.is_file() {
                        files.push(p);
                    }
                }
            }
        }
    }
    files
}

fn remove_empty_dir_tree(root: &Path) {
    let mut dirs = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        dirs.push(dir.clone());
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if entry.metadata().map(|m| m.is_dir()).unwrap_or(false) {
                    stack.push(entry.path());
                }
            }
        }
    }
    // Remove deepest first
    dirs.sort_by_key(|d| std::cmp::Reverse(d.as_os_str().len()));
    for dir in dirs {
        let _ = fs::remove_dir(dir);
    }
}

fn sanitize_for_path(val: &str) -> String {
    val.trim()
        .replace(['\\', '/', ':', '*', '?', '"', '<', '>', '|'], "")
        .trim()
        .to_string()
}

fn normalize_key(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let stripped = raw
        .strip_prefix("\\\\?\\")
        .or_else(|| raw.strip_prefix("\\??\\"))
        .unwrap_or(&raw);
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
    parts.join("\\")
}

fn is_within_key(child: &str, parent: &str) -> bool {
    child == parent || child.starts_with(&format!("{parent}\\"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_refuses_roots_and_profile_folders() {
        assert!(guard_leftover_path(Path::new("C:\\")).is_err());
        assert!(guard_leftover_path(Path::new("C:\\Windows")).is_err());
        assert!(guard_leftover_path(Path::new("C:\\Program Files")).is_err());

        if let Ok(profile) = std::env::var("USERPROFILE") {
            assert!(guard_leftover_path(Path::new(&profile)).is_err());
            assert!(guard_leftover_path(&Path::new(&profile).join("Documents")).is_err());
        }
    }

    #[test]
    fn guard_accepts_nested_app_directory() {
        if let Ok(appdata) = std::env::var("LOCALAPPDATA") {
            let app_folder = PathBuf::from(appdata).join("CyronTestApp");
            assert!(guard_leftover_path(&app_folder).is_ok());
        }
    }
}
