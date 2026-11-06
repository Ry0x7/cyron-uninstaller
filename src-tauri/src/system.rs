//! Device diagnostics and system hardware information queries.
//! Uses safe, asynchronous Win32 APIs and registry queries with zero telemetry.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_SET_VALUE, REG_BINARY};
use winreg::RegKey;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemSpecs {
    pub os_name: String,
    pub os_build: String,
    pub os_architecture: String,
    pub cpu_brand: String,
    pub cpu_logical_cores: u32,
    pub total_memory_bytes: u64,
    pub available_memory_bytes: u64,
    pub memory_in_use_percent: u32,
    pub gpu_brand: Option<String>,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupItem {
    pub id: String,
    pub name: String,
    pub command: String,
    pub location: String,
    pub enabled: bool,
    pub publisher: Option<String>,
    pub is_protected: bool,
    pub can_toggle: bool,
}

// 5-second in-memory cache for system specs to avoid rapid repeated WMI/Registry queries
static SPECS_CACHE: Mutex<Option<(Instant, SystemSpecs)>> = Mutex::new(None);

pub fn query_system_specs() -> SystemSpecs {
    if let Ok(guard) = SPECS_CACHE.lock() {
        if let Some((ts, ref specs)) = *guard {
            if ts.elapsed() < Duration::from_secs(5) {
                return specs.clone();
            }
        }
    }

    let specs = query_system_specs_live();
    if let Ok(mut guard) = SPECS_CACHE.lock() {
        *guard = Some((Instant::now(), specs.clone()));
    }
    specs
}

fn query_system_specs_live() -> SystemSpecs {
    use windows_sys::Win32::System::SystemInformation::{
        GetNativeSystemInfo, GetTickCount64, GlobalMemoryStatusEx, MEMORYSTATUSEX, SYSTEM_INFO,
    };

    // 1. Memory
    let mut total_mem = 0u64;
    let mut avail_mem = 0u64;
    let mut mem_pct = 0u32;
    unsafe {
        let mut mem_status = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            dwMemoryLoad: 0,
            ullTotalPhys: 0,
            ullAvailPhys: 0,
            ullTotalPageFile: 0,
            ullAvailPageFile: 0,
            ullTotalVirtual: 0,
            ullAvailVirtual: 0,
            ullAvailExtendedVirtual: 0,
        };
        if GlobalMemoryStatusEx(&mut mem_status) != 0 {
            total_mem = mem_status.ullTotalPhys;
            avail_mem = mem_status.ullAvailPhys;
            mem_pct = mem_status.dwMemoryLoad;
        }
    }

    // 2. CPU info & Native OS Architecture
    let mut sys_info: SYSTEM_INFO = unsafe { std::mem::zeroed() };
    unsafe {
        GetNativeSystemInfo(&mut sys_info);
    }
    let logical_cores = sys_info.dwNumberOfProcessors.max(1);
    let arch = match unsafe { sys_info.Anonymous.Anonymous.wProcessorArchitecture } {
        9 => "x64".to_string(),
        12 => "ARM64".to_string(),
        0 => "x86".to_string(),
        5 => "ARM".to_string(),
        _ => if cfg!(target_arch = "x86_64") { "x64".to_string() } else { "x86".to_string() },
    };

    // 3. Uptime
    let uptime_seconds = unsafe { GetTickCount64() / 1000 };

    // 4. Registry queries for OS & CPU brand & GPU
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let mut os_name = "Windows".to_string();
    let mut os_build = String::new();
    let mut cpu_brand = "Unknown Processor".to_string();
    let gpu_brand;

    if let Ok(cv) = hklm.open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion") {
        let prod: Result<String, _> = cv.get_value("ProductName");
        let display_ver: Result<String, _> = cv.get_value("DisplayVersion");
        let build: Result<String, _> = cv.get_value("CurrentBuild");

        if let Ok(mut p) = prod {
            let build_num: u32 = build.as_ref().ok().and_then(|b| b.parse().ok()).unwrap_or(0);
            if build_num >= 22000 && p.contains("Windows 10") {
                p = p.replacen("Windows 10", "Windows 11", 1);
            }
            os_name = p;
            if let Ok(v) = display_ver {
                os_name = format!("{os_name} {v}");
            }
        }
        if let Ok(b) = build {
            os_build = format!("Build {b}");
            if let Ok(ubr) = cv.get_value::<u32, _>("UBR") {
                os_build = format!("{os_build}.{ubr}");
            }
        }
    }

    if let Ok(cpu_key) = hklm.open_subkey(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0") {
        if let Ok(name) = cpu_key.get_value::<String, _>("ProcessorNameString") {
            cpu_brand = name.trim().to_string();
        }
    }

    // Enumerate display adapters from registry class
    let mut gpus = Vec::new();
    let video_base = r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";
    if let Ok(vkey) = hklm.open_subkey(video_base) {
        for sub in vkey.enum_keys().flatten() {
            if sub.chars().all(|c| c.is_ascii_digit()) {
                if let Ok(adapter_key) = vkey.open_subkey(&sub) {
                    if let Ok(desc) = adapter_key.get_value::<String, _>("DriverDesc") {
                        let trimmed = desc.trim();
                        if !trimmed.is_empty() {
                            let lower = trimmed.to_ascii_lowercase();
                            let is_virtual = lower.contains("remote")
                                || lower.contains("miracast")
                                || lower.contains("virtualbox")
                                || lower.contains("vmware")
                                || lower.contains("rdpdd")
                                || lower.contains("basic display");
                            if !is_virtual && !gpus.contains(&trimmed.to_string()) {
                                gpus.push(trimmed.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    // Prefer dedicated/discrete GPU adapter over integrated
    if let Some(discrete) = gpus.iter().find(|g| {
        let l = g.to_ascii_lowercase();
        l.contains("nvidia")
            || l.contains("geforce")
            || l.contains("radeon rx")
            || l.contains("rtx")
            || l.contains("arc")
    }) {
        gpu_brand = Some(discrete.clone());
    } else {
        gpu_brand = gpus.into_iter().next();
    }

    SystemSpecs {
        os_name,
        os_build,
        os_architecture: arch,
        cpu_brand,
        cpu_logical_cores: logical_cores,
        total_memory_bytes: total_mem,
        available_memory_bytes: avail_mem,
        memory_in_use_percent: mem_pct,
        gpu_brand,
        uptime_seconds,
    }
}

pub fn is_critical_startup_entry(name: &str, cmd: &str) -> bool {
    let lower_name = name.to_ascii_lowercase();
    let lower_cmd = cmd.to_ascii_lowercase();
    lower_name.contains("securityhealth")
        || lower_name.contains("windows defender")
        || lower_name.contains("smartscreen")
        || lower_cmd.contains("securityhealthsystray")
        || lower_cmd.contains("smartscreen.exe")
        || lower_cmd.contains("msseces.exe")
}

fn get_windows_filetime_now() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    // 100-nanosecond intervals since January 1, 1601 (UTC)
    (nanos / 100) as u64 + 116_444_736_000_000_000u64
}

fn is_hklm_startup_writable() -> bool {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    hklm.open_subkey_with_flags(
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run",
        KEY_SET_VALUE,
    )
    .is_ok()
}

fn get_startup_approved_state(entry_name: &str, is_hklm: bool, location_type: &str) -> (bool, Option<Vec<u8>>) {
    let root = if is_hklm {
        RegKey::predef(HKEY_LOCAL_MACHINE)
    } else {
        RegKey::predef(HKEY_CURRENT_USER)
    };

    let subkey_name = match location_type {
        "folder" => r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\StartupFolder",
        "run32" => r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run32",
        _ => r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run",
    };

    if let Ok(key) = root.open_subkey(subkey_name) {
        if let Ok(raw) = key.get_raw_value(entry_name) {
            if !raw.bytes.is_empty() {
                // In Windows StartupApproved:
                // First byte: 0x02 = Enabled, 0x03 = Disabled by user (any odd byte is disabled)
                let enabled = raw.bytes[0] % 2 == 0;
                return (enabled, Some(raw.bytes));
            }
        }
    }
    (true, None) // Enabled by default if no approval entry exists
}

pub fn list_startup_items() -> Vec<StartupItem> {
    let mut items = Vec::new();
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let hklm_writable = is_hklm_startup_writable();

    // 1. HKCU Run
    if let Ok(run_key) = hkcu.open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run") {
        for (name, v) in run_key.enum_values().flatten() {
            let cmd = v.to_string();
            let is_protected = is_critical_startup_entry(&name, &cmd);
            let (enabled, _) = get_startup_approved_state(&name, false, "run");
            items.push(StartupItem {
                id: format!("hkcu-run-{}", name),
                name,
                command: cmd,
                location: "HKCU\\Run".into(),
                enabled,
                publisher: None,
                is_protected,
                can_toggle: !is_protected,
            });
        }
    }

    // 2. HKCU RunOnce
    if let Ok(run_key) = hkcu.open_subkey(r"Software\Microsoft\Windows\CurrentVersion\RunOnce") {
        for (name, v) in run_key.enum_values().flatten() {
            let cmd = v.to_string();
            let is_protected = is_critical_startup_entry(&name, &cmd);
            items.push(StartupItem {
                id: format!("hkcu-runonce-{}", name),
                name,
                command: cmd,
                location: "HKCU\\RunOnce".into(),
                enabled: true,
                publisher: None,
                is_protected,
                can_toggle: false,
            });
        }
    }

    // 3. HKLM Run
    if let Ok(run_key) = hklm.open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run") {
        for (name, v) in run_key.enum_values().flatten() {
            let cmd = v.to_string();
            let is_protected = is_critical_startup_entry(&name, &cmd);
            let (enabled, _) = get_startup_approved_state(&name, true, "run");
            items.push(StartupItem {
                id: format!("hklm-run-{}", name),
                name,
                command: cmd,
                location: "HKLM\\Run".into(),
                enabled,
                publisher: None,
                is_protected,
                can_toggle: !is_protected && hklm_writable,
            });
        }
    }

    // 4. HKLM RunOnce
    if let Ok(run_key) = hklm.open_subkey(r"Software\Microsoft\Windows\CurrentVersion\RunOnce") {
        for (name, v) in run_key.enum_values().flatten() {
            let cmd = v.to_string();
            let is_protected = is_critical_startup_entry(&name, &cmd);
            items.push(StartupItem {
                id: format!("hklm-runonce-{}", name),
                name,
                command: cmd,
                location: "HKLM\\RunOnce".into(),
                enabled: true,
                publisher: None,
                is_protected,
                can_toggle: false,
            });
        }
    }

    // 5. WOW6432Node Run
    if let Ok(run_key) = hklm.open_subkey(r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Run") {
        for (name, v) in run_key.enum_values().flatten() {
            let cmd = v.to_string();
            let is_protected = is_critical_startup_entry(&name, &cmd);
            let (enabled, _) = get_startup_approved_state(&name, true, "run32");
            items.push(StartupItem {
                id: format!("hklm-wow-run-{}", name),
                name,
                command: cmd,
                location: "HKLM\\WOW6432Node\\Run".into(),
                enabled,
                publisher: None,
                is_protected,
                can_toggle: !is_protected && hklm_writable,
            });
        }
    }

    // 6. User Startup Folder
    if let Ok(appdata) = std::env::var("APPDATA") {
        let startup_dir = std::path::PathBuf::from(appdata).join(r"Microsoft\Windows\Start Menu\Programs\Startup");
        if let Ok(entries) = std::fs::read_dir(startup_dir) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if meta.is_file() {
                        let path = entry.path();
                        let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                        let stem = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                        let is_protected = is_critical_startup_entry(&stem, &path.to_string_lossy());
                        let (enabled, _) = get_startup_approved_state(&file_name, false, "folder");
                        items.push(StartupItem {
                            id: format!("startup-user-{}", stem),
                            name: stem,
                            command: path.to_string_lossy().to_string(),
                            location: "User Startup Folder".into(),
                            enabled,
                            publisher: None,
                            is_protected,
                            can_toggle: !is_protected,
                        });
                    }
                }
            }
        }
    }

    // 7. All-Users Startup Folder
    let common_appdata = std::env::var("ProgramData")
        .or_else(|_| std::env::var("ALLUSERSPROFILE"))
        .ok();
    if let Some(cd) = common_appdata {
        let common_startup = std::path::PathBuf::from(cd).join(r"Microsoft\Windows\Start Menu\Programs\Startup");
        if let Ok(entries) = std::fs::read_dir(common_startup) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if meta.is_file() {
                        let path = entry.path();
                        let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                        let stem = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                        let is_protected = is_critical_startup_entry(&stem, &path.to_string_lossy());
                        let (enabled, _) = get_startup_approved_state(&file_name, true, "folder");
                        items.push(StartupItem {
                            id: format!("startup-common-{}", stem),
                            name: stem,
                            command: path.to_string_lossy().to_string(),
                            location: "All Users Startup Folder".into(),
                            enabled,
                            publisher: None,
                            is_protected,
                            can_toggle: !is_protected && hklm_writable,
                        });
                    }
                }
            }
        }
    }

    items
}

pub fn set_startup_item_enabled(id: &str, enabled: bool) -> Result<StartupItem, String> {
    let items = list_startup_items();
    let item = items.into_iter().find(|i| i.id == id)
        .ok_or_else(|| format!("Startup registration not found: {id}"))?;

    if item.is_protected {
        return Err("Cannot modify protected Windows security startup item.".into());
    }

    if !item.can_toggle {
        if item.location.contains("HKLM") || item.location.contains("All Users") {
            return Err("Modifying machine-wide startup items requires administrator privileges.".into());
        } else if item.location.contains("RunOnce") {
            return Err("RunOnce entries are temporary one-time startup tasks and cannot be toggled.".into());
        } else {
            return Err("This startup item cannot be modified.".into());
        }
    }

    let is_hklm = item.location.contains("HKLM") || item.location.contains("All Users");
    let is_folder = item.location.contains("Startup");
    let is_wow32 = item.location.contains("WOW6432Node");

    let subkey_name = if is_folder {
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\StartupFolder"
    } else if is_wow32 {
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run32"
    } else {
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run"
    };

    let root = if is_hklm {
        RegKey::predef(HKEY_LOCAL_MACHINE)
    } else {
        RegKey::predef(HKEY_CURRENT_USER)
    };

    let (approval_key, _) = root
        .create_subkey(subkey_name)
        .map_err(|e| format!("Could not open or create StartupApproved registry key: {e}"))?;

    let target_name = if is_folder {
        let p = std::path::Path::new(&item.command);
        p.file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| item.name.clone())
    } else {
        item.name.clone()
    };

    // Query existing raw value to preserve exact original bytes and structure
    let existing_raw = approval_key.get_raw_value(&target_name).ok();

    if let Some(mut raw) = existing_raw {
        // Preserve all existing binary bytes, only toggle byte 0 and update timestamp in bytes 4..12
        if raw.bytes.is_empty() {
            raw.bytes = vec![0u8; 12];
        } else if raw.bytes.len() < 12 {
            raw.bytes.resize(12, 0);
        }
        raw.bytes[0] = if enabled { 0x02 } else { 0x03 };
        let ft = get_windows_filetime_now();
        raw.bytes[4..12].copy_from_slice(&ft.to_le_bytes());

        let reg_val = winreg::RegValue {
            vtype: REG_BINARY,
            bytes: raw.bytes,
        };
        approval_key
            .set_raw_value(&target_name, &reg_val)
            .map_err(|e| format!("Could not write startup approval state: {e}"))?;
    } else if !enabled {
        // Create standard 12-byte payload with current FILETIME
        let mut data = vec![0u8; 12];
        data[0] = 0x03; // Disabled
        let ft = get_windows_filetime_now();
        data[4..12].copy_from_slice(&ft.to_le_bytes());

        let reg_val = winreg::RegValue {
            vtype: REG_BINARY,
            bytes: data,
        };
        approval_key
            .set_raw_value(&target_name, &reg_val)
            .map_err(|e| format!("Could not write startup approval state: {e}"))?;
    } else {
        // Enabled and no prior approval entry: deleting the value restores the pristine unmanaged Windows default
        let _ = approval_key.delete_value(&target_name);
    }

    let mut updated = item;
    updated.enabled = enabled;
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries_valid_system_specs() {
        let specs = query_system_specs();
        assert!(!specs.os_name.is_empty());
        assert!(!specs.os_architecture.is_empty());
        assert!(specs.cpu_logical_cores >= 1);
        assert!(specs.total_memory_bytes > 0);
    }

    #[test]
    fn lists_startup_items_without_panic() {
        let items = list_startup_items();
        for item in items {
            assert!(!item.id.is_empty());
            assert!(!item.name.is_empty());
        }
    }

    #[test]
    fn protected_security_items_are_recognized() {
        assert!(is_critical_startup_entry("SecurityHealthSystray", "C:\\Windows\\System32\\SecurityHealthSystray.exe"));
        assert!(is_critical_startup_entry("Windows Defender", "smartscreen.exe"));
        assert!(!is_critical_startup_entry("Spotify", "C:\\Users\\dev\\AppData\\Roaming\\Spotify\\Spotify.exe"));
    }

    #[test]
    fn isolated_startup_toggle_enable_disable_restoration() {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let test_name = "CyronIsolatedStartupTestItem";
        let test_cmd = r#""C:\Program Files\Test\test.exe" /start"#;

        // Create test Run entry
        let (run_key, _) = hkcu.create_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run").expect("create test run key");
        run_key.set_value(test_name, &test_cmd).expect("write test run value");

        let items = list_startup_items();
        let test_item = items.into_iter().find(|i| i.name == test_name);
        assert!(test_item.is_some(), "test item must be discovered");
        let item = test_item.unwrap();
        assert!(item.can_toggle);
        assert!(!item.is_protected);

        // 1. Disable test item
        let disabled = set_startup_item_enabled(&item.id, false).expect("disable startup item");
        assert!(!disabled.enabled);

        // 2. Re-list and verify disabled state
        let updated_items = list_startup_items();
        let reloaded = updated_items.into_iter().find(|i| i.name == test_name).expect("find disabled item");
        assert!(!reloaded.enabled);

        // 3. Enable test item (restoration)
        let restored = set_startup_item_enabled(&item.id, true).expect("restore startup item");
        assert!(restored.enabled);

        // Clean up test registry entries
        let _ = run_key.delete_value(test_name);
        if let Ok(approval_key) = hkcu.open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run") {
            let _ = approval_key.delete_value(test_name);
        }
    }
}
