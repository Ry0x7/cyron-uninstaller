//! Dead-entry scanning for startup registrations and uninstall registrations.
//!
//! Scanning is read-only. Removal is review-first: a server-held, single-use,
//! five-minute plan is bound to a SHA-256 fingerprint of the exact startup
//! value, and execution re-reads that value immediately before deletion.
//! A single-value `.reg` backup, a durable recovery journal and a quarantine
//! manifest are written and verified before the value is removed. Uninstall
//! registrations reuse the existing broken-entry plan and its recursive backup.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_SET_VALUE};
use winreg::RegKey;

const PLAN_TTL_SECS: u64 = 300;
const DELETE_ACCESS: u32 = 0x0001_0000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadStartupEntry {
    pub id: String,
    pub name: String,
    pub command: String,
    pub location: String,
    pub registry_key_path: String,
    pub value_name: String,
    pub reason: String,
    pub can_remove: bool,
    pub requires_elevation: bool,
    pub is_protected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadRegistryEntry {
    pub app_id: String,
    pub hive: String,
    pub subkey_name: String,
    pub registry_key_path: String,
    pub display_name: String,
    pub publisher: Option<String>,
    pub display_version: Option<String>,
    pub install_location: Option<String>,
    pub reason: String,
    pub can_remove: bool,
    pub requires_elevation: bool,
    pub is_protected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadEntryScanReport {
    pub startup: Vec<DeadStartupEntry>,
    pub registry: Vec<DeadRegistryEntry>,
    pub warnings: Vec<String>,
    pub scanned_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadStartupPlan {
    pub plan_id: String,
    pub startup_id: String,
    pub name: String,
    pub command: String,
    pub location: String,
    pub registry_key_path: String,
    pub value_name: String,
    pub fingerprint: String,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadStartupRemovalReport {
    pub run_id: String,
    pub name: String,
    pub location: String,
    pub value_name: String,
    pub registry_backup_path: String,
    pub quarantine_dir: String,
}

/// Expands `%VAR%` references so Run values such as `%ProgramFiles%\App\app.exe`
/// are checked against the real install path.
pub(crate) fn expand_env_vars(input: &str) -> String {
    let mut output = String::new();
    let mut rest = input;
    while let Some(start) = rest.find('%') {
        output.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        if let Some(end) = after.find('%') {
            let name = &after[..end];
            if !name.is_empty() {
                if let Ok(value) = std::env::var(name) {
                    output.push_str(&value);
                    rest = &after[end + 1..];
                    continue;
                }
            }
        }
        output.push('%');
        rest = &rest[start + 1..];
    }
    output.push_str(rest);
    output
}

/// Resolves the executable a startup command launches even when that file is
/// gone. Bare names are unknown (PATH is not guessed); only absolute paths are
/// classified. Uninstall parsing cannot be reused here because it rejects a
/// missing executable by design.
pub(crate) fn startup_target_path(command: &str) -> Option<String> {
    let expanded = expand_env_vars(command.trim());
    if expanded.is_empty() {
        return None;
    }
    let candidate = if let Some(rest) = expanded.strip_prefix('"') {
        let end = rest.find('"')?;
        rest[..end].trim().to_string()
    } else if let Some(index) = expanded.to_ascii_lowercase().find(".exe") {
        expanded[..index + 4].trim().to_string()
    } else {
        expanded.split_whitespace().next()?.to_string()
    };
    if candidate.is_empty() || !Path::new(&candidate).is_absolute() {
        return None;
    }
    Some(candidate)
}

/// Maps a startup item to the exact registry value that owns it. Startup-folder
/// shortcuts are intentionally excluded: resolving their shell-link target is a
/// different operation, so they are never reported as dead by this scanner.
fn startup_registry_target(item: &crate::system::StartupItem) -> Option<(String, String)> {
    let key = match item.location.as_str() {
        "HKCU\\Run" => r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
        "HKCU\\RunOnce" => r"HKCU\Software\Microsoft\Windows\CurrentVersion\RunOnce",
        "HKLM\\Run" => r"HKLM\Software\Microsoft\Windows\CurrentVersion\Run",
        "HKLM\\RunOnce" => r"HKLM\Software\Microsoft\Windows\CurrentVersion\RunOnce",
        "HKLM\\WOW6432Node\\Run" => {
            r"HKLM\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Run"
        }
        _ => return None,
    };
    Some((key.to_string(), item.name.clone()))
}

fn split_hive(path: &str) -> Option<(RegKey, String)> {
    if let Some(rest) = path.strip_prefix("HKCU\\") {
        Some((RegKey::predef(HKEY_CURRENT_USER), rest.to_string()))
    } else if let Some(rest) = path.strip_prefix("HKLM\\") {
        Some((RegKey::predef(HKEY_LOCAL_MACHINE), rest.to_string()))
    } else {
        None
    }
}

fn can_write_key(path: &str) -> bool {
    split_hive(path)
        .and_then(|(root, subkey)| root.open_subkey_with_flags(&subkey, KEY_SET_VALUE).ok())
        .is_some()
}

fn can_delete_key(path: &str) -> bool {
    split_hive(path)
        .and_then(|(root, subkey)| {
            root.open_subkey_with_flags(&subkey, KEY_READ | DELETE_ACCESS)
                .ok()
        })
        .is_some()
}

fn is_machine_location(location: &str) -> bool {
    location.starts_with("HKLM")
}

fn fingerprint(item: &crate::system::StartupItem, key_path: &str, value_name: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for part in [
        item.id.as_str(),
        item.name.as_str(),
        item.command.as_str(),
        item.location.as_str(),
        key_path,
        value_name,
    ] {
        hasher.update(part.as_bytes());
        hasher.update(b"|");
    }
    crate::leftovers::hex_digest(&hasher.finalize())
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn list_dead_startup() -> Vec<DeadStartupEntry> {
    let mut dead = Vec::new();
    for item in crate::system::list_startup_items() {
        let Some((key_path, value_name)) = startup_registry_target(&item) else {
            continue;
        };
        let Some(target) = startup_target_path(&item.command) else {
            continue;
        };
        if Path::new(&target).exists() {
            continue;
        }
        let writable = can_write_key(&key_path);
        let requires_elevation = !writable && is_machine_location(&item.location);
        dead.push(DeadStartupEntry {
            id: item.id,
            name: item.name,
            command: item.command,
            location: item.location,
            registry_key_path: key_path,
            value_name,
            reason: format!("The startup program no longer exists: {target}"),
            can_remove: writable && !item.is_protected,
            requires_elevation,
            is_protected: item.is_protected,
        });
    }
    dead
}

fn uninstaller_still_exists(meta: &crate::leftovers::BrokenEntryMetadata) -> bool {
    let command = meta
        .uninstall_string
        .as_deref()
        .or(meta.quiet_uninstall_string.as_deref());
    let Some(command) = command else {
        return false;
    };
    let parsed = crate::uninstall::parse_uninstall_command(command.trim());
    if parsed.is_msi {
        return false;
    }
    let path = Path::new(&parsed.executable);
    path.is_absolute() && path.exists()
}

fn broken_reason(meta: &crate::leftovers::BrokenEntryMetadata) -> String {
    if uninstaller_still_exists(meta) {
        "The registered install folder no longer exists.".into()
    } else {
        "The registered uninstaller no longer exists.".into()
    }
}

pub fn list_dead_registry(warnings: &mut Vec<String>) -> Vec<DeadRegistryEntry> {
    let inventory = crate::apps::discover_apps();
    warnings.extend(inventory.warnings.iter().take(6).cloned());
    let mut seen: HashSet<String> = HashSet::new();
    let mut dead = Vec::new();

    for app in inventory.apps.iter().filter(|app| app.source == "win32") {
        if !matches!(
            app.hive.as_str(),
            "user-64" | "user-32" | "machine-64" | "machine-32"
        ) {
            continue;
        }
        let subkey = app
            .registry_key_name
            .clone()
            .or_else(|| app.id.split_once('|').map(|(_, sub)| sub.to_string()));
        let Some(subkey) = subkey else {
            continue;
        };
        let Ok(meta) = crate::leftovers::read_live_broken_entry_metadata(&app.hive, &subkey)
        else {
            continue;
        };
        if meta.is_protected || meta.system_component {
            continue;
        }
        if crate::leftovers::verify_live_broken_status(&meta).is_err() {
            continue;
        }
        if !seen.insert(meta.registry_key_path.to_ascii_lowercase()) {
            continue;
        }
        let can_remove = can_delete_key(&meta.registry_key_path);
        dead.push(DeadRegistryEntry {
            app_id: app.id.clone(),
            hive: meta.hive.clone(),
            subkey_name: meta.subkey_name.clone(),
            registry_key_path: meta.registry_key_path.clone(),
            display_name: meta.display_name.clone(),
            publisher: meta.publisher.clone(),
            display_version: meta.display_version.clone(),
            install_location: meta.install_location.clone(),
            reason: broken_reason(&meta),
            can_remove,
            requires_elevation: !can_remove && meta.hive.starts_with("machine"),
            is_protected: meta.is_protected,
        });
    }
    dead.sort_by(|a, b| {
        a.display_name
            .to_ascii_lowercase()
            .cmp(&b.display_name.to_ascii_lowercase())
    });
    dead
}

pub fn scan_dead_entries() -> DeadEntryScanReport {
    let mut warnings = Vec::new();
    let startup = list_dead_startup();
    let registry = list_dead_registry(&mut warnings);
    DeadEntryScanReport {
        startup,
        registry,
        warnings,
        scanned_at: now_secs(),
    }
}

/// Prepares the single-use plan for one dead startup registry value.
pub fn prepare_dead_startup_plan(startup_id: &str) -> Result<DeadStartupPlan, String> {
    let id = startup_id.trim();
    if id.is_empty() {
        return Err("Startup entry ID is required.".into());
    }
    let item = crate::system::list_startup_items()
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| "Startup entry was not found. Rescan before removing it.".to_string())?;

    let (registry_key_path, value_name) = startup_registry_target(&item).ok_or_else(|| {
        "Only registry Run/RunOnce entries can be removed. Startup-folder shortcuts are left in place."
            .to_string()
    })?;

    if item.is_protected || crate::system::is_critical_startup_entry(&item.name, &item.command) {
        return Err("Windows security startup entries cannot be removed.".into());
    }
    if !can_write_key(&registry_key_path) {
        return Err("This startup entry needs an elevated Cyron session to remove.".into());
    }

    let target = startup_target_path(&item.command)
        .ok_or_else(|| "The startup command target could not be resolved.".to_string())?;
    if Path::new(&target).exists() {
        return Err(
            "The startup program exists again. Rescan before removing this entry.".into(),
        );
    }

    let expires_at = now_secs() + PLAN_TTL_SECS;
    let entry_fingerprint = fingerprint(&item, &registry_key_path, &value_name);
    Ok(DeadStartupPlan {
        plan_id: crate::leftovers::generate_collision_resistant_plan_id("dead-startup"),
        startup_id: item.id.clone(),
        name: item.name.clone(),
        command: item.command.clone(),
        location: item.location.clone(),
        registry_key_path,
        value_name,
        fingerprint: entry_fingerprint,
        expires_at,
    })
}

fn open_registry_key(path: &str, access: u32) -> Result<(RegKey, String), String> {
    let (root, subkey) = split_hive(path)
        .ok_or_else(|| format!("Unsupported registry path: {path}"))?;
    let key = root
        .open_subkey_with_flags(&subkey, access)
        .map_err(|error| format!("Could not open {path}: {error}"))?;
    Ok((key, subkey))
}

/// Removes one dead startup value after writing and verifying a single-value
/// `.reg` backup, a durable write-ahead journal and a quarantine manifest.
pub fn execute_dead_startup_removal(
    plan: &DeadStartupPlan,
    quarantine_root: &Path,
) -> Result<DeadStartupRemovalReport, String> {
    let now = now_secs();
    if now > plan.expires_at {
        return Err("The dead startup removal plan has expired. Please rescan.".into());
    }

    let item = crate::system::list_startup_items()
        .into_iter()
        .find(|item| item.id == plan.startup_id)
        .ok_or_else(|| "Startup entry no longer exists. Nothing was removed.".to_string())?;
    let (key_path, value_name) = startup_registry_target(&item)
        .ok_or_else(|| "Startup entry changed location. Nothing was removed.".to_string())?;
    if key_path != plan.registry_key_path || value_name != plan.value_name {
        return Err("Startup entry changed since the plan was prepared. Rescan before removing.".into());
    }
    if fingerprint(&item, &key_path, &value_name) != plan.fingerprint {
        return Err("Startup entry metadata changed since the plan was prepared. Rescan before removing.".into());
    }
    if item.is_protected || crate::system::is_critical_startup_entry(&item.name, &item.command) {
        return Err("Windows security startup entries cannot be removed.".into());
    }
    let target = startup_target_path(&item.command)
        .ok_or_else(|| "The startup command target could not be resolved.".to_string())?;
    if Path::new(&target).exists() {
        return Err("The startup program exists again. Nothing was removed.".into());
    }

    let (read_key, subkey_name) = open_registry_key(&key_path, KEY_READ)?;
    let value = read_key
        .get_raw_value(&value_name)
        .map_err(|_| "The startup value no longer exists. Nothing was removed.".to_string())?;

    let batch = quarantine_root.join(&plan.plan_id);
    fs::create_dir_all(&batch).map_err(|error| format!("Could not create quarantine folder: {error}"))?;
    let backup_path = batch.join("registry_backup.reg");

    {
        use std::io::Write;
        let mut file = fs::File::create(&backup_path)
            .map_err(|error| format!("Could not create registry backup: {error}"))?;
        writeln!(file, "Windows Registry Editor Version 5.00\r\n")
            .map_err(|error| format!("Could not write registry backup header: {error}"))?;
        let full_hive = if key_path.starts_with("HKCU\\") {
            key_path.replacen("HKCU\\", "HKEY_CURRENT_USER\\", 1)
        } else {
            key_path.replacen("HKLM\\", "HKEY_LOCAL_MACHINE\\", 1)
        };
        writeln!(file, "[{full_hive}]")
            .map_err(|error| format!("Could not write registry backup section: {error}"))?;
        crate::leftovers::write_registry_value_line(&mut file, &value_name, &value)?;
        writeln!(file).map_err(|error| error.to_string())?;
        file.flush().map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
    }

    crate::leftovers::verify_registry_backup(&backup_path)?;
    crate::leftovers::verify_registry_backup_scope(&backup_path, &key_path)?;
    let backup_sha256 = crate::leftovers::compute_file_sha256(&backup_path)?;
    let backup_size = fs::metadata(&backup_path).map(|meta| meta.len()).unwrap_or(0);
    if backup_size == 0 {
        return Err("Registry backup file is empty. Aborting removal.".into());
    }

    {
        use std::io::Write;
        let journal_path = batch.join(crate::cleanup::JOURNAL_FILE);
        let entry = serde_json::json!({
            "originalPath": format!("{key_path}\\{value_name}"),
            "storedPath": backup_path.to_string_lossy(),
            "size": backup_size,
            "sha256": backup_sha256,
            "action": "registry_backup"
        });
        let mut journal = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&journal_path)
            .map_err(|error| format!("Could not open recovery journal: {error}"))?;
        writeln!(journal, "{}", serde_json::to_string(&entry).unwrap_or_default())
            .map_err(|error| format!("Could not write recovery journal: {error}"))?;
        journal
            .flush()
            .map_err(|error| format!("Could not flush recovery journal: {error}"))?;
        journal
            .sync_all()
            .map_err(|error| format!("Could not sync recovery journal: {error}"))?;
    }

    let manifest = crate::cleanup::BatchManifest {
        run_id: plan.plan_id.clone(),
        created_at: now,
        expires_at: now + 7 * 86_400,
        target_ids: vec![plan.startup_id.clone()],
        items: Vec::new(),
        registry_backup: Some("registry_backup.reg".into()),
        registry_key_path: Some(key_path.clone()),
        hive: Some(if key_path.starts_with("HKCU\\") {
            "hkcu".into()
        } else {
            "hklm".into()
        }),
        subkey_name: Some(subkey_name.clone()),
        app_id: Some(plan.startup_id.clone()),
        backup_sha256: Some(backup_sha256.clone()),
        backup_size_bytes: Some(backup_size),
    };
    crate::cleanup::write_manifest(&batch, &manifest)?;

    let manifest_on_disk = crate::cleanup::read_manifest(&batch)
        .ok_or_else(|| "Manifest could not be read back. Aborting removal.".to_string())?;
    if manifest_on_disk.run_id != plan.plan_id
        || manifest_on_disk.backup_sha256.as_deref() != Some(backup_sha256.as_str())
        || manifest_on_disk.registry_key_path.as_deref() != Some(key_path.as_str())
    {
        return Err("Manifest integrity check failed. Aborting removal.".into());
    }

    // Re-read the exact value immediately before deletion to keep the race window small.
    let (fresh_key, _) = open_registry_key(&key_path, KEY_READ)?;
    if let Ok(fresh_value) = fresh_key.get_raw_value(&value_name) {
        if fresh_value.bytes != value.bytes || fresh_value.vtype != value.vtype {
            return Err("The startup value changed immediately before removal. Aborting.".into());
        }
    } else {
        return Err("The startup value no longer exists. Nothing was removed.".into());
    }
    let after = crate::system::list_startup_items()
        .into_iter()
        .find(|item| item.id == plan.startup_id);
    let Some(after) = after else {
        return Err("Startup entry disappeared immediately before removal. Aborting.".into());
    };
    if fingerprint(&after, &key_path, &value_name) != plan.fingerprint {
        return Err("Startup entry changed immediately before removal. Aborting.".into());
    }

    let (write_key, _) = open_registry_key(&key_path, KEY_SET_VALUE)?;
    write_key
        .delete_value(&value_name)
        .map_err(|error| format!("Could not remove the startup value: {error}"))?;

    let (check_key, _) = open_registry_key(&key_path, KEY_READ)?;
    if check_key.get_raw_value(&value_name).is_ok() {
        return Err("The startup value is still present after removal.".into());
    }

    Ok(DeadStartupRemovalReport {
        run_id: plan.plan_id.clone(),
        name: plan.name.clone(),
        location: plan.location.clone(),
        value_name: plan.value_name.clone(),
        registry_backup_path: backup_path.to_string_lossy().to_string(),
        quarantine_dir: batch.to_string_lossy().to_string(),
    })
}

/// Test-only helper: whether the value still exists.
#[cfg(test)]
pub(crate) fn startup_value_exists(key_path: &str, value_name: &str) -> bool {
    open_registry_key(key_path, KEY_READ)
        .and_then(|(key, _)| {
            key.get_raw_value(value_name)
                .map_err(|error| error.to_string())
        })
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_windows_environment_variables() {
        let windir = std::env::var("WINDIR").expect("WINDIR is set on Windows");
        let expanded = expand_env_vars("%WINDIR%\\System32\\notepad.exe");
        assert_eq!(expanded, format!("{windir}\\System32\\notepad.exe"));
        assert!(!expanded.contains('%'));
    }

    #[test]
    fn extracts_absolute_targets_and_ignores_bare_names() {
        let quoted = startup_target_path("\"C:\\Program Files\\App\\app.exe\" --tray");
        assert_eq!(quoted.as_deref(), Some("C:\\Program Files\\App\\app.exe"));
        let unquoted = startup_target_path("C:\\Windows\\System32\\notepad.exe -x");
        assert_eq!(unquoted.as_deref(), Some("C:\\Windows\\System32\\notepad.exe"));
        assert_eq!(startup_target_path("notepad.exe"), None);
        assert_eq!(startup_target_path(""), None);
    }

    #[test]
    fn scan_is_read_only_and_does_not_panic() {
        let report = scan_dead_entries();
        assert!(report.scanned_at > 0);
    }

    #[test]
    fn dead_startup_plan_removes_and_restores_one_value() {
        use winreg::RegValue;

        let key_path = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
        let value_name = format!("CyronDeadStartupFixture{}", std::process::id());
        let command = r"C:\definitely-missing-cyron-fixture\missing.exe --startup";
        let original = RegValue {
            vtype: winreg::enums::REG_SZ,
            bytes: command
                .encode_utf16()
                .chain(std::iter::once(0))
                .flat_map(|unit| unit.to_le_bytes())
                .collect(),
        };

        let (root, _) = split_hive(key_path).expect("HKCU path");
        let (key, _) = root
            .create_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run")
            .expect("create Run key");
        key.set_raw_value(&value_name, &original)
            .expect("write fixture value");

        let quarantine =
            std::env::temp_dir().join(format!("cyron-health-test-{}", std::process::id()));
        let batch_path = |report: &DeadStartupRemovalReport| std::path::PathBuf::from(&report.quarantine_dir);
        let _ = fs::remove_dir_all(&quarantine);

        let outcome: Result<(), String> = (|| {
            let entry = crate::system::list_startup_items()
                .into_iter()
                .find(|item| item.name == value_name)
                .ok_or_else(|| "fixture startup entry is listed".to_string())?;
            let plan = prepare_dead_startup_plan(&entry.id)?;
            let report = execute_dead_startup_removal(&plan, &quarantine)?;
            if startup_value_exists(key_path, &value_name) {
                return Err("value still exists after removal".into());
            }
            if !Path::new(&report.registry_backup_path).exists() {
                return Err("registry backup is missing".into());
            }

            let batch = batch_path(&report);
            let manifest = crate::cleanup::read_manifest(&batch)
                .ok_or_else(|| "manifest is missing".to_string())?;
            crate::leftovers::restore_registry_backup_with_policy(
                Path::new(&report.registry_backup_path),
                Some(&manifest),
                true,
            )?;
            if !startup_value_exists(key_path, &value_name) {
                return Err("value was not restored".into());
            }
            let restored = key
                .get_raw_value(&value_name)
                .map_err(|error| format!("restored value read failed: {error}"))?;
            if restored.bytes != original.bytes {
                return Err("restored value bytes differ".into());
            }
            Ok(())
        })();

        let _ = key.delete_value(&value_name);
        let _ = fs::remove_dir_all(&quarantine);
        assert!(outcome.is_ok(), "fixture flow failed: {outcome:?}");
    }
}
