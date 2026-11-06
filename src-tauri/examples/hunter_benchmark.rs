//! Reproducible Hunter Mode Targeting and Rust processing performance benchmark.
//! Measures p50, p95, p99 latencies, cached vs uncached app matching,
//! and process memory during prolonged targeting.

use std::time::Instant;
use windows_sys::Win32::Foundation::POINT;
use windows_sys::Win32::System::ProcessStatus::*;
use windows_sys::Win32::System::Threading::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use cyron_lib::apps::{self, InstalledApp};
use cyron_lib::hunter;

fn get_process_working_set_bytes() -> u64 {
    unsafe {
        let handle = GetCurrentProcess();
        let mut counters: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        if GetProcessMemoryInfo(handle, &mut counters, counters.cb) != 0 {
            counters.WorkingSetSize as u64
        } else {
            0
        }
    }
}

fn percentile(sorted: &[u128], pct: f64) -> u128 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() as f64) * pct).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn main() {
    println!("=== Cyron Hunter Mode Performance Benchmark ===");

    // 1. Create 500 mock installed applications in memory
    let mut mock_apps = Vec::with_capacity(500);
    for i in 0..500 {
        mock_apps.push(InstalledApp {
            id: format!("user-64|MockApp_{i}"),
            name: format!("Mock Application {i}"),
            version: Some("1.0.0".into()),
            publisher: Some(format!("Vendor {i}")),
            install_location: Some(format!(r"C:\Program Files\Vendor{i}\App{i}")),
            install_date: None,
            uninstall_string: Some(format!(r"C:\Program Files\Vendor{i}\App{i}\uninstall.exe")),
            quiet_uninstall_string: None,
            display_icon: None,
            estimated_size: Some(1024 * 1024 * (i as u64 + 1)),
            hive: "user-64".into(),
            scope: "user".into(),
            architecture: "x64".into(),
            is_protected: false,
            source: "win32".into(),
            uninstall_support: "standard".into(),
            confidence: "confirmed".into(),
            detection_reason: None,
            registry_key_name: Some(format!("MockApp_{i}")),
        });
    }

    // 2. Measure cached in-memory app matching across 500 apps
    let test_proc_path = r"C:\Program Files\Vendor250\App250\bin\app.exe";
    let mut cached_durations = Vec::with_capacity(1000);
    for _ in 0..1000 {
        let t0 = Instant::now();
        let (matched_id, matched_name) = hunter::match_process_to_cached_apps(test_proc_path, &mock_apps);
        let elapsed = t0.elapsed().as_nanos();
        assert_eq!(matched_id.as_deref(), Some("user-64|MockApp_250"));
        assert_eq!(matched_name.as_deref(), Some("Mock Application 250"));
        cached_durations.push(elapsed);
    }
    cached_durations.sort_unstable();

    let cached_min = cached_durations[0];
    let cached_max = cached_durations[cached_durations.len() - 1];
    let cached_mean = cached_durations.iter().sum::<u128>() / cached_durations.len() as u128;
    let cached_p50 = percentile(&cached_durations, 0.50);
    let cached_p95 = percentile(&cached_durations, 0.95);
    let cached_p99 = percentile(&cached_durations, 0.99);

    println!("\n[1] In-Memory App Matching (500 apps inventory, 1,000 iterations):");
    println!("    Min:  {:>8.2} µs", cached_min as f64 / 1000.0);
    println!("    Mean: {:>8.2} µs", cached_mean as f64 / 1000.0);
    println!("    p50:  {:>8.2} µs", cached_p50 as f64 / 1000.0);
    println!("    p95:  {:>8.2} µs", cached_p95 as f64 / 1000.0);
    println!("    p99:  {:>8.2} µs", cached_p99 as f64 / 1000.0);
    println!("    Max:  {:>8.2} µs", cached_max as f64 / 1000.0);

    // 3. Measure uncached live system discovery
    println!("\n[2] Live Uncached App Discovery (full system scan):");
    let t_uncached_start = Instant::now();
    let live_inv = apps::discover_apps();
    let uncached_ms = t_uncached_start.elapsed().as_millis();
    println!("    Discovered {} installed applications in {} ms", live_inv.apps.len(), uncached_ms);
    let speedup = (uncached_ms as f64 * 1000.0) / (cached_mean as f64 / 1000.0);
    println!("    Speedup of cached matching over uncached re-scan: {:.0}x", speedup);

    // 4. Measure Win32 Window Targeting Resolution
    let mut pt = POINT { x: 0, y: 0 };
    unsafe {
        GetCursorPos(&mut pt);
    }

    let mut targeting_durations = Vec::with_capacity(1000);
    for _ in 0..1000 {
        let t0 = Instant::now();
        let _ = hunter::resolve_target_from_point(pt, &mock_apps);
        targeting_durations.push(t0.elapsed().as_nanos());
    }
    targeting_durations.sort_unstable();

    let tgt_min = targeting_durations[0];
    let tgt_max = targeting_durations[targeting_durations.len() - 1];
    let tgt_mean = targeting_durations.iter().sum::<u128>() / targeting_durations.len() as u128;
    let tgt_p50 = percentile(&targeting_durations, 0.50);
    let tgt_p95 = percentile(&targeting_durations, 0.95);
    let tgt_p99 = percentile(&targeting_durations, 0.99);

    println!("\n[3] Win32 Window Targeting Resolution (WindowFromPoint + PID + query + matching, 1,000 iterations):");
    println!("    Min:  {:>8.2} µs", tgt_min as f64 / 1000.0);
    println!("    Mean: {:>8.2} µs", tgt_mean as f64 / 1000.0);
    println!("    p50:  {:>8.2} µs", tgt_p50 as f64 / 1000.0);
    println!("    p95:  {:>8.2} µs", tgt_p95 as f64 / 1000.0);
    println!("    p99:  {:>8.2} µs", tgt_p99 as f64 / 1000.0);
    println!("    Max:  {:>8.2} µs", tgt_max as f64 / 1000.0);

    // 5. Prolonged targeting CPU & memory stability test (continuous 250 iterations at 20Hz)
    println!("\n[4] Prolonged Targeting Stability (500 continuous targeting checks):");
    let mem_before = get_process_working_set_bytes();
    let start_bench = Instant::now();
    for _ in 0..500 {
        let _ = hunter::resolve_target_from_point(pt, &mock_apps);
    }
    let total_elapsed = start_bench.elapsed();
    let mem_after = get_process_working_set_bytes();

    println!("    Completed 500 targeting checks in {:?}", total_elapsed);
    println!("    Working set before: {} KB", mem_before / 1024);
    println!("    Working set after:  {} KB", mem_after / 1024);
    let mem_delta_kb = (mem_after as i64 - mem_before as i64) / 1024;
    println!("    Memory delta:       {} KB (stable, bounded)", mem_delta_kb);
    assert!(mem_delta_kb.abs() < 5120, "Memory must remain stable during prolonged targeting (< 5 MB variance)");

    println!("\nHUNTER_BENCHMARK_OK");
}
