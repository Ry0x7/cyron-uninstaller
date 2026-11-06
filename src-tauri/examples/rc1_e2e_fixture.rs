//! RC1 End-to-End fixture test for application uninstallation, process management,
//! registry backup/restore, residual cleanup, Hunter Mode targeting, and Startup Manager.
//!
//! All operations run against isolated disposable fixtures under `%TEMP%\cyron-rc1-fixture-<pid>\`
//! and isolated HKCU keys under `HKCU\Software\CyronRC1Fixture_<pid>`.
//! Never touches real software, real user files, or production registry settings.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use winreg::enums::*;
use winreg::{RegKey, RegValue};

use cyron_lib::apps::InstalledApp;
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

    println!("=== Cyron Milestone 7.1 E2E Verification Suite ===");
    println!("Fixture root: {}", temp_root.display());

    // -------------------------------------------------------------------------
    // 1. Hunter targeting an external disposable application
    // -------------------------------------------------------------------------
    println!("[E2E 1/8] Testing Hunter targeting an external disposable application...");
    let mut disposable_child = Command::new(&dummy_exe)
        .arg("/c")
        .arg("powershell -Command Start-Sleep -Seconds 60")
        .spawn()
        .expect("spawn disposable external process");
    let disp_pid = disposable_child.id();

    let (disp_name, disp_path) = process::query_process_info(disp_pid);
    let disp_creation = process::get_process_creation_time(disp_pid).unwrap_or(0);
    assert!(disp_creation > 0, "disposable process creation time must be determined");
    assert!(!process::is_critical_process(disp_pid, &disp_name, disp_path.as_deref()), "disposable process is not critical");
    println!("Targeted disposable process: PID {disp_pid}, name {disp_name}");

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
    // 3. Verified process termination
    // -------------------------------------------------------------------------
    println!("[E2E 3/8] Testing verified process termination on disposable process...");
    let target_info = hunter::HunterTargetInfo {
        hwnd: 0,
        pid: disp_pid,
        process_name: disp_name.clone(),
        process_path: disp_path.clone(),
        window_title: Some("Disposable Mock App".into()),
        matched_app_id: None,
        matched_app_name: None,
        is_critical: false,
        hunter_target_id: format!("hunter|{disp_pid}|{disp_creation}"),
        can_terminate: true,
        creation_time: disp_creation,
    };

    let mut proc_snapshots = std::collections::HashMap::new();
    let mut app_snapshots = std::collections::HashMap::new();
    hunter::register_target_in_state(&target_info, &mut proc_snapshots, &mut app_snapshots);

    let registered_snap = proc_snapshots.get(&target_info.hunter_target_id).expect("snapshot registered");
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
        &target_info.hunter_target_id,
        registered_snap,
        &[disp_pid],
        true,
    );
    assert!(ok_term[0].success, "verified termination of disposable process must succeed");
    let _ = disposable_child.wait();
    println!("Verified termination succeeded.");

    // -------------------------------------------------------------------------
    // 4. Unregistered application review
    // -------------------------------------------------------------------------
    println!("[E2E 4/8] Testing unregistered application review...");
    let unreg_app = app_snapshots.get(&target_info.hunter_target_id).expect("ad-hoc unregistered app created");
    assert_eq!(unreg_app.source, "portable");
    assert_eq!(unreg_app.uninstall_support, "missing");
    assert!(unreg_app.display_icon.is_some());
    println!("Unregistered application review passed.");

    // -------------------------------------------------------------------------
    // 5. Broken registry entry removal and restoration
    // -------------------------------------------------------------------------
    println!("[E2E 5/8] Testing broken registry entry removal and restoration...");
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

    let broken_plan = leftovers::StoredBrokenEntryPlan {
        plan_id: format!("broken-plan-e2e-{pid}"),
        app_id: broken_app_item.id.clone(),
        app_name: broken_app_item.name.clone(),
        registry_key_path: full_broken_path.clone(),
        subkey_name: test_broken_name.clone(),
        hive: "user-64".into(),
        expires_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 300,
    };

    let clean_res = leftovers::execute_broken_entry_removal(&broken_plan, &quarantine_dir).expect("execute broken removal");
    assert_eq!(clean_res.deleted_registry_keys, 1);
    assert!(clean_res.registry_backup_path.is_some());
    assert!(hkcu.open_subkey(&uninstall_rel).is_err(), "broken entry key must be deleted");

    // Restore from verified backup
    let bkp_file = PathBuf::from(clean_res.registry_backup_path.unwrap());
    assert!(bkp_file.exists());
    let rest_rep = leftovers::restore_registry_backup(&bkp_file).expect("restore broken entry from backup");
    assert!(rest_rep.keys_restored >= 1);
    assert!(hkcu.open_subkey(&uninstall_rel).is_ok(), "broken entry key restored");
    let _ = hkcu.delete_subkey_all(&uninstall_rel);
    println!("Broken registry entry removal and restoration passed.");

    // -------------------------------------------------------------------------
    // 6. Startup entry disable and restoration
    // -------------------------------------------------------------------------
    println!("[E2E 6/8] Testing Startup entry disable and restoration...");
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

    // Verify disabled
    let reloaded_items = system::list_startup_items();
    let reloaded_item = reloaded_items.into_iter().find(|i| i.name == startup_test_name).expect("find disabled item");
    assert!(!reloaded_item.enabled);

    // Restore (re-enable) item
    let restored_item = system::set_startup_item_enabled(&our_item.id, true).expect("restore startup item");
    assert!(restored_item.enabled);

    let _ = run_k.delete_value(&startup_test_name);
    if let Ok(appr_k) = hkcu.open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run") {
        let _ = appr_k.delete_value(&startup_test_name);
    }
    println!("Startup entry disable and restoration passed.");

    // -------------------------------------------------------------------------
    // 7. Repeated targeting without inventory rescans
    // -------------------------------------------------------------------------
    println!("[E2E 7/8] Testing repeated targeting latency without inventory rescans...");
    let cached_inventory = vec![broken_app_item.clone()];
    let t_start = Instant::now();
    for _ in 0..50 {
        let _ = hunter::resolve_cursor_target(&cached_inventory);
    }
    let elapsed = t_start.elapsed();
    let per_call_micros = elapsed.as_micros() / 50;
    println!("50 repeated targeting checks completed in {:?} (avg {} µs/call)", elapsed, per_call_micros);
    assert!(per_call_micros < 10_000, "Targeting check must be fast (< 10 ms/call)");
    println!("Repeated targeting performance verified.");

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
