//! Hunter Mode targeting engine.
//! Resolves desktop windows under cursor coordinates, retrieves process ownership,
//! matches with registered application inventory, and validates safety boundaries.

use std::collections::HashMap;
use std::path::Path;
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
    pub hunter_target_id: String,
    pub can_terminate: bool,
    pub creation_time: u64,
}

pub fn resolve_cursor_target(
    cached_apps: &[crate::apps::InstalledApp],
) -> Result<Option<HunterTargetInfo>, String> {
    unsafe {
        let mut pt = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut pt) == 0 {
            return Ok(None);
        }
        resolve_target_from_point(pt, cached_apps)
    }
}

pub fn resolve_target_from_point(
    pt: POINT,
    cached_apps: &[crate::apps::InstalledApp],
) -> Result<Option<HunterTargetInfo>, String> {
    unsafe {
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
        let creation_time = crate::process::get_process_creation_time(pid).unwrap_or(0);

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

        // Match against installed applications in memory using strict directory boundaries
        let (matched_app_id, matched_app_name) = if let Some(ref path) = proc_path {
            match_process_to_cached_apps(path, cached_apps)
        } else {
            (None, None)
        };

        // Always generate a unique opaque target token using OS-backed cryptographic entropy
        static TARGET_NONCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let count = TARGET_NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let crypto_nonce = crate::leftovers::generate_crypto_nonce(8);
        let hunter_target_id = format!("hunter-target-{now_nanos}-{count}-{pid}-{crypto_nonce}");

        let can_terminate = !is_critical && creation_time > 0;

        Ok(Some(HunterTargetInfo {
            hwnd: target_hwnd as usize,
            pid,
            process_name: proc_name,
            process_path: proc_path,
            window_title,
            matched_app_id,
            matched_app_name,
            is_critical,
            hunter_target_id,
            can_terminate,
            creation_time,
        }))
    }
}

pub fn match_process_to_cached_apps(
    proc_path: &str,
    apps: &[crate::apps::InstalledApp],
) -> (Option<String>, Option<String>) {
    let p = Path::new(proc_path);
    let proc_key = crate::process::normalize_key(p);

    for app in apps {
        if let Some(ref loc) = app.install_location {
            let loc_trimmed = loc.trim();
            if !loc_trimmed.is_empty() {
                let loc_key = crate::process::normalize_key(Path::new(loc_trimmed));
                if is_path_in_directory(&proc_key, &loc_key) {
                    return (Some(app.id.clone()), Some(app.name.clone()));
                }
            }
        }
        if let Some(ref uninst) = app.uninstall_string {
            let roots = crate::process::extract_candidate_roots(None, Some(uninst), app.display_icon.as_deref());
            for root in roots {
                let root_key = crate::process::normalize_key(&root);
                if is_path_in_directory(&proc_key, &root_key) {
                    return (Some(app.id.clone()), Some(app.name.clone()));
                }
            }
        }
    }

    (None, None)
}

pub fn is_path_in_directory(path_key: &str, dir_key: &str) -> bool {
    if dir_key.is_empty() {
        return false;
    }
    let dir_clean = dir_key.trim_end_matches('\\');
    path_key == dir_clean || path_key.starts_with(&format!("{dir_clean}\\"))
}

pub fn register_target_in_state(
    target: &HunterTargetInfo,
    hunter_snapshots: &mut HashMap<String, crate::process::StoredProcessSnapshot>,
    hunter_app_snapshots: &mut HashMap<String, crate::apps::InstalledApp>,
) {
    if target.pid <= 4 {
        return;
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Prune stale hunter snapshots older than 300s (5-minute TTL)
    hunter_snapshots.retain(|_, s| now.saturating_sub(s.timestamp) < 300);
    hunter_app_snapshots.retain(|k, _| hunter_snapshots.contains_key(k));

    let parent_dir = target
        .process_path
        .as_deref()
        .and_then(|p| Path::new(p).parent())
        .map(|p| p.to_path_buf());

    let candidate_roots = if let Some(ref dir) = parent_dir {
        if crate::process::is_candidate_root_safe(dir) {
            vec![crate::process::normalize_key(dir)]
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    let install_location = parent_dir
        .as_ref()
        .filter(|d| crate::process::is_candidate_root_safe(d))
        .map(|d| d.to_string_lossy().to_string())
        .unwrap_or_default();

    // Register verified process snapshot in isolated Hunter snapshot registry
    let snapshot = crate::process::StoredProcessSnapshot {
        app_id: target.hunter_target_id.clone(),
        install_location: install_location.clone(),
        candidate_roots,
        timestamp: now,
        processes: vec![crate::process::AppProcess {
            pid: target.pid,
            name: target.process_name.clone(),
            path: target.process_path.clone(),
            is_critical: target.is_critical,
            title: target.window_title.clone(),
            creation_time: target.creation_time,
        }],
    };
    hunter_snapshots.insert(target.hunter_target_id.clone(), snapshot);

    // Register backend InstalledApp snapshot in isolated Hunter app snapshot registry
    let app = crate::apps::InstalledApp {
        id: target.hunter_target_id.clone(),
        name: target
            .window_title
            .clone()
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| target.process_name.clone()),
        version: None,
        publisher: None,
        install_location: if install_location.is_empty() {
            None
        } else {
            Some(install_location)
        },
        install_date: None,
        uninstall_string: None,
        quiet_uninstall_string: None,
        display_icon: target.process_path.clone(),
        estimated_size: None,
        hive: "portable".into(),
        scope: "user".into(),
        architecture: "x64".into(),
        is_protected: target.is_critical,
        source: "portable".into(),
        uninstall_support: "missing".into(),
        confidence: "heuristic".into(),
        detection_reason: Some(format!("Targeted via Hunter Mode (PID {})", target.pid)),
        registry_key_name: None,
    };
    hunter_app_snapshots.insert(target.hunter_target_id.clone(), app);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_resolution_returns_result_or_none() {
        let res = resolve_cursor_target(&[]);
        assert!(res.is_ok());
    }

    #[test]
    fn directory_boundary_matching_rejects_prefix_collisions() {
        // C:\Apps\Foo must NOT match C:\Apps\FooBar\test.exe
        assert!(!is_path_in_directory(r"c:\apps\foobar\test.exe", r"c:\apps\foo"));
        assert!(!is_path_in_directory(r"c:\apps\foobar", r"c:\apps\foo"));
        // C:\Apps\Foo must match C:\Apps\Foo\test.exe
        assert!(is_path_in_directory(r"c:\apps\foo\test.exe", r"c:\apps\foo"));
        assert!(is_path_in_directory(r"c:\apps\foo\nested\test.exe", r"c:\apps\foo\"));
    }
}
