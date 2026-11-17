use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use winreg::enums::*;
use winreg::RegKey;

/// Represents where the application should route upon launch based on CLI arguments.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StartupRoute {
    pub view: String,
    pub path: Option<String>,
    pub subview: Option<String>,
    pub target_app_id: Option<String>,
    pub action: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerIntegrationStatus {
    pub is_registered: bool,
    pub standard_actions_registered: bool,
    pub advanced_actions_registered: bool,
    pub folder_actions_registered: bool,
    pub exe_actions_registered: bool,
    pub registered_exe_path: Option<String>,
    pub current_exe_path: String,
    pub needs_repair: bool,
    pub details: Vec<String>,
}

/// Registry verbs registered under HKCU\Software\Classes
#[derive(Debug, Clone, Copy)]
pub struct ShellVerb {
    pub parent_key: &'static str,
    pub verb_id: &'static str,
    pub menu_text: &'static str,
    pub arg_flag: &'static str,
    pub use_quote_v: bool,
    pub is_advanced: bool,
}

pub const SHELL_VERBS: &[ShellVerb] = &[
    // Folders & Directories (Standard)
    ShellVerb {
        parent_key: r"Directory\shell",
        verb_id: "Cyron.Analyze",
        menu_text: "Analyze with Cyron",
        arg_flag: "--analyze",
        use_quote_v: true,
        is_advanced: false,
    },
    ShellVerb {
        parent_key: r"Directory\shell",
        verb_id: "Cyron.Duplicates",
        menu_text: "Find Duplicates with Cyron",
        arg_flag: "--duplicates",
        use_quote_v: true,
        is_advanced: false,
    },
    ShellVerb {
        parent_key: r"Directory\shell",
        verb_id: "Cyron.Storage",
        menu_text: "View Storage Breakdown with Cyron",
        arg_flag: "--storage",
        use_quote_v: true,
        is_advanced: false,
    },
    // Folder background (when right clicking inside a directory)
    ShellVerb {
        parent_key: r"Directory\Background\shell",
        verb_id: "Cyron.Analyze",
        menu_text: "Analyze this folder with Cyron",
        arg_flag: "--analyze",
        use_quote_v: true,
        is_advanced: false,
    },
    // Drives
    ShellVerb {
        parent_key: r"Drive\shell",
        verb_id: "Cyron.Analyze",
        menu_text: "Analyze with Cyron",
        arg_flag: "--analyze",
        use_quote_v: true,
        is_advanced: false,
    },
    ShellVerb {
        parent_key: r"Drive\shell",
        verb_id: "Cyron.Duplicates",
        menu_text: "Find Duplicates with Cyron",
        arg_flag: "--duplicates",
        use_quote_v: true,
        is_advanced: false,
    },
    ShellVerb {
        parent_key: r"Drive\shell",
        verb_id: "Cyron.Storage",
        menu_text: "View Storage Breakdown with Cyron",
        arg_flag: "--storage",
        use_quote_v: true,
        is_advanced: false,
    },
    // Executables (Standard)
    ShellVerb {
        parent_key: r"SystemFileAssociations\.exe\shell",
        verb_id: "Cyron.Uninstall",
        menu_text: "Uninstall with Cyron",
        arg_flag: "--uninstall-target",
        use_quote_v: false,
        is_advanced: false,
    },
    ShellVerb {
        parent_key: r"SystemFileAssociations\.exe\shell",
        verb_id: "Cyron.Related",
        menu_text: "Find Related Files with Cyron",
        arg_flag: "--find-related",
        use_quote_v: false,
        is_advanced: false,
    },
    // Advanced & Destructive Actions
    // Files (*\shell)
    ShellVerb {
        parent_key: r"*\shell",
        verb_id: "Cyron.ForceDelete",
        menu_text: "Force Delete with Cyron",
        arg_flag: "--force-delete",
        use_quote_v: false,
        is_advanced: true,
    },
    ShellVerb {
        parent_key: r"*\shell",
        verb_id: "Cyron.LockingProcesses",
        menu_text: "Find Locking Processes with Cyron",
        arg_flag: "--find-locks",
        use_quote_v: false,
        is_advanced: true,
    },
    // Folders (Directory\shell)
    ShellVerb {
        parent_key: r"Directory\shell",
        verb_id: "Cyron.ForceDelete",
        menu_text: "Force Delete with Cyron",
        arg_flag: "--force-delete",
        use_quote_v: true,
        is_advanced: true,
    },
    ShellVerb {
        parent_key: r"Directory\shell",
        verb_id: "Cyron.LockingProcesses",
        menu_text: "Find Locking Processes with Cyron",
        arg_flag: "--find-locks",
        use_quote_v: true,
        is_advanced: true,
    },
    // Executables (Kill process)
    ShellVerb {
        parent_key: r"SystemFileAssociations\.exe\shell",
        verb_id: "Cyron.KillProcess",
        menu_text: "Kill Running Process with Cyron",
        arg_flag: "--kill-process",
        use_quote_v: false,
        is_advanced: true,
    },
];

/* ------------------------------------------------------------------ types */

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockingProcessInfo {
    pub pid: u32,
    pub name: String,
    pub path: Option<String>,
    pub window_title: Option<String>,
    pub is_critical: bool,
    pub creation_time: u64,
}

/// Identity captured from a handle at preview time. It is intentionally not
/// sent to the webview; execution binds to it so a different file at the same
/// pathname cannot be substituted after the user confirms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileIdentity {
    pub volume_serial: u32,
    pub file_index: u64,
    pub creation_time: u64,
    pub size_bytes: u64,
    pub attributes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileTargetInspection {
    pub path: String,
    pub canonical_path: String,
    pub is_directory: bool,
    pub exists: bool,
    pub size_bytes: u64,
    pub file_count: u64,
    pub dir_count: u64,
    pub is_readonly: bool,
    pub is_hidden: bool,
    pub is_system: bool,
    pub is_reparse_point: bool,
    pub owner: Option<String>,
    pub reasons_for_failure: Vec<String>,
    pub sample_items: Vec<String>,
    #[serde(skip)]
    pub identity: Option<FileIdentity>,
}

#[derive(Debug, Clone)]
pub struct StoredForceDeletePlan {
    pub plan_id: String,
    pub target_path: PathBuf,
    pub canonical_path: PathBuf,
    pub is_directory: bool,
    pub expires_at: u64,
    pub inspection: FileTargetInspection,
    pub locking_processes: Vec<LockingProcessInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForceDeletePlanPreview {
    pub plan_id: String,
    pub inspection: FileTargetInspection,
    pub locking_processes: Vec<LockingProcessInfo>,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForceDeleteResult {
    pub success: bool,
    pub mode: String,
    pub deleted_files: u64,
    pub deleted_dirs: u64,
    pub freed_bytes: u64,
    pub quarantine_batch_id: Option<String>,
    pub message: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct StoredKillProcessPlan {
    pub plan_id: String,
    pub exe_path: PathBuf,
    pub canonical_exe_path: PathBuf,
    pub expires_at: u64,
    pub matching_processes: Vec<LockingProcessInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KillProcessPlanPreview {
    pub plan_id: String,
    pub exe_path: String,
    pub matching_processes: Vec<LockingProcessInfo>,
    pub expires_at: u64,
}

/* ------------------------------------------------------------- validation */

/// Validates that a path provided via CLI exists and is absolute.
pub fn sanitize_target_path(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim().trim_matches('"');
    if trimmed.is_empty() {
        return None;
    }
    let path = Path::new(trimmed);
    if !path.is_absolute() {
        return None;
    }
    if !path.exists() {
        return None;
    }
    // Preserve the caller's path instead of resolving it. Force Delete must see
    // a junction or symlink itself and refuse it, not silently retarget the
    // deletion at whatever the link points to.
    Some(path.to_path_buf())
}

fn normalize_path_for_guard(path: &Path) -> String {
    let s = path.to_string_lossy().replace('/', "\\");
    let without_verbatim = s.strip_prefix(r"\\?\").unwrap_or(&s);
    without_verbatim.trim_end_matches('\\').to_ascii_lowercase()
}

/// Refuses protected Windows, boot, security, and root targets.
pub fn is_protected_delete_target(path: &Path) -> Result<(), String> {
    let canonical = match path.canonicalize() {
        Ok(c) => c,
        Err(_) => path.to_path_buf(),
    };
    let lower = normalize_path_for_guard(&canonical);
    let raw_lower = normalize_path_for_guard(path);

    // 1. Refuse drive roots: C:, C:\, etc.
    let parts: Vec<&str> = lower.split('\\').filter(|p| !p.is_empty()).collect();
    if parts.len() <= 1 {
        return Err("Drive roots and bare volume mounts cannot be deleted.".into());
    }

    // 2. Windows dir and system subdirectories
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    let windir_key = normalize_path_for_guard(Path::new(&windir));
    if lower == windir_key
        || lower.starts_with(&format!("{windir_key}\\"))
        || raw_lower == windir_key
        || raw_lower.starts_with(&format!("{windir_key}\\"))
    {
        return Err("Windows operating system directory and its contents are protected and cannot be deleted.".into());
    }

    // 3. User profile root (e.g. C:\Users or C:\Users\Username)
    if let Ok(userprof) = std::env::var("USERPROFILE") {
        let userprof_key = normalize_path_for_guard(Path::new(&userprof));
        if lower == userprof_key || raw_lower == userprof_key {
            return Err("The current user profile root directory cannot be deleted.".into());
        }
    }
    if lower == "c:\\users" || lower.ends_with(":\\users") || raw_lower == "c:\\users" || raw_lower.ends_with(":\\users") {
        return Err("The Windows Users directory cannot be deleted.".into());
    }

    // 4. Program Files roots (e.g. C:\Program Files or C:\Program Files (x86))
    if let Ok(pf) = std::env::var("ProgramFiles") {
        let pf_key = normalize_path_for_guard(Path::new(&pf));
        if lower == pf_key || raw_lower == pf_key {
            return Err("The Program Files directory root cannot be deleted.".into());
        }
    }
    if let Ok(pf86) = std::env::var("ProgramFiles(x86)") {
        let pf86_key = normalize_path_for_guard(Path::new(&pf86));
        if lower == pf86_key || raw_lower == pf86_key {
            return Err("The Program Files (x86) directory root cannot be deleted.".into());
        }
    }

    // 5. ProgramData root
    if let Ok(pd) = std::env::var("ProgramData") {
        let pd_key = normalize_path_for_guard(Path::new(&pd));
        if lower == pd_key || raw_lower == pd_key {
            return Err("The ProgramData directory root cannot be deleted.".into());
        }
    }

    // 6. AppData roots and the temporary directory. Deleting these roots
    // removes every application's state, not one disposable item.
    for variable in ["APPDATA", "LOCALAPPDATA", "TEMP", "TMP"] {
        if let Ok(value) = std::env::var(variable) {
            let key = normalize_path_for_guard(Path::new(&value));
            if lower == key || raw_lower == key {
                return Err(format!(
                    "The {variable} directory root cannot be deleted."
                ));
            }
        }
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        let appdata = normalize_path_for_guard(&Path::new(&profile).join("AppData"));
        if lower == appdata || raw_lower == appdata {
            return Err("The AppData directory root cannot be deleted.".into());
        }
    }

    // 7. Critical boot and memory files
    let file_name = canonical
        .file_name()
        .or_else(|| path.file_name())
        .and_then(|f| f.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if matches!(
        file_name.as_str(),
        "pagefile.sys"
            | "swapfile.sys"
            | "hiberfil.sys"
            | "bootmgr"
            | "bootnxt"
            | "dumpstack.log"
            | "ntldr"
            | "ntuser.dat"
    ) {
        return Err(format!("The critical Windows system file '{file_name}' is protected and cannot be deleted."));
    }

    // 8. System Volume Information, $Recycle.Bin
    if file_name == "system volume information" || file_name == "$recycle.bin" {
        return Err(format!("The system directory '{file_name}' is protected and cannot be deleted."));
    }

    Ok(())
}

/* ----------------------------------------------------------- restart manager */

/// Finds processes locking the specified file or folder using Windows Restart Manager.
pub fn find_locking_processes(target_path: &Path) -> Result<Vec<LockingProcessInfo>, String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::System::RestartManager::{
        RmEndSession, RmGetList, RmRegisterResources, RmStartSession, CCH_RM_SESSION_KEY,
        RM_PROCESS_INFO,
    };

    let canonical = match target_path.canonicalize() {
        Ok(c) => c,
        Err(_) => target_path.to_path_buf(),
    };

    let path_str = canonical.to_string_lossy();
    let clean_path = path_str.strip_prefix(r"\\?\").unwrap_or(&path_str);
    let wide_path: Vec<u16> = OsStr::new(clean_path).encode_wide().chain(std::iter::once(0)).collect();

    let mut session_handle = 0u32;
    let mut session_key = [0u16; (CCH_RM_SESSION_KEY + 1) as usize];

    let start_err = unsafe { RmStartSession(&mut session_handle, 0, session_key.as_mut_ptr()) };
    if start_err != 0 {
        return Err(format!("RmStartSession failed with error code: {start_err}"));
    }

    struct SessionGuard(u32);
    impl Drop for SessionGuard {
        fn drop(&mut self) {
            unsafe { RmEndSession(self.0) };
        }
    }
    let _guard = SessionGuard(session_handle);

    let file_paths = [wide_path.as_ptr()];
    let reg_err = unsafe {
        RmRegisterResources(
            session_handle,
            1,
            file_paths.as_ptr(),
            0,
            std::ptr::null(),
            0,
            std::ptr::null(),
        )
    };
    if reg_err != 0 {
        return Err(format!("RmRegisterResources failed with error code: {reg_err}"));
    }

    let mut n_proc_info_needed = 0u32;
    let mut n_proc_info = 0u32;
    let mut reboot_reasons = 0u32;

    let list_err = unsafe {
        RmGetList(
            session_handle,
            &mut n_proc_info_needed,
            &mut n_proc_info,
            std::ptr::null_mut(),
            &mut reboot_reasons,
        )
    };

    if (list_err != 0 && list_err != 234) || n_proc_info_needed == 0 {
        return Ok(Vec::new());
    }

    let mut proc_info: Vec<RM_PROCESS_INFO> = vec![unsafe { std::mem::zeroed() }; n_proc_info_needed as usize];
    n_proc_info = n_proc_info_needed;

    let list_err2 = unsafe {
        RmGetList(
            session_handle,
            &mut n_proc_info_needed,
            &mut n_proc_info,
            proc_info.as_mut_ptr(),
            &mut reboot_reasons,
        )
    };

    if list_err2 != 0 {
        return Ok(Vec::new());
    }

    let self_pid = std::process::id();
    let mut results = Vec::new();

    for info in proc_info.iter().take(n_proc_info as usize) {
        let pid = info.Process.dwProcessId;
        if pid <= 4 || pid == self_pid {
            continue;
        }

        let (name, path) = crate::process::query_process_info(pid);
        let is_critical = crate::process::is_critical_process(pid, &name, path.as_deref());
        let creation_time = crate::process::get_process_creation_time(pid).unwrap_or(0);
        let title = crate::process::find_window_title(pid);

        results.push(LockingProcessInfo {
            pid,
            name,
            path,
            window_title: title,
            is_critical,
            creation_time,
        });
    }

    Ok(results)
}

/* ----------------------------------------------------------- target inspection */

/// Queries file owner using Win32 security APIs.
pub fn query_file_owner(path: &Path) -> Option<String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Security::{
        GetFileSecurityW, GetSecurityDescriptorOwner, LookupAccountSidW, OWNER_SECURITY_INFORMATION,
        PSECURITY_DESCRIPTOR, SID_NAME_USE,
    };
    type PSID = *mut core::ffi::c_void;

    let path_str = path.to_string_lossy();
    let clean = path_str.strip_prefix(r"\\?\").unwrap_or(&path_str);
    let wide: Vec<u16> = OsStr::new(clean).encode_wide().chain(std::iter::once(0)).collect();

    let mut needed = 0u32;
    unsafe {
        GetFileSecurityW(
            wide.as_ptr(),
            OWNER_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            0,
            &mut needed,
        );
    }
    if needed == 0 {
        return None;
    }

    let mut buffer: Vec<u8> = vec![0u8; needed as usize];
    let ok = unsafe {
        GetFileSecurityW(
            wide.as_ptr(),
            OWNER_SECURITY_INFORMATION,
            buffer.as_mut_ptr() as PSECURITY_DESCRIPTOR,
            needed,
            &mut needed,
        )
    };
    if ok == 0 {
        return None;
    }

    let mut psid: PSID = std::ptr::null_mut();
    let mut defaulted = 0i32;
    let ok_owner = unsafe {
        GetSecurityDescriptorOwner(buffer.as_ptr() as PSECURITY_DESCRIPTOR, &mut psid, &mut defaulted)
    };
    if ok_owner == 0 || psid.is_null() {
        return None;
    }

    let mut name_buf = [0u16; 256];
    let mut domain_buf = [0u16; 256];
    let mut name_len = name_buf.len() as u32;
    let mut domain_len = domain_buf.len() as u32;
    let mut sid_type: SID_NAME_USE = 0;

    let ok_lookup = unsafe {
        LookupAccountSidW(
            std::ptr::null(),
            psid,
            name_buf.as_mut_ptr(),
            &mut name_len,
            domain_buf.as_mut_ptr(),
            &mut domain_len,
            &mut sid_type,
        )
    };

    if ok_lookup != 0 {
        let domain = String::from_utf16_lossy(&domain_buf[..domain_len as usize]);
        let name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
        if domain.is_empty() {
            Some(name)
        } else {
            Some(format!("{domain}\\{name}"))
        }
    } else {
        None
    }
}

/// Clears the read-only attribute on a file or folder if set.
pub fn clear_readonly_attribute(path: &Path) {
    if let Ok(meta) = std::fs::metadata(path) {
        let mut perms = meta.permissions();
        if perms.readonly() {
            #[allow(clippy::permissions_set_readonly_false)]
            perms.set_readonly(false);
            let _ = std::fs::set_permissions(path, perms);
        }
    }
}

/// Recursively clears read-only attributes on all items in a directory.
pub fn clear_readonly_recursive(root: &Path) {
    clear_readonly_attribute(root);
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                clear_readonly_recursive(&p);
            } else {
                clear_readonly_attribute(&p);
            }
        }
    }
}

/// Reads the stable identity of a file or directory from an opened handle.
/// `FILE_FLAG_OPEN_REPARSE_POINT` keeps a link itself from being resolved.
fn file_identity(path: &Path) -> Result<FileIdentity, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, OPEN_EXISTING,
    };
    const FILE_READ_ATTRIBUTES: u32 = 0x80;

    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_READ_ATTRIBUTES,
            7,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(format!(
            "Could not read the identity of {}: {}",
            path.display(),
            std::io::Error::last_os_error()
        ));
    }
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    let read = unsafe { GetFileInformationByHandle(handle, &mut info) };
    unsafe { CloseHandle(handle) };
    if read == 0 {
        return Err(format!(
            "Could not read the identity of {}: {}",
            path.display(),
            std::io::Error::last_os_error()
        ));
    }
    Ok(FileIdentity {
        volume_serial: info.dwVolumeSerialNumber,
        file_index: ((info.nFileIndexHigh as u64) << 32) | info.nFileIndexLow as u64,
        creation_time: ((info.ftCreationTime.dwHighDateTime as u64) << 32)
            | info.ftCreationTime.dwLowDateTime as u64,
        size_bytes: ((info.nFileSizeHigh as u64) << 32) | info.nFileSizeLow as u64,
        attributes: info.dwFileAttributes,
    })
}

/// Comprehensive inspection of a file or folder target for deletion review.
pub fn inspect_target(target_path: &Path) -> Result<FileTargetInspection, String> {
    if !target_path.exists() {
        return Err(format!("The path does not exist: {}", target_path.display()));
    }
    let canonical = target_path
        .canonicalize()
        .map_err(|e| format!("Windows refused access to {}: {e}", target_path.display()))?;

    is_protected_delete_target(&canonical)?;

    let meta = std::fs::symlink_metadata(&canonical)
        .map_err(|e| format!("Could not read target metadata: {e}"))?;

    let is_directory = meta.is_dir();
    let mut file_count = 0u64;
    let mut dir_count = 0u64;
    let mut size_bytes = meta.len();
    let mut sample_items = Vec::new();

    if is_directory {
        let mut stack = vec![canonical.clone()];
        while let Some(dir) = stack.pop() {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if let Ok(m) = std::fs::symlink_metadata(&p) {
                        if is_reparse_entry(&m) {
                            // Never walk into a junction or symlink while
                            // preparing a deletion. The link is counted as one
                            // entry and is never treated as its target's tree.
                            file_count += 1;
                        } else if m.is_dir() {
                            dir_count += 1;
                            if dir_count < 50_000 {
                                stack.push(p.clone());
                            }
                        } else {
                            file_count += 1;
                            size_bytes += m.len();
                        }
                        if sample_items.len() < 10 {
                            if let Ok(rel) = p.strip_prefix(&canonical) {
                                sample_items.push(rel.to_string_lossy().to_string());
                            } else {
                                sample_items.push(p.file_name().unwrap_or_default().to_string_lossy().to_string());
                            }
                        }
                    }
                }
            }
        }
    } else {
        file_count = 1;
        sample_items.push(canonical.file_name().unwrap_or_default().to_string_lossy().to_string());
    }

    let (is_readonly, is_hidden, is_system, is_reparse_point) = {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileAttributesW, FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_READONLY,
            FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_SYSTEM, INVALID_FILE_ATTRIBUTES,
        };

        let path_str = canonical.to_string_lossy();
        let clean = path_str.strip_prefix(r"\\?\").unwrap_or(&path_str);
        let wide: Vec<u16> = OsStr::new(clean).encode_wide().chain(std::iter::once(0)).collect();

        let attrs = unsafe { GetFileAttributesW(wide.as_ptr()) };
        if attrs != INVALID_FILE_ATTRIBUTES {
            (
                (attrs & FILE_ATTRIBUTE_READONLY) != 0,
                (attrs & FILE_ATTRIBUTE_HIDDEN) != 0,
                (attrs & FILE_ATTRIBUTE_SYSTEM) != 0,
                (attrs & FILE_ATTRIBUTE_REPARSE_POINT) != 0,
            )
        } else {
            (false, false, false, false)
        }
    };

    let owner = query_file_owner(&canonical);
    let identity = file_identity(&canonical).ok();

    let mut reasons = Vec::new();
    if is_readonly {
        reasons.push("Read-only attribute is set; ordinary deletion will fail unless attribute is cleared.".into());
    }
    if is_system {
        reasons.push("Item has Windows system attribute set.".into());
    }
    if is_reparse_point {
        reasons.push("Item is a reparse point (junction or symlink); standard recursive deletion may fail.".into());
    }

    Ok(FileTargetInspection {
        path: target_path.to_string_lossy().to_string(),
        canonical_path: canonical.to_string_lossy().to_string(),
        is_directory,
        exists: true,
        size_bytes,
        file_count,
        dir_count,
        is_readonly,
        is_hidden,
        is_system,
        is_reparse_point,
        owner,
        reasons_for_failure: reasons,
        sample_items,
        identity,
    })
}

/* ---------------------------------------------------- startup argument parsing */

/// Parses command line arguments into a startup routing decision.
pub fn parse_startup_args(args: &[String]) -> Option<StartupRoute> {
    if args.len() < 2 {
        return None;
    }

    let mut i = 1;
    while i < args.len() {
        let flag = args[i].as_str();
        match flag {
            "--analyze" | "-a" => {
                let target = args.get(i + 1)?;
                let sanitized = sanitize_target_path(target)?;
                return Some(StartupRoute {
                    view: "analyzer".to_string(),
                    path: Some(sanitized.to_string_lossy().to_string()),
                    subview: Some("map".to_string()),
                    target_app_id: None,
                    action: Some("scan".to_string()),
                    note: None,
                });
            }
            "--duplicates" | "-d" => {
                let target = args.get(i + 1)?;
                let sanitized = sanitize_target_path(target)?;
                return Some(StartupRoute {
                    view: "analyzer".to_string(),
                    path: Some(sanitized.to_string_lossy().to_string()),
                    subview: Some("duplicates".to_string()),
                    target_app_id: None,
                    action: Some("scan".to_string()),
                    note: None,
                });
            }
            "--storage" | "-s" => {
                let target = args.get(i + 1)?;
                let sanitized = sanitize_target_path(target)?;
                return Some(StartupRoute {
                    view: "analyzer".to_string(),
                    path: Some(sanitized.to_string_lossy().to_string()),
                    subview: Some("storage".to_string()),
                    target_app_id: None,
                    action: Some("scan".to_string()),
                    note: None,
                });
            }
            "--uninstall-target" | "-u" => {
                let target = args.get(i + 1)?;
                let sanitized = sanitize_target_path(target)?;
                let path_str = sanitized.to_string_lossy().to_string();

                let inventory = crate::apps::discover_apps();
                let matched_app = find_matching_app_for_exe(&inventory.apps, &sanitized);

                return Some(StartupRoute {
                    view: "uninstaller".to_string(),
                    path: Some(path_str.clone()),
                    subview: None,
                    target_app_id: matched_app.as_ref().map(|a| a.id.clone()),
                    action: if matched_app.is_some() {
                        Some("review".to_string())
                    } else {
                        Some("unregistered".to_string())
                    },
                    note: if matched_app.is_some() {
                        None
                    } else {
                        Some(format!("Executable is not registered in Windows uninstall registry: {path_str}"))
                    },
                });
            }
            "--find-related" | "-r" => {
                let target = args.get(i + 1)?;
                let sanitized = sanitize_target_path(target)?;
                let path_str = sanitized.to_string_lossy().to_string();

                let inventory = crate::apps::discover_apps();
                let matched_app = find_matching_app_for_exe(&inventory.apps, &sanitized);

                return Some(StartupRoute {
                    view: "uninstaller".to_string(),
                    path: Some(path_str),
                    subview: None,
                    target_app_id: matched_app.as_ref().map(|a| a.id.clone()),
                    action: Some("leftovers".to_string()),
                    note: None,
                });
            }
            "--force-delete" | "-fd" => {
                let target = args.get(i + 1)?;
                let sanitized = sanitize_target_path(target)?;
                return Some(StartupRoute {
                    view: "overview".to_string(),
                    path: Some(sanitized.to_string_lossy().to_string()),
                    subview: None,
                    target_app_id: None,
                    action: Some("force-delete".to_string()),
                    note: None,
                });
            }
            "--find-locks" | "-fl" => {
                let target = args.get(i + 1)?;
                let sanitized = sanitize_target_path(target)?;
                return Some(StartupRoute {
                    view: "overview".to_string(),
                    path: Some(sanitized.to_string_lossy().to_string()),
                    subview: None,
                    target_app_id: None,
                    action: Some("find-locks".to_string()),
                    note: None,
                });
            }
            "--kill-process" | "-kp" => {
                let target = args.get(i + 1)?;
                let sanitized = sanitize_target_path(target)?;
                return Some(StartupRoute {
                    view: "overview".to_string(),
                    path: Some(sanitized.to_string_lossy().to_string()),
                    subview: None,
                    target_app_id: None,
                    action: Some("kill-process".to_string()),
                    note: None,
                });
            }
            _ => {
                i += 1;
            }
        }
    }

    None
}

/// Matches an executable path against known installed applications.
fn find_matching_app_for_exe<'a>(
    apps: &'a [crate::apps::InstalledApp],
    exe_path: &Path,
) -> Option<&'a crate::apps::InstalledApp> {
    let exe_norm = exe_path.to_string_lossy().to_lowercase();
    let exe_name = exe_path.file_name()?.to_string_lossy().to_lowercase();

    for app in apps {
        if let Some(ref icon) = app.display_icon {
            let icon_clean = icon.split(',').next().unwrap_or(icon).trim().trim_matches('"');
            if !icon_clean.is_empty() && icon_clean.to_lowercase() == exe_norm {
                return Some(app);
            }
        }
    }

    let mut best_match: Option<(&crate::apps::InstalledApp, usize)> = None;
    for app in apps {
        if let Some(ref loc) = app.install_location {
            let loc_clean = loc.trim().trim_matches('"');
            if !loc_clean.is_empty() {
                let loc_norm = loc_clean.to_lowercase();
                if exe_norm.starts_with(&loc_norm) {
                    let len = loc_norm.len();
                    if best_match.as_ref().map_or(true, |(_, best_len)| len > *best_len) {
                        best_match = Some((app, len));
                    }
                }
            }
        }
    }

    if let Some((app, _)) = best_match {
        return Some(app);
    }

    for app in apps {
        if let Some(ref uninst) = app.uninstall_string {
            if uninst.to_lowercase().contains(&exe_name) {
                return Some(app);
            }
        }
    }

    None
}

/* ---------------------------------------------------- integration management */

/// Retrieves the status of Explorer context menu registration in HKCU\Software\Classes.
pub fn get_integration_status() -> ExplorerIntegrationStatus {
    let current_exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let classes_res = hkcu.open_subkey_with_flags(r"Software\Classes", KEY_READ);

    let classes = match classes_res {
        Ok(k) => k,
        Err(e) => {
            return ExplorerIntegrationStatus {
                is_registered: false,
                standard_actions_registered: false,
                advanced_actions_registered: false,
                folder_actions_registered: false,
                exe_actions_registered: false,
                registered_exe_path: None,
                current_exe_path: current_exe,
                needs_repair: false,
                details: vec![format!("Failed to open HKCU\\Software\\Classes: {e}")],
            }
        }
    };

    let mut details = Vec::new();
    let mut standard_ok = true;
    let mut advanced_ok = true;
    let mut folder_ok = true;
    let mut exe_ok = true;
    let mut any_registered = false;
    let mut registered_path: Option<String> = None;

    for verb in SHELL_VERBS {
        let subkey_path = format!(r"{}\{}\command", verb.parent_key, verb.verb_id);
        match classes.open_subkey_with_flags(&subkey_path, KEY_READ) {
            Ok(cmd_key) => {
                any_registered = true;
                let cmd_str: String = cmd_key.get_value("").unwrap_or_default();
                if registered_path.is_none() && !cmd_str.is_empty() {
                    if let Some(first_quote) = cmd_str.find('"') {
                        if let Some(second_quote) = cmd_str[first_quote + 1..].find('"') {
                            registered_path = Some(cmd_str[first_quote + 1..first_quote + 1 + second_quote].to_string());
                        }
                    }
                }
            }
            Err(_) => {
                if verb.is_advanced {
                    advanced_ok = false;
                } else {
                    standard_ok = false;
                }
                if verb.parent_key.starts_with("Directory") || verb.parent_key.starts_with("Drive") {
                    folder_ok = false;
                } else if verb.parent_key.contains(".exe") {
                    exe_ok = false;
                }
            }
        }
    }

    let is_registered = any_registered && standard_ok;
    let mut needs_repair = false;

    if let Some(ref reg_exe) = registered_path {
        if !current_exe.is_empty() && !reg_exe.eq_ignore_ascii_case(&current_exe) {
            needs_repair = true;
            details.push(format!("Registered executable ({reg_exe}) does not match current binary ({current_exe})"));
        }
    }

    if any_registered && !is_registered && !advanced_ok {
        needs_repair = true;
        details.push("Some context menu entries are missing or partially registered".to_string());
    }

    ExplorerIntegrationStatus {
        is_registered,
        standard_actions_registered: standard_ok,
        advanced_actions_registered: advanced_ok,
        folder_actions_registered: folder_ok,
        exe_actions_registered: exe_ok,
        registered_exe_path: registered_path,
        current_exe_path: current_exe,
        needs_repair,
        details,
    }
}

/// Registers or unregisters Cyron context menu entries in HKCU\Software\Classes.
pub fn register_explorer_integration(standard: bool, advanced: bool) -> Result<(), String> {
    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Could not resolve current executable path: {e}"))?;
    let exe_path_str = current_exe.to_string_lossy().to_string();

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let classes = hkcu
        .create_subkey_with_flags(r"Software\Classes", KEY_ALL_ACCESS)
        .map_err(|e| format!("Failed to open or create HKCU\\Software\\Classes: {e}"))?
        .0;

    for verb in SHELL_VERBS {
        let verb_path = format!(r"{}\{}", verb.parent_key, verb.verb_id);
        let should_register = if verb.is_advanced { advanced } else { standard };

        if should_register {
            let (verb_key, _) = classes
                .create_subkey_with_flags(&verb_path, KEY_ALL_ACCESS)
                .map_err(|e| format!("Failed to create {verb_path}: {e}"))?;

            verb_key
                .set_value("", &verb.menu_text)
                .map_err(|e| format!("Failed to set text on {verb_path}: {e}"))?;

            let icon_val = format!("\"{exe_path_str}\",0");
            let _ = verb_key.set_value("Icon", &icon_val);

            let (cmd_key, _) = verb_key
                .create_subkey_with_flags("command", KEY_ALL_ACCESS)
                .map_err(|e| format!("Failed to create command key for {verb_path}: {e}"))?;

            let param = if verb.use_quote_v { "\"%V\"" } else { "\"%1\"" };
            let command_line = format!("\"{exe_path_str}\" {} {}", verb.arg_flag, param);

            cmd_key
                .set_value("", &command_line)
                .map_err(|e| format!("Failed to set command on {verb_path}: {e}"))?;
        } else {
            let _ = classes.delete_subkey_all(&verb_path);
        }
    }

    Ok(())
}

/// Unregisters all Cyron context menu entries from HKCU\Software\Classes.
pub fn unregister_explorer_integration() -> Result<(), String> {
    register_explorer_integration(false, false)
}

/* ---------------------------------------------------------------- plans */

static PLAN_COUNTER: AtomicU64 = AtomicU64::new(1);

fn generate_plan_id(prefix: &str) -> String {
    let count = PLAN_COUNTER.fetch_add(1, Ordering::Relaxed);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let nonce = crate::leftovers::generate_crypto_nonce(8);
    format!("{prefix}-{now}-{count}-{nonce}")
}

/// Prepares an execution plan for Force Delete with target inspection and locking process detection.
pub fn prepare_force_delete_plan(target_path_str: &str) -> Result<ForceDeletePlanPreview, String> {
    let raw_path = PathBuf::from(target_path_str.trim().trim_matches('"'));
    if !raw_path.exists() {
        return Err(format!("The specified target does not exist: {target_path_str}"));
    }
    let link_meta = std::fs::symlink_metadata(&raw_path)
        .map_err(|e| format!("Cannot inspect target path: {e}"))?;
    if is_reparse_entry(&link_meta) {
        return Err(
            "Force Delete refuses junctions and symbolic links: resolving one could delete its target. Remove the link itself in Explorer instead."
                .into(),
        );
    }
    let canonical = raw_path
        .canonicalize()
        .map_err(|e| format!("Cannot resolve target path: {e}"))?;

    is_protected_delete_target(&canonical)?;

    let inspection = inspect_target(&canonical)?;
    let locking_processes = find_locking_processes(&canonical).unwrap_or_default();

    let plan_id = generate_plan_id("fd-plan");
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let expires_at = now_secs + 600;

    Ok(ForceDeletePlanPreview {
        plan_id,
        inspection,
        locking_processes,
        expires_at,
    })
}

/// Prepares an execution plan for killing processes matching an executable path.
pub fn prepare_kill_process_plan(exe_path_str: &str) -> Result<KillProcessPlanPreview, String> {
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::Foundation::CloseHandle;

    let raw_path = PathBuf::from(exe_path_str.trim().trim_matches('"'));
    if !raw_path.exists() {
        return Err(format!("The executable does not exist: {exe_path_str}"));
    }
    let canonical_exe = raw_path
        .canonicalize()
        .map_err(|e| format!("Cannot resolve executable path: {e}"))?;

    let target_key = crate::process::normalize_key(&canonical_exe).to_ascii_lowercase();

    let mut matches = Vec::new();
    let self_pid = std::process::id();

    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if !snap.is_null() && snap != windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

            if Process32FirstW(snap, &mut entry) != 0 {
                loop {
                    let pid = entry.th32ProcessID;
                    if pid > 4 && pid != self_pid {
                        let (name, path) = crate::process::query_process_info(pid);
                        if let Some(ref p) = path {
                            let p_key = crate::process::normalize_key(Path::new(p)).to_ascii_lowercase();
                            if p_key == target_key {
                                // A process whose creation time cannot be read cannot
                                // be bound to a verified plan, so it is not offered.
                                if let Some(creation_time) = crate::process::get_process_creation_time(pid)
                                    .filter(|time| *time != 0)
                                {
                                    let is_critical =
                                        crate::process::is_critical_process(pid, &name, Some(p));
                                    let title = crate::process::find_window_title(pid);

                                    matches.push(LockingProcessInfo {
                                        pid,
                                        name,
                                        path: Some(p.clone()),
                                        window_title: title,
                                        is_critical,
                                        creation_time,
                                    });
                                }
                            }
                        }
                    }

                    if Process32NextW(snap, &mut entry) == 0 {
                        break;
                    }
                }
            }
            CloseHandle(snap);
        }
    }

    let plan_id = generate_plan_id("kp-plan");
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let expires_at = now_secs + 600;

    Ok(KillProcessPlanPreview {
        plan_id,
        exe_path: canonical_exe.to_string_lossy().to_string(),
        matching_processes: matches,
        expires_at,
    })
}

fn is_reparse_entry(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    (meta.file_attributes() & 0x0400) != 0
}

fn remove_dir_contents_safe(
    dir: &Path,
    warnings: &mut Vec<String>,
    deleted_files: &mut u64,
    deleted_dirs: &mut u64,
    deleted_bytes: &mut u64,
) {
    let Ok(entries) = std::fs::read_dir(dir) else { return; };
    for entry in entries.flatten() {
        let path = entry.path();
        if let Ok(meta) = entry.metadata() {
            if is_reparse_entry(&meta) {
                clear_readonly_attribute(&path);
                if std::fs::remove_dir(&path).is_ok() || std::fs::remove_file(&path).is_ok() {
                    *deleted_files += 1;
                } else {
                    warnings.push(format!("Could not remove link: {}", path.display()));
                }
            } else if meta.is_dir() {
                remove_dir_contents_safe(
                    &path,
                    warnings,
                    deleted_files,
                    deleted_dirs,
                    deleted_bytes,
                );
                clear_readonly_attribute(&path);
                if std::fs::remove_dir(&path).is_ok() {
                    *deleted_dirs += 1;
                } else {
                    warnings.push(format!("Could not remove folder: {}", path.display()));
                }
            } else {
                let size = meta.len();
                clear_readonly_attribute(&path);
                if std::fs::remove_file(&path).is_ok() {
                    *deleted_files += 1;
                    *deleted_bytes += size;
                } else {
                    warnings.push(format!("Could not remove file: {}", path.display()));
                }
            }
        }
    }
}

/// Compares volume prefixes so the caller can choose between an atomic rename
/// and a copy-then-delete quarantine fallback.
fn same_volume(left: &Path, right: &Path) -> bool {
    use std::path::Component;
    let prefix = |path: &Path| match path.components().next() {
        Some(Component::Prefix(prefix)) => {
            Some(prefix.as_os_str().to_string_lossy().to_ascii_lowercase())
        }
        _ => None,
    };
    match (prefix(left), prefix(right)) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dest)
        .map_err(|e| format!("Failed to create directory {}: {e}", dest.display()))?;
    let entries = std::fs::read_dir(src)
        .map_err(|e| format!("Failed to read {}: {e}", src.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let target = dest.join(entry.file_name());
        // A link cannot be represented by a plain copy. Refuse the whole
        // cross-volume quarantine instead of dropping the link from the
        // recovered tree and later reporting a successful deletion.
        let meta = std::fs::symlink_metadata(&path)
            .map_err(|e| format!("Metadata error for {}: {e}", path.display()))?;
        if is_reparse_entry(&meta) {
            return Err(format!(
                "{} is a junction or symbolic link. Cross-volume quarantine cannot preserve links; nothing was moved.",
                path.display()
            ));
        }
        if meta.is_dir() {
            copy_dir_recursive(&path, &target)?;
        } else {
            clear_readonly_attribute(&path);
            std::fs::copy(&path, &target)
                .map_err(|e| format!("Copy failed {}: {e}", path.display()))?;
        }
    }
    Ok(())
}

/// Executes a verified force delete plan using either 7-day recoverable quarantine or confirmed permanent deletion.
pub fn execute_force_delete(
    plan: &StoredForceDeletePlan,
    mode: &str,
    quarantine_root: &Path,
) -> Result<ForceDeleteResult, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    if now > plan.expires_at {
        return Err("Execution plan has expired for safety. Please generate a fresh preview.".into());
    }

    if !plan.canonical_path.exists() {
        return Err("Target was already removed or moved before execution could complete.".into());
    }

    let current_canonical = plan
        .canonical_path
        .canonicalize()
        .map_err(|e| format!("Could not re-verify target path: {e}"))?;
    if current_canonical != plan.canonical_path {
        return Err("Target path changed or was replaced since preview was generated. Aborting for safety.".into());
    }

    is_protected_delete_target(&plan.canonical_path)?;

    // Bind execution to the exact object the user reviewed. A pathname alone is
    // a TOCTOU hole: a sync tool, installer or another process can replace the
    // file or folder while the confirmation dialog is open.
    let current_meta = std::fs::symlink_metadata(&plan.canonical_path)
        .map_err(|e| format!("Could not re-read the target before deletion: {e}"))?;
    if is_reparse_entry(&current_meta) {
        return Err(
            "The target became a junction or symbolic link after preview. Nothing was deleted; review it again."
                .into(),
        );
    }
    let expected_identity = plan
        .inspection
        .identity
        .as_ref()
        .ok_or("This plan has no verified file identity. Review and confirm the target again.")?;
    let current_identity = file_identity(&plan.canonical_path)?;
    if &current_identity != expected_identity {
        return Err(
            "The target was replaced or changed after preview. Nothing was deleted; review it again."
                .into(),
        );
    }
    if plan.is_directory {
        let current = inspect_target(&plan.canonical_path)?;
        if current.file_count != plan.inspection.file_count
            || current.dir_count != plan.inspection.dir_count
            || current.size_bytes != plan.inspection.size_bytes
        {
            return Err(
                "The folder contents changed after preview. Nothing was deleted; review the new contents first."
                    .into(),
            );
        }
    } else if current_meta.len() != plan.inspection.size_bytes {
        return Err(
            "The file size changed after preview. Nothing was deleted; review it again.".into(),
        );
    }

    let target_name = plan
        .canonical_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let mut warnings = Vec::new();

    if mode == "quarantine" {
        let batch_id = format!(
            "quarantine-{}-{}",
            now,
            crate::leftovers::generate_crypto_nonce(6)
        );
        let batch_dir = quarantine_root.join(&batch_id);
        std::fs::create_dir_all(&batch_dir)
            .map_err(|e| format!("Failed to create quarantine folder: {e}"))?;

        let dest = batch_dir.join(&target_name);
        let manifest_item = crate::cleanup::ManifestItem {
            original_path: plan.canonical_path.to_string_lossy().to_string(),
            stored_path: dest.to_string_lossy().to_string(),
            size: plan.inspection.size_bytes,
        };
        let journal_path = batch_dir.join(crate::cleanup::JOURNAL_FILE);
        let open_journal = || -> Result<std::fs::File, String> {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&journal_path)
                .map_err(|error| {
                    format!("Could not open the quarantine recovery journal: {error}")
                })
        };

        clear_readonly_attribute(&plan.canonical_path);

        let mut journal = match open_journal() {
            Ok(file) => file,
            Err(error) => {
                let _ = crate::cleanup::remove_entry_no_follow(&batch_dir);
                return Err(error);
            }
        };

        if same_volume(&plan.canonical_path, quarantine_root) {
            // Journal first: the rename is atomic, so a crash leaves either the
            // original untouched or a recorded quarantine entry.
            if let Err(error) =
                crate::cleanup::append_journal_entry_file(&mut journal, &manifest_item)
            {
                drop(journal);
                let _ = crate::cleanup::remove_entry_no_follow(&batch_dir);
                return Err(error);
            }
            if std::fs::rename(&plan.canonical_path, &dest).is_err() {
                // The failed rename changed nothing. Restart the journal so a
                // crash during the copy fallback cannot advertise a partial copy.
                drop(journal);
                let _ = std::fs::remove_file(&journal_path);
                if let Err(error) =
                    copy_into_quarantine(&plan.canonical_path, &dest, plan.is_directory)
                {
                    let _ = crate::cleanup::remove_entry_no_follow(&batch_dir);
                    return Err(error);
                }
                journal = match open_journal() {
                    Ok(file) => file,
                    Err(error) => {
                        let _ = crate::cleanup::remove_entry_no_follow(&batch_dir);
                        return Err(error);
                    }
                };
                if let Err(error) =
                    crate::cleanup::append_journal_entry_file(&mut journal, &manifest_item)
                {
                    drop(journal);
                    let _ = crate::cleanup::remove_entry_no_follow(&batch_dir);
                    return Err(error);
                }
                if let Err(error) = remove_original_after_copy(
                    &plan.canonical_path,
                    plan.is_directory,
                    &mut warnings,
                ) {
                    drop(journal);
                    let _ = crate::cleanup::remove_entry_no_follow(&batch_dir);
                    return Err(error);
                }
            }
        } else {
            // Cross-volume: copy first. The originals stay untouched until the
            // journal records the completed copy.
            if let Err(error) = copy_into_quarantine(
                &plan.canonical_path,
                &dest,
                plan.is_directory,
            ) {
                drop(journal);
                let _ = crate::cleanup::remove_entry_no_follow(&batch_dir);
                return Err(error);
            }
            if let Err(error) =
                crate::cleanup::append_journal_entry_file(&mut journal, &manifest_item)
            {
                drop(journal);
                let _ = crate::cleanup::remove_entry_no_follow(&batch_dir);
                return Err(error);
            }
            if let Err(error) = remove_original_after_copy(
                &plan.canonical_path,
                plan.is_directory,
                &mut warnings,
            ) {
                drop(journal);
                let _ = crate::cleanup::remove_entry_no_follow(&batch_dir);
                return Err(error);
            }
        }
        drop(journal);

        let manifest = crate::cleanup::BatchManifest {
            run_id: batch_id.clone(),
            created_at: now,
            expires_at: now + 7 * 86400,
            target_ids: vec!["force-delete".to_string()],
            items: vec![manifest_item],
            registry_backup: None,
            registry_key_path: None,
            hive: None,
            subkey_name: None,
            app_id: None,
            backup_sha256: None,
            backup_size_bytes: None,
        };
        if let Err(error) = crate::cleanup::write_manifest(&batch_dir, &manifest) {
            warnings.push(format!(
                "The quarantined copy is recorded in the recovery journal, but the manifest could not be finalized: {error}"
            ));
        }

        let success = warnings.is_empty();
        Ok(ForceDeleteResult {
            success,
            mode: "quarantine".into(),
            deleted_files: plan.inspection.file_count,
            deleted_dirs: plan.inspection.dir_count,
            freed_bytes: if success { plan.inspection.size_bytes } else { 0 },
            quarantine_batch_id: Some(batch_id),
            message: if success {
                format!(
                    "Successfully quarantined '{}' (restorable for 7 days in Cleaner Quarantine).",
                    target_name
                )
            } else {
                format!(
                    "Quarantined '{}' with warnings. Review the batch in Cleaner Quarantine before purging.",
                    target_name
                )
            },
            warnings,
        })
    } else if mode == "permanent" {
        let mut deleted_files = 0u64;
        let mut deleted_dirs = 0u64;
        let mut deleted_bytes = 0u64;

        if plan.is_directory {
            remove_dir_contents_safe(
                &plan.canonical_path,
                &mut warnings,
                &mut deleted_files,
                &mut deleted_dirs,
                &mut deleted_bytes,
            );
            clear_readonly_attribute(&plan.canonical_path);
            if std::fs::remove_dir(&plan.canonical_path).is_ok() {
                deleted_dirs += 1;
            } else {
                warnings.push(format!("Could not remove root folder: {}", plan.canonical_path.display()));
            }
        } else {
            let size = std::fs::metadata(&plan.canonical_path)
                .map(|meta| meta.len())
                .unwrap_or(0);
            clear_readonly_attribute(&plan.canonical_path);
            std::fs::remove_file(&plan.canonical_path)
                .map_err(|e| format!("Windows refused to permanently delete file: {e}"))?;
            deleted_files += 1;
            deleted_bytes = size;
        }

        let success = warnings.is_empty();
        Ok(ForceDeleteResult {
            success,
            mode: "permanent".into(),
            deleted_files,
            deleted_dirs,
            freed_bytes: deleted_bytes,
            quarantine_batch_id: None,
            message: if success {
                format!(
                    "Permanently deleted '{}' ({} items freed).",
                    target_name,
                    deleted_files + deleted_dirs
                )
            } else {
                format!(
                    "Permanently deleted part of '{}': {} files and {} folders removed, with {} warning(s).",
                    target_name,
                    deleted_files,
                    deleted_dirs,
                    warnings.len()
                )
            },
            warnings,
        })
    } else {
        Err(format!(
            "Unknown force delete mode '{mode}'. Use quarantine or permanent."
        ))
    }
}

fn copy_into_quarantine(source: &Path, dest: &Path, is_directory: bool) -> Result<(), String> {
    if is_directory {
        copy_dir_recursive(source, dest)
    } else {
        std::fs::copy(source, dest)
            .map(|_| ())
            .map_err(|error| format!("Could not copy file to quarantine: {error}"))
    }
}

fn remove_original_after_copy(
    source: &Path,
    is_directory: bool,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    if is_directory {
        let mut files = 0;
        let mut dirs = 0;
        let mut bytes = 0;
        remove_dir_contents_safe(source, warnings, &mut files, &mut dirs, &mut bytes);
        clear_readonly_attribute(source);
        if let Err(error) = std::fs::remove_dir(source) {
            warnings.push(format!(
                "Could not remove the original folder after copying it to quarantine: {error}"
            ));
        }
        Ok(())
    } else {
        clear_readonly_attribute(source);
        std::fs::remove_file(source).map_err(|error| {
            format!("Could not remove original after copying to quarantine: {error}")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_analyze_flag() {
        let current_dir = std::env::current_dir().unwrap().to_string_lossy().to_string();
        let args = vec![
            "cyron.exe".to_string(),
            "--analyze".to_string(),
            current_dir.clone(),
        ];
        let route = parse_startup_args(&args).expect("Route should be parsed");
        assert_eq!(route.view, "analyzer");
        assert_eq!(route.subview.as_deref(), Some("map"));
        assert_eq!(route.action.as_deref(), Some("scan"));
        assert!(route.path.is_some());
    }

    #[test]
    fn parses_duplicates_flag() {
        let current_dir = std::env::current_dir().unwrap().to_string_lossy().to_string();
        let args = vec![
            "cyron.exe".to_string(),
            "--duplicates".to_string(),
            current_dir,
        ];
        let route = parse_startup_args(&args).expect("Route should be parsed");
        assert_eq!(route.view, "analyzer");
        assert_eq!(route.subview.as_deref(), Some("duplicates"));
    }

    #[test]
    fn parses_storage_flag() {
        let current_dir = std::env::current_dir().unwrap().to_string_lossy().to_string();
        let args = vec![
            "cyron.exe".to_string(),
            "--storage".to_string(),
            current_dir,
        ];
        let route = parse_startup_args(&args).expect("Route should be parsed");
        assert_eq!(route.view, "analyzer");
        assert_eq!(route.subview.as_deref(), Some("storage"));
    }

    #[test]
    fn parses_force_delete_flag() {
        let current_dir = std::env::current_dir().unwrap().to_string_lossy().to_string();
        let args = vec![
            "cyron.exe".to_string(),
            "--force-delete".to_string(),
            current_dir.clone(),
        ];
        let route = parse_startup_args(&args).expect("Route should be parsed");
        assert_eq!(route.view, "overview");
        assert_eq!(route.action.as_deref(), Some("force-delete"));
        assert!(route.path.is_some());
    }

    #[test]
    fn parses_find_locks_flag() {
        let current_dir = std::env::current_dir().unwrap().to_string_lossy().to_string();
        let args = vec![
            "cyron.exe".to_string(),
            "--find-locks".to_string(),
            current_dir.clone(),
        ];
        let route = parse_startup_args(&args).expect("Route should be parsed");
        assert_eq!(route.view, "overview");
        assert_eq!(route.action.as_deref(), Some("find-locks"));
        assert!(route.path.is_some());
    }

    #[test]
    fn parses_kill_process_flag() {
        let current_dir = std::env::current_dir().unwrap().to_string_lossy().to_string();
        let args = vec![
            "cyron.exe".to_string(),
            "--kill-process".to_string(),
            current_dir.clone(),
        ];
        let route = parse_startup_args(&args).expect("Route should be parsed");
        assert_eq!(route.view, "overview");
        assert_eq!(route.action.as_deref(), Some("kill-process"));
        assert!(route.path.is_some());
    }

    #[test]
    fn rejects_protected_windows_and_system_targets() {
        assert!(is_protected_delete_target(Path::new("C:\\Windows")).is_err());
        assert!(is_protected_delete_target(Path::new("C:\\Windows\\System32")).is_err());
        assert!(is_protected_delete_target(Path::new("C:\\")).is_err());
        assert!(is_protected_delete_target(Path::new("C:\\Users")).is_err());
        assert!(is_protected_delete_target(Path::new("C:\\Program Files")).is_err());
        assert!(is_protected_delete_target(Path::new("C:\\ProgramData")).is_err());
        assert!(is_protected_delete_target(Path::new("C:\\pagefile.sys")).is_err());
    }

    #[test]
    fn allows_disposable_temp_target_for_deletion() {
        let temp_dir = std::env::temp_dir().join(format!("cyron-test-safe-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let check = is_protected_delete_target(&temp_dir);
        let _ = std::fs::remove_dir(&temp_dir);
        assert!(check.is_ok());
    }

    #[test]
    fn refuses_appdata_and_temp_roots() {
        for variable in ["APPDATA", "LOCALAPPDATA", "TEMP"] {
            let Ok(value) = std::env::var(variable) else {
                continue;
            };
            assert!(
                is_protected_delete_target(Path::new(&value)).is_err(),
                "{variable} root must be protected"
            );
        }
    }

    #[test]
    fn cross_volume_copy_refuses_a_link_instead_of_dropping_it() {
        let root = disposable_fixture("copy-link");
        let source = root.join("source");
        let target = root.join("target");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(source.join("keep.txt"), b"keep").unwrap();
        let link = source.join("link");
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&target)
            .status()
            .expect("mklink");
        if !status.success() {
            let _ = std::fs::remove_dir_all(&root);
            panic!("could not create a junction fixture");
        }

        let destination = root.join("copy");
        let result = copy_dir_recursive(&source, &destination);
        assert!(result.is_err(), "a link must refuse the copy");
        assert!(source.join("keep.txt").exists(), "source must stay untouched");
        assert!(target.exists(), "link target must stay untouched");
        let _ = std::fs::remove_dir_all(&destination);

        let _ = std::fs::remove_dir(&link);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn force_delete_refuses_a_junction_instead_of_its_target() {
        let root = disposable_fixture("junction");
        let target = root.join("target");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("keep.txt"), b"keep").unwrap();
        let link = root.join("link");
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&target)
            .status()
            .expect("mklink");
        if !status.success() {
            let _ = std::fs::remove_dir_all(&root);
            panic!("could not create a junction fixture");
        }

        let result = prepare_force_delete_plan(&link.to_string_lossy());
        assert!(result.is_err(), "a junction must be refused");
        assert!(target.join("keep.txt").exists(), "the target must stay untouched");

        let _ = std::fs::remove_dir(&link);
        let _ = std::fs::remove_dir_all(&root);
    }

    fn file_plan(target: &Path) -> StoredForceDeletePlan {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        StoredForceDeletePlan {
            plan_id: "fd-file-test".into(),
            target_path: target.to_path_buf(),
            canonical_path: target.canonicalize().unwrap(),
            is_directory: false,
            expires_at: now + 600,
            inspection: inspect_target(target).unwrap(),
            locking_processes: Vec::new(),
        }
    }

    #[test]
    fn force_delete_refuses_a_file_replaced_after_preview() {
        let root = disposable_fixture("replaced-file");
        let target = root.join("reviewed.txt");
        std::fs::write(&target, b"reviewed").unwrap();
        let plan = file_plan(&target);

        std::fs::remove_file(&target).unwrap();
        std::fs::write(&target, b"replacement").unwrap();

        let result = execute_force_delete(&plan, "permanent", &root.join("quarantine"));
        assert!(result.is_err(), "a replaced file must be refused");
        assert_eq!(std::fs::read(&target).unwrap(), b"replacement");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn force_delete_refuses_a_folder_that_gained_content_after_preview() {
        let root = disposable_fixture("changed-folder");
        let target = root.join("reviewed-folder");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("one.txt"), b"one").unwrap();
        let plan = directory_plan(&target);

        std::fs::write(target.join("two.txt"), b"two").unwrap();

        let result = execute_force_delete(&plan, "quarantine", &root.join("quarantine"));
        assert!(result.is_err(), "changed contents must be refused");
        assert!(target.join("one.txt").exists());
        assert!(target.join("two.txt").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    fn disposable_fixture(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "cyron-explorer-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn directory_plan(target: &Path) -> StoredForceDeletePlan {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        StoredForceDeletePlan {
            plan_id: "fd-test".into(),
            target_path: target.to_path_buf(),
            canonical_path: target.canonicalize().unwrap(),
            is_directory: true,
            expires_at: now + 600,
            inspection: inspect_target(target).unwrap(),
            locking_processes: Vec::new(),
        }
    }

    #[test]
    fn quarantine_writes_journal_and_restores_a_folder() {
        let root = disposable_fixture("quarantine-restore");
        let target = root.join("disposable-folder");
        std::fs::create_dir_all(target.join("nested")).unwrap();
        std::fs::write(target.join("nested").join("file.txt"), b"disposable").unwrap();

        let plan = directory_plan(&target);
        let quarantine = root.join("quarantine");
        let report = execute_force_delete(&plan, "quarantine", &quarantine).unwrap();
        let batch_id = report.quarantine_batch_id.clone().unwrap();
        let batch_dir = quarantine.join(&batch_id);

        assert!(!target.exists(), "the target should have moved");
        assert!(report.success, "unexpected warnings: {:?}", report.warnings);
        assert!(batch_dir.join(crate::cleanup::JOURNAL_FILE).exists());
        assert!(batch_dir.join(crate::cleanup::MANIFEST).exists());

        let restore = crate::cleanup::restore_batch(&quarantine, &batch_id).unwrap();
        assert_eq!(restore.restored, 1);
        assert_eq!(restore.failed, 0);
        assert_eq!(
            std::fs::read(target.join("nested").join("file.txt")).unwrap(),
            b"disposable"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn purge_removes_a_quarantined_folder_entry() {
        let root = disposable_fixture("quarantine-purge");
        let target = root.join("disposable-folder");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("file.txt"), b"disposable").unwrap();

        let plan = directory_plan(&target);
        let quarantine = root.join("quarantine");
        let report = execute_force_delete(&plan, "quarantine", &quarantine).unwrap();
        let batch_id = report.quarantine_batch_id.unwrap();

        let purge = crate::cleanup::purge_batch(&quarantine, &batch_id).unwrap();
        assert_eq!(purge.removed_files, 1);
        assert!(!quarantine.join(&batch_id).exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_an_unknown_force_delete_mode() {
        let root = disposable_fixture("unknown-mode");
        let target = root.join("disposable-file.txt");
        std::fs::write(&target, b"disposable").unwrap();
        let plan = directory_plan(&root);

        let result = execute_force_delete(&plan, "everything", &root.join("quarantine"));
        assert!(result.is_err());
        assert!(target.exists(), "an unknown mode must not modify the target");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_missing_or_invalid_paths() {
        let args = vec![
            "cyron.exe".to_string(),
            "--analyze".to_string(),
            r"Z:\NonExistent\Folder\That\Does\Not\Exist\12345".to_string(),
        ];
        let route = parse_startup_args(&args);
        assert!(route.is_none());
    }

    #[test]
    fn handles_empty_and_unknown_flags() {
        let args = vec!["cyron.exe".to_string()];
        assert!(parse_startup_args(&args).is_none());

        let args2 = vec!["cyron.exe".to_string(), "--unknown".to_string(), "foo".to_string()];
        assert!(parse_startup_args(&args2).is_none());
    }
}
