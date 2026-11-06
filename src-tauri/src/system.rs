//! Device diagnostics and system hardware information queries.
//! Uses safe, asynchronous Win32 APIs and registry queries with zero telemetry.

use serde::{Deserialize, Serialize};
use winreg::enums::HKEY_LOCAL_MACHINE;
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
}

pub fn query_system_specs() -> SystemSpecs {
    use windows_sys::Win32::System::SystemInformation::{
        GetSystemInfo, GetTickCount64, GlobalMemoryStatusEx, MEMORYSTATUSEX, SYSTEM_INFO,
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

    // 2. CPU info
    let logical_cores: u32;
    let arch = "x64".to_string();
    unsafe {
        let mut sys_info = SYSTEM_INFO {
            Anonymous: std::mem::zeroed(),
            dwPageSize: 0,
            lpMinimumApplicationAddress: std::ptr::null_mut(),
            lpMaximumApplicationAddress: std::ptr::null_mut(),
            dwActiveProcessorMask: 0,
            dwNumberOfProcessors: 0,
            dwProcessorType: 0,
            dwAllocationGranularity: 0,
            wProcessorLevel: 0,
            wProcessorRevision: 0,
        };
        GetSystemInfo(&mut sys_info);
        logical_cores = sys_info.dwNumberOfProcessors.max(1);
    }

    // 3. Uptime
    let uptime_seconds = unsafe { GetTickCount64() / 1000 };

    // 4. Registry queries for OS & CPU brand & GPU
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let mut os_name = "Windows".to_string();
    let mut os_build = String::new();
    let mut cpu_brand = "Unknown Processor".to_string();
    let mut gpu_brand = None;

    if let Ok(cv) = hklm.open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion") {
        let prod: Result<String, _> = cv.get_value("ProductName");
        let display_ver: Result<String, _> = cv.get_value("DisplayVersion");
        let build: Result<String, _> = cv.get_value("CurrentBuild");

        if let Ok(p) = prod {
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

    // GPU detection from registry
    let video_base = r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";
    if let Ok(vkey) = hklm.open_subkey(video_base) {
        for sub in vkey.enum_keys().flatten() {
            if sub.chars().all(|c| c.is_ascii_digit()) {
                if let Ok(adapter_key) = vkey.open_subkey(&sub) {
                    if let Ok(desc) = adapter_key.get_value::<String, _>("DriverDesc") {
                        if !desc.is_empty() && !desc.to_ascii_lowercase().contains("remote") {
                            gpu_brand = Some(desc);
                            break;
                        }
                    }
                }
            }
        }
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

pub fn list_startup_items() -> Vec<StartupItem> {
    let mut items = Vec::new();
    let hkcu = RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    // HKCU Run
    if let Ok(run_key) = hkcu.open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run") {
        for val in run_key.enum_values().flatten() {
            let (name, v) = val;
            let cmd = v.to_string();
            items.push(StartupItem {
                id: format!("hkcu-run-{}", name),
                name,
                command: cmd,
                location: "HKCU\\Run".into(),
                enabled: true,
                publisher: None,
            });
        }
    }

    // HKLM Run
    if let Ok(run_key) = hklm.open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run") {
        for val in run_key.enum_values().flatten() {
            let (name, v) = val;
            let cmd = v.to_string();
            items.push(StartupItem {
                id: format!("hklm-run-{}", name),
                name,
                command: cmd,
                location: "HKLM\\Run".into(),
                enabled: true,
                publisher: None,
            });
        }
    }

    // WOW6432Node Run
    if let Ok(run_key) = hklm.open_subkey(r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Run") {
        for val in run_key.enum_values().flatten() {
            let (name, v) = val;
            let cmd = v.to_string();
            items.push(StartupItem {
                id: format!("hklm-wow-run-{}", name),
                name,
                command: cmd,
                location: "HKLM\\WOW6432Node\\Run".into(),
                enabled: true,
                publisher: None,
            });
        }
    }

    // Startup folders
    if let Ok(appdata) = std::env::var("APPDATA") {
        let startup_dir = std::path::PathBuf::from(appdata).join(r"Microsoft\Windows\Start Menu\Programs\Startup");
        if let Ok(entries) = std::fs::read_dir(startup_dir) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if meta.is_file() {
                        let path = entry.path();
                        let name = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                        items.push(StartupItem {
                            id: format!("startup-user-{}", name),
                            name,
                            command: path.to_string_lossy().to_string(),
                            location: "User Startup Folder".into(),
                            enabled: true,
                            publisher: None,
                        });
                    }
                }
            }
        }
    }

    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries_valid_system_specs() {
        let specs = query_system_specs();
        assert!(!specs.os_name.is_empty());
        assert!(specs.cpu_logical_cores >= 1);
        assert!(specs.total_memory_bytes > 0);
    }

    #[test]
    fn lists_startup_items_without_panic() {
        let items = list_startup_items();
        // Just verify execution succeeds and returns valid entries
        for item in items {
            assert!(!item.id.is_empty());
            assert!(!item.name.is_empty());
        }
    }
}
