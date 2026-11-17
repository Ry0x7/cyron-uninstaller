#![windows_subsystem = "windows"]

//! Disposable Windows fixtures for the real-desktop E2E suite.
//!
//! This binary is never part of Cyron. It creates isolated HKCU uninstall
//! entries, a startup entry, a developer project, an analyzer tree and one
//! absent safe cache target. Cleanup is limited to those exact paths/keys.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::time::{SystemTime, UNIX_EPOCH};
use winreg::enums::*;
use winreg::RegKey;

const UNINSTALL_ROOT: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";
const RUN_ROOT: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVED_ROOT: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

fn main() {
    let args: Vec<String> = env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("");
    let result = match mode {
        "setup" => setup(&args),
        "teardown" => teardown(&args),
        "--uninstall-key" => uninstall(&args, true, false),
        "--uninstall-keep" => uninstall(&args, false, false),
        "--uninstall-cancel" => uninstall(&args, false, true),
        "--uninstall-fail" => {
            write_marker(arg(&args, "--marker"));
            exit(1);
        }
        _ => Err("usage: desktop_e2e_fixture setup|teardown|--uninstall-*".into()),
    };
    if let Err(error) = result {
        eprintln!("desktop_e2e_fixture: {error}");
        exit(2);
    }
}

fn arg<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|value| value == name)
        .and_then(|index| args.get(index + 1))
        .map(String::as_str)
}

fn uninstall(args: &[String], delete_key: bool, cancelled: bool) -> Result<(), String> {
    if let Some(marker) = arg(args, "--marker") {
        write_marker(Some(marker));
    }
    if delete_key {
        let subkey = arg(args, "--subkey").ok_or("missing --subkey")?;
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let path = format!("{UNINSTALL_ROOT}\\{subkey}");
        match hkcu.delete_subkey_all(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("could not delete fixture key: {error}")),
        }
    }
    if cancelled {
        exit(1602);
    }
    Ok(())
}

fn write_marker(path: Option<&str>) {
    let Some(path) = path else { return };
    if let Some(parent) = Path::new(path).parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, b"cyron desktop e2e uninstaller ran\n");
}

fn setup(args: &[String]) -> Result<(), String> {
    let workspace = args.get(2).ok_or("missing workspace")?;
    let state_path = args.get(3).ok_or("missing state path")?;
    let workspace = PathBuf::from(workspace);
    if workspace.exists() {
        fs::remove_dir_all(&workspace).map_err(|error| error.to_string())?;
    }
    fs::create_dir_all(&workspace).map_err(|error| error.to_string())?;
    let current_exe = env::current_exe().map_err(|error| error.to_string())?;
    let bin_dir = workspace.join("bin");
    fs::create_dir_all(&bin_dir).map_err(|error| error.to_string())?;
    let uninstaller = bin_dir.join("cyron-e2e-uninstaller.exe");
    fs::copy(&current_exe, &uninstaller).map_err(|error| error.to_string())?;

    let run_id = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    let markers = workspace.join("markers");
    fs::create_dir_all(&markers).map_err(|error| error.to_string())?;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let uninstall_root = hkcu
        .create_subkey(UNINSTALL_ROOT)
        .map_err(|error| error.to_string())?
        .0;
    // Remove fixture entries left behind by an interrupted earlier run.
    for stale in uninstall_root.enum_keys().flatten().collect::<Vec<_>>() {
        if stale.starts_with("CyronDesktopE2E_") {
            let _ = hkcu.delete_subkey_all(format!("{UNINSTALL_ROOT}\\{stale}"));
        }
    }
    if let Ok(run_root) = hkcu.open_subkey(RUN_ROOT) {
        for stale in run_root.enum_values().flatten().map(|(name, _)| name).collect::<Vec<_>>() {
            if stale.starts_with("CyronDesktopE2EStartup_") {
                let _ = run_root.delete_value(&stale);
            }
        }
    }

    let mut apps = BTreeMap::new();
    for (kind, label, command) in [
        ("removable", "Cyron Desktop E2E Removable", "uninstall-key"),
        ("remaining", "Cyron Desktop E2E Remaining", "uninstall-keep"),
        ("cancelled", "Cyron Desktop E2E Cancelled", "uninstall-cancel"),
    ] {
        let key = format!("CyronDesktopE2E_{run_id}_{kind}");
        let install = workspace.join("apps").join(kind);
        fs::create_dir_all(&install).map_err(|error| error.to_string())?;
        fs::copy(&uninstaller, install.join("fixture-app.exe"))
            .map_err(|error| error.to_string())?;
        fs::write(install.join("data.bin"), vec![0x43, 0x59, 0x52, 0x4f, 0x4e])
            .map_err(|error| error.to_string())?;
        let marker = markers.join(format!("{kind}.txt"));
        let uninstall_string = if command == "uninstall-key" {
            format!(
                "\"{}\" --uninstall-key --subkey {} --marker \"{}\"",
                uninstaller.display(),
                key,
                marker.display()
            )
        } else {
            format!(
                "\"{}\" --{} --marker \"{}\"",
                uninstaller.display(),
                command,
                marker.display()
            )
        };
        let (app_key, _) = uninstall_root
            .create_subkey(&key)
            .map_err(|error| error.to_string())?;
        app_key
            .set_value("DisplayName", &label)
            .map_err(|error| error.to_string())?;
        app_key
            .set_value("DisplayVersion", &"1.2.3")
            .map_err(|error| error.to_string())?;
        app_key
            .set_value("Publisher", &"Cyron Desktop E2E")
            .map_err(|error| error.to_string())?;
        app_key
            .set_value("InstallLocation", &install.to_string_lossy().to_string())
            .map_err(|error| error.to_string())?;
        app_key
            .set_value("UninstallString", &uninstall_string)
            .map_err(|error| error.to_string())?;
        app_key
            .set_value("EstimatedSize", &24u32)
            .map_err(|error| error.to_string())?;
        app_key
            .set_value("NoModify", &1u32)
            .map_err(|error| error.to_string())?;
        app_key
            .set_value("NoRepair", &1u32)
            .map_err(|error| error.to_string())?;
        apps.insert(
            kind.to_string(),
            serde_json::json!({
                "name": label,
                "key": key,
                "installLocation": install.to_string_lossy(),
                "marker": marker.to_string_lossy(),
            }),
        );
    }

    let broken_key = format!("CyronDesktopE2E_{run_id}_broken");
    let broken_install = workspace.join("apps").join("broken");
    fs::create_dir_all(&broken_install).map_err(|error| error.to_string())?;
    let (broken_reg, _) = uninstall_root
        .create_subkey(&broken_key)
        .map_err(|error| error.to_string())?;
    broken_reg
        .set_value("DisplayName", &"Cyron Desktop E2E Broken")
        .map_err(|error| error.to_string())?;
    broken_reg
        .set_value("Publisher", &"Cyron Desktop E2E")
        .map_err(|error| error.to_string())?;
    broken_reg
        .set_value(
            "UninstallString",
            &format!(
                "\"{}\" --missing",
                workspace
                    .join("missing")
                    .join("does-not-exist.exe")
                    .display()
            ),
        )
        .map_err(|error| error.to_string())?;
    broken_reg
        .set_value("InstallLocation", &broken_install.to_string_lossy().to_string())
        .map_err(|error| error.to_string())?;
    apps.insert(
        "broken".to_string(),
        serde_json::json!({
            "name": "Cyron Desktop E2E Broken",
            "key": broken_key,
            "installLocation": broken_install.to_string_lossy(),
            "marker": null,
        }),
    );

    let startup_name = format!("CyronDesktopE2EStartup_{run_id}");
    let startup_command = format!(
        "\"{}\" --startup-fixture",
        workspace
            .join("bin")
            .join("cyron-e2e-uninstaller.exe")
            .display()
    );
    hkcu.create_subkey(RUN_ROOT)
        .map_err(|error| error.to_string())?
        .0
        .set_value(&startup_name, &startup_command)
        .map_err(|error| error.to_string())?;

    let project = workspace.join("project");
    write_file(&project.join("package.json"), r#"{"name":"cyron-desktop-e2e-project","version":"1.0.0","private":true,"scripts":{"build":"tsc"},"dependencies":{"react":"^19.0.0"}}"#)?;
    write_file(&project.join("package-lock.json"), r#"{"name":"cyron-desktop-e2e-project","lockfileVersion":3,"packages":{}}"#)?;
    write_file(&project.join("src").join("index.ts"), "export const value = 1;\n")?;
    write_file(&project.join("README.md"), "# Cyron Desktop E2E project\n")?;
    write_file(&project.join(".env"), "CYRON_E2E_SECRET=never-clean-this\n")?;
    write_file(&project.join(".git").join("config"), "[core]\n\trepositoryformatversion = 0\n")?;
    write_file(&project.join("node_modules").join("react").join("package.json"), r#"{"name":"react","version":"19.0.0"}"#)?;
    write_file(&project.join("node_modules").join("react").join("index.js"), "module.exports = {};\n")?;
    write_file(&project.join("node_modules").join(".package-lock.json"), "{}\n")?;
    write_file(&project.join("dist").join("app.js"), "console.log('built');\n")?;
    write_file(&project.join("dist").join("app.css"), "body{color:black}\n")?;

    let analyzer = workspace.join("analyzer");
    write_file(&analyzer.join("alpha").join("one.bin"), &vec![0x61; 4096])?;
    write_file(&analyzer.join("alpha").join("two.bin"), &vec![0x62; 4096])?;
    write_file(&analyzer.join("beta").join("unique.bin"), &vec![0x63; 8192])?;
    write_file(&analyzer.join("beta").join("nested").join("one-copy.bin"), &vec![0x61; 4096])?;
    write_file(&analyzer.join("notes.txt"), "analyzer fixture\n")?;

    let explorer = workspace.join("explorer");
    write_file(&explorer.join("sample.txt"), "cyron explorer fixture\n")?;
    fs::create_dir_all(explorer.join("sample-folder"))
        .map_err(|error| error.to_string())?;

    let local_appdata = env::var("LOCALAPPDATA").map_err(|_| "LOCALAPPDATA missing")?;
    let yarn_parent = Path::new(&local_appdata).join("Yarn");
    let yarn_existed = yarn_parent.exists();
    let cache_dir = yarn_parent.join("Cache");
    let cache_existed = cache_dir.exists();
    fs::create_dir_all(&cache_dir).map_err(|error| error.to_string())?;
    let cache_file = cache_dir.join(format!("cyron-desktop-e2e-{run_id}.tgz"));
    fs::write(&cache_file, vec![0x43; 16_384]).map_err(|error| error.to_string())?;

    let state = serde_json::json!({
        "runId": run_id,
        "workspace": workspace.to_string_lossy(),
        "uninstaller": uninstaller.to_string_lossy(),
        "apps": apps,
        "startup": { "name": startup_name, "command": startup_command },
        "project": {
            "root": project.to_string_lossy(),
            "nodeModules": project.join("node_modules").to_string_lossy(),
            "dist": project.join("dist").to_string_lossy(),
            "source": project.join("src").join("index.ts").to_string_lossy(),
            "env": project.join(".env").to_string_lossy(),
            "git": project.join(".git").to_string_lossy(),
            "keepFile": project.join("src").join("index.ts").to_string_lossy(),
        },
        "analyzer": {
            "root": analyzer.to_string_lossy(),
            "duplicateA": analyzer.join("alpha").join("one.bin").to_string_lossy(),
            "duplicateB": analyzer.join("beta").join("nested").join("one-copy.bin").to_string_lossy(),
        },
        "cache": {
            "file": cache_file.to_string_lossy(),
            "path": cache_dir.to_string_lossy(),
            "label": "Yarn Package cache",
            "yarnExisted": yarn_existed,
            "cacheExisted": cache_existed,
        },
        "explorer": {
            "file": explorer.join("sample.txt").to_string_lossy(),
            "folder": explorer.join("sample-folder").to_string_lossy(),
        }
    });
    fs::write(state_path, serde_json::to_string_pretty(&state).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn write_file(path: &Path, contents: impl AsRef<[u8]>) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(path, contents).map_err(|error| error.to_string())
}

fn teardown(args: &[String]) -> Result<(), String> {
    let state_path = args.get(2).ok_or("missing state path")?;
    let raw = fs::read_to_string(state_path).map_err(|error| error.to_string())?;
    let state: serde_json::Value = serde_json::from_str(&raw).map_err(|error| error.to_string())?;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Some(apps) = state.get("apps").and_then(|value| value.as_object()) {
        for app in apps.values() {
            if let Some(key) = app.get("key").and_then(|value| value.as_str()) {
                let path = format!("{UNINSTALL_ROOT}\\{key}");
                let _ = hkcu.delete_subkey_all(path);
            }
        }
    }
    if let Some(name) = state
        .get("startup")
        .and_then(|value| value.get("name"))
        .and_then(|value| value.as_str())
    {
        if let Ok(run) = hkcu.open_subkey_with_flags(RUN_ROOT, KEY_ALL_ACCESS) {
            let _ = run.delete_value(name);
        }
        if let Ok(approved) = hkcu.open_subkey_with_flags(APPROVED_ROOT, KEY_ALL_ACCESS) {
            let _ = approved.delete_value(name);
        }
    }
    if let Some(cache) = state.get("cache") {
        if let Some(file) = cache.get("file").and_then(|value| value.as_str()) {
            let _ = fs::remove_file(file);
        }
        let cache_existed = cache
            .get("cacheExisted")
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        if !cache_existed {
            if let Some(path) = cache.get("path").and_then(|value| value.as_str()) {
                let cache_dir = PathBuf::from(path);
                let yarn_parent = cache_dir.parent();
                let _ = fs::remove_dir(&cache_dir);
                let yarn_existed = cache
                    .get("yarnExisted")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(true);
                if !yarn_existed {
                    if let Some(parent) = yarn_parent {
                        let _ = fs::remove_dir(parent);
                    }
                }
            }
        }
    }
    if let Some(workspace) = state.get("workspace").and_then(|value| value.as_str()) {
        let _ = fs::remove_dir_all(workspace);
    }
    Ok(())
}
