use cyron_lib::{caches, scan};
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
            "cyron-read-only-{tag}-{}-{nanos}",
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

fn scan(path: &Path) -> scan::ScanResult {
    scan::scan_directory(
        "read-only-test",
        path,
        &scan::ScanOptions::default(),
        Arc::new(AtomicBool::new(false)),
        &|_| {},
    )
}

#[test]
fn backend_has_no_unscoped_destructive_operations() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rust_files(&source, &mut files);
    assert!(!files.is_empty(), "backend source files should be present");

    let mut command_invocations = Vec::new();
    let mut remove_file_invocations = Vec::new();
    let forbidden = [
        "remove_dir_all(",
        "remove_dir(",
        "RegKey::delete_",
        ".delete_subkey",
        ".delete_value",
        "RegDelete",
        "WriteFile(",
        "SetFileAttributes",
        "TerminateProcess(",
        "taskkill",
        "powershell",
        "cmd.exe",
        "Command::new(\"cmd\"",
        "Command::new(\"powershell\"",
        "Command::new(\"reg\"",
    ];

    for file in &files {
        let text = fs::read_to_string(file).expect("read backend Rust source");
        let production = without_test_modules(&text);
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
                command_invocations.push((file.clone(), line_number + 1, code.trim().to_string()));
            }
            if code.contains("remove_file(") {
                remove_file_invocations.push((file.clone(), line_number + 1, code.trim().to_string()));
            }
        }
    }

    // Explorer is the sole child process: it reveals a validated scan target.
    assert_eq!(command_invocations.len(), 1, "only Explorer may be launched");
    let (command_file, _, command_line) = &command_invocations[0];
    assert!(command_file.ends_with("commands.rs"));
    assert!(command_line.contains("explorer.exe"));

    // Phase 1 exposes only explicit history clearing and removal of its own
    // failed-write temp file; neither path points into scanned user content.
    assert_eq!(remove_file_invocations.len(), 2, "only history may be removed");
    for (history_file, _, remove_line) in &remove_file_invocations {
        assert!(history_file.ends_with("history.rs"));
        assert!(
            remove_line.contains("&path") || remove_line.contains("&temp"),
            "unrecognized history removal target: {remove_line}"
        );
    }
    let history_source = fs::read_to_string(&remove_file_invocations[0].0)
        .expect("read history source");
    assert!(history_source.contains("dir.join(\"history.json\")"));
}

/// Removes complete `#[cfg(test)] mod ... { ... }` blocks before examining
/// production code, so fixture cleanup inside unit tests is not misclassified.
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

        // Test module braces are structurally balanced. Counting them here is
        // sufficient for these Rust modules and avoids inspecting fixture code.
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

    let result = scan(root.path());
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

    let result = scan(root.path());

    assert_eq!(result.file_count, 2);
    assert_eq!(result.deduped_files, 1);
    assert_eq!(result.total_size, 1024 * 1024, "logical size counts both names");
    assert!(result.total_allocated < result.total_size);
}

#[test]
fn missing_scan_path_returns_an_error_without_panicking() {
    let root = TempDir::new("missing");
    let missing = root.path().join("does-not-exist");

    let result = scan(&missing);

    assert_eq!(result.total_size, 0);
    assert_eq!(result.file_count, 0);
    assert!(!result.errors.is_empty());
}

#[test]
fn sensitive_cache_catalog_entries_are_protected() {
    let inventory = caches::discover_caches();
    for id in [
        "claude-code-projects",
        "codex-sessions",
        "gemini-history",
        "ollama-models",
        "lmstudio-models",
        "cursor-history",
        "vscode-history",
        "antigravity-user",
        "docker-wsl",
        "wsl-distros",
        "firefox-profiles",
    ] {
        let target = inventory
            .targets
            .iter()
            .find(|target| target.id == id)
            .unwrap_or_else(|| panic!("catalog entry {id} was not discovered"));
        assert_eq!(target.risk, "protected", "{id} must never be offered as cleanup");
    }
}
