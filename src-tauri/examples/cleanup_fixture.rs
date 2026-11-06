//! End-to-end verification of the cleanup engine against an isolated fixture.
//!
//! Everything this binary touches lives under `%TEMP%\cyron-cleanup-<pid>\`.
//! It never references a real cache location, the user profile or a drive root.
//! `scripts/verify-cleanup.mjs` runs it and requires the final marker.
//!
//! Covered: preview accuracy, quarantine execution, outside-file safety,
//! manifest presence, batch listing, byte-identical restore, purge accounting,
//! a sharing-violation lock that must not abort the run, and read-only refusal.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use cyron_lib::cleanup::{
    execute_plan, list_quarantine, plan_specs, purge_batch, restore_batch, CleanupProgress,
    CleanupSpec,
};
use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_READ, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{CreateFileW, OPEN_EXISTING};

const MARKER: &str = "CLEANUP_FIXTURE_OK";

fn fail(message: &str, fixture: &Path) -> ! {
    eprintln!("cleanup_fixture: FAILED: {message}");
    eprintln!("cleanup_fixture: fixture kept at {}", fixture.display());
    std::process::exit(1);
}

fn spec(id: &str, path: &Path) -> CleanupSpec {
    CleanupSpec {
        id: id.to_string(),
        tool: "Cyron fixture".into(),
        label: "Fixture cache".into(),
        risk: "safe".into(),
        category: "windows".into(),
        note: "Isolated fixture used only by the verification binary.".into(),
        impact: "Nothing real is affected; the fixture is disposable.".into(),
        path: path.to_path_buf(),
    }
}

fn write(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create fixture parent");
    }
    fs::write(path, bytes).expect("write fixture file");
}

/// Opens a handle with no sharing so a rename of the file fails with a sharing
/// violation, the way a running application holds its own cache file.
fn lock_file(path: &Path) -> HANDLE {
    use std::os::windows::ffi::OsStrExt;
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            GENERIC_READ,
            0,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        fail(
            &format!("could not open a no-share handle for {}", path.display()),
            path.parent().unwrap_or(path),
        );
    }
    handle
}

fn main() {
    let root = std::env::temp_dir().join(format!("cyron-cleanup-{}", std::process::id()));
    if root.exists() {
        let _ = fs::remove_dir_all(&root);
    }
    fs::create_dir_all(&root).expect("create fixture root");
    let cache = root.join("cache");
    let cache2 = root.join("cache2");
    let outside = root.join("outside");
    let quarantine = root.join("quarantine");

    // Deterministic content: four files, 2_155 bytes in total.
    write(&cache.join("a.txt"), &vec![b'a'; 100]);
    write(&cache.join("sub").join("b.bin"), &vec![b'b'; 2048]);
    write(
        &cache.join("sub").join("deep").join("c.txt"),
        &vec![b'c'; 7],
    );
    write(&cache.join("empty.dat"), b"");
    write(&outside.join("keep.txt"), b"do not touch");

    let expected_bytes: u64 = 100 + 2048 + 7;
    let expected_files: u64 = 4;

    /* ------------------------------------------------ 1. preview and execute */

    let (plan, preview) = match plan_specs(vec![spec("fixture-cache", &cache)], false) {
        Ok(value) => value,
        Err(error) => fail(&format!("plan failed: {error}"), &root),
    };
    if preview.plan_id.is_empty() {
        fail("preview returned an empty plan id", &root);
    }
    if preview.file_count != expected_files {
        fail(
            &format!(
                "preview counted {} files, expected {expected_files}",
                preview.file_count
            ),
            &root,
        );
    }
    if preview.bytes != expected_bytes {
        fail(
            &format!(
                "preview measured {} bytes, expected {expected_bytes}",
                preview.bytes
            ),
            &root,
        );
    }
    if preview.read_only {
        fail(
            "preview reports read-only although the plan was writable",
            &root,
        );
    }
    for file in &preview.files {
        if !Path::new(&file.path).starts_with(&cache) {
            fail(
                &format!("preview listed a path outside the target: {}", file.path),
                &root,
            );
        }
    }

    let cancel = AtomicBool::new(false);
    let no_progress = |_progress: CleanupProgress| {};
    let report = match execute_plan(&plan, &quarantine, &cancel, &no_progress) {
        Ok(report) => report,
        Err(error) => fail(&format!("execution failed: {error}"), &root),
    };
    if report.quarantined != expected_files {
        fail(
            &format!(
                "quarantined {} files, expected {expected_files} (skipped {}, failed {})",
                report.quarantined, report.skipped, report.failed
            ),
            &root,
        );
    }
    if report.bytes != expected_bytes {
        fail(
            &format!(
                "report accounted {} bytes, expected {expected_bytes}",
                report.bytes
            ),
            &root,
        );
    }
    for name in ["a.txt", "sub\\b.bin", "sub\\deep\\c.txt", "empty.dat"] {
        if cache.join(name).exists() {
            fail(&format!("{name} was not moved out of the cache"), &root);
        }
    }
    if fs::read(outside.join("keep.txt")).ok().as_deref() != Some(b"do not touch".as_slice()) {
        fail("a file outside the target was modified", &root);
    }
    let batch_dir = report
        .quarantine_dir
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| fail("report did not name a quarantine directory", &root));
    if !batch_dir.is_dir() {
        fail("the quarantine directory does not exist", &root);
    }
    let manifest = batch_dir.join("manifest.json");
    let manifest_text = fs::read_to_string(&manifest)
        .unwrap_or_else(|error| fail(&format!("manifest is unreadable: {error}"), &root));
    if !manifest_text.contains(&report.run_id) || !manifest_text.contains("items") {
        fail("manifest does not record the run or its items", &root);
    }

    /* ------------------------------------------------------ 2. list and restore */

    let batches = list_quarantine(&quarantine);
    let batch = match batches.iter().find(|batch| batch.run_id == report.run_id) {
        Some(batch) => batch,
        None => fail("list_quarantine did not report the new batch", &root),
    };
    if batch.bytes != expected_bytes || batch.file_count != expected_files {
        fail(
            &format!(
                "batch totals are {} bytes / {} files",
                batch.bytes, batch.file_count
            ),
            &root,
        );
    }

    let restored = match restore_batch(&quarantine, &report.run_id) {
        Ok(value) => value,
        Err(error) => fail(&format!("restore failed: {error}"), &root),
    };
    if restored.restored != expected_files || restored.failed != 0 {
        fail(
            &format!(
                "restore reported {} restored, {} skipped, {} failed",
                restored.restored, restored.skipped, restored.failed
            ),
            &root,
        );
    }
    let expectations: [(&str, usize, u8); 4] = [
        ("a.txt", 100, b'a'),
        ("sub\\b.bin", 2048, b'b'),
        ("sub\\deep\\c.txt", 7, b'c'),
        ("empty.dat", 0, b'd'),
    ];
    for (name, size, byte) in expectations {
        let content = fs::read(cache.join(name))
            .unwrap_or_else(|error| fail(&format!("{name} was not restored: {error}"), &root));
        if content.len() != size || content.iter().any(|value| *value != byte) {
            fail(
                &format!("{name} was restored with different content"),
                &root,
            );
        }
    }

    // A complete restore retires the batch: nothing is left to purge, so the
    // batch is gone from the listing and purging it is refused rather than
    // silently reporting a success for something that no longer exists.
    if list_quarantine(&quarantine)
        .iter()
        .any(|batch| batch.run_id == report.run_id)
    {
        fail("a fully restored batch stayed in the quarantine listing", &root);
    }
    if batch_dir.exists() {
        fail("the emptied batch directory survived a full restore", &root);
    }
    match purge_batch(&quarantine, &report.run_id) {
        Ok(_) => fail("purging a retired batch should be refused", &root),
        Err(error) => {
            if !error.contains("No quarantine batch") {
                fail(&format!("unexpected purge error: {error}"), &root);
            }
        }
    }

    /* --------------------------------------------- 3. locked file and purge size */

    write(&cache2.join("free.bin"), &vec![b'f'; 300]);
    write(&cache2.join("locked.bin"), &vec![b'l'; 512]);
    let (locked_plan, locked_preview) =
        match plan_specs(vec![spec("fixture-locked", &cache2)], false) {
            Ok(value) => value,
            Err(error) => fail(&format!("locked plan failed: {error}"), &root),
        };
    if locked_preview.file_count != 2 {
        fail(
            "locked fixture preview counted the wrong number of files",
            &root,
        );
    }
    let handle = lock_file(&cache2.join("locked.bin"));
    let locked_report = match execute_plan(&locked_plan, &quarantine, &cancel, &no_progress) {
        Ok(report) => report,
        Err(error) => {
            unsafe {
                CloseHandle(handle);
            }
            fail(
                &format!("a locked file aborted the whole run: {error}"),
                &root,
            );
        }
    };
    unsafe {
        CloseHandle(handle);
    }
    if locked_report.quarantined != 1 {
        fail(
            &format!(
                "expected exactly the unlocked file to quarantine, got {}",
                locked_report.quarantined
            ),
            &root,
        );
    }
    if locked_report.skipped + locked_report.failed < 1 {
        fail(
            "the locked file was not reported as skipped or failed",
            &root,
        );
    }
    if cache2.join("free.bin").exists() {
        fail("the unlocked file stayed in place", &root);
    }
    if !cache2.join("locked.bin").exists() {
        fail(
            "the locked file disappeared although it could not be moved",
            &root,
        );
    }
    let locked_batch = list_quarantine(&quarantine)
        .into_iter()
        .find(|batch| batch.run_id == locked_report.run_id)
        .unwrap_or_else(|| fail("the partial run has no quarantine batch", &root));
    if locked_batch.bytes != 300 {
        fail(
            &format!(
                "partial batch measured {} bytes, expected 300",
                locked_batch.bytes
            ),
            &root,
        );
    }
    let freed = match purge_batch(&quarantine, &locked_report.run_id) {
        Ok(value) => value,
        Err(error) => fail(&format!("purge failed: {error}"), &root),
    };
    if freed.removed_files != 1 || freed.freed_bytes != 300 {
        fail(
            &format!(
                "purge removed {} files / {} bytes, expected 1 / 300",
                freed.removed_files, freed.freed_bytes
            ),
            &root,
        );
    }

    /* -------------------------------------------------- 4. read-only mode refusal */

    let (read_only_plan, read_only_preview) =
        match plan_specs(vec![spec("fixture-read-only", &cache)], true) {
            Ok(value) => value,
            Err(error) => fail(&format!("read-only plan failed: {error}"), &root),
        };
    if !read_only_preview.read_only {
        fail("read-only plan did not mark the preview read-only", &root);
    }
    match execute_plan(&read_only_plan, &quarantine, &cancel, &no_progress) {
        Ok(_) => fail("read-only mode executed a cleanup", &root),
        Err(error) if error.contains("Read-only") => {}
        Err(error) => fail(
            &format!("unexpected read-only refusal text: {error}"),
            &root,
        ),
    }
    if fs::read(cache.join("a.txt")).is_err() {
        fail("read-only mode modified the fixture", &root);
    }

    // The fixture is disposable and created by this binary only.
    fs::remove_dir_all(&root).expect("remove fixture root");
    println!("{MARKER}");
}
