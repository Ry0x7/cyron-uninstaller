//! Disposable Win32 discovery/uninstaller/quarantine fixture.
//!
//! The fixture writes one temporary HKCU uninstall registration and copies its
//! own binary into `%TEMP%\cyron-phase2-win32-<pid>\App`. The copied binary acts
//! as the vendor uninstaller: with `--uninstall <subkey>` it removes the
//! registration and one data file, then exits 0. The parent then performs real
//! Cyron discovery, plan creation, process execution, inventory reconciliation,
//! leftover scan, quarantine and restore. Nothing outside the disposable HKCU
//! key and `%TEMP%` folder is touched.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{SystemTime, UNIX_EPOCH};
use winreg::enums::*;
use winreg::RegKey;

const MARKER: &str = "PHASE2_WIN32_FIXTURE_OK";
const UNINSTALL_ROOT: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--uninstall") {
        run_as_vendor_uninstaller(&args);
        return;
    }
    run_fixture();
}

fn run_as_vendor_uninstaller(args: &[String]) {
    let Some(subkey) = args.get(2) else {
        eprintln!("fixture uninstaller: missing registry subkey");
        std::process::exit(2);
    };
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let removed = hkcu
        .delete_subkey_all(format!("{UNINSTALL_ROOT}\\{subkey}"))
        .is_ok();
    if !removed {
        eprintln!("fixture uninstaller: could not remove the registration");
        std::process::exit(1605);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let _ = fs::remove_file(dir.join("payload.dat"));
        }
    }
    println!("fixture uninstaller: registration removed");
}

fn run_fixture() {
    let pid = std::process::id();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let subkey = format!("CyronPhase2Fixture_{pid}_{nonce}");
    let display_name = format!("Cyron Phase 2 Fixture {pid}-{nonce}");
    let root = std::env::temp_dir().join(format!("cyron-phase2-win32-{pid}-{nonce}"));
    let app_dir = root.join("App");
    fs::create_dir_all(&app_dir).expect("create fixture app folder");
    let vendor_exe = app_dir.join("fixture-uninstall.exe");
    let current = std::env::current_exe().expect("current fixture executable");
    fs::copy(&current, &vendor_exe).expect("copy fixture uninstaller");
    fs::write(app_dir.join("payload.dat"), b"disposable phase 2 payload").expect("write payload");

    let uninstall_string = format!("\"{}\" --uninstall {subkey}", vendor_exe.display());
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    {
        let (key, _) = hkcu
            .create_subkey(format!("{UNINSTALL_ROOT}\\{subkey}"))
            .expect("create disposable uninstall key");
        key.set_value("DisplayName", &display_name).expect("set display name");
        key.set_value("DisplayVersion", &"1.2.3").expect("set version");
        key.set_value("Publisher", &"Cyron Fixture Publisher").expect("set publisher");
        key.set_value("InstallLocation", &app_dir.to_string_lossy().to_string())
            .expect("set install location");
        key.set_value("UninstallString", &uninstall_string).expect("set uninstall command");
        key.set_value("EstimatedSize", &16u32).expect("set size");
    }

    let guard = FixtureGuard {
        subkey: subkey.clone(),
        root: root.clone(),
    };

    let app = cyron_lib::apps::discover_apps()
        .apps
        .into_iter()
        .find(|app| app.name == display_name)
        .expect("fixture app should be discovered from the live HKCU uninstall key");
    assert_eq!(app.id, format!("user-64|{subkey}"));
    assert_eq!(app.source, "win32");
    assert_eq!(app.uninstall_support, "standard");
    assert_eq!(
        app.install_location.as_deref().map(Path::new),
        Some(app_dir.as_path())
    );

    let plan = cyron_lib::uninstall::create_uninstall_plan(&app, false)
        .expect("fixture uninstaller command should form a valid plan");
    let result = cyron_lib::uninstall::execute_uninstaller(
        &app.id,
        &plan.command,
        &AtomicBool::new(false),
        120,
    )
    .expect("fixture uninstaller should launch");
    assert_eq!(result.exit_code, Some(0));

    let after = cyron_lib::apps::discover_apps();
    let still_registered = after.apps.iter().any(|current| current.id == app.id);
    assert!(!still_registered, "vendor uninstaller removed the registration");
    let (status, _) = cyron_lib::uninstall::reconcile_status(
        result.exit_code,
        still_registered,
        result.restart_required,
    );
    assert_eq!(status, "completed");

    let (leftover_plan, report) = cyron_lib::leftovers::scan_app_leftovers(
        &app.id,
        &app.name,
        app.publisher.as_deref(),
        app.install_location.as_deref(),
    );
    let install_candidate = report
        .candidates
        .iter()
        .find(|candidate| Path::new(&candidate.path) == app_dir.as_path())
        .expect("exact install folder should be a leftover candidate");
    assert_eq!(install_candidate.risk, "safe");
    assert!(install_candidate.selected);

    let quarantine_root = root.join("quarantine");
    fs::create_dir_all(&quarantine_root).expect("create quarantine root");
    let clean = cyron_lib::leftovers::clean_leftovers(
        &leftover_plan,
        &[install_candidate.id.clone()],
        &quarantine_root,
    )
    .expect("fixture leftover cleanup should succeed");
    assert!(clean.quarantined_files > 0);
    assert!(!vendor_exe.exists(), "vendor executable moved to quarantine");
    let run_id = clean.run_id.clone();
    let restored = cyron_lib::cleanup::restore_batch(&quarantine_root, &run_id)
        .expect("fixture quarantine should restore");
    assert_eq!(restored.failed, 0);
    assert!(vendor_exe.exists(), "vendor executable restored");
    assert!(
        !app_dir.join("payload.dat").exists(),
        "the vendor uninstaller deleted its payload"
    );

    drop(guard);
    println!("{MARKER}");
}

struct FixtureGuard {
    subkey: String,
    root: PathBuf,
}

impl Drop for FixtureGuard {
    fn drop(&mut self) {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let _ = hkcu.delete_subkey_all(format!("{UNINSTALL_ROOT}\\{}", self.subkey));
        let _ = fs::remove_dir_all(&self.root);
    }
}
