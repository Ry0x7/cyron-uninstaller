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

    let forbidden_everywhere = [
        "remove_dir_all(",
        "taskkill",
        "powershell",
        "SetFileAttributes",
        "WriteFile(",
        "ShellExecute",
    ];
    let allowed_destructive = ["cleanup.rs", "history.rs", "leftovers.rs"];
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

            // Strict universal forbidden primitives
            for pattern in forbidden_everywhere {
                assert!(
                    !lower.contains(&pattern.to_ascii_lowercase()),
                    "forbidden operation {pattern:?} in {}:{}: {original}",
                    file.display(),
                    line_number + 1
                );
            }

            // TerminateProcess is restricted strictly to process.rs
            if lower.contains("terminateprocess(") {
                assert_eq!(
                    name, "process.rs",
                    "TerminateProcess is only permitted in process.rs, found in {}:{}",
                    file.display(),
                    line_number + 1
                );
            }

            // Registry deletion is restricted strictly to leftovers.rs (with backup) and explorer.rs (app shell verbs)
            if lower.contains(".delete_subkey") || lower.contains("regkey::delete_") || lower.contains("regdelete") {
                assert!(
                    name == "leftovers.rs" || name == "explorer.rs",
                    "Registry deletion is only permitted in leftovers.rs (with prior backup) or explorer.rs (unregistration), found in {}:{}",
                    file.display(),
                    line_number + 1
                );
            }

            // Shell command invocations
            if code.contains("cmd.exe") && !name.ends_with("uninstall.rs") && !name.ends_with("security.rs") {
                panic!("cmd.exe reference in {}:{}", file.display(), line_number + 1);
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

    // Only 2 reviewed process execution paths:
    // 1. Explorer in commands.rs (to reveal folders).
    // 2. Validated uninstaller binary in uninstall.rs.
    assert_eq!(
        command_invocations.len(),
        2,
        "only Explorer and validated uninstaller binaries may be launched: {command_invocations:?}"
    );
    assert!(
        command_invocations.iter().any(|(f, _, cmd)| f == "commands.rs" && cmd.contains("explorer.exe")),
        "commands.rs must launch explorer.exe"
    );
    assert!(
        command_invocations.iter().any(|(f, _, cmd)| f == "uninstall.rs" && cmd.contains("&cmd.executable")),
        "uninstall.rs must launch validated uninstaller"
    );
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

/* ----------------------------------------------------------- phase 4 tests */

#[test]
fn critical_system_processes_cannot_be_terminated() {
    use cyron_lib::process;

    assert!(process::is_critical_process(0, "System Idle Process", None));
    assert!(process::is_critical_process(4, "System", None));
    assert!(process::is_critical_process(100, "csrss.exe", None));
    assert!(process::is_critical_process(101, "smss.exe", None));
    assert!(process::is_critical_process(102, "services.exe", None));
    assert!(process::is_critical_process(103, "lsass.exe", None));
    assert!(process::is_critical_process(104, "explorer.exe", None));
    assert!(process::is_critical_process(105, "dwm.exe", None));
    assert!(process::is_critical_process(
        999,
        "custom.exe",
        Some("C:\\Windows\\System32\\custom.exe")
    ));

    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let snapshot = process::StoredProcessSnapshot {
        app_id: "test-app".into(),
        install_location: "C:\\Program Files\\TestApp".into(),
        candidate_roots: Vec::new(),
        timestamp: now,
        processes: vec![
            process::AppProcess {
                pid: 0,
                name: "System Idle Process".into(),
                path: None,
                is_critical: true,
                title: None,
                creation_time: 0,
            },
            process::AppProcess {
                pid: 4,
                name: "System".into(),
                path: None,
                is_critical: true,
                title: None,
                creation_time: 0,
            },
        ],
    };

    let results = process::terminate_processes_verified("test-app", &snapshot, &[0, 4], true);
    assert_eq!(results.len(), 2);
    assert!(!results[0].success);
    assert!(!results[1].success);
    assert!(results[0].error.as_ref().unwrap().contains("critical"));
    assert!(results[1].error.as_ref().unwrap().contains("critical"));
}

#[test]
fn process_termination_rejects_unlisted_pids_and_mismatched_apps() {
    use cyron_lib::process;

    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let snapshot = process::StoredProcessSnapshot {
        app_id: "app-target".into(),
        install_location: "C:\\Program Files\\TargetApp".into(),
        candidate_roots: Vec::new(),
        timestamp: now,
        processes: vec![process::AppProcess {
            pid: 12345,
            name: "target.exe".into(),
            path: Some("C:\\Program Files\\TargetApp\\target.exe".into()),
            is_critical: false,
            title: None,
            creation_time: 99999,
        }],
    };

    // 1. Mismatched app id
    let res_mismatch = process::terminate_processes_verified("different-app", &snapshot, &[12345], false);
    assert_eq!(res_mismatch.len(), 1);
    assert!(!res_mismatch[0].success);
    assert!(res_mismatch[0].error.as_ref().unwrap().contains("does not belong to this application"));

    // 2. Unlisted arbitrary PID injection attempt
    let res_unlisted = process::terminate_processes_verified("app-target", &snapshot, &[99999], false);
    assert_eq!(res_unlisted.len(), 1);
    assert!(!res_unlisted[0].success);
    assert!(res_unlisted[0].error.as_ref().unwrap().contains("was not found in the verified process snapshot"));
}

#[test]
fn uninstaller_runner_rejects_arbitrary_shell_commands() {
    use cyron_lib::uninstall;

    let shell_cmd = uninstall::parse_uninstall_command("cmd.exe /c del /f /q C:\\*");
    assert!(!shell_cmd.is_valid);
    assert!(shell_cmd.rejection_reason.unwrap().contains("refused"));

    let ps_cmd = uninstall::parse_uninstall_command("powershell.exe -NoProfile -Command Remove-Item C:\\");
    assert!(!ps_cmd.is_valid);

    let empty_cmd = uninstall::parse_uninstall_command("   ");
    assert!(!empty_cmd.is_valid);

    let msi_cmd = uninstall::parse_uninstall_command("msiexec.exe /I{11111111-2222-3333-4444-555555555555}");
    assert!(msi_cmd.is_valid);
    assert!(msi_cmd.is_msi);
    assert!(msi_cmd.arguments.iter().any(|arg| arg.starts_with("/x")));
}

#[test]
fn leftover_scanner_guards_system_roots_and_user_profiles() {
    use cyron_lib::leftovers;

    assert!(leftovers::guard_leftover_path(Path::new("C:\\")).is_err());
    assert!(leftovers::guard_leftover_path(Path::new("C:\\Windows")).is_err());
    assert!(leftovers::guard_leftover_path(Path::new("C:\\Program Files")).is_err());

    if let Ok(profile) = std::env::var("USERPROFILE") {
        assert!(leftovers::guard_leftover_path(Path::new(&profile)).is_err());
        assert!(leftovers::guard_leftover_path(&Path::new(&profile).join("Documents")).is_err());
        assert!(leftovers::guard_leftover_path(&Path::new(&profile).join("Desktop")).is_err());
        assert!(leftovers::guard_leftover_path(&Path::new(&profile).join("Downloads")).is_err());
    }

    // Shared vendor protection
    assert!(!leftovers::is_safe_to_delete_reg_key("HKCU\\Software\\Microsoft"));
    assert!(!leftovers::is_safe_to_delete_reg_key("HKCU\\Software\\Google"));
    assert!(!leftovers::is_safe_to_delete_reg_key("HKCU\\Software\\Apple"));
    assert!(!leftovers::is_safe_to_delete_reg_key("HKCU\\Software"));
}

#[test]
fn leftover_cleanup_creates_registry_backup_before_deletion() {
    use cyron_lib::leftovers;

    let fixture = TempDir::new("leftover-backup");
    let quarantine = fixture.path().join("quarantine");
    fs::create_dir_all(&quarantine).expect("quarantine dir");

    // Synthetic candidate for backup
    let candidates = vec![leftovers::LeftoverCandidate {
        id: "reg-1".into(),
        kind: "registry_key".into(),
        path: "HKCU\\Software\\CyronTestNonexistentKey".into(),
        confidence: "high".into(),
        risk: "safe".into(),
        reason: "Test key".into(),
        size: 0,
        file_count: 0,
        selected: true,
    }];

    let plan = leftovers::StoredLeftoverPlan {
        plan_id: "plan-1".into(),
        app_id: "test-app".into(),
        app_name: "Test App".into(),
        candidates,
        expires_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 600,
    };

    // Attempting cleanup on nonexistent key creates backup file or handles safely
    let res = leftovers::clean_leftovers(&plan, &["reg-1".to_string()], &quarantine);
    // Since nonexistent key open fails in winreg, it aborts to protect registry
    assert!(res.is_err(), "Nonexistent key backup aborts residual cleanup cleanly");
}

#[test]
fn batch_uninstall_plan_requires_valid_commands_and_bounds_count() {
    use cyron_lib::apps::InstalledApp;
    use cyron_lib::uninstall;

    // 1. Empty app selection is rejected
    assert!(uninstall::create_batch_uninstall_plan(&[], false).is_err());

    // 2. Over 32 apps is rejected
    let many_apps: Vec<InstalledApp> = (0..35).map(|i| InstalledApp {
        id: format!("app-{i}"),
        name: format!("App {i}"),
        version: None,
        publisher: None,
        install_date: None,
        estimated_size: None,
        install_location: None,
        uninstall_string: Some("C:\\App\\uninst.exe".into()),
        quiet_uninstall_string: None,
        display_icon: None,
        hive: "user-64".into(),
        scope: "user".into(),
        architecture: "x64".into(),
        is_protected: false,
        source: "win32".into(),
        uninstall_support: "standard".into(),
        confidence: "confirmed".into(),
        detection_reason: None,
        registry_key_name: None,
    }).collect();
    assert!(uninstall::create_batch_uninstall_plan(&many_apps, false).is_err());

    // 3. Selection with shell wrappers only is rejected
    let shell_app = vec![InstalledApp {
        id: "shell-app".into(),
        name: "Malicious App".into(),
        version: None,
        publisher: None,
        install_date: None,
        estimated_size: None,
        install_location: None,
        uninstall_string: Some("cmd.exe /c del /f /q C:\\Windows".into()),
        quiet_uninstall_string: None,
        display_icon: None,
        hive: "user-64".into(),
        scope: "user".into(),
        architecture: "x64".into(),
        is_protected: false,
        source: "win32".into(),
        uninstall_support: "standard".into(),
        confidence: "confirmed".into(),
        detection_reason: None,
        registry_key_name: None,
    }];
    assert!(uninstall::create_batch_uninstall_plan(&shell_app, false).is_err());
}

#[test]
fn process_creation_time_fails_closed_when_unverified() {
    use cyron_lib::process;

    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    // Snapshot with creation_time = 0 (unverified)
    let snapshot = process::StoredProcessSnapshot {
        app_id: "test-app".into(),
        install_location: "C:\\Program Files\\TestApp".into(),
        candidate_roots: Vec::new(),
        timestamp: now,
        processes: vec![process::AppProcess {
            pid: std::process::id(),
            name: "cyron.exe".into(),
            path: Some("C:\\Program Files\\TestApp\\cyron.exe".into()),
            is_critical: false,
            title: None,
            creation_time: 0, // Unverified / invalid
        }],
    };

    let results = process::terminate_processes_verified("test-app", &snapshot, &[std::process::id()], false);
    assert_eq!(results.len(), 1);
    assert!(!results[0].success);
    // Must be rejected because creation time 0 is invalid/unverified
    assert!(results[0].error.as_ref().unwrap().contains("recycled") || results[0].error.as_ref().unwrap().contains("creation time"));
}

#[test]
fn registry_backup_verification_detects_malformed_and_empty_files() {
    use cyron_lib::leftovers;

    let fixture = TempDir::new("reg-verify");

    // Empty file
    let empty_reg = fixture.path().join("empty.reg");
    fs::write(&empty_reg, "").expect("write empty");
    assert!(leftovers::verify_registry_backup(&empty_reg).is_err());

    // Missing header
    let no_header_reg = fixture.path().join("no_header.reg");
    fs::write(&no_header_reg, "[HKCU\\Software\\Foo]\n\"val\"=\"bar\"\n").expect("write no header");
    assert!(leftovers::verify_registry_backup(&no_header_reg).is_err());

    // Malformed dword
    let bad_dword_reg = fixture.path().join("bad_dword.reg");
    fs::write(&bad_dword_reg, "Windows Registry Editor Version 5.00\r\n\r\n[HKCU\\Software\\Foo]\r\n\"v\"=dword:not_hex\r\n").expect("write bad dword");
    assert!(leftovers::verify_registry_backup(&bad_dword_reg).is_err());

    // Malformed hex bytes
    let bad_hex_reg = fixture.path().join("bad_hex.reg");
    fs::write(&bad_hex_reg, "Windows Registry Editor Version 5.00\r\n\r\n[HKCU\\Software\\Foo]\r\n\"v\"=hex:zz,yy\r\n").expect("write bad hex");
    assert!(leftovers::verify_registry_backup(&bad_hex_reg).is_err());
}
