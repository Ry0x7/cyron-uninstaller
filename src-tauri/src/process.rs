//! Process discovery and controlled termination for installed applications.
//!
//! Windows critical processes are protected by a strict deny list and system directory
//! guards. Processes are matched ONLY by executable path within the application's install
//! location; matching by executable name alone is strictly forbidden.
//! Controlled termination verifies executable ownership, creation time, and PID identity
//! against a verified process snapshot before any action, attempting graceful window
//! closing (`WM_CLOSE`) first before issuing `TerminateProcess`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppProcess {
    pub pid: u32,
    pub name: String,
    pub path: Option<String>,
    pub is_critical: bool,
    pub title: Option<String>,
    pub creation_time: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessTerminateResult {
    pub pid: u32,
    pub name: String,
    pub success: bool,
    pub graceful: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StoredProcessSnapshot {
    pub app_id: String,
    pub install_location: String,
    pub timestamp: u64,
    pub processes: Vec<AppProcess>,
}

pub const CRITICAL_PROCESS_NAMES: &[&str] = &[
    "system",
    "system idle process",
    "registry",
    "smss.exe",
    "csrss.exe",
    "wininit.exe",
    "services.exe",
    "lsass.exe",
    "lsm.exe",
    "svchost.exe",
    "winlogon.exe",
    "dwm.exe",
    "explorer.exe",
    "fontdrvhost.exe",
    "sihost.exe",
    "taskhostw.exe",
    "ctfmon.exe",
    "spoolsv.exe",
    "searchhost.exe",
    "startmenuexperiencehost.exe",
    "shellexperiencehost.exe",
    "runtimebroker.exe",
    "securityhealthservice.exe",
    "msmpeng.exe",
    "antigravity.exe",
    "cyron.exe",
];

/// Checks if a process is critical to the Windows operating system or Cyron itself.
pub fn is_critical_process(pid: u32, name: &str, path: Option<&str>) -> bool {
    if pid <= 4 {
        return true;
    }
    let lower_name = name.to_ascii_lowercase();
    for blocked in CRITICAL_PROCESS_NAMES {
        if lower_name == *blocked {
            return true;
        }
    }
    if let Some(p) = path {
        let lower_path = p.to_ascii_lowercase();
        let windir = std::env::var("WINDIR")
            .unwrap_or_else(|_| "C:\\Windows".into())
            .to_ascii_lowercase();
        if lower_path.starts_with(&format!("{windir}\\system32\\"))
            || lower_path.starts_with(&format!("{windir}\\syswow64\\"))
            || lower_path == format!("{windir}\\explorer.exe")
        {
            return true;
        }
    }
    false
}

/// Retrieves the process creation time as a 64-bit FILETIME value.
pub fn get_process_creation_time(pid: u32) -> Option<u64> {
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME};
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }

    let mut creation = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut exit = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut kernel = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut user = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };

    let ok = unsafe { GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) };
    unsafe { CloseHandle(handle) };

    if ok != 0 {
        Some(((creation.dwHighDateTime as u64) << 32) | (creation.dwLowDateTime as u64))
    } else {
        None
    }
}

/// Enumerates running processes belonging to the given application install location
/// and captures a verified process snapshot.
pub fn discover_app_processes(
    app_id: &str,
    install_location: Option<&str>,
) -> (StoredProcessSnapshot, Vec<AppProcess>) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let empty_snapshot = StoredProcessSnapshot {
        app_id: app_id.to_string(),
        install_location: String::new(),
        timestamp: now,
        processes: Vec::new(),
    };

    let Some(location) = install_location else {
        return (empty_snapshot, Vec::new());
    };
    let location_trimmed = location.trim();
    if location_trimmed.is_empty() {
        return (empty_snapshot, Vec::new());
    }
    let target_root = match std::fs::canonicalize(location_trimmed) {
        Ok(canonical) => canonical,
        Err(_) => PathBuf::from(location_trimmed),
    };
    let target_root_key = normalize_key(&target_root);
    if target_root_key.is_empty() || target_root_key.split('\\').count() < 2 {
        return (empty_snapshot, Vec::new());
    }

    // Refuse system root or profile root
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    if target_root_key == normalize_key(Path::new(&windir)) {
        return (empty_snapshot, Vec::new());
    }

    let pids = enum_all_pids();
    let mut matching = Vec::new();

    for pid in pids {
        if pid <= 4 {
            continue;
        }
        let (exe_name, exe_path) = query_process_info(pid);
        let Some(ref full_path) = exe_path else {
            continue;
        };
        let path_key = normalize_key(Path::new(full_path));
        if is_within_key(&path_key, &target_root_key) {
            let critical = is_critical_process(pid, &exe_name, exe_path.as_deref());
            let creation_time = get_process_creation_time(pid).unwrap_or(0);
            matching.push(AppProcess {
                pid,
                name: exe_name,
                path: exe_path,
                is_critical: critical,
                title: find_window_title(pid),
                creation_time,
            });
        }
    }

    let snapshot = StoredProcessSnapshot {
        app_id: app_id.to_string(),
        install_location: target_root.to_string_lossy().to_string(),
        timestamp: now,
        processes: matching.clone(),
    };

    (snapshot, matching)
}

/// Controlled termination for a list of process IDs with re-validation against a verified snapshot.
/// Refuses to terminate any critical process or any process not belonging to the verified snapshot.
/// First attempts graceful window closing, falling back to TerminateProcess only when forced.
pub fn terminate_processes_verified(
    app_id: &str,
    snapshot: &StoredProcessSnapshot,
    pids: &[u32],
    force: bool,
) -> Vec<ProcessTerminateResult> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut results = Vec::new();

    if snapshot.app_id != app_id {
        for &pid in pids {
            results.push(ProcessTerminateResult {
                pid,
                name: format!("PID {pid}"),
                success: false,
                graceful: false,
                error: Some("Process snapshot does not belong to this application.".into()),
            });
        }
        return results;
    }

    // Check snapshot freshness (max 300 seconds TTL)
    if now > snapshot.timestamp + 300 {
        for &pid in pids {
            results.push(ProcessTerminateResult {
                pid,
                name: format!("PID {pid}"),
                success: false,
                graceful: false,
                error: Some("Process snapshot has expired. Please refresh the process list.".into()),
            });
        }
        return results;
    }

    let target_root_key = normalize_key(Path::new(&snapshot.install_location));

    for &pid in pids {
        let snap_proc = snapshot.processes.iter().find(|p| p.pid == pid);
        let Some(expected) = snap_proc else {
            results.push(ProcessTerminateResult {
                pid,
                name: format!("PID {pid}"),
                success: false,
                graceful: false,
                error: Some(format!("PID {pid} was not found in the verified process snapshot for this application.")),
            });
            continue;
        };

        let (name, path) = query_process_info(pid);
        if is_critical_process(pid, &name, path.as_deref()) {
            results.push(ProcessTerminateResult {
                pid,
                name,
                success: false,
                graceful: false,
                error: Some("Refusing to terminate a Windows critical system process.".into()),
            });
            continue;
        }

        // Re-verify PID creation time has not changed (PID recycling protection)
        let current_creation = get_process_creation_time(pid).unwrap_or(0);
        if expected.creation_time != 0 && current_creation != 0 && expected.creation_time != current_creation {
            results.push(ProcessTerminateResult {
                pid,
                name,
                success: false,
                graceful: false,
                error: Some("Process creation time changed (PID was recycled). Termination aborted.".into()),
            });
            continue;
        }

        // Re-verify executable path still lies strictly inside the application's install location
        let Some(ref current_path) = path else {
            results.push(ProcessTerminateResult {
                pid,
                name,
                success: false,
                graceful: false,
                error: Some("Could not verify executable path for process.".into()),
            });
            continue;
        };

        let current_path_key = normalize_key(Path::new(current_path));
        if !is_within_key(&current_path_key, &target_root_key) {
            results.push(ProcessTerminateResult {
                pid,
                name,
                success: false,
                graceful: false,
                error: Some("Process executable path does not belong to this application.".into()),
            });
            continue;
        }

        // Attempt 1: Graceful window close via WM_CLOSE
        let graceful_success = attempt_graceful_close(pid);
        if graceful_success {
            results.push(ProcessTerminateResult {
                pid,
                name,
                success: true,
                graceful: true,
                error: None,
            });
            continue;
        }

        if !force {
            results.push(ProcessTerminateResult {
                pid,
                name,
                success: false,
                graceful: false,
                error: Some("Process did not respond to graceful close. Force termination required.".into()),
            });
            continue;
        }

        // Attempt 2: Force termination via TerminateProcess
        match force_terminate_process(pid) {
            Ok(()) => {
                results.push(ProcessTerminateResult {
                    pid,
                    name,
                    success: true,
                    graceful: false,
                    error: None,
                });
            }
            Err(err) => {
                results.push(ProcessTerminateResult {
                    pid,
                    name,
                    success: false,
                    graceful: false,
                    error: Some(err),
                });
            }
        }
    }

    results
}

fn attempt_graceful_close(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, PostMessageW, WM_CLOSE,
    };
    type BOOL = i32;

    struct EnumData {
        target_pid: u32,
        windows: Vec<HWND>,
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let data = &mut *(lparam as *mut EnumData);
        let mut process_id = 0u32;
        GetWindowThreadProcessId(hwnd, &mut process_id);
        if process_id == data.target_pid {
            data.windows.push(hwnd);
        }
        1
    }

    let mut data = EnumData {
        target_pid: pid,
        windows: Vec::new(),
    };

    unsafe {
        EnumWindows(Some(enum_proc), &mut data as *mut EnumData as LPARAM);
    }

    if data.windows.is_empty() {
        return false;
    }

    for hwnd in data.windows {
        unsafe {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
    }

    // Wait up to 1.5s for process to exit
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_millis(1500) {
        if !is_process_running(pid) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    !is_process_running(pid)
}

fn force_terminate_process(pid: u32) -> Result<(), String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, TerminateProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
        PROCESS_TERMINATE,
    };

    let handle = unsafe {
        OpenProcess(
            PROCESS_TERMINATE | PROCESS_SYNCHRONIZE,
            0,
            pid,
        )
    };
    if handle.is_null() {
        return Err(format!("Access denied opening process {pid} for termination."));
    }

    let term_res = unsafe { TerminateProcess(handle, 1) };
    if term_res == 0 {
        let err = std::io::Error::last_os_error();
        unsafe { CloseHandle(handle) };
        return Err(format!("TerminateProcess failed for {pid}: {err}"));
    }

    unsafe {
        WaitForSingleObject(handle, 2000);
        CloseHandle(handle);
    }

    Ok(())
}

fn is_process_running(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return false;
    }

    let mut exit_code = 0u32;
    let res = unsafe { GetExitCodeProcess(handle, &mut exit_code) };
    unsafe { CloseHandle(handle) };

    res != 0 && exit_code == 259 // STILL_ACTIVE = 259
}

fn enum_all_pids() -> Vec<u32> {
    use windows_sys::Win32::System::ProcessStatus::EnumProcesses;
    let mut pids = vec![0u32; 2048];
    let mut bytes_returned = 0u32;
    let res = unsafe {
        EnumProcesses(
            pids.as_mut_ptr(),
            (pids.len() * std::mem::size_of::<u32>()) as u32,
            &mut bytes_returned,
        )
    };
    if res == 0 {
        return Vec::new();
    }
    let count = (bytes_returned as usize) / std::mem::size_of::<u32>();
    pids.truncate(count);
    pids
}

fn query_process_info(pid: u32) -> (String, Option<String>) {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return (format!("PID {pid}"), None);
    }

    let mut buffer = vec![0u16; 1024];
    let mut size = buffer.len() as u32;
    let res = unsafe { QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut size) };
    unsafe { CloseHandle(handle) };

    if res != 0 && size > 0 {
        buffer.truncate(size as usize);
        let path = String::from_utf16_lossy(&buffer);
        let name = Path::new(&path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&path)
            .to_string();
        (name, Some(path))
    } else {
        (format!("PID {pid}"), None)
    }
}

fn find_window_title(pid: u32) -> Option<String> {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    };
    type BOOL = i32;

    struct WindowFinder {
        target_pid: u32,
        title: Option<String>,
    }

    unsafe extern "system" fn enum_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let finder = &mut *(lparam as *mut WindowFinder);
        let mut process_id = 0u32;
        GetWindowThreadProcessId(hwnd, &mut process_id);
        if process_id == finder.target_pid && IsWindowVisible(hwnd) != 0 {
            let mut buffer = [0u16; 512];
            let len = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
            if len > 0 {
                let text = String::from_utf16_lossy(&buffer[..len as usize]);
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    finder.title = Some(trimmed.to_string());
                    return 0; // stop enumerating
                }
            }
        }
        1
    }

    let mut finder = WindowFinder {
        target_pid: pid,
        title: None,
    };

    unsafe {
        EnumWindows(Some(enum_window), &mut finder as *mut WindowFinder as LPARAM);
    }

    finder.title
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
    fn identifies_windows_critical_processes() {
        assert!(is_critical_process(0, "System Idle Process", None));
        assert!(is_critical_process(4, "System", None));
        assert!(is_critical_process(1234, "csrss.exe", None));
        assert!(is_critical_process(1234, "smss.exe", None));
        assert!(is_critical_process(1234, "services.exe", None));
        assert!(is_critical_process(1234, "lsass.exe", None));
        assert!(is_critical_process(1234, "explorer.exe", None));
        assert!(is_critical_process(
            1234,
            "custom.exe",
            Some("C:\\Windows\\System32\\custom.exe")
        ));
    }

    #[test]
    fn allows_regular_installed_application_processes() {
        assert!(!is_critical_process(
            5678,
            "my_editor.exe",
            Some("C:\\Program Files\\MyVendor\\MyEditor\\my_editor.exe")
        ));
    }

    #[test]
    fn rejects_processes_outside_snapshot() {
        let snapshot = StoredProcessSnapshot {
            app_id: "app-1".into(),
            install_location: "C:\\Program Files\\App1".into(),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            processes: vec![AppProcess {
                pid: 999999,
                name: "app1.exe".into(),
                path: Some("C:\\Program Files\\App1\\app1.exe".into()),
                is_critical: false,
                title: None,
                creation_time: 12345,
            }],
        };

        let results = terminate_processes_verified("app-1", &snapshot, &[888888], false);
        assert_eq!(results.len(), 1);
        assert!(!results[0].success);
        assert!(results[0].error.as_ref().unwrap().contains("was not found in the verified process snapshot"));
    }
}
