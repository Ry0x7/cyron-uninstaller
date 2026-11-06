//! Stress and failure-path coverage for the Phase 3 cleanup engine.
//!
//! Every fixture lives under the process temp directory and is removed on drop.
//! No real cache location is touched.

use cyron_lib::cleanup::{
    execute_plan, plan_specs, purge_batch, restore_batch, CleanupPlan, CleanupReport, CleanupSpec,
    MAX_PREVIEW_FILES,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/* --------------------------------------------------------------- fixtures */

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "cyron-cleanup-stress-{tag}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create fixture root");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Builds `count` files spread over 50 folders, plus two deliberately deep
/// paths, and returns the exact logical byte total.
fn build_wide_tree(root: &Path, count: usize) -> u64 {
    let mut bytes = 0u64;
    for index in 0..count {
        let folder = root.join(format!("bucket-{:02}", index % 50));
        let path = folder.join(format!("file-{index:05}.bin"));
        fs::create_dir_all(&folder).expect("create bucket");
        let size = (index % 7) as usize + 1;
        fs::write(&path, vec![0x41u8; size]).expect("write fixture file");
        bytes += size as u64;
    }
    let deep = root.join("deep/one/two/three/four");
    fs::create_dir_all(&deep).expect("create deep folder");
    write_file(&deep.join("deep-a.bin"), 11);
    bytes += 11;
    write_file(&root.join("deep/one/two/deep-b.bin"), 13);
    bytes += 13;
    bytes
}

fn write_file(path: &Path, size: usize) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, vec![0x5au8; size]).expect("write file");
}

fn spec(id: &str, path: &Path) -> CleanupSpec {
    CleanupSpec {
        id: id.to_string(),
        tool: "Stress fixture".into(),
        label: "Fixture cache".into(),
        risk: "safe".into(),
        category: "windows".into(),
        note: "Disposable fixture cache used by the cleanup stress suite.".into(),
        impact: "Recreated automatically; nothing outside the fixture is touched.".into(),
        path: path.to_path_buf(),
    }
}

fn count_files(root: &Path) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(meta) = fs::symlink_metadata(entry.path()) else {
                continue;
            };
            if meta.file_type().is_dir() && !is_reparse(&meta) {
                stack.push(entry.path());
            } else if meta.file_type().is_file() {
                total += 1;
            }
        }
    }
    total
}

#[cfg(windows)]
fn is_reparse(meta: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    meta.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse(_meta: &fs::Metadata) -> bool {
    false
}

fn run(plan: &CleanupPlan, quarantine: &Path, cancel: &AtomicBool) -> CleanupReport {
    execute_plan(plan, quarantine, cancel, &|_progress| {}).expect("execution must not hard-fail")
}

fn consistency(report: &CleanupReport, expected_total: u64) {
    let accounted = report.quarantined + report.skipped + report.failed;
    assert!(
        accounted <= expected_total,
        "report claims {accounted} files out of {expected_total}"
    );
}

/* ------------------------------------------------------------------ cases */

#[test]
fn wide_fixture_preview_is_bounded_and_execution_moves_everything() {
    const COUNT: usize = 5_000;
    let fixture = TempDir::new("wide");
    let cache = fixture.path().join("cache");
    let quarantine = fixture.path().join("quarantine");
    fs::create_dir_all(&cache).expect("create cache");
    let expected_bytes = build_wide_tree(&cache, COUNT);
    let expected_files = COUNT as u64 + 2;

    let started = Instant::now();
    let (plan, preview) =
        plan_specs(vec![spec("stress-wide", &cache)], false).expect("plan a fixture");
    assert_eq!(preview.file_count, expected_files, "exact file count");
    assert_eq!(preview.bytes, expected_bytes, "exact byte total");
    assert!(
        preview.files.len() <= MAX_PREVIEW_FILES,
        "preview listed {} files",
        preview.files.len()
    );
    assert_eq!(
        preview.files.len() as u64 + preview.hidden_count,
        preview.file_count,
        "hidden count must explain every unlisted file"
    );
    assert!(preview.truncated, "a 5,000-file target is truncated");

    let cancel = AtomicBool::new(false);
    let report = run(&plan, &quarantine, &cancel);
    let elapsed = started.elapsed();

    assert_eq!(report.quarantined, expected_files, "{report:?}");
    assert_eq!(report.skipped, 0, "{report:?}");
    assert_eq!(report.failed, 0, "{report:?}");
    assert_eq!(report.bytes, expected_bytes, "bytes moved must be exact");
    assert!(report.dirs_removed > 0, "emptied folders are removed");
    assert_eq!(count_files(&cache), 0, "no file may remain in the target");
    assert!(elapsed.as_secs() < 120, "5,000 files took {:?}", elapsed);
    eprintln!(
        "wide: files={} bytes={} listed={} hidden={} quarantined={} dirs_removed={} elapsed_ms={}",
        preview.file_count,
        preview.bytes,
        preview.files.len(),
        preview.hidden_count,
        report.quarantined,
        report.dirs_removed,
        elapsed.as_millis()
    );
}

#[test]
fn pre_cancelled_execution_claims_nothing_it_did_not_move() {
    const COUNT: usize = 300;
    let fixture = TempDir::new("cancel");
    let cache = fixture.path().join("cache");
    let quarantine = fixture.path().join("quarantine");
    fs::create_dir_all(&cache).expect("create cache");
    build_wide_tree(&cache, COUNT);

    let (plan, _preview) =
        plan_specs(vec![spec("stress-cancel", &cache)], false).expect("plan a fixture");
    let cancel = AtomicBool::new(true);
    let report = run(&plan, &quarantine, &cancel);

    assert!(report.cancelled, "a pre-set flag must cancel the run");
    consistency(&report, COUNT as u64 + 2);
    let remaining = count_files(&cache);
    assert_eq!(
        report.quarantined,
        COUNT as u64 + 2 - remaining,
        "quarantined count must match the files actually gone"
    );
    assert_eq!(report.failed, 0, "{report:?}");
    eprintln!(
        "cancel: remaining={} quarantined={} skipped={} failed={}",
        remaining, report.quarantined, report.skipped, report.failed
    );
}

#[test]
fn a_locked_file_is_reported_and_the_rest_still_move() {
    const COUNT: usize = 200;
    let fixture = TempDir::new("locked");
    let cache = fixture.path().join("cache");
    let quarantine = fixture.path().join("quarantine");
    fs::create_dir_all(&cache).expect("create cache");
    build_wide_tree(&cache, COUNT);

    let locked = cache.join("locked.bin");
    write_file(&locked, 64);
    let handle = lock_exclusive(&locked).expect("exclusive handle on the fixture file");

    let (plan, preview) =
        plan_specs(vec![spec("stress-locked", &cache)], false).expect("plan a fixture");
    let expected_total = COUNT as u64 + 3;
    assert_eq!(preview.file_count, expected_total);

    let report = run(&plan, &quarantine, &AtomicBool::new(false));
    consistency(&report, expected_total);
    let refused = report
        .items
        .iter()
        .find(|item| item.path.to_lowercase() == locked.to_string_lossy().to_lowercase());
    let refused = refused.expect("the locked file must be reported");
    assert!(
        refused.status == "skipped" || refused.status == "failed",
        "unexpected status {:?}",
        refused.status
    );
    assert!(
        refused.reason.as_deref().map(|reason| !reason.is_empty()) == Some(true),
        "a refusal needs a reason"
    );
    assert_eq!(
        report.quarantined,
        expected_total - 1,
        "every unlocked file must still move"
    );
    assert!(locked.exists(), "the locked file must stay in place");

    // Restore only what was quarantined; the locked file is already home.
    let restore = restore_batch(&quarantine, &report.run_id).expect("restore the batch");
    assert_eq!(restore.restored, report.quarantined as u64, "{restore:?}");
    assert!(locked.exists(), "restore must not disturb the locked file");
    assert_eq!(
        count_files(&cache),
        expected_total,
        "every fixture file must be back"
    );
    drop(handle);
    eprintln!(
        "locked: quarantined={} refused={} restored={} skipped={} failed={}",
        report.quarantined, refused.path, restore.restored, report.skipped, report.failed
    );
}

#[test]
fn empty_and_missing_targets_plan_and_execute_without_error() {
    let fixture = TempDir::new("empty");
    let empty = fixture.path().join("empty-cache");
    let missing = fixture.path().join("missing-cache");
    fs::create_dir_all(&empty).expect("create empty cache");
    let quarantine = fixture.path().join("quarantine");

    let (plan, preview) = plan_specs(
        vec![
            spec("stress-empty", &empty),
            spec("stress-missing", &missing),
        ],
        false,
    )
    .expect("empty and missing targets are still plannable");
    assert_eq!(preview.file_count, 0);
    assert_eq!(preview.bytes, 0);
    assert!(preview.files.is_empty());

    let report = run(&plan, &quarantine, &AtomicBool::new(false));
    assert_eq!(report.quarantined, 0);
    assert_eq!(report.bytes, 0);
    assert!(
        report.errors.is_empty(),
        "an empty target is not an error: {:?}",
        report.errors
    );
    let batches = cyron_lib::cleanup::list_quarantine(&quarantine);
    assert!(
        batches
            .iter()
            .all(|batch| batch.run_id != report.run_id || batch.file_count == 0),
        "an empty run must not claim stored files"
    );
    eprintln!(
        "empty: files=0 quarantined={} errors={}",
        report.quarantined,
        report.errors.len()
    );
}

#[test]
fn consecutive_batches_stay_independent_through_purge_and_restore() {
    const COUNT: usize = 50;
    let fixture = TempDir::new("batches");
    let cache = fixture.path().join("cache");
    let quarantine = fixture.path().join("quarantine");

    fs::create_dir_all(&cache).expect("create cache");
    build_wide_tree(&cache, COUNT);
    let (first_plan, _) =
        plan_specs(vec![spec("stress-first", &cache)], false).expect("plan first batch");
    let first = run(&first_plan, &quarantine, &AtomicBool::new(false));
    assert_eq!(first.quarantined, COUNT as u64 + 2);
    assert_ne!(first.run_id, String::new());

    fs::create_dir_all(&cache).expect("recreate cache");
    build_wide_tree(&cache, COUNT);
    let (second_plan, _) =
        plan_specs(vec![spec("stress-second", &cache)], false).expect("plan second batch");
    let second = run(&second_plan, &quarantine, &AtomicBool::new(false));
    assert_eq!(second.quarantined, COUNT as u64 + 2);
    assert_ne!(first.run_id, second.run_id, "run ids must be unique");

    let batches = cyron_lib::cleanup::list_quarantine(&quarantine);
    let ids: Vec<&str> = batches.iter().map(|batch| batch.run_id.as_str()).collect();
    assert!(ids.contains(&first.run_id.as_str()), "{ids:?}");
    assert!(ids.contains(&second.run_id.as_str()), "{ids:?}");

    let purged = purge_batch(&quarantine, &first.run_id).expect("purge the first batch");
    assert!(purged.removed_files > 0, "{purged:?}");
    assert_eq!(purged.freed_bytes, first.bytes, "freed bytes must match");
    let remaining: Vec<String> = cyron_lib::cleanup::list_quarantine(&quarantine)
        .into_iter()
        .map(|batch| batch.run_id)
        .collect();
    assert!(
        !remaining.contains(&first.run_id),
        "the purged batch must be gone"
    );
    assert!(
        remaining.contains(&second.run_id),
        "purging one batch must not touch another"
    );

    let restored = restore_batch(&quarantine, &second.run_id).expect("restore the second batch");
    assert_eq!(restored.restored, second.quarantined as u64, "{restored:?}");
    assert_eq!(count_files(&cache), COUNT as u64 + 2);
    eprintln!(
        "batches: first={} second={} purged_files={} freed={} restored={}",
        first.run_id, second.run_id, purged.removed_files, purged.freed_bytes, restored.restored
    );
}

/* ------------------------------------------------------ exclusive fixture handle */

#[cfg(windows)]
fn lock_exclusive(path: &Path) -> std::io::Result<ExclusiveHandle> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateFileW(
            name: *const u16,
            access: u32,
            share: u32,
            security: *const core::ffi::c_void,
            disposition: u32,
            flags: u32,
            template: isize,
        ) -> isize;
    }

    const GENERIC_READ: u32 = 0x8000_0000;
    // Deliberately omit FILE_SHARE_DELETE so a rename or delete is refused.
    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const OPEN_EXISTING: u32 = 3;
    const FILE_ATTRIBUTE_NORMAL: u32 = 0x0080;
    const INVALID_HANDLE_VALUE: isize = -1;

    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            0,
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error());
    }
    Ok(ExclusiveHandle(handle))
}

#[cfg(windows)]
struct ExclusiveHandle(isize);

#[cfg(windows)]
impl Drop for ExclusiveHandle {
    fn drop(&mut self) {
        #[link(name = "kernel32")]
        extern "system" {
            fn CloseHandle(handle: isize) -> i32;
        }
        unsafe {
            CloseHandle(self.0);
        }
    }
}

#[cfg(not(windows))]
fn lock_exclusive(path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::File::open(path)
}
