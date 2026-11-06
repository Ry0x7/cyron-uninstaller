//! Destructive-operation security suite.
//!
//! Phase 3 adds the first code path in Cyron that moves user files. This suite
//! replaces the blanket Phase 1/2 "read-only" audit with explicit boundary
//! tests: the catalog and the planner decide what may be touched, the engine
//! refuses protected data, and destructive calls are confined to reviewed
//! modules.
//!
//! Everything here works on disposable fixtures under `std::env::temp_dir()`.
//! No test reads, moves or deletes a real cache location.

use cyron_lib::{caches, cleanup, scan};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "cyron-security-{tag}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create test fixture directory");
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

fn write_fixture(path: &Path, bytes: usize) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create fixture parent");
    }
    fs::write(path, vec![0x5a; bytes]).expect("write fixture file");
}

/// A synthetic spec pointing at a disposable fixture, so plan/execute can be
/// exercised without naming any real cache location.
fn fixture_spec(path: &Path) -> cleanup::CleanupSpec {
    cleanup::CleanupSpec {
        id: "fixture-cache".to_string(),
        tool: "Fixture".to_string(),
        label: "Fixture cache".to_string(),
        risk: "safe".to_string(),
        category: "windows".to_string(),
        note: "Synthetic fixture used by the security suite.".to_string(),
        impact: "Nothing outside the fixture is affected.".to_string(),
        path: path.to_path_buf(),
    }
}

fn protected_templates() -> Vec<String> {
    caches::CATALOG
        .iter()
        .filter(|entry| entry.risk == "protected")
        .map(|entry| normalize(&entry.path))
        .collect()
}

fn normalize(value: &str) -> String {
    value
        .replace('/', "\\")
        .split('\\')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\\")
        .to_lowercase()
}

fn is_prefix(parent: &str, child: &str) -> bool {
    child == parent || child.starts_with(&format!("{parent}\\"))
}

/* ------------------------------------------------------- catalog boundary */

#[test]
fn protected_catalog_entries_can_never_be_planned() {
    let mut checked = 0;
    for entry in caches::CATALOG {
        if entry.risk != "protected" {
            continue;
        }
        checked += 1;
        // Protected entries are rejected by the planner even when a caller
        // presents the exact id the UI knows about.
        let error = cleanup::catalog_specs(&[entry.id.to_string()])
            .expect_err("protected entries must be rejected");
        let lower = error.to_lowercase();
        assert!(
            lower.contains("protected") || lower.contains("never"),
            "unexpected rejection for {}: {error}",
            entry.id
        );
    }
    assert!(
        checked >= 8,
        "the catalog must still contain protected locations, found {checked}"
    );
}

#[test]
fn unknown_and_hostile_ids_are_rejected() {
    let ids = [
        String::new(),
        "   ".to_string(),
        "does-not-exist".to_string(),
        "..\\..\\Windows".to_string(),
        "C:\\Windows".to_string(),
        "%TEMP%".to_string(),
        "claude-code-projects\n".to_string(),
        "x".repeat(200),
    ];
    for id in ids {
        assert!(
            cleanup::catalog_specs(&[id.clone()]).is_err(),
            "id {id:?} must be rejected"
        );
    }
    // A valid id must still work, so the rejections above are not vacuous.
    let safe = caches::CATALOG
        .iter()
        .find(|entry| entry.risk == "safe" && entry.id == "user-temp")
        .expect("user-temp is catalogued");
    let specs = cleanup::catalog_specs(&[safe.id.to_string()]).expect("safe id resolves");
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].id, safe.id);
}

#[test]
fn guard_rejects_roots_and_ancestors_of_protected_data() {
    let mut candidates: Vec<PathBuf> = Vec::new();
    for variable in [
        "USERPROFILE",
        "LOCALAPPDATA",
        "APPDATA",
        "WINDIR",
        "ProgramFiles",
        "ProgramFiles(x86)",
    ] {
        if let Ok(value) = std::env::var(variable) {
            if !value.trim().is_empty() {
                candidates.push(PathBuf::from(value));
            }
        }
    }
    if let Ok(system_drive) = std::env::var("SystemDrive") {
        candidates.push(PathBuf::from(format!("{system_drive}\\")));
    }
    assert!(
        candidates.len() >= 4,
        "expected the standard Windows profile variables"
    );

    for candidate in candidates {
        assert!(
            cleanup::guard_path(&candidate).is_err(),
            "guard must reject {}",
            candidate.display()
        );
    }
}

#[test]
fn every_cleanup_location_passes_the_guard() {
    let existing: Vec<String> = caches::CATALOG
        .iter()
        .filter(|entry| entry.risk != "protected")
        .filter(|entry| cleanup::catalog_specs(&[entry.id.to_string()]).is_ok())
        .map(|entry| entry.id.to_string())
        .collect();
    assert!(existing.len() >= 20, "catalog should resolve most locations");

    let specs = cleanup::catalog_specs(&existing).expect("cleanup ids resolve");
    let mut guarded = 0;
    for spec in &specs {
        assert_ne!(spec.risk, "protected", "{} must not be plannable", spec.id);
        if !spec.path.exists() {
            continue;
        }
        cleanup::guard_path(&spec.path).unwrap_or_else(|error| {
            panic!("cleanup location {} would be blocked: {error}", spec.id)
        });
        guarded += 1;
    }
    assert!(guarded >= 3, "guard should accept the real catalog, got {guarded}");
}

#[test]
fn no_cleanup_location_contains_protected_data() {
    let protected = protected_templates();
    assert!(!protected.is_empty());

    for entry in caches::CATALOG {
        if entry.risk == "protected" {
            continue;
        }
        let cleanup_template = normalize(&entry.path);
        for protected_template in &protected {
            assert!(
                !is_prefix(&cleanup_template, protected_template),
                "cleanup location {} ({}) contains protected {}",
                entry.id,
                entry.path,
                protected_template
            );
        }
    }
}

/* -------------------------------------------------------- engine boundary */

#[test]
fn read_only_mode_blocks_execution() {
    let root = TempDir::new("read-only");
    let target = root.path().join("cache");
    write_fixture(&target.join("a.bin"), 32);
    let quarantine = root.path().join("quarantine");
    fs::create_dir_all(&quarantine).expect("quarantine root");

    let (plan, preview) =
        cleanup::plan_specs(vec![fixture_spec(&target)], true).expect("plan builds");
    assert!(plan.read_only, "read-only must be recorded on the plan");
    assert!(preview.read_only);
    assert_eq!(preview.file_count, 1);

    let cancel = Arc::new(AtomicBool::new(false));
    let error = cleanup::execute_plan(&plan, &quarantine, &cancel, &|_| {})
        .expect_err("read-only plans must not execute");
    assert!(
        error.to_lowercase().contains("read-only"),
        "unexpected error: {error}"
    );
    assert!(target.join("a.bin").exists(), "the file must still be there");
}

#[test]
fn fixture_cleanup_quarantines_restores_and_purges() {
    let root = TempDir::new("end-to-end");
    let target = root.path().join("cache");
    let nested = target.join("nested");
    write_fixture(&nested.join("one.bin"), 512);
    write_fixture(&nested.join("two.bin"), 1_024);
    write_fixture(&target.join("three.tmp"), 7);
    let outside = root.path().join("outside").join("keep.txt");
    write_fixture(&outside, 99);
    let content = fs::read(&outside).expect("read outside file");
    let quarantine = root.path().join("quarantine");
    fs::create_dir_all(&quarantine).expect("quarantine root");

    let (plan, preview) =
        cleanup::plan_specs(vec![fixture_spec(&target)], false).expect("plan builds");
    assert_eq!(preview.file_count, 3);
    assert_eq!(preview.bytes, 512 + 1_024 + 7);
    assert!(!preview.files.is_empty());
    assert!(!preview.read_only);

    let cancel = Arc::new(AtomicBool::new(false));
    let report = cleanup::execute_plan(&plan, &quarantine, &cancel, &|_| {})
        .expect("execute succeeds");
    assert!(report.failed == 0, "unexpected failures: {:?}", report.errors);
    assert_eq!(report.quarantined, 3);
    assert_eq!(report.files, 3);
    assert_eq!(report.bytes, 512 + 1_024 + 7);
    assert!(report.quarantine_dir.is_some());
    assert!(!nested.join("one.bin").exists());
    assert!(!target.join("three.tmp").exists());
    assert_eq!(
        fs::read(&outside).expect("outside file survives"),
        content,
        "nothing outside the planned target may move"
    );

    let batches = cleanup::list_quarantine(&quarantine);
    assert_eq!(batches.len(), 1);
    let batch = &batches[0];
    assert_eq!(batch.file_count, 3);
    assert_eq!(batch.bytes, 512 + 1_024 + 7);
    assert_eq!(batch.target_ids, vec!["fixture-cache".to_string()]);

    let restore = cleanup::restore_batch(&quarantine, &batch.run_id).expect("restore succeeds");
    assert_eq!(restore.restored, 3);
    assert_eq!(restore.failed, 0);
    assert_eq!(fs::read(nested.join("one.bin")).expect("restored"), vec![0x5a; 512]);
    assert_eq!(fs::read(target.join("three.tmp")).expect("restored"), vec![0x5a; 7]);

    // Restore removes the batch, so purge now has nothing to report but must
    // not error on a missing batch.
    assert!(cleanup::list_quarantine(&quarantine).is_empty());
}

#[test]
fn purge_removes_a_quarantine_batch() {
    let root = TempDir::new("purge");
    let target = root.path().join("cache");
    write_fixture(&target.join("a.bin"), 256);
    let quarantine = root.path().join("quarantine");
    fs::create_dir_all(&quarantine).expect("quarantine root");

    let (plan, _) = cleanup::plan_specs(vec![fixture_spec(&target)], false).expect("plan builds");
    let cancel = Arc::new(AtomicBool::new(false));
    let report =
        cleanup::execute_plan(&plan, &quarantine, &cancel, &|_| {}).expect("execute succeeds");
    let batch_dir = report.quarantine_dir.clone().expect("batch directory");

    let purge = cleanup::purge_batch(&quarantine, &report.run_id).expect("purge succeeds");
    assert_eq!(purge.removed_files, 1);
    assert_eq!(purge.freed_bytes, 256);
    assert!(!Path::new(&batch_dir).exists());
    assert!(cleanup::list_quarantine(&quarantine).is_empty());
}

#[test]
fn restore_never_overwrites_an_existing_file() {
    let root = TempDir::new("restore-conflict");
    let target = root.path().join("cache");
    write_fixture(&target.join("keep.bin"), 128);
    let quarantine = root.path().join("quarantine");
    fs::create_dir_all(&quarantine).expect("quarantine root");

    let (plan, _) = cleanup::plan_specs(vec![fixture_spec(&target)], false).expect("plan builds");
    let cancel = Arc::new(AtomicBool::new(false));
    let report =
        cleanup::execute_plan(&plan, &quarantine, &cancel, &|_| {}).expect("execute succeeds");
    assert_eq!(report.quarantined, 1);

    // A new file appeared at the original location after the cleanup.
    write_fixture(&target.join("keep.bin"), 4);
    let restore = cleanup::restore_batch(&quarantine, &report.run_id).expect("restore runs");
    assert_eq!(restore.skipped, 1, "existing files must not be overwritten");
    assert_eq!(restore.restored, 0);
    assert_eq!(
        fs::read(target.join("keep.bin")).expect("new file"),
        vec![0x5a; 4],
        "the new file keeps its content"
    );
    let batches = cleanup::list_quarantine(&quarantine);
    assert_eq!(batches.len(), 1, "the quarantined copy stays recoverable");
    assert_eq!(batches[0].file_count, 1);
}

#[test]
fn quarantine_operations_reject_hostile_run_ids() {
    let root = TempDir::new("hostile-run");
    let quarantine = root.path().join("quarantine");
    fs::create_dir_all(&quarantine).expect("quarantine root");

    for run_id in ["", "missing-run", "..", "..\\..", "../..", "a/b"] {
        assert!(
            cleanup::restore_batch(&quarantine, run_id).is_err(),
            "restore must reject {run_id:?}"
        );
        assert!(
            cleanup::purge_batch(&quarantine, run_id).is_err(),
            "purge must reject {run_id:?}"
        );
    }
}

#[test]
fn cleanup_never_moves_links_or_directories() {
    let root = TempDir::new("links");
    let target = root.path().join("cache");
    write_fixture(&target.join("file.bin"), 64);
    fs::create_dir_all(target.join("sub")).expect("sub directory");
    write_fixture(&target.join("sub").join("nested.bin"), 16);
    let quarantine = root.path().join("quarantine");
    fs::create_dir_all(&quarantine).expect("quarantine root");

    assert!(!cleanup::is_reparse(&target.join("file.bin")));

    let (plan, preview) =
        cleanup::plan_specs(vec![fixture_spec(&target)], false).expect("plan builds");
    assert_eq!(preview.file_count, 2);
    assert!(
        preview
            .files
            .iter()
            .all(|file| file.path.ends_with(".bin")),
        "only files are planned, never directories"
    );

    let cancel = Arc::new(AtomicBool::new(false));
    let report = cleanup::execute_plan(&plan, &quarantine, &cancel, &|_| {}).expect("execute");
    assert_eq!(report.quarantined, 2);
    assert!(!target.join("file.bin").exists());
    assert!(!target.join("sub").join("nested.bin").exists());
}

/* ------------------------------------------------ permission boundary audit */

#[test]
fn destructive_calls_stay_inside_reviewed_modules() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rust_files(&source, &mut files);
    assert!(!files.is_empty(), "backend source files should be present");

    let forbidden = [
        "remove_dir_all(",
        "TerminateProcess(",
        "taskkill",
        "cmd.exe",
        "powershell",
        "RegKey::delete_",
        ".delete_subkey",
        ".delete_value",
        "RegDelete",
        "SetFileAttributes",
        "WriteFile(",
        "ShellExecute",
    ];
    let allowed_destructive = ["cleanup.rs", "history.rs"];
    let mut command_invocations = Vec::new();

    for file in &files {
        let text = fs::read_to_string(file).expect("read backend Rust source");
        let production = without_test_modules(&text);
        let name = file
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string();

        for (line_number, original) in production.lines().enumerate() {
            let code = original.split("//").next().unwrap_or("");
            let lower = code.to_ascii_lowercase();
            for pattern in forbidden {
                assert!(
                    !lower.contains(&pattern.to_ascii_lowercase()),
                    "forbidden operation {pattern:?} in {}:{}: {original}",
                    file.display(),
                    line_number + 1
                );
            }

            if code.contains("Command::new(") {
                command_invocations.push((name.clone(), line_number + 1, code.trim().to_string()));
            }

            for destructive in ["fs::rename(", "fs::remove_file(", "fs::remove_dir("] {
                if code.contains(destructive) {
                    assert!(
                        allowed_destructive.contains(&name.as_str()),
                        "{destructive} must live in a reviewed module, found in {}:{}",
                        file.display(),
                        line_number + 1
                    );
                }
            }
        }
    }

    // Explorer is the only child process Cyron starts, and only to reveal a
    // path the user asked for.
    assert_eq!(
        command_invocations.len(),
        1,
        "only Explorer may be launched: {command_invocations:?}"
    );
    let (command_file, _, command_line) = &command_invocations[0];
    assert_eq!(command_file, "commands.rs");
    assert!(command_line.contains("explorer.exe"));
}

/// Removes complete `#[cfg(test)] mod ... { ... }` blocks before auditing, so
/// fixture cleanup inside unit tests is not misclassified as production code.
fn without_test_modules(source: &str) -> String {
    let mut output = String::new();
    let mut in_test_module = false;
    let mut depth: i32 = 0;

    for line in source.lines() {
        if !in_test_module {
            if line.trim() == "#[cfg(test)]" {
                in_test_module = true;
                depth = 0;
                continue;
            }
            output.push_str(line);
            output.push('\n');
            continue;
        }

        depth += line.chars().filter(|character| *character == '{').count() as i32;
        depth -= line.chars().filter(|character| *character == '}').count() as i32;
        if depth == 0 && line.contains("}") {
            in_test_module = false;
        }
    }
    output
}

fn collect_rust_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(directory).expect("enumerate backend source");
    for entry in entries {
        let path = entry.expect("source directory entry").path();
        if path.is_dir() {
            collect_rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

/* -------------------------------------------------------- scanner safety */

fn scan_path(path: &Path) -> scan::ScanResult {
    scan::scan_directory(
        "security-test",
        path,
        &scan::ScanOptions::default(),
        Arc::new(AtomicBool::new(false)),
        &|_| {},
    )
}

#[test]
fn scanner_does_not_follow_a_junction() {
    let root = TempDir::new("junction-root");
    let outside = TempDir::new("junction-target");
    write_fixture(&root.path().join("inside.bin"), 73);
    write_fixture(&outside.path().join("outside.bin"), 2_000_000);
    let junction = root.path().join("outside-link");
    let created = std::process::Command::new("cmd.exe")
        .arg("/C")
        .arg("mklink")
        .arg("/J")
        .arg(&junction)
        .arg(outside.path())
        .status()
        .is_ok_and(|status| status.success());
    if !created {
        eprintln!("skipping junction assertion: Windows refused junction creation");
        return;
    }

    let result = scan_path(root.path());
    let _ = fs::remove_dir(&junction);
    assert_eq!(result.total_size, 73, "bytes beyond the junction are excluded");
    assert!(result.skipped_reparse >= 1, "the junction is reported as skipped");
}

#[test]
fn hard_links_are_counted_once_for_allocated_bytes() {
    let root = TempDir::new("hard-links");
    let original = root.path().join("original.bin");
    write_fixture(&original, 512 * 1024);
    let hard_link = root.path().join("second-name.bin");
    if fs::hard_link(&original, &hard_link).is_err() {
        eprintln!("skipping hard-link assertion: filesystem does not support hard links");
        return;
    }

    let result = scan_path(root.path());
    assert_eq!(result.file_count, 2);
    assert_eq!(result.deduped_files, 1);
    assert_eq!(result.total_size, 1024 * 1024, "logical size counts both names");
    assert!(result.total_allocated < result.total_size);
}

#[test]
fn missing_scan_path_returns_an_error_without_panicking() {
    let root = TempDir::new("missing");
    let missing = root.path().join("does-not-exist");
    let result = scan_path(&missing);
    assert_eq!(result.total_size, 0);
    assert_eq!(result.file_count, 0);
    assert!(!result.errors.is_empty());
}

/* ------------------------------------------- time-of-check / time-of-use */

fn create_junction(link: &Path, target: &Path) -> bool {
    std::process::Command::new("cmd.exe")
        .arg("/C")
        .arg("mklink")
        .arg("/J")
        .arg(link)
        .arg(target)
        .status()
        .is_ok_and(|status| status.success())
}

#[test]
fn a_junction_swapped_in_after_the_preview_moves_nothing() {
    let root = TempDir::new("toctou-root");
    let outside = TempDir::new("toctou-outside");
    let target = root.path().join("cache");
    let nested = target.join("nested");
    write_fixture(&nested.join("victim.bin"), 640);
    let secret = outside.path().join("secret.txt");
    write_fixture(&secret, 32);
    let secret_bytes = fs::read(&secret).expect("read outside file");
    let quarantine = root.path().join("quarantine");
    fs::create_dir_all(&quarantine).expect("quarantine root");

    let (plan, preview) =
        cleanup::plan_specs(vec![fixture_spec(&target)], false).expect("plan builds");
    assert_eq!(preview.file_count, 1, "the preview saw the real folder");

    // The previewed subfolder is replaced by a junction to unrelated data
    // before the user confirms.
    fs::remove_dir_all(&nested).expect("remove previewed folder");
    if !create_junction(&nested, outside.path()) {
        eprintln!("skipping TOCTOU assertion: Windows refused junction creation");
        return;
    }

    let cancel = Arc::new(AtomicBool::new(false));
    let report = cleanup::execute_plan(&plan, &quarantine, &cancel, &|_| {}).expect("execute");
    let _ = fs::remove_dir(&nested);

    assert_eq!(report.quarantined, 0, "nothing behind the junction may move");
    assert_eq!(
        fs::read(&secret).expect("outside file survives"),
        secret_bytes,
        "data outside the target must not be quarantined"
    );
    assert!(
        report
            .items
            .iter()
            .all(|item| !Path::new(&item.path).starts_with(outside.path())),
        "no result may point at data outside the target"
    );
}

#[test]
fn a_target_replaced_by_a_junction_is_reported_instead_of_silently_skipped() {
    let root = TempDir::new("toctou-swap");
    let outside = TempDir::new("toctou-swap-outside");
    let target = root.path().join("cache");
    write_fixture(&target.join("old.bin"), 128);
    let secret = outside.path().join("secret.txt");
    write_fixture(&secret, 48);
    let secret_bytes = fs::read(&secret).expect("read outside file");
    let quarantine = root.path().join("quarantine");
    fs::create_dir_all(&quarantine).expect("quarantine root");

    let (plan, preview) =
        cleanup::plan_specs(vec![fixture_spec(&target)], false).expect("plan builds");
    assert_eq!(preview.file_count, 1);

    fs::remove_dir_all(&target).expect("remove previewed target");
    if !create_junction(&target, outside.path()) {
        eprintln!("skipping TOCTOU assertion: Windows refused junction creation");
        return;
    }

    let cancel = Arc::new(AtomicBool::new(false));
    let report = cleanup::execute_plan(&plan, &quarantine, &cancel, &|_| {}).expect("execute");
    let _ = fs::remove_dir(&target);

    assert_eq!(report.quarantined, 0);
    assert_eq!(report.files, 0);
    assert_eq!(
        fs::read(&secret).expect("outside file survives"),
        secret_bytes
    );
    assert!(
        report
            .errors
            .iter()
            .any(|error| error.contains("junction") || error.contains("link")),
        "the swap must be explained, not silent: {:?}",
        report.errors
    );
}
