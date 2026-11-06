//! Hunter Mode targeting engine.
//! Resolves desktop windows under cursor coordinates, retrieves process ownership,
//! matches with registered application inventory, and validates safety boundaries.

use serde::{Deserialize, Serialize};
use windows_sys::Win32::Foundation::POINT;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetAncestor, GetCursorPos, GetWindowTextW, GetWindowThreadProcessId, WindowFromPoint, GA_ROOT,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HunterTargetInfo {
    pub hwnd: usize,
    pub pid: u32,
    pub process_name: String,
    pub process_path: Option<String>,
    pub window_title: Option<String>,
    pub matched_app_id: Option<String>,
    pub matched_app_name: Option<String>,
    pub is_critical: bool,
}

pub fn resolve_cursor_target() -> Result<Option<HunterTargetInfo>, String> {
    unsafe {
        let mut pt = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut pt) == 0 {
            return Ok(None);
        }

        let hwnd_raw = WindowFromPoint(pt);
        if hwnd_raw.is_null() {
            return Ok(None);
        }

        // Get top-level ancestor window
        let root_hwnd = GetAncestor(hwnd_raw, GA_ROOT);
        let target_hwnd = if root_hwnd.is_null() { hwnd_raw } else { root_hwnd };

        let mut pid = 0u32;
        GetWindowThreadProcessId(target_hwnd, &mut pid);
        if pid <= 4 {
            return Ok(None);
        }

        // Query process info
        let (proc_name, proc_path) = crate::process::query_process_info(pid);
        let is_critical = crate::process::is_critical_process(pid, &proc_name, proc_path.as_deref());

        // Get window title
        let mut title_buf = [0u16; 512];
        let len = GetWindowTextW(target_hwnd, title_buf.as_mut_ptr(), title_buf.len() as i32);
        let window_title = if len > 0 {
            let text = String::from_utf16_lossy(&title_buf[..len as usize]);
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                Some(trimmed.to_string())
            } else {
                None
            }
        } else {
            None
        };

        // Match against installed applications
        let (matched_app_id, matched_app_name) = if let Some(ref path) = proc_path {
            match_process_to_app(path)
        } else {
            (None, None)
        };

        Ok(Some(HunterTargetInfo {
            hwnd: target_hwnd as usize,
            pid,
            process_name: proc_name,
            process_path: proc_path,
            window_title,
            matched_app_id,
            matched_app_name,
            is_critical,
        }))
    }
}

fn match_process_to_app(proc_path: &str) -> (Option<String>, Option<String>) {
    let inventory = crate::apps::discover_apps();
    let proc_path_lower = proc_path.to_ascii_lowercase();

    for app in inventory.apps {
        if let Some(ref loc) = app.install_location {
            let loc_lower = loc.trim().trim_end_matches('\\').to_ascii_lowercase();
            if !loc_lower.is_empty() && proc_path_lower.starts_with(&loc_lower) {
                return (Some(app.id), Some(app.name));
            }
        }
        if let Some(ref uninst) = app.uninstall_string {
            let roots = crate::process::extract_candidate_roots(None, Some(uninst), app.display_icon.as_deref());
            for root in roots {
                let root_lower = root.to_string_lossy().to_ascii_lowercase();
                if proc_path_lower.starts_with(&root_lower) {
                    return (Some(app.id), Some(app.name));
                }
            }
        }
    }

    (None, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_resolution_returns_result_or_none() {
        let res = resolve_cursor_target();
        assert!(res.is_ok());
    }
}
