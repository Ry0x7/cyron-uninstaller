use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use winreg::enums::*;
use winreg::RegKey;

/// Represents where the application should route upon launch based on CLI arguments.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StartupRoute {
    pub view: String,
    pub path: Option<String>,
    pub subview: Option<String>,
    pub target_app_id: Option<String>,
    pub action: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerIntegrationStatus {
    pub is_registered: bool,
    pub folder_actions_registered: bool,
    pub exe_actions_registered: bool,
    pub registered_exe_path: Option<String>,
    pub current_exe_path: String,
    pub needs_repair: bool,
    pub details: Vec<String>,
}

/// Registry verbs registered under HKCU\Software\Classes
struct ShellVerb {
    parent_key: &'static str,
    verb_id: &'static str,
    menu_text: &'static str,
    arg_flag: &'static str,
    use_quote_v: bool,
}

const SHELL_VERBS: &[ShellVerb] = &[
    // Folders & Directories
    ShellVerb {
        parent_key: r"Directory\shell",
        verb_id: "Cyron.Analyze",
        menu_text: "Analyze with Cyron",
        arg_flag: "--analyze",
        use_quote_v: true,
    },
    ShellVerb {
        parent_key: r"Directory\shell",
        verb_id: "Cyron.Duplicates",
        menu_text: "Find Duplicates with Cyron",
        arg_flag: "--duplicates",
        use_quote_v: true,
    },
    ShellVerb {
        parent_key: r"Directory\shell",
        verb_id: "Cyron.Storage",
        menu_text: "View Storage Breakdown with Cyron",
        arg_flag: "--storage",
        use_quote_v: true,
    },
    // Folder background (when right clicking inside a directory)
    ShellVerb {
        parent_key: r"Directory\Background\shell",
        verb_id: "Cyron.Analyze",
        menu_text: "Analyze this folder with Cyron",
        arg_flag: "--analyze",
        use_quote_v: true,
    },
    // Drives
    ShellVerb {
        parent_key: r"Drive\shell",
        verb_id: "Cyron.Analyze",
        menu_text: "Analyze with Cyron",
        arg_flag: "--analyze",
        use_quote_v: true,
    },
    ShellVerb {
        parent_key: r"Drive\shell",
        verb_id: "Cyron.Duplicates",
        menu_text: "Find Duplicates with Cyron",
        arg_flag: "--duplicates",
        use_quote_v: true,
    },
    ShellVerb {
        parent_key: r"Drive\shell",
        verb_id: "Cyron.Storage",
        menu_text: "View Storage Breakdown with Cyron",
        arg_flag: "--storage",
        use_quote_v: true,
    },
    // Executables
    ShellVerb {
        parent_key: r"SystemFileAssociations\.exe\shell",
        verb_id: "Cyron.Uninstall",
        menu_text: "Uninstall with Cyron",
        arg_flag: "--uninstall-target",
        use_quote_v: false,
    },
    ShellVerb {
        parent_key: r"SystemFileAssociations\.exe\shell",
        verb_id: "Cyron.Related",
        menu_text: "Find Related Files with Cyron",
        arg_flag: "--find-related",
        use_quote_v: false,
    },
];

/// Validates that a path provided via CLI exists and is not malicious.
pub fn sanitize_target_path(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim().trim_matches('"');
    if trimmed.is_empty() {
        return None;
    }
    let path = Path::new(trimmed);
    if !path.is_absolute() {
        return None;
    }
    // Check that path exists
    if !path.exists() {
        return None;
    }
    // Canonicalize or return cleaned path
    std::fs::canonicalize(path).ok().or_else(|| Some(path.to_path_buf()))
}

/// Parses command line arguments into a startup routing decision.
pub fn parse_startup_args(args: &[String]) -> Option<StartupRoute> {
    if args.len() < 2 {
        return None;
    }

    let mut i = 1;
    while i < args.len() {
        let flag = args[i].as_str();
        match flag {
            "--analyze" | "-a" => {
                let target = args.get(i + 1)?;
                let sanitized = sanitize_target_path(target)?;
                return Some(StartupRoute {
                    view: "analyzer".to_string(),
                    path: Some(sanitized.to_string_lossy().to_string()),
                    subview: Some("map".to_string()),
                    target_app_id: None,
                    action: Some("scan".to_string()),
                    note: None,
                });
            }
            "--duplicates" | "-d" => {
                let target = args.get(i + 1)?;
                let sanitized = sanitize_target_path(target)?;
                return Some(StartupRoute {
                    view: "analyzer".to_string(),
                    path: Some(sanitized.to_string_lossy().to_string()),
                    subview: Some("duplicates".to_string()),
                    target_app_id: None,
                    action: Some("scan".to_string()),
                    note: None,
                });
            }
            "--storage" | "-s" => {
                let target = args.get(i + 1)?;
                let sanitized = sanitize_target_path(target)?;
                return Some(StartupRoute {
                    view: "analyzer".to_string(),
                    path: Some(sanitized.to_string_lossy().to_string()),
                    subview: Some("storage".to_string()),
                    target_app_id: None,
                    action: Some("scan".to_string()),
                    note: None,
                });
            }
            "--uninstall-target" | "-u" => {
                let target = args.get(i + 1)?;
                let sanitized = sanitize_target_path(target)?;
                let path_str = sanitized.to_string_lossy().to_string();
                
                // Match with registered installed apps
                let inventory = crate::apps::discover_apps();
                let matched_app = find_matching_app_for_exe(&inventory.apps, &sanitized);

                return Some(StartupRoute {
                    view: "uninstaller".to_string(),
                    path: Some(path_str.clone()),
                    subview: None,
                    target_app_id: matched_app.as_ref().map(|a| a.id.clone()),
                    action: if matched_app.is_some() {
                        Some("review".to_string())
                    } else {
                        Some("unregistered".to_string())
                    },
                    note: if matched_app.is_some() {
                        None
                    } else {
                        Some(format!("Executable is not registered in Windows uninstall registry: {path_str}"))
                    },
                });
            }
            "--find-related" | "-r" => {
                let target = args.get(i + 1)?;
                let sanitized = sanitize_target_path(target)?;
                let path_str = sanitized.to_string_lossy().to_string();

                let inventory = crate::apps::discover_apps();
                let matched_app = find_matching_app_for_exe(&inventory.apps, &sanitized);

                return Some(StartupRoute {
                    view: "uninstaller".to_string(),
                    path: Some(path_str),
                    subview: None,
                    target_app_id: matched_app.as_ref().map(|a| a.id.clone()),
                    action: Some("leftovers".to_string()),
                    note: None,
                });
            }
            _ => {
                i += 1;
            }
        }
    }

    None
}

/// Matches an executable path against known installed applications.
fn find_matching_app_for_exe<'a>(
    apps: &'a [crate::apps::InstalledApp],
    exe_path: &Path,
) -> Option<&'a crate::apps::InstalledApp> {
    let exe_norm = exe_path.to_string_lossy().to_lowercase();
    let exe_name = exe_path.file_name()?.to_string_lossy().to_lowercase();

    // 1. Direct match on display_icon
    for app in apps {
        if let Some(ref icon) = app.display_icon {
            let icon_clean = icon.split(',').next().unwrap_or(icon).trim().trim_matches('"');
            if !icon_clean.is_empty() && icon_clean.to_lowercase() == exe_norm {
                return Some(app);
            }
        }
    }

    // 2. Direct match or prefix match on install_location
    let mut best_match: Option<(&crate::apps::InstalledApp, usize)> = None;
    for app in apps {
        if let Some(ref loc) = app.install_location {
            let loc_clean = loc.trim().trim_matches('"');
            if !loc_clean.is_empty() {
                let loc_norm = loc_clean.to_lowercase();
                if exe_norm.starts_with(&loc_norm) {
                    let len = loc_norm.len();
                    if best_match.as_ref().map_or(true, |(_, best_len)| len > *best_len) {
                        best_match = Some((app, len));
                    }
                }
            }
        }
    }

    if let Some((app, _)) = best_match {
        return Some(app);
    }

    // 3. Match uninstall_string if it contains the executable
    for app in apps {
        if let Some(ref uninst) = app.uninstall_string {
            if uninst.to_lowercase().contains(&exe_name) {
                return Some(app);
            }
        }
    }

    None
}

/// Retrieves the status of Explorer context menu registration in HKCU\Software\Classes.
pub fn get_integration_status() -> ExplorerIntegrationStatus {
    let current_exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let classes_res = hkcu.open_subkey_with_flags(r"Software\Classes", KEY_READ);

    let classes = match classes_res {
        Ok(k) => k,
        Err(e) => {
            return ExplorerIntegrationStatus {
                is_registered: false,
                folder_actions_registered: false,
                exe_actions_registered: false,
                registered_exe_path: None,
                current_exe_path: current_exe,
                needs_repair: false,
                details: vec![format!("Failed to open HKCU\\Software\\Classes: {e}")],
            }
        }
    };

    let mut details = Vec::new();
    let mut folder_ok = true;
    let mut exe_ok = true;
    let mut any_registered = false;
    let mut registered_path: Option<String> = None;

    for verb in SHELL_VERBS {
        let subkey_path = format!(r"{}\{}\command", verb.parent_key, verb.verb_id);
        match classes.open_subkey_with_flags(&subkey_path, KEY_READ) {
            Ok(cmd_key) => {
                any_registered = true;
                let cmd_str: String = cmd_key.get_value("").unwrap_or_default();
                if registered_path.is_none() && !cmd_str.is_empty() {
                    // Extract executable path from quotes
                    if let Some(first_quote) = cmd_str.find('"') {
                        if let Some(second_quote) = cmd_str[first_quote + 1..].find('"') {
                            registered_path = Some(cmd_str[first_quote + 1..first_quote + 1 + second_quote].to_string());
                        }
                    }
                }
            }
            Err(_) => {
                if verb.parent_key.starts_with("Directory") || verb.parent_key.starts_with("Drive") {
                    folder_ok = false;
                } else {
                    exe_ok = false;
                }
            }
        }
    }

    let is_registered = any_registered && folder_ok && exe_ok;
    let mut needs_repair = false;

    if let Some(ref reg_exe) = registered_path {
        if !current_exe.is_empty() && !reg_exe.eq_ignore_ascii_case(&current_exe) {
            needs_repair = true;
            details.push(format!("Registered executable ({reg_exe}) does not match current binary ({current_exe})"));
        }
    }

    if any_registered && !is_registered {
        needs_repair = true;
        details.push("Some context menu entries are missing or partially registered".to_string());
    }

    ExplorerIntegrationStatus {
        is_registered,
        folder_actions_registered: folder_ok,
        exe_actions_registered: exe_ok,
        registered_exe_path: registered_path,
        current_exe_path: current_exe,
        needs_repair,
        details,
    }
}

/// Registers Cyron context menu entries in HKCU\Software\Classes.
pub fn register_explorer_integration() -> Result<(), String> {
    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Could not resolve current executable path: {e}"))?;
    let exe_path_str = current_exe.to_string_lossy().to_string();

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let classes = hkcu
        .create_subkey_with_flags(r"Software\Classes", KEY_ALL_ACCESS)
        .map_err(|e| format!("Failed to open or create HKCU\\Software\\Classes: {e}"))?
        .0;

    for verb in SHELL_VERBS {
        let verb_path = format!(r"{}\{}", verb.parent_key, verb.verb_id);
        let (verb_key, _) = classes
            .create_subkey_with_flags(&verb_path, KEY_ALL_ACCESS)
            .map_err(|e| format!("Failed to create {verb_path}: {e}"))?;

        // Set Menu display text
        verb_key
            .set_value("", &verb.menu_text)
            .map_err(|e| format!("Failed to set text on {verb_path}: {e}"))?;

        // Set icon to current_exe,0
        let icon_val = format!("\"{exe_path_str}\",0");
        let _ = verb_key.set_value("Icon", &icon_val);

        // Create command subkey
        let (cmd_key, _) = verb_key
            .create_subkey_with_flags("command", KEY_ALL_ACCESS)
            .map_err(|e| format!("Failed to create command key for {verb_path}: {e}"))?;

        let param = if verb.use_quote_v { "\"%V\"" } else { "\"%1\"" };
        let command_line = format!("\"{exe_path_str}\" {} {}", verb.arg_flag, param);

        cmd_key
            .set_value("", &command_line)
            .map_err(|e| format!("Failed to set command on {verb_path}: {e}"))?;
    }

    Ok(())
}

/// Unregisters all Cyron context menu entries from HKCU\Software\Classes.
pub fn unregister_explorer_integration() -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let classes = match hkcu.open_subkey_with_flags(r"Software\Classes", KEY_ALL_ACCESS) {
        Ok(k) => k,
        Err(_) => return Ok(()), // If Classes doesn't exist, nothing to unregister
    };

    for verb in SHELL_VERBS {
        let verb_path = format!(r"{}\{}", verb.parent_key, verb.verb_id);
        // Recursively delete subkey
        let _ = classes.delete_subkey_all(&verb_path);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_analyze_flag() {
        let current_dir = std::env::current_dir().unwrap().to_string_lossy().to_string();
        let args = vec![
            "cyron.exe".to_string(),
            "--analyze".to_string(),
            current_dir.clone(),
        ];
        let route = parse_startup_args(&args).expect("Route should be parsed");
        assert_eq!(route.view, "analyzer");
        assert_eq!(route.subview.as_deref(), Some("map"));
        assert_eq!(route.action.as_deref(), Some("scan"));
        assert!(route.path.is_some());
    }

    #[test]
    fn parses_duplicates_flag() {
        let current_dir = std::env::current_dir().unwrap().to_string_lossy().to_string();
        let args = vec![
            "cyron.exe".to_string(),
            "--duplicates".to_string(),
            current_dir,
        ];
        let route = parse_startup_args(&args).expect("Route should be parsed");
        assert_eq!(route.view, "analyzer");
        assert_eq!(route.subview.as_deref(), Some("duplicates"));
    }

    #[test]
    fn parses_storage_flag() {
        let current_dir = std::env::current_dir().unwrap().to_string_lossy().to_string();
        let args = vec![
            "cyron.exe".to_string(),
            "--storage".to_string(),
            current_dir,
        ];
        let route = parse_startup_args(&args).expect("Route should be parsed");
        assert_eq!(route.view, "analyzer");
        assert_eq!(route.subview.as_deref(), Some("storage"));
    }

    #[test]
    fn rejects_missing_or_invalid_paths() {
        let args = vec![
            "cyron.exe".to_string(),
            "--analyze".to_string(),
            r"Z:\NonExistent\Folder\That\Does\Not\Exist\12345".to_string(),
        ];
        let route = parse_startup_args(&args);
        assert!(route.is_none());
    }

    #[test]
    fn handles_empty_and_unknown_flags() {
        let args = vec!["cyron.exe".to_string()];
        assert!(parse_startup_args(&args).is_none());

        let args2 = vec!["cyron.exe".to_string(), "--unknown".to_string(), "foo".to_string()];
        assert!(parse_startup_args(&args2).is_none());
    }
}
