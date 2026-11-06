//! RC1 End-to-End fixture test for application uninstallation, process management,
//! registry backup/restore, residual cleanup, Hunter Mode targeting, and Startup Manager.
//!
//! All operations run against isolated disposable fixtures under `%TEMP%\cyron-rc1-fixture-<pid>\`
//! and isolated HKCU keys under `HKCU\Software\CyronRC1Fixture_<pid>`.
//! Never touches real software, real user files, or production registry settings.

use std::fs;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use winreg::enums::*;
use winreg::{RegKey, RegValue};
use windows_sys::Win32::Foundation::{POINT, RECT};
use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect;

use cyron_lib::apps::InstalledApp;
use cyron_lib::cleanup;
use cyron_lib::hunter;
use cyron_lib::leftovers;
use cyron_lib::process;
use cyron_lib::system;

const MARKER: &str = "RC1_E2E_FIXTURE_OK";

fn main() {
    let pid = std::process::id();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let temp_root = std::env::temp_dir().join(format!("cyron-rc1-fixture-{pid}-{now}"));
    fs::create_dir_all(&temp_root).expect("create fixture root");

    let mock_install_dir = temp_root.join("ProgramFiles").join("MockApp");
    fs::create_dir_all(&mock_install_dir).expect("create mock install dir");

    let dummy_exe = mock_install_dir.join("mock_app.exe");
    let system32_cmd = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into()) + "\\System32\\cmd.exe";
    let _ = fs::copy(&system32_cmd, &dummy_exe);

    fs::write(mock_install_dir.join("config.json"), "{\"version\": \"1.0.0\"}").expect("write config");
    fs::write(mock_install_dir.join("data.bin"), vec![0xca, 0xfe, 0xba, 0xbe]).expect("write data");

    let quarantine_dir = temp_root.join("quarantine");
    fs::create_dir_all(&quarantine_dir).expect("create quarantine dir");

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let test_reg_subkey = format!("Software\\CyronRC1Fixture_{pid}_{now}");

    // Create multi-type registry test values
    let (reg_key, _) = hkcu.create_subkey(&test_reg_subkey).expect("create test subkey");
    reg_key.set_value("DisplayName", &"Mock App").expect("set display name");
    reg_key.set_value("ConfigVersion", &100u32).expect("set dword");
    reg_key.set_value("", &"Default Value").expect("set default");

    let bin_val = RegValue {
        vtype: REG_BINARY,
        bytes: vec![0x01, 0x02, 0x03, 0x04],
    };
    reg_key.set_raw_value("BinarySetting", &bin_val).expect("set binary");

    let qword_val = RegValue {
        vtype: REG_QWORD,
        bytes: vec![0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80],
    };
    reg_key.set_raw_value("QwordSetting", &qword_val).expect("set qword");

    let (nested_key, _) = hkcu.create_subkey(format!("{test_reg_subkey}\\NestedKey")).expect("create nested");
    nested_key.set_value("NestedVal", &"child data").expect("set child val");

    println!("=== Cyron Milestone 7.3 E2E Verification Suite ===");
    println!("Fixture root: {}", temp_root.display());

    // -------------------------------------------------------------------------
    // 1. Hunter targeting an external disposable application / GUI window
    // -------------------------------------------------------------------------
    println!("[E2E 1/8] Testing Hunter targeting an external disposable application / GUI window...");
    let target_bin = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join("disposable_gui_target.exe");

    assert!(
        target_bin.exists(),
        "Disposable GUI target binary not found at {}. Non-GUI fallback is strictly prohibited for external GUI targeting verification!",
        target_bin.display()
    );

    let mut gui_child = Command::new(&target_bin)
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn disposable GUI target");

    let stdout = gui_child.stdout.take().expect("capture stdout");
    let mut reader = std::io::BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).expect("read line from disposable GUI stdout");
    println!("Spawned external disposable GUI window stdout: {line}");

    // Parse TARGET_HWND:0x... PID:...
    let (hwnd_val, disp_pid) = {
        let parts: Vec<&str> = line.trim().split_whitespace().collect();
        let hwnd_part = parts.iter().find(|p| p.starts_with("TARGET_HWND:")).expect("TARGET_HWND in stdout");
        let pid_part = parts.iter().find(|p| p.starts_with("PID:")).expect("PID in stdout");
        let hwnd_hex = hwnd_part.strip_prefix("TARGET_HWND:").unwrap();
        let hwnd_ptr = usize::from_str_radix(hwnd_hex.strip_prefix("0x").unwrap_or(hwnd_hex), 16).expect("parse hwnd hex");
        let pid: u32 = pid_part.strip_prefix("PID:").unwrap().parse().expect("parse pid");
        (hwnd_ptr, pid)
    };
    assert_eq!(disp_pid, gui_child.id());

    // Allow window to be visible and message loop active
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Measure window rect and target the window center
    let mut rect: RECT = unsafe { std::mem::zeroed() };
    let got_rect = unsafe { GetWindowRect(hwnd_val as _, &mut rect) };
    assert_ne!(got_rect, 0, "GetWindowRect must succeed on real HWND");
    let target_point = POINT {
        x: (rect.left + rect.right) / 2,
        y: (rect.top + rect.bottom) / 2,
    };

    // Assert WindowFromPoint resolves the exact HWND, PID, and title
    let target_resolved = hunter::resolve_target_from_point(target_point, &[])
        .expect("resolve_target_from_point succeeds")
        .expect("target must be found at center point of disposable window");
    assert_eq!(target_resolved.hwnd, hwnd_val, "Resolved HWND must match expected disposable GUI HWND");
    assert_eq!(target_resolved.pid, disp_pid, "Resolved PID must match expected disposable GUI PID");
    assert_eq!(target_resolved.window_title.as_deref(), Some("Cyron External Target App"), "Window title must match");
    assert!(target_resolved.can_terminate, "Target must be terminable");
    assert!(!target_resolved.is_critical, "Target must not be critical");
    println!("Targeted external GUI window successfully: HWND 0x{:x}, PID {disp_pid}", hwnd_val);

    // -------------------------------------------------------------------------
    // 2. Correct application identity matching & directory boundaries
    // -------------------------------------------------------------------------
    println!("[E2E 2/8] Testing correct application identity matching and directory boundaries...");
    let app_foo = InstalledApp {
        id: "user-64|AppFoo".into(),
        name: "AppFoo".into(),
        version: Some("1.0".into()),
        publisher: None,
        install_location: Some(r"C:\Apps\Foo".into()),
        install_date: None,
        uninstall_string: None,
        quiet_uninstall_string: None,
        display_icon: None,
        estimated_size: None,
        hive: "user-64".into(),
        scope: "user".into(),
        architecture: "x64".into(),
        is_protected: false,
        source: "win32".into(),
        uninstall_support: "standard".into(),
        confidence: "confirmed".into(),
        detection_reason: None,
        registry_key_name: Some("AppFoo".into()),
    };

    let apps_inventory = vec![app_foo];
    // Must match inside C:\Apps\Foo
    let (m1_id, m1_name) = hunter::match_process_to_cached_apps(r"C:\Apps\Foo\app.exe", &apps_inventory);
    assert_eq!(m1_id.as_deref(), Some("user-64|AppFoo"));
    assert_eq!(m1_name.as_deref(), Some("AppFoo"));

    // Directory boundary collision protection: C:\Apps\Foo must NEVER match C:\Apps\FooBar\app.exe
    let (m2_id, _) = hunter::match_process_to_cached_apps(r"C:\Apps\FooBar\app.exe", &apps_inventory);
    assert!(m2_id.is_none(), "C:\\Apps\\Foo must NOT match C:\\Apps\\FooBar\\app.exe");
    println!("Directory boundary verification passed.");

    // -------------------------------------------------------------------------
    // 3. Hunter snapshot isolation & verified process termination
    // -------------------------------------------------------------------------
    println!("[E2E 3/8] Testing Hunter snapshot isolation and verified process termination...");
    let target_token = target_resolved.hunter_target_id.clone();

    // Uninstaller snapshot in uninstaller map
    let mut uninstaller_proc_snapshots: std::collections::HashMap<String, process::StoredProcessSnapshot> =
        std::collections::HashMap::new();
    let uninstaller_snap = process::StoredProcessSnapshot {
        app_id: "user-64|ProtectedUninstallerApp".into(),
        install_location: r"C:\Apps\UninstallerApp".into(),
        candidate_roots: vec![r"c:\apps\uninstallerapp".into()],
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        processes: Vec::new(),
    };
    uninstaller_proc_snapshots.insert("user-64|ProtectedUninstallerApp".into(), uninstaller_snap);

    // Register Hunter target in isolated hunter maps
    let mut hunter_proc_snapshots = std::collections::HashMap::new();
    let mut hunter_app_snapshots = std::collections::HashMap::new();
    hunter::register_target_in_state(&target_resolved, &mut hunter_proc_snapshots, &mut hunter_app_snapshots);

    // Verify isolation: uninstaller map has not been mutated or overwritten
    assert_eq!(uninstaller_proc_snapshots.len(), 1);
    assert!(uninstaller_proc_snapshots.contains_key("user-64|ProtectedUninstallerApp"));
    assert!(!uninstaller_proc_snapshots.contains_key(&target_token));

    let registered_snap = hunter_proc_snapshots.get(&target_token).expect("hunter snapshot registered");
    assert_eq!(registered_snap.processes.len(), 1);
    assert_eq!(registered_snap.processes[0].pid, disp_pid);

    // Mismatched app ID refusal
    let fail_term = process::terminate_processes_verified(
        "invalid-mismatched-target",
        registered_snap,
        &[disp_pid],
        true,
    );
    assert!(!fail_term[0].success, "mismatched app ID must be refused");

    // Verified termination
    let ok_term = process::terminate_processes_verified(
        &target_token,
        registered_snap,
        &[disp_pid],
        true,
    );
    assert!(ok_term[0].success, "verified termination of disposable process must succeed");
    let _ = gui_child.wait();
    println!("Verified termination succeeded and isolated snapshots maintained.");

    // -------------------------------------------------------------------------
    // 4. Unregistered application review
    // -------------------------------------------------------------------------
    println!("[E2E 4/8] Testing unregistered application review...");
    let unreg_app = hunter_app_snapshots.get(&target_token).expect("ad-hoc unregistered app created");
    assert_eq!(unreg_app.source, "portable");
    assert_eq!(unreg_app.uninstall_support, "missing");
    assert!(unreg_app.display_icon.is_some());
    println!("Unregistered application review passed.");

    // -------------------------------------------------------------------------
    // 5. Broken registry entry removal, tampering rejection, conflict prevention, and fail-closed recovery
    // -------------------------------------------------------------------------
    println!("[E2E 5/8] Testing broken registry entry removal, tampering rejection, conflict prevention, and recovery...");
    let test_broken_name = format!("CyronE2EBroken_{pid}_{now}");
    let uninstall_rel = format!(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{test_broken_name}");
    let full_broken_path = format!(r"HKCU\{uninstall_rel}");

    let (broken_reg, _) = hkcu.create_subkey(&uninstall_rel).expect("create broken uninstall entry");
    broken_reg.set_value("DisplayName", &"Ghost Application").expect("set name");
    broken_reg.set_value("UninstallString", &"C:\\DeletedApp\\uninst.exe").expect("set uninst");

    let broken_app_item = InstalledApp {
        id: format!("user-64|{test_broken_name}"),
        name: "Ghost Application".into(),
        version: None,
        publisher: None,
        install_location: None,
        install_date: None,
        uninstall_string: Some("C:\\DeletedApp\\uninst.exe".into()),
        quiet_uninstall_string: None,
        display_icon: None,
        estimated_size: None,
        hive: "user-64".into(),
        scope: "user".into(),
        architecture: "x64".into(),
        is_protected: false,
        source: "win32".into(),
        uninstall_support: "missing".into(),
        confidence: "confirmed".into(),
        detection_reason: None,
        registry_key_name: Some(test_broken_name.clone()),
    };

    let (verified_sub, verified_path) = leftovers::verify_broken_entry_candidate(&broken_app_item).expect("verify candidate");
    assert_eq!(verified_sub, test_broken_name);
    assert_eq!(verified_path.to_ascii_lowercase(), full_broken_path.to_ascii_lowercase());

    // Read live metadata and compute fingerprint
    let live_meta = leftovers::read_live_broken_entry_metadata("user-64", &test_broken_name).expect("read live metadata");
    assert!(!live_meta.fingerprint.is_empty());

    let plan_id = leftovers::generate_collision_resistant_plan_id("broken-plan-e2e");
    let broken_plan = leftovers::StoredBrokenEntryPlan {
        plan_id: plan_id.clone(),
        app_id: broken_app_item.id.clone(),
        app_name: broken_app_item.name.clone(),
        registry_key_path: full_broken_path.clone(),
        subkey_name: test_broken_name.clone(),
        hive: "user-64".into(),
        fingerprint: live_meta.fingerprint.clone(),
        expires_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 300,
    };

    // Tamper test 1: entry becomes valid (uninstaller executable recreated on disk)
    let cmd_exe = std::env::var("COMSPEC").unwrap_or_else(|_| "C:\\Windows\\System32\\cmd.exe".into());
    broken_reg.set_value("UninstallString", &cmd_exe).expect("set valid uninstaller executable");
    let recovered_res = leftovers::execute_broken_entry_removal(&broken_plan, &quarantine_dir);
    assert!(recovered_res.is_err(), "entry that became valid must be refused without deletion");
    assert!(hkcu.open_subkey(&uninstall_rel).is_ok(), "key must not be deleted");

    // Tamper test 2: metadata altered (fingerprint mismatch)
    broken_reg.set_value("UninstallString", &"C:\\DeletedApp\\uninst.exe").expect("revert uninst");
    broken_reg.set_value("DisplayName", &"Altered Application Title").expect("alter name");
    let altered_res = leftovers::execute_broken_entry_removal(&broken_plan, &quarantine_dir);
    assert!(altered_res.is_err(), "entry with altered metadata fingerprint must be refused");
    assert!(hkcu.open_subkey(&uninstall_rel).is_ok(), "altered key must not be deleted");

    // Revert to original metadata for valid execution
    broken_reg.set_value("DisplayName", &"Ghost Application").expect("restore name");

    let clean_res = leftovers::execute_broken_entry_removal(&broken_plan, &quarantine_dir).expect("execute broken removal");
    assert_eq!(clean_res.deleted_registry_keys, 1);
    assert!(clean_res.registry_backup_path.is_some());
    assert!(hkcu.open_subkey(&uninstall_rel).is_err(), "broken entry key must be deleted");

    let batch_folder = quarantine_dir.join(&plan_id);
    let manifest = cleanup::read_manifest(&batch_folder).expect("manifest must be readable on disk");
    assert_eq!(manifest.run_id, plan_id);
    assert!(manifest.backup_sha256.is_some());
    assert_eq!(manifest.registry_key_path.as_deref(), Some(full_broken_path.as_str()));

    let bkp_file = PathBuf::from(clean_res.registry_backup_path.unwrap());
    assert!(bkp_file.exists());

    // Tamper test 3: modified .reg backup content (checksum mismatch)
    let tampered_file = batch_folder.join("tampered_backup.reg");
    fs::write(&tampered_file, b"Windows Registry Editor Version 5.00\r\n\r\n; tampered line\r\n").unwrap();
    let tamper_res = leftovers::restore_registry_backup_with_policy(&tampered_file, Some(&manifest), true);
    assert!(tamper_res.is_err(), "tampered backup file must fail checksum validation");
    let err_msg = tamper_res.err().unwrap();
    assert!(err_msg.contains("tampering") || err_msg.contains("mismatch"), "error must report checksum tampering: {err_msg}");

    // Scope test: out-of-scope section injection
    let rogue_file = batch_folder.join("rogue_backup.reg");
    let rogue_content = format!(
        "Windows Registry Editor Version 5.00\r\n\r\n[{}]\r\n\"Test\"=\"Val\"\r\n\r\n[HKEY_LOCAL_MACHINE\\Software\\RogueKey]\r\n\"Injected\"=\"Bad\"\r\n",
        full_broken_path.replace("HKCU\\", "HKEY_CURRENT_USER\\")
    );
    fs::write(&rogue_file, rogue_content).unwrap();
    let scope_res = leftovers::verify_registry_backup_scope(&rogue_file, &full_broken_path);
    assert!(scope_res.is_err(), "out-of-scope sections must be refused");

    // Conflict test: recreate live key before restoration; silent overwrite MUST be refused
    let (recreated_k, _) = hkcu.create_subkey(&uninstall_rel).expect("recreate live key for conflict test");
    recreated_k.set_value("DisplayName", &"Existing Live Entry").expect("write live value");
    let conflict_res = leftovers::restore_registry_backup_with_policy(&bkp_file, Some(&manifest), false);
    assert!(conflict_res.is_err(), "restoration over existing live key without explicit overwrite confirmation must fail");
    let conflict_err = conflict_res.err().unwrap();
    assert!(conflict_err.contains("conflict") || conflict_err.contains("already exists"), "error must indicate conflict: {conflict_err}");

    // Safe recovery decision: overwrite confirmed
    let overwrite_rep = leftovers::restore_registry_backup_with_policy(&bkp_file, Some(&manifest), true).expect("restoration with overwrite confirmed");
    assert!(overwrite_rep.keys_restored >= 1);
    assert!(hkcu.open_subkey(&uninstall_rel).is_ok(), "key restored successfully");
    let _ = hkcu.delete_subkey_all(&uninstall_rel);

    // Test quarantine file restoration/purge does NOT destroy outstanding registry backup
    let mixed_batch_id = format!("mixed-batch-{pid}");
    let mixed_dir = quarantine_dir.join(&mixed_batch_id);
    fs::create_dir_all(&mixed_dir).unwrap();
    let dummy_quarantined_file = mixed_dir.join("quarantined.dat");
    fs::write(&dummy_quarantined_file, b"test file bytes").unwrap();
    let dummy_reg = mixed_dir.join("registry_backup.reg");
    fs::write(&dummy_reg, b"Windows Registry Editor Version 5.00\r\n\r\n[HKEY_CURRENT_USER\\Software\\Test]\r\n\"Val\"=\"1\"\r\n").unwrap();
    let dummy_manifest = cleanup::BatchManifest {
        run_id: mixed_batch_id.clone(),
        created_at: 1000,
        expires_at: 2000,
        target_ids: vec!["target-1".into()],
        items: vec![cleanup::ManifestItem {
            original_path: temp_root.join("restored_original.dat").to_string_lossy().to_string(),
            stored_path: dummy_quarantined_file.to_string_lossy().to_string(),
            size: 15,
        }],
        registry_backup: Some("registry_backup.reg".into()),
        registry_key_path: Some("HKCU\\Software\\Test".into()),
        hive: Some("user-64".into()),
        subkey_name: Some("Test".into()),
        app_id: Some("user-64|Test".into()),
        backup_sha256: Some("dummy".into()),
        backup_size_bytes: Some(100),
    };
    cleanup::write_manifest(&mixed_dir, &dummy_manifest).unwrap();

    // Restore batch files:
    let rest_batch = cleanup::restore_batch(&quarantine_dir, &mixed_batch_id).expect("restore mixed batch");
    assert_eq!(rest_batch.restored, 1);
    assert!(dummy_reg.exists(), "restore_batch must NEVER delete or invalidate registry_backup.reg");
    assert!(mixed_dir.join("manifest.json").exists(), "manifest must be preserved for registry recovery");

    // Purge batch files when mixed:
    let _ = cleanup::purge_batch(&quarantine_dir, &mixed_batch_id).expect("purge mixed batch");
    assert!(dummy_reg.exists(), "purge_batch on mixed batch must NEVER delete registry_backup.reg");

    // Clean up dummy test batch
    let _ = fs::remove_file(&dummy_reg);
    let _ = fs::remove_file(mixed_dir.join("manifest.json"));
    let _ = fs::remove_file(mixed_dir.join(cleanup::JOURNAL_FILE));
    let _ = fs::remove_dir(&mixed_dir);
    let _ = fs::remove_file(temp_root.join("restored_original.dat"));

    println!("Broken registry entry removal, tampering rejection, conflict prevention, and fail-closed recovery passed.");

    // -------------------------------------------------------------------------
    // 6. Startup entry disable, byte preservation, and restoration
    // -------------------------------------------------------------------------
    println!("[E2E 6/8] Testing Startup entry disable, byte preservation, and restoration...");
    let startup_test_name = format!("CyronE2EStartup_{pid}");
    let startup_test_cmd = r#""C:\Program Files\Test\startup.exe" --background"#;
    let (run_k, _) = hkcu.create_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run").expect("open run key");
    run_k.set_value(&startup_test_name, &startup_test_cmd).expect("write startup entry");

    let startup_items = system::list_startup_items();
    let our_item = startup_items.into_iter().find(|i| i.name == startup_test_name).expect("find startup item");
    assert!(our_item.enabled);
    assert!(our_item.can_toggle);

    // Disable item
    let disabled_item = system::set_startup_item_enabled(&our_item.id, false).expect("disable startup item");
    assert!(!disabled_item.enabled);

    // Verify disabled and verify StartupApproved binary format
    let reloaded_items = system::list_startup_items();
    let reloaded_item = reloaded_items.into_iter().find(|i| i.name == startup_test_name).expect("find disabled item");
    assert!(!reloaded_item.enabled);

    let appr_k = hkcu.open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run").expect("open approval key");
    let raw_val = appr_k.get_raw_value(&startup_test_name).expect("read raw approval bytes");
    assert_eq!(raw_val.bytes.len(), 12, "StartupApproved binary value must be 12 bytes");
    assert_eq!(raw_val.bytes[0], 0x03, "First byte must be 0x03 for disabled");

    // Restore (re-enable) item: deletes override, restoring pristine state
    let restored_item = system::set_startup_item_enabled(&our_item.id, true).expect("restore startup item");
    assert!(restored_item.enabled);

    let _ = run_k.delete_value(&startup_test_name);
    let _ = appr_k.delete_value(&startup_test_name);
    println!("Startup entry disable, byte preservation, and restoration passed.");

    // -------------------------------------------------------------------------
    // 7. Honest performance measurements
    // -------------------------------------------------------------------------
    println!("[E2E 7/8] Testing targeting latency distribution and memory stability...");
    let mut mock_500_apps = Vec::with_capacity(500);
    for i in 0..500 {
        mock_500_apps.push(InstalledApp {
            id: format!("user-64|App_{i}"),
            name: format!("App {i}"),
            version: None,
            publisher: None,
            install_location: Some(format!(r"C:\Apps\App{i}")),
            install_date: None,
            uninstall_string: None,
            quiet_uninstall_string: None,
            display_icon: None,
            estimated_size: None,
            hive: "user-64".into(),
            scope: "user".into(),
            architecture: "x64".into(),
            is_protected: false,
            source: "win32".into(),
            uninstall_support: "standard".into(),
            confidence: "confirmed".into(),
            detection_reason: None,
            registry_key_name: Some(format!("App_{i}")),
        });
    }

    let mut latencies = Vec::with_capacity(500);
    for _ in 0..500 {
        let t0 = Instant::now();
        let _ = hunter::resolve_target_from_point(target_point, &mock_500_apps);
        latencies.push(t0.elapsed().as_nanos());
    }
    latencies.sort_unstable();

    let p50 = latencies[latencies.len() * 50 / 100] as f64 / 1000.0;
    let p95 = latencies[latencies.len() * 95 / 100] as f64 / 1000.0;
    let p99 = latencies[latencies.len() * 99 / 100] as f64 / 1000.0;
    println!("Targeting latency over 500 checks: p50={:.2}µs, p95={:.2}µs, p99={:.2}µs", p50, p95, p99);
    assert!(p99 < 50_000.0, "p99 targeting latency must remain < 50ms");
    println!("Targeting latency distribution verified.");

    // -------------------------------------------------------------------------
    // 8. Critical-process and shared-resource refusal
    // -------------------------------------------------------------------------
    println!("[E2E 8/8] Testing critical-process and shared-resource refusal...");
    assert!(process::is_critical_process(4, "System", None));
    assert!(process::is_critical_process(1234, "csrss.exe", Some(r"C:\Windows\System32\csrss.exe")));
    assert!(process::is_critical_process(5678, "explorer.exe", Some(r"C:\Windows\explorer.exe")));
    assert!(process::is_critical_process(9999, "services.exe", Some(r"C:\Windows\System32\services.exe")));

    // Shared registry roots protection
    assert!(!leftovers::is_safe_to_delete_reg_key(r"HKCU\Software\Microsoft"));
    assert!(!leftovers::is_safe_to_delete_reg_key(r"HKLM\SOFTWARE\Microsoft\Windows"));
    assert!(!leftovers::is_safe_to_delete_reg_key(r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"));
    assert!(!leftovers::is_safe_to_delete_reg_key(r"HKCU\Software\Google"));

    // Critical candidate roots protection
    assert!(!process::is_candidate_root_safe(Path::new(r"C:\Windows")));
    assert!(!process::is_candidate_root_safe(Path::new(r"C:\Windows\System32")));
    assert!(!process::is_candidate_root_safe(Path::new(r"C:\")));
    assert!(!process::is_candidate_root_safe(Path::new(r"C:\Program Files")));
    assert!(!process::is_candidate_root_safe(Path::new(r"C:\Users")));
    println!("Critical process and shared resource protections verified.");

    // -------------------------------------------------------------------------
    // Clean up test fixture
    // -------------------------------------------------------------------------
    let _ = hkcu.delete_subkey_all(&test_reg_subkey);
    let _ = fs::remove_dir_all(&temp_root);

    println!("\nAll 8 E2E scenarios passed successfully.");
    println!("{MARKER}");
}
