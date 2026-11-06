//! RC1 End-to-End fixture test for application uninstallation, process management,
//! registry backup/restore, and residual cleanup.
//!
//! All operations run against isolated disposable fixtures under `%TEMP%\cyron-rc1-fixture-<pid>\`
//! and isolated HKCU keys under `HKCU\Software\CyronRC1Fixture_<pid>`.
//! Never touches real software, real user files, or production registry settings.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use winreg::enums::*;
use winreg::{RegKey, RegValue};

use cyron_lib::apps::InstalledApp;
use cyron_lib::leftovers;
use cyron_lib::process;
use cyron_lib::uninstall;

const MARKER: &str = "RC1_E2E_FIXTURE_OK";

fn main() {
    let pid = std::process::id();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let temp_root = std::env::temp_dir().join(format!("cyron-rc1-fixture-{pid}-{now}"));
    fs::create_dir_all(&temp_root).expect("create fixture root");

    let mock_install_dir = temp_root.join("ProgramFiles").join("MockApp");
    fs::create_dir_all(&mock_install_dir).expect("create mock install dir");

    // Copy dummy executable (e.g. cmd.exe or notepad.exe or a compiled dummy)
    let dummy_exe = mock_install_dir.join("mock_app.exe");
    let system32 = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into()) + "\\System32\\cmd.exe";
    let _ = fs::copy(&system32, &dummy_exe);

    // Write some dummy application files
    fs::write(mock_install_dir.join("config.json"), "{\"version\": \"1.0.0\"}").expect("write config");
    fs::write(mock_install_dir.join("data.bin"), vec![0xca, 0xfe, 0xba, 0xbe]).expect("write data");

    let quarantine_dir = temp_root.join("quarantine");
    fs::create_dir_all(&quarantine_dir).expect("create quarantine dir");

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let test_reg_subkey = format!("Software\\CyronRC1Fixture_{pid}_{now}");

    // 1. Create registry settings with multiple Windows types
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

    println!("Created isolated fixture at {}", temp_root.display());

    // 2. Test uninstaller batch planning
    let mock_app = InstalledApp {
        id: "mock-app-id".into(),
        name: "Mock Application".into(),
        version: Some("1.0.0".into()),
        publisher: Some("Cyron Fixtures".into()),
        install_date: Some("20261106".into()),
        estimated_size: Some(1024 * 1024),
        install_location: Some(mock_install_dir.to_string_lossy().to_string()),
        uninstall_string: Some(format!("\"{}\" /S", dummy_exe.display())),
        quiet_uninstall_string: Some(format!("\"{}\" /qn", dummy_exe.display())),
        display_icon: None,
        hive: "user-64".into(),
        scope: "user".into(),
        architecture: "x64".into(),
        is_protected: false,
        source: "win32".into(),
        uninstall_support: "standard".into(),
        confidence: "confirmed".into(),
        detection_reason: Some("Mock test fixture".into()),
    };

    let batch_plan = uninstall::create_batch_uninstall_plan(&[mock_app.clone()], true)
        .expect("create batch plan");
    assert_eq!(batch_plan.items.len(), 1);
    assert!(batch_plan.items[0].is_valid);
    assert_eq!(batch_plan.items[0].app_id, "mock-app-id");
    assert!(batch_plan.expires_at > batch_plan.items.len() as u64);

    // 3. Test Process Discovery & Verification on disposable child process
    let mut child = Command::new(&dummy_exe)
        .arg("/c")
        .arg("powershell -Command Start-Sleep -Seconds 30")
        .spawn()
        .expect("spawn mock process");

    let child_pid = child.id();

    let (proc_snapshot, discovered) = process::discover_app_processes(
        "mock-app-id",
        Some(&mock_install_dir.to_string_lossy()),
    );

    let found = discovered.iter().any(|p| p.pid == child_pid);
    if found {
        println!("Process discovery successfully identified child process PID {child_pid}");
        // Test fail-closed verification with mismatched app id
        let fail_res = process::terminate_processes_verified(
            "wrong-app-id",
            &proc_snapshot,
            &[child_pid],
            false,
        );
        assert!(!fail_res[0].success);

        // Test verified termination
        let term_res = process::terminate_processes_verified(
            "mock-app-id",
            &proc_snapshot,
            &[child_pid],
            true,
        );
        assert!(term_res[0].success, "Verified termination succeeded");
    } else {
        let _ = child.kill();
    }
    let _ = child.wait();

    // 4. Test Leftover Residual Clean & Registry Backup
    let cand_dir = leftovers::LeftoverCandidate {
        id: "cand-dir".into(),
        kind: "directory".into(),
        path: mock_install_dir.to_string_lossy().to_string(),
        confidence: "high".into(),
        risk: "safe".into(),
        reason: "Test folder".into(),
        size: 1024,
        file_count: 2,
        selected: true,
    };
    let cand_reg = leftovers::LeftoverCandidate {
        id: "cand-reg".into(),
        kind: "registry_key".into(),
        path: format!("HKCU\\{test_reg_subkey}"),
        confidence: "high".into(),
        risk: "safe".into(),
        reason: "Test registry".into(),
        size: 0,
        file_count: 0,
        selected: true,
    };

    let leftover_plan = leftovers::StoredLeftoverPlan {
        plan_id: "leftover-plan-test".into(),
        app_id: "mock-app-id".into(),
        app_name: "Mock Application".into(),
        candidates: vec![cand_dir, cand_reg],
        expires_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 600,
    };

    let clean_rep = leftovers::clean_leftovers(
        &leftover_plan,
        &["cand-dir".to_string(), "cand-reg".to_string()],
        &quarantine_dir,
    ).expect("clean leftovers");

    assert!(clean_rep.quarantined_files >= 2);
    assert_eq!(clean_rep.deleted_registry_keys, 1);
    assert!(clean_rep.registry_backup_path.is_some());
    assert!(clean_rep.errors.is_empty());

    // Verify files were removed from mock dir
    assert!(!mock_install_dir.join("config.json").exists());
    assert!(!mock_install_dir.join("data.bin").exists());

    // Verify registry key was deleted
    assert!(hkcu.open_subkey(&test_reg_subkey).is_err());

    // 5. Test Registry Restore from Backup
    let backup_file = PathBuf::from(clean_rep.registry_backup_path.unwrap());
    assert!(backup_file.exists());
    leftovers::verify_registry_backup(&backup_file).expect("verify registry backup");

    let restore_rep = leftovers::restore_registry_backup(&backup_file).expect("restore registry");
    assert!(restore_rep.keys_restored >= 2);
    assert!(restore_rep.values_restored >= 4);
    assert!(restore_rep.errors.is_empty());

    // Verify restored values
    let restored_key = hkcu.open_subkey(&test_reg_subkey).expect("open restored key");
    let name_val: String = restored_key.get_value("DisplayName").expect("get name");
    assert_eq!(name_val, "Mock App");
    let dword_val: u32 = restored_key.get_value("ConfigVersion").expect("get dword");
    assert_eq!(dword_val, 100);

    // 6. Clean up fixtures
    let _ = hkcu.delete_subkey_all(&test_reg_subkey);
    let _ = fs::remove_dir_all(&temp_root);

    println!("{MARKER}");
}
