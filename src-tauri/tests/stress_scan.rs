//! Adversarial scanner tests: wide directories, depth caps, denied paths,
//! cancellation and exclusion. These use only the public scan API and build
//! their fixtures under the system temp directory.

use cyron_lib::scan::{scan_directory, ScanOptions, ScanProgress, ScanResult};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

struct Fixture {
    path: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "cyron-stress-{label}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create fixture root");
        Self { path }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn write_file(path: &Path, contents: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    let mut file = fs::File::create(path).expect("create fixture file");
    file.write_all(contents).expect("write fixture file");
    file.flush().expect("flush fixture file");
}

fn scan(path: &Path, options: ScanOptions, cancelled: bool) -> ScanResult {
    let cancel = Arc::new(AtomicBool::new(cancelled));
    let progress = |_payload: ScanProgress| {};
    scan_directory("stress-job", path, &options, cancel, &progress)
}

fn file_count_in_tree(node: &cyron_lib::scan::ScanNode) -> u64 {
    node.children.iter().map(|child| child.file_count).sum()
}

fn tree_shape(node: &cyron_lib::scan::ScanNode, depth: usize) -> (usize, usize) {
    node.children.iter().fold((1, depth), |(count, max_depth), child| {
        let (child_count, child_depth) = tree_shape(child, depth + 1);
        (count + child_count, max_depth.max(child_depth))
    })
}

#[test]
fn wide_directory_reports_exact_totals_with_bounded_children() {
    let fixture = Fixture::new("wide");
    let mut expected_files = 0u64;
    let mut expected_bytes = 0u64;
    for index in 0..2_000u64 {
        let len = 8 + (index % 50) as usize;
        let contents = vec![b'x'; len];
        expected_files += 1;
        expected_bytes += len as u64;
        write_file(&fixture.path.join(format!("f{index:04}.dat")), &contents);
    }

    let started = Instant::now();
    let result = scan(&fixture.path, ScanOptions::default(), false);
    let elapsed = started.elapsed();

    assert!(!result.cancelled, "scan should not report cancellation");
    assert_eq!(result.file_count, expected_files, "every file is counted");
    assert_eq!(result.total_size, expected_bytes, "logical bytes are exact");
    assert_eq!(result.method, "directory", "the fixture uses the fallback walker");
    assert_eq!(
        result.categories.iter().map(|category| category.file_count).sum::<u64>(),
        expected_files,
        "storage categories account for every scanned file"
    );
    assert_eq!(
        result.categories.iter().map(|category| category.bytes).sum::<u64>(),
        expected_bytes,
        "category logical bytes equal the scan total"
    );
    assert_eq!(
        result.categories.iter().map(|category| category.allocated).sum::<u64>(),
        result.total_allocated,
        "category allocated bytes equal the scan total"
    );
    assert_eq!(
        result.age_buckets.iter().map(|bucket| bucket.file_count).sum::<u64>(),
        expected_files,
        "age buckets account for every scanned file"
    );
    assert_eq!(
        result.age_buckets.iter().map(|bucket| bucket.bytes).sum::<u64>(),
        expected_bytes,
        "age-bucket logical bytes equal the scan total"
    );
    assert!(result.categories.len() <= 10, "category output has a fixed bound");
    assert!(result.age_buckets.len() <= 5, "age output has a fixed bound");
    assert!(result.insights.len() <= 6, "insight output has a fixed bound");
    let (node_count, max_depth) = tree_shape(&result.root, 0);
    assert!(node_count <= 100_001, "retained IPC nodes exceed the global budget: {node_count}");
    assert!(max_depth <= 8, "retained IPC tree exceeds the depth budget: {max_depth}");
    assert!(
        result.root.children.len() <= 200,
        "IPC children stay bounded, saw {}",
        result.root.children.len()
    );

    let group = result
        .root
        .children
        .iter()
        .find(|child| child.kind == "group")
        .expect("a wide directory folds small files into a group node");
    assert_eq!(
        file_count_in_tree(&result.root),
        expected_files,
        "group + retained file nodes preserve the file total"
    );
    assert_eq!(
        group.file_count + result.root.children.iter().filter(|c| c.kind == "file").map(|c| c.file_count).sum::<u64>(),
        expected_files
    );
    assert_eq!(result.errors, Vec::<String>::new(), "no fixture errors");

    println!(
        "wide: files={} bytes={} children={} group_files={} elapsed_ms={}",
        result.file_count,
        result.total_size,
        result.root.children.len(),
        group.file_count,
        elapsed.as_millis()
    );
}

#[test]
fn deep_nesting_respects_depth_cap_without_overflow() {
    let fixture = Fixture::new("deep");
    let mut current = fixture.path.clone();
    let mut sizes = Vec::new();
    for level in 0..60u64 {
        let contents = vec![b'd'; 16 + level as usize];
        sizes.push(contents.len() as u64);
        write_file(&current.join(format!("level-{level}.txt")), &contents);
        current = current.join("a");
        fs::create_dir_all(&current).expect("create nested directory");
    }

    let options = ScanOptions {
        max_depth: 10,
        ..ScanOptions::default()
    };
    let result = scan(&fixture.path, options, false);

    assert!(!result.cancelled);
    assert_eq!(result.file_count, 11, "depth 0..=10 each hold one file");
    assert_eq!(result.dir_count, 11, "root plus ten walked directories");
    let expected: u64 = sizes.iter().take(11).sum();
    assert_eq!(result.total_size, expected);
    assert!(
        result.total_size < sizes.iter().sum::<u64>(),
        "files below the depth cap are not claimed"
    );
    // `truncated` currently reflects IPC-tree pruning (depth 8 here), not only the
    // walk depth cap, so record the observed value instead of asserting a cause.
    println!(
        "deep: files={} dirs={} bytes={} truncated={} errors={:?}",
        result.file_count, result.dir_count, result.total_size, result.truncated, result.errors
    );
}

#[test]
fn access_denied_directories_report_errors_instead_of_panicking() {
    let candidates = [r"C:\System Volume Information", r"C:\Windows\CSC"];
    let mut checked = 0;
    for candidate in candidates {
        let path = Path::new(candidate);
        if !path.is_dir() {
            continue;
        }
        if fs::read_dir(path).is_ok() {
            println!("skipped {candidate}: readable in this context");
            continue;
        }
        let result = scan(path, ScanOptions::default(), false);
        assert!(
            !result.errors.is_empty(),
            "{candidate} should be reported, not silently ignored"
        );
        assert_eq!(result.file_count, 0, "no entries are invented");
        println!("denied: {} errors={:?}", candidate, result.errors);
        checked += 1;
    }
    if checked == 0 {
        println!("access-denied path was readable or absent; nothing to assert");
    }
}

#[test]
fn cancellation_before_start_returns_an_empty_cancelled_result() {
    let fixture = Fixture::new("cancel");
    for index in 0..200 {
        write_file(&fixture.path.join(format!("c{index}.bin")), b"cancelled");
    }

    let result = scan(&fixture.path, ScanOptions::default(), true);

    assert!(result.cancelled, "a pre-set cancel flag is reported");
    assert_eq!(result.file_count, 0);
    assert_eq!(result.total_size, 0);
    assert!(result.root.children.is_empty());
    assert!(!result.truncated);
}

#[test]
fn excluded_subtree_contributes_nothing() {
    let fixture = Fixture::new("excluded");
    let keep = fixture.path.join("keep");
    let drop_dir = fixture.path.join("drop");
    write_file(&keep.join("a.txt"), b"aaaa");
    write_file(&keep.join("b.txt"), b"bbbb");
    write_file(&drop_dir.join("c.txt"), b"cccccc");
    write_file(&drop_dir.join("d.txt"), b"dddddd");
    write_file(&drop_dir.join("e.txt"), b"eeeeee");

    let options = ScanOptions {
        exclusions: vec![drop_dir.to_string_lossy().into_owned()],
        ..ScanOptions::default()
    };
    let result = scan(&fixture.path, options, false);

    assert_eq!(result.file_count, 2, "only the kept subtree is counted");
    assert_eq!(result.total_size, 8);
    assert!(
        result.errors.is_empty(),
        "an exclusion is not a scan error: {:?}",
        result.errors
    );
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

#[cfg(windows)]
fn peak_working_set() -> u64 {
    use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    let mut counters: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
    counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    let ok = unsafe {
        GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut counters,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        )
    };
    if ok == 0 {
        0
    } else {
        counters.PeakWorkingSetSize as u64
    }
}

#[cfg(not(windows))]
fn peak_working_set() -> u64 {
    0
}

/// Opt-in large-directory scan. Set CYRON_STRESS_DIR to an existing directory,
/// for example `CYRON_STRESS_DIR=C:\Windows`.
#[test]
#[ignore]
fn large_directory_scan_reports_real_numbers() {
    let Ok(target) = std::env::var("CYRON_STRESS_DIR") else {
        println!("CYRON_STRESS_DIR is not set; skipping the large-directory scan");
        return;
    };
    let path = PathBuf::from(&target);
    assert!(path.is_dir(), "CYRON_STRESS_DIR must be an existing directory");

    let started = Instant::now();
    let result = scan(&path, ScanOptions::default(), false);
    let elapsed = started.elapsed();
    let peak = peak_working_set();

    println!(
        "large: path={} files={} dirs={} logical={} allocated={} errors={} truncated={} elapsed_ms={} elapsed={:?} peak_ws={} ({})",
        path.display(),
        result.file_count,
        result.dir_count,
        result.total_size,
        result.total_allocated,
        result.errors.len(),
        result.truncated,
        result.elapsed_ms,
        elapsed,
        format_bytes(peak),
        peak
    );
    assert!(result.file_count > 0, "the target should contain files");
}
