//! Review-first, exact-set actions for Analyzer selections.
//!
//! This module is deliberately separate from Cleaner quarantine: permanent
//! deletion is explicit, and recycle actions ask the Windows shell to recycle.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const PLAN_TTL_SECS: u64 = 600;
const MAX_ITEMS: usize = 1_000_000;
const MAX_PATHS: usize = 64;
const MAX_PLAN_ITEMS: usize = 200_000;
const MAX_STORED_PLANS: usize = 8;
#[cfg(test)]
static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AnalyzerAction {
    Recycle,
    Permanent,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzerActionTargetPreview {
    pub path: String,
    pub files: u64,
    pub dirs: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzerActionPreview {
    pub plan_id: String,
    pub operation: AnalyzerAction,
    pub targets: Vec<AnalyzerActionTargetPreview>,
    pub total_files: u64,
    pub total_dirs: u64,
    pub total_bytes: u64,
    pub expires_at: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzerActionProgress {
    pub plan_id: String,
    pub completed: u64,
    pub total: u64,
    pub current_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzerActionItemResult {
    pub path: String,
    pub status: String,
    pub bytes: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzerActionReport {
    pub plan_id: String,
    pub operation: AnalyzerAction,
    pub items: Vec<AnalyzerActionItemResult>,
    pub completed_files: u64,
    pub completed_dirs: u64,
    pub completed_bytes: u64,
    pub cancelled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Identity {
    volume: u32,
    index: u64,
    len: u64,
    attrs: u32,
    directory: bool,
    created: u64,
    modified: u64,
}

#[derive(Debug, Clone)]
struct SnapshotItem {
    path: PathBuf,
    identity: Identity,
}

#[derive(Debug, Clone)]
struct TargetPlan {
    root: PathBuf,
    ancestors: Vec<SnapshotItem>,
    items: Vec<SnapshotItem>,
    files: u64,
    dirs: u64,
    bytes: u64,
}

#[derive(Debug, Clone)]
struct StoredPlan {
    id: String,
    operation: AnalyzerAction,
    targets: Vec<TargetPlan>,
    expires_at: u64,
}

#[derive(Default)]
pub struct AnalyzerActionState {
    plans: Mutex<HashMap<String, StoredPlan>>,
    jobs: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

pub fn prepare_analyzer_action(
    state: &AnalyzerActionState,
    paths: Vec<String>,
    operation: AnalyzerAction,
) -> Result<AnalyzerActionPreview, String> {
    if paths.is_empty() {
        return Err("Select at least one path to continue.".into());
    }
    if paths.len() > MAX_PATHS {
        return Err(format!("Select at most {MAX_PATHS} paths at a time."));
    }
    let mut targets = Vec::new();
    let mut keys = HashSet::new();
    for raw in paths {
        let input = PathBuf::from(raw.trim());
        if !input.is_absolute() {
            return Err("Analyzer actions require absolute paths.".into());
        }
        reject_reparse_ancestors(&input)?;
        let root = input
            .canonicalize()
            .map_err(|e| format!("Could not resolve {}: {e}", input.display()))?;
        guard_analyzer_path(&root)?;
        if !keys.insert(path_key(&root)) {
            continue;
        }
        let snapshot = snapshot_tree(&root)?;
        targets.push(snapshot);
    }
    if targets.is_empty() {
        return Err("Select at least one path to continue.".into());
    }
    for i in 0..targets.len() {
        for j in i + 1..targets.len() {
            if targets[i].root.starts_with(&targets[j].root)
                || targets[j].root.starts_with(&targets[i].root)
            {
                return Err(
                    "Overlapping selections are not allowed; select each path only once.".into(),
                );
            }
        }
    }
    let now = now_secs();
    let item_count: usize = targets.iter().map(|t| t.items.len()).sum();
    if item_count > MAX_PLAN_ITEMS {
        return Err(format!(
            "Selection exceeds the {MAX_PLAN_ITEMS} item plan safety limit."
        ));
    }
    let mut plans = state
        .plans
        .lock()
        .map_err(|_| "Analyzer action registry is unavailable.".to_string())?;
    plans.retain(|_, plan| plan.expires_at > now);
    if plans.len() >= MAX_STORED_PLANS {
        return Err(
            "Too many Analyzer previews are pending; execute or discard one and try again.".into(),
        );
    }
    let id = loop {
        let candidate = format!("analyzer-{}", crate::leftovers::generate_crypto_nonce(16));
        if !plans.contains_key(&candidate) {
            break candidate;
        }
    };
    let preview = AnalyzerActionPreview {
        plan_id: id.clone(),
        operation,
        targets: targets
            .iter()
            .map(|t| AnalyzerActionTargetPreview {
                path: t.root.to_string_lossy().into_owned(),
                files: t.files,
                dirs: t.dirs,
                bytes: t.bytes,
            })
            .collect(),
        total_files: targets.iter().map(|t| t.files).sum(),
        total_dirs: targets.iter().map(|t| t.dirs).sum(),
        total_bytes: targets.iter().map(|t| t.bytes).sum(),
        expires_at: now + PLAN_TTL_SECS,
        warnings: if operation == AnalyzerAction::Recycle && targets.iter().any(|t| t.dirs > 0) {
            vec!["Recycle Bin handles files one by one; selected directories are removed only when empty.".into()]
        } else {
            Vec::new()
        },
    };
    let plan = StoredPlan {
        id: id.clone(),
        operation,
        targets,
        expires_at: preview.expires_at,
    };
    plans.insert(id, plan);
    Ok(preview)
}

pub fn cancel_analyzer_action(state: &AnalyzerActionState, plan_id: &str) -> Result<(), String> {
    let jobs = state
        .jobs
        .lock()
        .map_err(|_| "Analyzer action registry is unavailable.".to_string())?;
    let cancel = jobs
        .get(plan_id)
        .ok_or_else(|| "Analyzer action is not running.".to_string())?;
    cancel.store(true, Ordering::Relaxed);
    Ok(())
}

pub fn execute_analyzer_action(
    state: &AnalyzerActionState,
    plan_id: &str,
    progress: impl Fn(AnalyzerActionProgress),
) -> Result<AnalyzerActionReport, String> {
    let plan = {
        let mut plans = state
            .plans
            .lock()
            .map_err(|_| "Analyzer action registry is unavailable.".to_string())?;
        plans.remove(plan_id).ok_or_else(|| {
            "Analyzer action plan is missing, expired, or already used.".to_string()
        })?
    };
    if plan.expires_at <= now_secs() {
        return Err("Analyzer action plan expired; preview the selection again.".into());
    }
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut jobs = state
            .jobs
            .lock()
            .map_err(|_| "Analyzer action registry is unavailable.".to_string())?;
        jobs.insert(plan.id.clone(), Arc::clone(&cancel));
    }
    let result = execute_plan(&plan, &cancel, &progress);
    if let Ok(mut jobs) = state.jobs.lock() {
        jobs.remove(&plan.id);
    }
    result
}

fn execute_plan(
    plan: &StoredPlan,
    cancel: &AtomicBool,
    progress: &impl Fn(AnalyzerActionProgress),
) -> Result<AnalyzerActionReport, String> {
    let total = plan.targets.iter().map(|t| t.items.len() as u64).sum();
    let mut items = Vec::with_capacity(total as usize);
    let mut completed_files = 0;
    let mut completed_dirs = 0;
    let mut completed_bytes = 0;
    let mut seen = 0;
    let mut cancelled = false;
    // Validate every selected tree before the first mutation so a stale later
    // selection cannot turn a multi-path action into an unreported partial run.
    for target in &plan.targets {
        if let Err(reason) = verify_snapshot(target) {
            let failed_items = plan
                .targets
                .iter()
                .flat_map(|target| target.items.iter())
                .map(|item| AnalyzerActionItemResult {
                    path: item.path.to_string_lossy().into_owned(),
                    status: "failed".into(),
                    bytes: 0,
                    error: Some(format!(
                        "Action aborted before changes because {}: {reason}",
                        target.root.display()
                    )),
                })
                .collect();
            return Ok(AnalyzerActionReport {
                plan_id: plan.id.clone(),
                operation: plan.operation,
                items: failed_items,
                completed_files: 0,
                completed_dirs: 0,
                completed_bytes: 0,
                cancelled: false,
            });
        }
    }
    for target in &plan.targets {
        // Files first, then deepest directories; each directory removal is empty-only.
        let mut ordered = target.items.clone();
        ordered.sort_by(|a, b| {
            a.identity
                .directory
                .cmp(&b.identity.directory)
                .then_with(|| {
                    if a.identity.directory {
                        b.path
                            .components()
                            .count()
                            .cmp(&a.path.components().count())
                    } else {
                        a.path.cmp(&b.path)
                    }
                })
        });
        for item in ordered {
            if cancel.load(Ordering::Relaxed) {
                cancelled = true;
                break;
            }
            seen += 1;
            progress(AnalyzerActionProgress {
                plan_id: plan.id.clone(),
                completed: seen - 1,
                total,
                current_path: item.path.to_string_lossy().into_owned(),
            });
            let result = match verify_item(&item) {
                Err(e) => Err(e),
                Ok(()) => match plan.operation {
                    AnalyzerAction::Permanent => permanent_remove(target, &item),
                    AnalyzerAction::Recycle if item.identity.directory => {
                        permanent_remove(target, &item)
                    }
                    AnalyzerAction::Recycle => recycle_one(target, &item),
                },
            };
            match result {
                Ok(()) => {
                    if item.identity.directory {
                        completed_dirs += 1;
                    } else {
                        completed_files += 1;
                        completed_bytes += item.identity.len;
                    }
                    items.push(AnalyzerActionItemResult {
                        path: item.path.to_string_lossy().into_owned(),
                        status: "completed".into(),
                        bytes: if item.identity.directory {
                            0
                        } else {
                            item.identity.len
                        },
                        error: None,
                    });
                }
                Err(error) => items.push(AnalyzerActionItemResult {
                    path: item.path.to_string_lossy().into_owned(),
                    status: "failed".into(),
                    bytes: 0,
                    error: Some(error),
                }),
            }
            progress(AnalyzerActionProgress {
                plan_id: plan.id.clone(),
                completed: seen,
                total,
                current_path: item.path.to_string_lossy().into_owned(),
            });
        }
        if cancelled {
            break;
        }
    }
    if cancelled {
        let done: HashSet<String> = items.iter().map(|i| path_key(Path::new(&i.path))).collect();
        for target in &plan.targets {
            for item in &target.items {
                if !done.contains(&path_key(&item.path)) {
                    items.push(AnalyzerActionItemResult {
                        path: item.path.to_string_lossy().into_owned(),
                        status: "cancelled".into(),
                        bytes: 0,
                        error: None,
                    });
                }
            }
        }
    }
    Ok(AnalyzerActionReport {
        plan_id: plan.id.clone(),
        operation: plan.operation,
        items,
        completed_files,
        completed_dirs,
        completed_bytes,
        cancelled,
    })
}

fn snapshot_tree(root: &Path) -> Result<TargetPlan, String> {
    let mut items = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    let mut files = 0;
    let mut dirs = 0;
    let mut bytes: u64 = 0;
    while let Some(path) = stack.pop() {
        let meta = fs::symlink_metadata(&path)
            .map_err(|e| format!("Could not inspect {}: {e}", path.display()))?;
        if is_reparse(&meta) {
            return Err(format!(
                "Reparse points are not allowed in an Analyzer action: {}",
                path.display()
            ));
        }
        let identity = identity_path(&path, &meta)?;
        guard_analyzer_path(&path)?;
        if identity.directory {
            dirs += 1;
            let entries = fs::read_dir(&path)
                .map_err(|e| format!("Could not list {}: {e}", path.display()))?;
            for entry in entries {
                stack.push(
                    entry
                        .map_err(|e| format!("Could not read directory entry: {e}"))?
                        .path(),
                );
            }
        } else {
            files += 1;
            bytes = bytes.saturating_add(identity.len);
        }
        items.push(SnapshotItem { path, identity });
        if items.len() > MAX_ITEMS {
            return Err(format!(
                "Selection exceeds the {MAX_ITEMS} item safety limit."
            ));
        }
    }
    let mut parents = Vec::new();
    let mut parent = root.parent();
    while let Some(path) = parent {
        let meta = fs::symlink_metadata(path)
            .map_err(|e| format!("Could not inspect parent {}: {e}", path.display()))?;
        if is_reparse(&meta) {
            return Err(format!(
                "Reparse point in path is not allowed: {}",
                path.display()
            ));
        }
        parents.push(SnapshotItem {
            path: path.to_path_buf(),
            identity: identity_path(path, &meta)?,
        });
        parent = path.parent();
    }
    Ok(TargetPlan {
        root: root.to_path_buf(),
        ancestors: parents,
        items,
        files,
        dirs,
        bytes,
    })
}

fn verify_snapshot(target: &TargetPlan) -> Result<(), String> {
    let fresh = snapshot_tree(&target.root)?;
    if fresh.items.len() != target.items.len() {
        return Err("the approved descendant set changed".into());
    }
    let approved: HashMap<String, &Identity> = target
        .items
        .iter()
        .map(|i| (path_key(&i.path), &i.identity))
        .collect();
    for item in &fresh.items {
        match approved.get(&path_key(&item.path)) {
            Some(identity) if **identity == item.identity => (),
            Some(_) => return Err(format!("{} was replaced or changed", item.path.display())),
            None => return Err(format!("{} was added after preview", item.path.display())),
        }
    }
    Ok(())
}

fn verify_item(item: &SnapshotItem) -> Result<(), String> {
    reject_reparse_ancestors(&item.path)?;
    let meta =
        fs::symlink_metadata(&item.path).map_err(|e| format!("Could not recheck target: {e}"))?;
    if is_reparse(&meta)
        || !identity_matches(&identity_path(&item.path, &meta)?, &item.identity, true)
    {
        return Err("target was replaced or changed after preview".into());
    }
    Ok(())
}

fn permanent_remove(target: &TargetPlan, item: &SnapshotItem) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, SetFileInformationByHandle, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT, OPEN_EXISTING,
    };
    const FILE_READ_ATTRIBUTES: u32 = 0x80;
    const DELETE_ACCESS: u32 = 0x0001_0000;
    const FILE_SHARE_READ: u32 = 1;
    const FILE_DISPOSITION_INFO: i32 = 4;
    #[repr(C)]
    struct Disposition {
        delete_file: i32,
    }
    struct Held(HANDLE);
    impl Drop for Held {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    fn open(path: &Path, access: u32, directory: bool) -> Result<Held, String> {
        let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        wide.push(0);
        let flags = FILE_FLAG_OPEN_REPARSE_POINT
            | if directory {
                FILE_FLAG_BACKUP_SEMANTICS
            } else {
                0
            };
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                access,
                FILE_SHARE_READ,
                std::ptr::null(),
                OPEN_EXISTING,
                flags,
                std::ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(format!(
                "Could not safely open {} (Windows error {}).",
                path.display(),
                std::io::Error::last_os_error()
            ));
        }
        Ok(Held(handle))
    }
    fn same(handle: HANDLE, expected: &Identity) -> Result<bool, String> {
        let actual = handle_identity(handle)?;
        Ok(identity_matches(&actual, expected, false))
    }
    // Holding each approved ancestor without FILE_SHARE_DELETE prevents it from
    // being renamed while the exact leaf handle is marked for deletion.
    let mut held = Vec::new();
    let mut ancestors = item.path.ancestors().skip(1).collect::<Vec<_>>();
    ancestors.reverse();
    for ancestor in ancestors {
        if let Ok(meta) = fs::symlink_metadata(ancestor) {
            if is_reparse(&meta) {
                return Err("A reparse point appeared in the target path.".into());
            }
            let handle = open(ancestor, FILE_READ_ATTRIBUTES, true)?;
            let actual = handle_identity(handle.0)?;
            // Ancestors outside the approved tree were snapshotted when the plan was made.
            let expected = target
                .items
                .iter()
                .find(|candidate| candidate.path == ancestor)
                .map(|candidate| &candidate.identity)
                .or_else(|| {
                    target
                        .ancestors
                        .iter()
                        .find(|candidate| candidate.path == ancestor)
                        .map(|candidate| &candidate.identity)
                });
            if expected.is_some_and(|expected| !identity_matches(&actual, expected, false)) {
                return Err("A target ancestor was replaced after preview.".into());
            }
            held.push(handle);
        } else {
            return Err("A target ancestor disappeared after preview.".into());
        }
    }
    let handle = open(
        &item.path,
        DELETE_ACCESS | FILE_READ_ATTRIBUTES,
        item.identity.directory,
    )?;
    if !same(handle.0, &item.identity)? {
        return Err("Target was replaced or changed after preview.".into());
    }
    let disposition = Disposition { delete_file: 1 };
    if unsafe {
        SetFileInformationByHandle(
            handle.0,
            FILE_DISPOSITION_INFO,
            &disposition as *const _ as *const _,
            std::mem::size_of::<Disposition>() as u32,
        )
    } == 0
    {
        return Err(format!(
            "Windows refused the permanent removal: {}",
            std::io::Error::last_os_error()
        ));
    }
    drop(handle);
    drop(held);
    Ok(())
}

fn handle_identity(handle: windows_sys::Win32::Foundation::HANDLE) -> Result<Identity, String> {
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    if unsafe { GetFileInformationByHandle(handle, &mut info) } == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(Identity {
        volume: info.dwVolumeSerialNumber,
        index: ((info.nFileIndexHigh as u64) << 32) | info.nFileIndexLow as u64,
        len: ((info.nFileSizeHigh as u64) << 32) | info.nFileSizeLow as u64,
        attrs: info.dwFileAttributes,
        directory: info.dwFileAttributes & 0x10 != 0,
        created: ((info.ftCreationTime.dwHighDateTime as u64) << 32)
            | info.ftCreationTime.dwLowDateTime as u64,
        modified: ((info.ftLastWriteTime.dwHighDateTime as u64) << 32)
            | info.ftLastWriteTime.dwLowDateTime as u64,
    })
}

fn identity_path(path: &Path, meta: &fs::Metadata) -> Result<Identity, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, OPEN_EXISTING,
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
            FILE_FLAG_OPEN_REPARSE_POINT
                | if meta.is_dir() {
                    FILE_FLAG_BACKUP_SEMANTICS
                } else {
                    0
                },
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(format!(
            "Could not safely inspect {}: {}",
            path.display(),
            std::io::Error::last_os_error()
        ));
    }
    let identity = handle_identity(handle);
    unsafe {
        CloseHandle(handle);
    }
    identity
}

fn recycle_one(target: &TargetPlan, item: &SnapshotItem) -> Result<(), String> {
    if item.identity.directory {
        return permanent_remove(target, item);
    }
    // Shell delete is per-file, with recursion and connected-item expansion disabled.
    // The final pathname call has a narrow rename race that Windows exposes no
    // atomic handle-to-Recycle-Bin operation to close; identity is rechecked directly before it.
    verify_item(item)?;
    with_locked_ancestors(target, item, || {
        verify_item(item)?;
        recycle_shell(item)
    })
}

fn with_locked_ancestors<T>(
    target: &TargetPlan,
    item: &SnapshotItem,
    action: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, OPEN_EXISTING,
    };
    const FILE_READ_ATTRIBUTES: u32 = 0x80;
    const FILE_SHARE_READ: u32 = 1;
    struct Held(windows_sys::Win32::Foundation::HANDLE);
    impl Drop for Held {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    let mut paths = item.path.ancestors().skip(1).collect::<Vec<_>>();
    paths.reverse();
    let mut held = Vec::new();
    for path in paths {
        let meta =
            fs::symlink_metadata(path).map_err(|e| format!("Could not recheck parent: {e}"))?;
        if is_reparse(&meta) {
            return Err("A reparse point appeared in the target path.".into());
        }
        let expected = target
            .items
            .iter()
            .find(|i| i.path == path)
            .map(|i| &i.identity)
            .or_else(|| {
                target
                    .ancestors
                    .iter()
                    .find(|i| i.path == path)
                    .map(|i| &i.identity)
            })
            .ok_or("Target ancestor was not in the reviewed snapshot.")?;
        let mut w: Vec<u16> = path.as_os_str().encode_wide().collect();
        w.push(0);
        let h = unsafe {
            CreateFileW(
                w.as_ptr(),
                FILE_READ_ATTRIBUTES,
                FILE_SHARE_READ,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS,
                std::ptr::null_mut(),
            )
        };
        if h == INVALID_HANDLE_VALUE {
            return Err(format!(
                "Could not hold parent {} safely: {}",
                path.display(),
                std::io::Error::last_os_error()
            ));
        }
        let h = Held(h);
        if !identity_matches(&handle_identity(h.0)?, expected, false) {
            return Err("Target ancestor changed after preview.".into());
        }
        held.push(h);
    }
    action()
}

fn recycle_shell(item: &SnapshotItem) -> Result<(), String> {
    use core::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Guid {
        d1: u32,
        d2: u16,
        d3: u16,
        d4: [u8; 8],
    }
    #[link(name = "ole32")]
    extern "system" {
        fn CoInitializeEx(r: *mut c_void, f: u32) -> i32;
        fn CoUninitialize();
        fn CoCreateInstance(
            c: *const Guid,
            o: *mut c_void,
            ctx: u32,
            i: *const Guid,
            out: *mut *mut c_void,
        ) -> i32;
    }
    #[link(name = "shell32")]
    extern "system" {
        fn SHCreateItemFromParsingName(
            p: *const u16,
            b: *mut c_void,
            i: *const Guid,
            out: *mut *mut c_void,
        ) -> i32;
    }
    const CLSID: Guid = Guid {
        d1: 0x3ad05575,
        d2: 0x8857,
        d3: 0x4850,
        d4: [0x92, 0x77, 0x11, 0xb8, 0x5b, 0xdb, 0x8e, 9],
    };
    const IID_OP: Guid = Guid {
        d1: 0x947aab5f,
        d2: 0x0a5c,
        d3: 0x4c13,
        d4: [0xb4, 0xd6, 0x4b, 0xf7, 0x83, 0x6f, 0xc9, 0xf8],
    };
    const IID_ITEM: Guid = Guid {
        d1: 0x43826d1e,
        d2: 0xe718,
        d3: 0x42ee,
        d4: [0xbc, 0x55, 0xa1, 0xe2, 0x61, 0xc3, 0x7b, 0xfe],
    };
    unsafe fn method(o: *mut c_void, n: usize) -> *const c_void {
        *(*(o as *mut *mut *mut c_void)).add(n)
    }
    struct Com(*mut c_void);
    impl Drop for Com {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    let f: unsafe extern "system" fn(*mut c_void) -> u32 =
                        std::mem::transmute(method(self.0, 2));
                    f(self.0);
                }
            }
        }
    }
    let init = unsafe { CoInitializeEx(std::ptr::null_mut(), 2) };
    if init < 0 {
        return Err("Windows shell recycle service is unavailable in this thread.".into());
    }
    struct Uninit(bool);
    impl Drop for Uninit {
        fn drop(&mut self) {
            if self.0 {
                unsafe {
                    CoUninitialize();
                }
            }
        }
    }
    let _uninit = Uninit(init == 0 || init == 1);
    let mut op = std::ptr::null_mut();
    let hr = unsafe { CoCreateInstance(&CLSID, std::ptr::null_mut(), 1, &IID_OP, &mut op) };
    if hr < 0 {
        return Err(format!(
            "Recycle Bin is unavailable (HRESULT 0x{:08X}).",
            hr as u32
        ));
    }
    let op = Com(op);
    // Shell parsing rejects the Win32 verbatim drive prefix from canonicalize().
    let shell_path = PathBuf::from(crate::scan::display_path(&item.path.to_string_lossy()));
    let mut p: Vec<u16> = shell_path.as_os_str().encode_wide().collect();
    p.push(0);
    let mut shell = std::ptr::null_mut();
    let hr = unsafe {
        SHCreateItemFromParsingName(p.as_ptr(), std::ptr::null_mut(), &IID_ITEM, &mut shell)
    };
    if hr < 0 {
        return Err(format!(
            "Could not prepare item for recycling (HRESULT 0x{:08X}).",
            hr as u32
        ));
    }
    let shell = Com(shell);
    let set: unsafe extern "system" fn(*mut c_void, u32) -> i32 =
        unsafe { std::mem::transmute(method(op.0, 5)) };
    let del: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void) -> i32 =
        unsafe { std::mem::transmute(method(op.0, 18)) };
    let perform: unsafe extern "system" fn(*mut c_void) -> i32 =
        unsafe { std::mem::transmute(method(op.0, 21)) };
    let aborted: unsafe extern "system" fn(*mut c_void, *mut i32) -> i32 =
        unsafe { std::mem::transmute(method(op.0, 22)) };
    let hr = unsafe {
        set(
            op.0,
            0x0008_0000 | 0x0010_0000 | 0x1000 | 0x2000 | 0x10 | 0x400 | 0x4,
        )
    };
    if hr < 0 {
        return Err(format!(
            "Could not set recycle-only shell flags (HRESULT 0x{:08X}); the file was not changed.",
            hr as u32
        ));
    }
    let hr = unsafe { del(op.0, shell.0, std::ptr::null_mut()) };
    if hr < 0 {
        return Err(format!("Windows could not queue this file for recycling (HRESULT 0x{:08X}); it was not permanently deleted.",hr as u32));
    }
    let hr = unsafe { perform(op.0) };
    if hr < 0 {
        return Err(format!("Windows could not recycle this file (HRESULT 0x{:08X}); it was not permanently deleted.",hr as u32));
    }
    let mut stopped = 0;
    let hr = unsafe { aborted(op.0, &mut stopped) };
    if hr < 0 || stopped != 0 {
        return Err(
            "Windows did not complete recycling this file; it was not permanently deleted.".into(),
        );
    }
    if item.path.exists() {
        return Err(
            "Windows reported success but left the file in place; it was not counted as recycled."
                .into(),
        );
    }
    Ok(())
}

fn reject_reparse_ancestors(path: &Path) -> Result<(), String> {
    let abs = if path.is_absolute() {
        path
    } else {
        return Err("Analyzer actions require absolute paths.".into());
    };
    let mut current = PathBuf::new();
    for component in abs.components() {
        current.push(component.as_os_str());
        if let Ok(meta) = fs::symlink_metadata(&current) {
            if is_reparse(&meta) {
                return Err(format!(
                    "Reparse point in path is not allowed: {}",
                    current.display()
                ));
            }
        }
    }
    Ok(())
}

fn is_reparse(meta: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    meta.file_attributes() & 0x400 != 0
}

fn path_key(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}
fn identity_matches(actual: &Identity, expected: &Identity, strict_directory: bool) -> bool {
    actual.volume == expected.volume
        && actual.index == expected.index
        && actual.attrs == expected.attrs
        && actual.directory == expected.directory
        && (actual.directory && !strict_directory
            || actual.len == expected.len && actual.modified == expected.modified)
        && actual.created == expected.created
}
fn guard_analyzer_path(path: &Path) -> Result<(), String> {
    crate::explorer::is_protected_delete_target(path)?;
    let components: Vec<String> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_ascii_lowercase())
        .collect();
    if components
        .iter()
        .any(|c| matches!(c.as_str(), "$recycle.bin" | "system volume information"))
    {
        return Err("Recycle Bin and System Volume Information trees are protected.".into());
    }
    let normalized = path_key(path);
    let normalized = normalized.strip_prefix(r"\\?\").unwrap_or(&normalized);
    let parts: Vec<&str> = normalized
        .split('\\')
        .filter(|part| !part.is_empty())
        .collect();
    if parts
        .get(1)
        .is_some_and(|part| matches!(*part, "efi" | "recovery" | "boot"))
    {
        return Err("Boot and recovery stores are protected.".into());
    }
    Ok(())
}
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "cyron-analyzer-{}-{}",
                std::process::id(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&p).unwrap();
            Self(p)
        }
        fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let p = self.0.join(name);
            fs::write(&p, bytes).unwrap();
            p
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn plans_expire_and_are_single_use() {
        let f = Fixture::new();
        f.file("a", b"x");
        let state = AnalyzerActionState::default();
        let p = prepare_analyzer_action(
            &state,
            vec![f.0.to_string_lossy().into_owned()],
            AnalyzerAction::Permanent,
        )
        .unwrap();
        let mut stored = state.plans.lock().unwrap().remove(&p.plan_id).unwrap();
        stored.expires_at = 0;
        state
            .plans
            .lock()
            .unwrap()
            .insert(p.plan_id.clone(), stored);
        assert!(execute_analyzer_action(&state, &p.plan_id, |_| {}).is_err());
        assert!(execute_analyzer_action(&state, &p.plan_id, |_| {}).is_err());
    }
    #[test]
    fn rejects_added_and_replaced_descendants() {
        let f = Fixture::new();
        f.file("a", b"x");
        let state = AnalyzerActionState::default();
        let p = prepare_analyzer_action(
            &state,
            vec![f.0.to_string_lossy().into_owned()],
            AnalyzerAction::Permanent,
        )
        .unwrap();
        f.file("b", b"new");
        let report = execute_analyzer_action(&state, &p.plan_id, |_| {}).unwrap();
        assert!(report.items.iter().all(|i| i.status == "failed"));
        assert!(f.0.join("a").exists());
        let p = prepare_analyzer_action(
            &state,
            vec![f.0.to_string_lossy().into_owned()],
            AnalyzerAction::Permanent,
        )
        .unwrap();
        fs::write(f.0.join("a"), b"y").unwrap(); // same length; identity still changes by last-write time.
        let report = execute_analyzer_action(&state, &p.plan_id, |_| {}).unwrap();
        assert!(report.items.iter().all(|i| i.status == "failed"));
        assert!(f.0.join("a").exists());
    }
    #[test]
    fn rejects_protected_and_reparse_targets() {
        assert!(crate::explorer::is_protected_delete_target(Path::new("C:\\Windows")).is_err());
        assert!(guard_analyzer_path(Path::new(r"C:\$Recycle.Bin\S-1\item.bin")).is_err());
        assert!(guard_analyzer_path(Path::new(r"C:\EFI\Boot\bootmgfw.efi")).is_err());
        let f = Fixture::new();
        let target = f.0.join("target");
        fs::create_dir(&target).unwrap();
        let link = f.0.join("link");
        let quoted = |path: &Path| format!("'{}'", path.to_string_lossy().replace('\'', "''"));
        let script = format!(
            "New-Item -ItemType Junction -Path {} -Target {} | Out-Null",
            quoted(&link),
            quoted(&target)
        );
        assert!(
            std::process::Command::new("powershell.exe")
                .args(["-NoProfile", "-Command", &script])
                .status()
                .expect("PowerShell for disposable junction fixture")
                .success(),
            "junction fixture could not be created"
        );
        let state = AnalyzerActionState::default();
        assert!(prepare_analyzer_action(
            &state,
            vec![link.to_string_lossy().into_owned()],
            AnalyzerAction::Permanent
        )
        .is_err());
        assert!(prepare_analyzer_action(
            &state,
            vec![link.join("child").to_string_lossy().into_owned()],
            AnalyzerAction::Permanent
        )
        .is_err());
    }
    #[test]
    fn reports_partial_failure_and_recycle_failure_without_fallback() {
        let f = Fixture::new();
        let first = f.file("a", b"one");
        let second = f.file("b", b"two");
        let state = AnalyzerActionState::default();
        let p = prepare_analyzer_action(
            &state,
            vec![
                first.to_string_lossy().into_owned(),
                second.to_string_lossy().into_owned(),
            ],
            AnalyzerAction::Permanent,
        )
        .unwrap();
        let plan = state.plans.lock().unwrap().remove(&p.plan_id).unwrap();
        let changed = AtomicBool::new(false);
        let result = execute_plan(&plan, &AtomicBool::new(false), &|p| {
            if p.completed == 0
                && changed
                    .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
                    .is_ok()
            {
                fs::remove_file(&second).unwrap();
            }
        })
        .unwrap();
        assert_eq!(
            result.items.iter().filter(|i| i.status == "failed").count(),
            1
        );
        assert_eq!(result.completed_files, 1);
        let path = f.file("recycle", b"still here");
        let meta = fs::metadata(&path).unwrap();
        let item = SnapshotItem {
            path: path.clone(),
            identity: identity_path(&path, &meta).unwrap(),
        };
        fs::write(&path, b"changed").unwrap();
        assert!(recycle_one(&plan.targets[0], &item).is_err());
        assert!(
            path.exists(),
            "a rejected recycle does not fall back to permanent deletion"
        );
    }
}
