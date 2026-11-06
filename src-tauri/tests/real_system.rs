//! Real-system smoke test: reads the live registry, the real cache locations
//! and scans a temporary directory with the production scanner.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use cyron_lib::{apps, caches, scan};

struct TempTree(PathBuf);

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn temp_tree() -> TempTree {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let path =
        std::env::temp_dir().join(format!("cyron-real-system-{}-{nanos}", std::process::id()));
    fs::create_dir_all(path.join("nested").join("deeper")).expect("create temp tree");
    TempTree(path)
}

fn write_file(path: &Path, bytes: usize) {
    fs::write(path, vec![b'x'; bytes]).expect("write temp file");
}

#[test]
fn discovers_apps_caches_and_scans_real_files() {
    let inventory = apps::discover_apps();
    println!("discovered_apps={}", inventory.apps.len());
    for warning in &inventory.warnings {
        println!("warning: {warning}");
    }
    assert!(
        !inventory.apps.is_empty(),
        "expected at least one installed application; warnings: {:?}",
        inventory.warnings
    );
    assert!(inventory.apps.iter().all(|app| !app.name.trim().is_empty()));
    assert!(inventory.apps.iter().any(|app| app.hive == "machine-64"));
    assert!(inventory
        .apps
        .iter()
        .all(|app| app.scope == "machine" || app.scope == "user"));

    let cache_inventory = caches::discover_caches();
    let existing = cache_inventory
        .targets
        .iter()
        .filter(|target| target.exists)
        .count();
    println!("cache_targets={existing}");
    for warning in &cache_inventory.warnings {
        println!("warning: {warning}");
    }
    // A clean machine may have no developer caches; do not fail on zero.
    let summed: u64 = cache_inventory
        .targets
        .iter()
        .map(|target| target.size)
        .sum();
    assert!(
        cache_inventory.total_size <= summed,
        "risk totals must not exceed the sum of individual targets"
    );

    let tree = temp_tree();
    write_file(&tree.0.join("a.bin"), 12_345);
    write_file(&tree.0.join("nested").join("b.txt"), 777);
    write_file(&tree.0.join("nested").join("deeper").join("c.dat"), 4_096);

    let expected_bytes = 12_345u64 + 777 + 4_096;
    let result = scan::scan_directory(
        "real-system",
        &tree.0,
        &scan::ScanOptions::default(),
        Arc::new(AtomicBool::new(false)),
        &|_| {},
    );

    println!(
        "scan_files={} scan_bytes={} scan_dirs={}",
        result.file_count, result.total_size, result.dir_count
    );
    for warning in &result.errors {
        println!("scan warning: {warning}");
    }
    assert_eq!(result.file_count, 3, "each temp file must be counted");
    assert_eq!(
        result.total_size, expected_bytes,
        "the scanner must report exactly the bytes written"
    );
    assert!(!result.cancelled);
    assert!(
        !result.root.children.is_empty(),
        "root should expose children"
    );

    println!("CYRON_REAL_SYSTEM_OK");
}
