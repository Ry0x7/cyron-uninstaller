//! Process discovery and controlled termination for installed applications.
//!
//! Windows critical processes are protected by a strict deny list. Processes are
//! matched ONLY by executable path within the application's install location;
//! matching by executable name alone is strictly forbidden. Controlled termination
//! first attempts graceful window closing (`WM_CLOSE`), waiting up to 1.5 seconds,
//! before issuing `TerminateProcess`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppProcess {
    pub pid: u32,
    pub name: String,
    pub path: Option<String>,
    pub is_critical: bool,
    pub title: Option<String>,
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
        {
            return true;
        }
    }
    false
}

/// Enumerates running processes belonging to the given application install location.
/// Matching is strictly path-based; matching by name alone is refused.
pub fn discover_app_processes(install_location: Option<&str>) -> Vec<AppProcess> {
    let Some(location) = install_location else {
        return Vec::new();
    };
    let location_trimmed = location.trim();
    if location_trimmed.is_empty() {
        return Vec::new();
    }
    let target_root = match std::fs::canonicalize(location_trimmed) {
        Ok(canonical) => canonical,
        Err(_) => PathBuf::from(location_trimmed),
    };
    let target_root_key = normalize_key(&target_root);
    if target_root_key.is_empty() || target_root_key.split('\\').count() < 2 {
        return Vec::new();
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
            matching.push(AppProcess {
                pid,
                name: exe_name,
                path: exe_path,
                is_critical: critical,
                title: find_window_title(pid),
            });
        }
    }

    matching
}

/// Controlled termination for a list of process IDs.
/// Refuses to terminate any critical process. First attempts graceful window closing,
/// falling back to TerminateProcess only when forced.
pub fn terminate_processes(pids: &[u32], force: bool) -> Vec<ProcessTerminateResult> {
    let mut results = Vec::new();

    for &pid in pids {
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
            let mut title_buf = vec![0u16; 512];
            let len = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), title_buf.len() as i32);
            if len > 0 {
                title_buf.truncate(len as usize);
                let title = String::from_utf16_lossy(&title_buf).trim().to_string();
                if !title.is_empty() {
                    finder.title = Some(title);
                    return 0; // Stop enumeration
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
    fn critical_processes_are_recognized() {
        assert!(is_critical_process(0, "System Idle Process", None));
        assert!(is_critical_process(4, "System", None));
        assert!(is_critical_process(1234, "csrss.exe", None));
        assert!(is_critical_process(5678, "explorer.exe", None));
        assert!(is_critical_process(
            9999,
            "host.exe",
            Some("C:\\Windows\\System32\\host.exe")
        ));
        assert!(!is_critical_process(
            11111,
            "myapp.exe",
            Some("C:\\Program Files\\MyApp\\myapp.exe")
        ));
    }

    #[test]
    fn refuses_to_terminate_critical_processes() {
        let results = terminate_processes(&[0, 4], true);
        assert_eq!(results.len(), 2);
        assert!(!results[0].success);
        assert!(!results[1].success);
        assert!(results[0].error.as_ref().unwrap().contains("critical"));
    }
}
