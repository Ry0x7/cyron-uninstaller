use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use winreg::enums::*;
use winreg::RegKey;

/// One installed application discovered from the Windows uninstall registry,
/// Microsoft Store packages, Steam libraries, or portable heuristics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub publisher: Option<String>,
    pub install_location: Option<String>,
    pub install_date: Option<String>,
    pub uninstall_string: Option<String>,
    pub quiet_uninstall_string: Option<String>,
    pub display_icon: Option<String>,
    /// EstimatedSize from the registry or manifest converted to bytes.
    pub estimated_size: Option<u64>,
    /// Registry location or package source: machine-64, machine-32, user-64, user-32, store, steam, portable.
    pub hive: String,
    /// machine or user.
    pub scope: String,
    /// x64 or x86.
    pub architecture: String,
    /// Located under the Windows directory or published as a Windows component.
    pub is_protected: bool,
    /// "win32", "store", "steam", or "portable".
    pub source: String,
    /// "standard", "silent_only", "store_managed", "manual_only", or "missing".
    pub uninstall_support: String,
    /// "confirmed" or "heuristic".
    pub confidence: String,
    /// Explanation of why and how this application was identified.
    pub detection_reason: Option<String>,
    /// Actual registry subkey name under the Uninstall key (for Win32 registry apps).
    #[serde(default)]
    pub registry_key_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInventory {
    pub apps: Vec<InstalledApp>,
    pub warnings: Vec<String>,
}

const UNINSTALL_PATH: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall";
const UNINSTALL_WOW_PATH: &str =
    "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall";
const APPMODEL_PACKAGES_PATH: &str =
    r"Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppModel\Repository\Packages";
const MAX_WARNINGS: usize = 12;

/// Raw registry values before filtering and normalisation.
#[derive(Debug, Clone, Default)]
struct RawApp {
    display_name: Option<String>,
    version: Option<String>,
    publisher: Option<String>,
    install_location: Option<String>,
    install_date: Option<String>,
    uninstall_string: Option<String>,
    quiet_uninstall_string: Option<String>,
    display_icon: Option<String>,
    estimated_size: Option<u64>,
    system_component: bool,
    parent_key_name: Option<String>,
    release_type: Option<String>,
}

/// Enumerates installed applications across Win32 registry, Store packages,
/// Steam game libraries, and bounded portable application folders.
pub fn discover_apps() -> AppInventory {
    let mut apps = Vec::new();
    let mut warnings = Vec::new();

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let view = KEY_READ | KEY_WOW64_64KEY;

    // 1. Conventional Win32 Registry hives
    read_hive(
        &hklm,
        view,
        UNINSTALL_PATH,
        "machine-64",
        "machine",
        "x64",
        &mut apps,
        &mut warnings,
    );
    read_hive(
        &hklm,
        view,
        UNINSTALL_WOW_PATH,
        "machine-32",
        "machine",
        "x86",
        &mut apps,
        &mut warnings,
    );
    read_hive(
        &hkcu,
        view,
        UNINSTALL_PATH,
        "user-64",
        "user",
        "x64",
        &mut apps,
        &mut warnings,
    );
    read_hive(
        &hkcu,
        view,
        UNINSTALL_WOW_PATH,
        "user-32",
        "user",
        "x86",
        &mut apps,
        &mut warnings,
    );

    // 2. Microsoft Store / AppX / MSIX read-only inventory
    discover_store_apps(&hkcu, &mut apps, &mut warnings);

    // 3. Steam installed-game detection
    discover_steam_apps(&hkcu, &mut apps, &mut warnings);

    // Deduplicate primary inventory first
    let mut deduped = dedupe(apps);

    // 4. Bounded portable application discovery
    discover_portable_apps(&mut deduped, &mut warnings);

    AppInventory {
        apps: deduped,
        warnings,
    }
}

#[allow(clippy::too_many_arguments)]
fn read_hive(
    root: &RegKey,
    view: u32,
    path: &str,
    hive: &str,
    scope: &str,
    architecture: &str,
    out: &mut Vec<InstalledApp>,
    warnings: &mut Vec<String>,
) {
    let base = match root.open_subkey_with_flags(path, view) {
        Ok(key) => key,
        Err(error) => {
            push_warning(
                warnings,
                format!("Could not open {hive} uninstall registry: {error}"),
            );
            return;
        }
    };

    for key_name in base.enum_keys().flatten() {
        match base.open_subkey_with_flags(key_name.as_str(), KEY_READ | KEY_WOW64_64KEY) {
            Ok(key) => {
                if let Some(app) = read_app(&key, &key_name, hive, scope, architecture) {
                    out.push(app);
                }
            }
            Err(error) => push_warning(
                warnings,
                format!("Could not read {hive} entry {key_name}: {error}"),
            ),
        }
    }
}

fn read_app(
    key: &RegKey,
    key_name: &str,
    hive: &str,
    scope: &str,
    architecture: &str,
) -> Option<InstalledApp> {
    let raw = RawApp {
        display_name: read_string(key, "DisplayName"),
        version: read_string(key, "DisplayVersion"),
        publisher: read_string(key, "Publisher"),
        install_location: read_string(key, "InstallLocation"),
        install_date: read_string(key, "InstallDate")
            .and_then(|value| normalize_install_date(&value)),
        uninstall_string: read_string(key, "UninstallString"),
        quiet_uninstall_string: read_string(key, "QuietUninstallString"),
        display_icon: read_string(key, "DisplayIcon"),
        estimated_size: read_dword(key, "EstimatedSize").and_then(estimated_size_bytes),
        system_component: read_dword(key, "SystemComponent").unwrap_or(0) == 1,
        parent_key_name: read_string(key, "ParentKeyName"),
        release_type: read_string(key, "ReleaseType"),
    };

    if !should_include(&raw) {
        return None;
    }

    let name = raw.display_name?;
    let windir = std::env::var("WINDIR").ok();

    // Check uninstall support and missing executable detection
    let uninstall_support = determine_win32_uninstall_support(
        raw.uninstall_string.as_deref(),
        raw.quiet_uninstall_string.as_deref(),
    );

    Some(InstalledApp {
        id: format!("{hive}|{key_name}"),
        name: name.clone(),
        version: raw.version,
        publisher: raw.publisher.clone(),
        install_location: raw.install_location.clone(),
        install_date: raw.install_date,
        uninstall_string: raw.uninstall_string,
        quiet_uninstall_string: raw.quiet_uninstall_string,
        display_icon: raw.display_icon,
        estimated_size: raw.estimated_size,
        hive: hive.to_string(),
        scope: scope.to_string(),
        architecture: architecture.to_string(),
        is_protected: is_protected(
            &name,
            raw.publisher.as_deref(),
            raw.install_location.as_deref(),
            windir.as_deref(),
        ),
        source: "win32".to_string(),
        uninstall_support,
        confidence: "confirmed".to_string(),
        detection_reason: Some(format!("Discovered in Windows Registry ({hive})")),
        registry_key_name: Some(key_name.to_string()),
    })
}

/// Evaluates whether an uninstaller command is standard, silent_only, or missing from disk.
fn determine_win32_uninstall_support(
    uninstall_string: Option<&str>,
    quiet_uninstall_string: Option<&str>,
) -> String {
    let uninst = uninstall_string.or(quiet_uninstall_string);
    let Some(cmd) = uninst else {
        return "missing".to_string();
    };

    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return "missing".to_string();
    }

    let parsed = crate::uninstall::parse_uninstall_command(trimmed);
    if parsed.is_msi {
        if quiet_uninstall_string.is_some() && uninstall_string.is_none() {
            return "silent_only".to_string();
        }
        return "standard".to_string();
    }

    if !parsed.executable.is_empty() {
        let path = Path::new(&parsed.executable);
        if path.is_absolute() && !path.exists() {
            return "missing".to_string();
        }
    } else {
        return "missing".to_string();
    }

    if quiet_uninstall_string.is_some() && uninstall_string.is_none() {
        "silent_only".to_string()
    } else {
        "standard".to_string()
    }
}

/// Discovers Microsoft Store / MSIX / AppX packages in HKCU AppModel Repository.
fn discover_store_apps(
    hkcu: &RegKey,
    out: &mut Vec<InstalledApp>,
    _warnings: &mut Vec<String>,
) {
    let packages_key = match hkcu.open_subkey_with_flags(APPMODEL_PACKAGES_PATH, KEY_READ) {
        Ok(k) => k,
        Err(_) => return, // Store package repository not present or inaccessible
    };

    for package_name in packages_key.enum_keys().flatten() {
        // Skip dependencies, frameworks and internal Windows components
        let lower = package_name.to_lowercase();
        if lower.contains("vclibs")
            || lower.contains("ui.xaml")
            || lower.contains("directx")
            || lower.contains(".net.native")
        {
            continue;
        }

        let pkg_key = match packages_key.open_subkey_with_flags(&package_name, KEY_READ) {
            Ok(k) => k,
            Err(_) => continue,
        };

        let root_folder: Option<String> = pkg_key.get_value("PackageRootFolder").ok();
        let display_name_raw: Option<String> = pkg_key.get_value("DisplayName").ok();

        let is_system_app = root_folder
            .as_deref()
            .map(|f| f.to_lowercase().contains("systemapps"))
            .unwrap_or(false);

        // Derive a human readable name if DisplayName is a resource reference
        let human_name = match display_name_raw {
            Some(ref name) if !name.starts_with("@{") && !name.trim().is_empty() => name.clone(),
            _ => {
                // Parse package family or package name prefix
                package_name
                    .split('_')
                    .next()
                    .unwrap_or(&package_name)
                    .to_string()
            }
        };

        if human_name.trim().is_empty() || is_system_app {
            continue;
        }

        // Parse version from package name (e.g. Name_1.2.3.4_x64...)
        let version = package_name.split('_').nth(1).map(|v| v.to_string());
        let arch = if lower.contains("_x64_") {
            "x64"
        } else if lower.contains("_x86_") {
            "x86"
        } else {
            "neutral"
        };

        out.push(InstalledApp {
            id: format!("store|{package_name}"),
            name: human_name,
            version,
            publisher: Some("Microsoft Store / AppX".to_string()),
            install_location: root_folder,
            install_date: None,
            uninstall_string: None,
            quiet_uninstall_string: None,
            display_icon: None,
            estimated_size: None,
            hive: "store".to_string(),
            scope: "user".to_string(),
            architecture: arch.to_string(),
            is_protected: false,
            source: "store".to_string(),
            uninstall_support: "store_managed".to_string(),
            confidence: "confirmed".to_string(),
            detection_reason: Some("Registered Microsoft Store / MSIX application package".to_string()),
            registry_key_name: None,
        });
    }
}

/// Discovers games installed across all Steam libraries.
fn discover_steam_apps(
    hkcu: &RegKey,
    out: &mut Vec<InstalledApp>,
    _warnings: &mut Vec<String>,
) {
    let steam_key = match hkcu.open_subkey_with_flags(r"Software\Valve\Steam", KEY_READ) {
        Ok(k) => k,
        Err(_) => return,
    };

    let steam_path_str: String = match steam_key.get_value("SteamPath") {
        Ok(p) => p,
        Err(_) => return,
    };

    let steam_root = PathBuf::from(steam_path_str.replace('/', "\\"));
    let library_vdf = steam_root.join(r"steamapps\libraryfolders.vdf");

    let mut library_dirs = Vec::new();
    library_dirs.push(steam_root.clone());

    if library_vdf.exists() {
        if let Ok(content) = std::fs::read_to_string(&library_vdf) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("\"path\"") {
                    if let Some(path_part) = extract_quoted_value(trimmed) {
                        let candidate = PathBuf::from(path_part.replace("\\\\", "\\"));
                        if candidate.exists() && !library_dirs.contains(&candidate) {
                            library_dirs.push(candidate);
                        }
                    }
                }
            }
        }
    }

    for lib_dir in library_dirs {
        let steamapps_dir = lib_dir.join("steamapps");
        let entries = match std::fs::read_dir(&steamapps_dir) {
            Ok(iter) => iter,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();
            if name_str.starts_with("appmanifest_") && name_str.ends_with(".acf") {
                if let Ok(acf_content) = std::fs::read_to_string(entry.path()) {
                    if let Some(game) = parse_steam_manifest(&acf_content, &lib_dir) {
                        out.push(game);
                    }
                }
            }
        }
    }
}

fn extract_quoted_value(line: &str) -> Option<String> {
    let mut parts = line.split('"').filter(|s| !s.trim().is_empty());
    parts.next()?; // Skip key
    parts.next().map(|s| s.to_string())
}

fn parse_steam_manifest(content: &str, library_dir: &Path) -> Option<InstalledApp> {
    let mut app_id = String::new();
    let mut name = String::new();
    let mut install_dir = String::new();
    let mut size_on_disk: Option<u64> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("\"appid\"") {
            if let Some(val) = extract_quoted_value(trimmed) {
                app_id = val;
            }
        } else if trimmed.starts_with("\"name\"") {
            if let Some(val) = extract_quoted_value(trimmed) {
                name = val;
            }
        } else if trimmed.starts_with("\"installdir\"") {
            if let Some(val) = extract_quoted_value(trimmed) {
                install_dir = val;
            }
        } else if trimmed.starts_with("\"SizeOnDisk\"") {
            if let Some(val) = extract_quoted_value(trimmed) {
                size_on_disk = val.parse::<u64>().ok();
            }
        }
    }

    if app_id.is_empty() || name.is_empty() {
        return None;
    }

    let install_location = if !install_dir.is_empty() {
        Some(library_dir.join("steamapps").join("common").join(install_dir).to_string_lossy().to_string())
    } else {
        None
    };

    Some(InstalledApp {
        id: format!("steam|{app_id}"),
        name,
        version: None,
        publisher: Some("Valve / Steam".to_string()),
        install_location,
        install_date: None,
        uninstall_string: Some(format!("steam://uninstall/{app_id}")),
        quiet_uninstall_string: None,
        display_icon: Some(library_dir.join("steam.exe").to_string_lossy().to_string()),
        estimated_size: size_on_disk,
        hive: "steam".to_string(),
        scope: "user".to_string(),
        architecture: "x64".to_string(),
        is_protected: false,
        source: "steam".to_string(),
        uninstall_support: "store_managed".to_string(),
        confidence: "confirmed".to_string(),
        detection_reason: Some(format!("Steam game library manifest (AppID {app_id})")),
        registry_key_name: None,
    })
}

/// Discovers standalone portable applications in bounded user directories.
fn discover_portable_apps(
    known_apps: &mut Vec<InstalledApp>,
    _warnings: &mut Vec<String>,
) {
    let mut known_paths = HashSet::new();
    for app in known_apps.iter() {
        if let Some(ref loc) = app.install_location {
            known_paths.insert(loc.to_lowercase().replace('/', "\\"));
        }
        if let Some(ref icon) = app.display_icon {
            let icon_clean = icon.split(',').next().unwrap_or(icon).trim().trim_matches('"');
            known_paths.insert(icon_clean.to_lowercase().replace('/', "\\"));
        }
    }

    let mut candidate_roots = Vec::new();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        candidate_roots.push(PathBuf::from(local).join("Programs"));
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        candidate_roots.push(PathBuf::from(appdata).join("Programs"));
    }

    let mut portable_candidates = Vec::new();

    for root in candidate_roots {
        if !root.exists() {
            continue;
        }

        let entries = match std::fs::read_dir(&root) {
            Ok(iter) => iter,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Look for direct executable in folder
                if let Ok(sub_entries) = std::fs::read_dir(&path) {
                    for sub in sub_entries.flatten() {
                        let sub_path = sub.path();
                        if sub_path.is_file()
                            && sub_path
                                .extension()
                                .map(|ext| ext.eq_ignore_ascii_case("exe"))
                                .unwrap_or(false)
                        {
                            let sub_norm = sub_path.to_string_lossy().to_lowercase();
                            let dir_norm = path.to_string_lossy().to_lowercase();

                            let already_registered = known_paths.iter().any(|k| {
                                sub_norm.contains(k) || dir_norm.contains(k) || k.contains(&dir_norm)
                            });

                            if !already_registered && is_pe_executable(&sub_path) {
                                let folder_name = path
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_string();
                                let size = std::fs::metadata(&sub_path).map(|m| m.len()).ok();

                                portable_candidates.push(InstalledApp {
                                    id: format!("portable|{}", hash_path(&sub_path)),
                                    name: folder_name,
                                    version: None,
                                    publisher: None,
                                    install_location: Some(path.to_string_lossy().to_string()),
                                    install_date: None,
                                    uninstall_string: None,
                                    quiet_uninstall_string: None,
                                    display_icon: Some(sub_path.to_string_lossy().to_string()),
                                    estimated_size: size,
                                    hive: "portable".to_string(),
                                    scope: "user".to_string(),
                                    architecture: "x64".to_string(),
                                    is_protected: false,
                                    source: "portable".to_string(),
                                    uninstall_support: "manual_only".to_string(),
                                    confidence: "heuristic".to_string(),
                                    detection_reason: Some("Standalone executable detected in user programs directory without Windows uninstaller registration".to_string()),
                                    registry_key_name: None,
                                });
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    known_apps.extend(portable_candidates);
}

fn is_pe_executable(path: &Path) -> bool {
    use std::io::Read;
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut magic = [0u8; 2];
    if file.read_exact(&mut magic).is_ok() {
        magic == [b'M', b'Z']
    } else {
        false
    }
}

fn hash_path(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(path.to_string_lossy().as_bytes());
    format!("{:x}", hasher.finalize())[..12].to_string()
}

fn read_string(key: &RegKey, name: &str) -> Option<String> {
    key.get_value::<String, _>(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Reads a DWORD that some installers write as a string instead.
fn read_dword(key: &RegKey, name: &str) -> Option<u32> {
    if let Ok(value) = key.get_value::<u32, _>(name) {
        return Some(value);
    }
    key.get_value::<String, _>(name)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
}

fn push_warning(warnings: &mut Vec<String>, message: String) {
    if warnings.len() < MAX_WARNINGS {
        warnings.push(message);
    }
}

fn should_include(raw: &RawApp) -> bool {
    let Some(name) = raw.display_name.as_deref() else {
        return false;
    };
    if name.trim().is_empty() || raw.system_component {
        return false;
    }
    if raw
        .parent_key_name
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        return false;
    }
    let release = raw
        .release_type
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_lowercase();
    if matches!(release.as_str(), "update" | "hotfix" | "security update") {
        return false;
    }

    let lower = name.to_lowercase();
    if lower.starts_with("update for") || lower.starts_with("security update for") {
        return false;
    }
    !is_kb_only(&lower)
}

fn is_kb_only(lower_name: &str) -> bool {
    match lower_name.strip_prefix("kb") {
        Some(rest) => !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()),
        None => false,
    }
}

fn normalize_install_date(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.len() == 8 && trimmed.chars().all(|c| c.is_ascii_digit()) {
        return Some(format!(
            "{}-{}-{}",
            &trimmed[0..4],
            &trimmed[4..6],
            &trimmed[6..8]
        ));
    }
    Some(trimmed.to_string())
}

fn estimated_size_bytes(kilobytes: u32) -> Option<u64> {
    if kilobytes == 0 {
        None
    } else {
        Some(u64::from(kilobytes) * 1024)
    }
}

pub(crate) fn is_protected(
    name: &str,
    publisher: Option<&str>,
    install_location: Option<&str>,
    windir: Option<&str>,
) -> bool {
    if let (Some(location), Some(windows)) = (install_location, windir) {
        let location = location
            .replace('/', "\\")
            .trim_end_matches('\\')
            .to_lowercase();
        let windows = windows
            .replace('/', "\\")
            .trim_end_matches('\\')
            .to_lowercase();
        if !windows.is_empty() && location.starts_with(&windows) {
            return true;
        }
    }

    let lower = name.to_lowercase();
    if lower.starts_with("windows ") {
        return true;
    }
    if lower.starts_with("microsoft windows")
        && publisher
            .map(|value| value.to_lowercase().contains("microsoft"))
            .unwrap_or(false)
    {
        return true;
    }
    false
}

fn hive_rank(hive: &str) -> u8 {
    match hive {
        "machine-64" => 0,
        "machine-32" => 1,
        "user-64" => 2,
        _ => 3,
    }
}

/// Products are commonly registered twice (64-bit and WOW6432Node with the
/// same name, version and location). Keep one record, preferring the 64-bit
/// machine hive, but never merge products installed in different locations.
fn dedupe(apps: Vec<InstalledApp>) -> Vec<InstalledApp> {
    let mut best: HashMap<String, InstalledApp> = HashMap::new();

    for app in apps {
        let key = format!(
            "{}\u{1}{}\u{1}{}",
            app.name.to_lowercase(),
            app.version.as_deref().unwrap_or("").to_lowercase(),
            app.install_location
                .as_deref()
                .unwrap_or("")
                .replace('/', "\\")
                .trim_end_matches('\\')
                .to_lowercase()
        );
        match best.get(&key) {
            Some(existing) if hive_rank(&existing.hive) <= hive_rank(&app.hive) => {}
            _ => {
                best.insert(key, app);
            }
        }
    }

    let mut apps: Vec<InstalledApp> = best.into_values().collect();
    apps.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    apps
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(name: &str) -> RawApp {
        RawApp {
            display_name: Some(name.to_string()),
            ..RawApp::default()
        }
    }

    #[test]
    fn hides_entries_that_are_not_products() {
        assert!(should_include(&raw("Mozilla Firefox")));
        assert!(!should_include(&RawApp::default()));
        assert!(!should_include(&raw("   ")));

        let mut system = raw("Windows Driver Package");
        system.system_component = true;
        assert!(!should_include(&system));

        let mut patch = raw("Visual Studio Update");
        patch.parent_key_name = Some("VisualStudio".into());
        assert!(!should_include(&patch));

        let mut update = raw("Some Product");
        update.release_type = Some("Security Update".into());
        assert!(!should_include(&update));

        assert!(!should_include(&raw("Update for Microsoft Edge")));
        assert!(!should_include(&raw("Security Update for Windows")));
        assert!(!should_include(&raw("KB5034441")));
        assert!(should_include(&raw("KB Studio")));
    }

    #[test]
    fn normalizes_install_dates() {
        assert_eq!(
            normalize_install_date("20240131").as_deref(),
            Some("2024-01-31")
        );
        assert_eq!(
            normalize_install_date(" 2024-01-31 ").as_deref(),
            Some("2024-01-31")
        );
        assert_eq!(normalize_install_date(""), None);
        assert_eq!(normalize_install_date("2024").as_deref(), Some("2024"));
    }

    #[test]
    fn converts_estimated_size_to_bytes() {
        assert_eq!(estimated_size_bytes(1500), Some(1_536_000));
        assert_eq!(estimated_size_bytes(0), None);
    }

    #[test]
    fn marks_system_locations_and_windows_components_protected() {
        assert!(is_protected(
            "Contoso Driver",
            Some("Contoso"),
            Some("C:\\Windows\\System32\\drivers"),
            Some("C:\\Windows"),
        ));
        assert!(is_protected("Windows Photos", None, None, None));
        assert!(is_protected(
            "Microsoft Windows Desktop Runtime",
            Some("Microsoft Corporation"),
            Some("C:\\Program Files\\dotnet"),
            Some("C:\\Windows"),
        ));
        assert!(!is_protected(
            "Mozilla Firefox",
            Some("Mozilla"),
            Some("C:\\Program Files\\Mozilla Firefox"),
            Some("C:\\Windows"),
        ));
    }

    #[test]
    fn parses_steam_manifest_accurately() {
        let manifest = r#"
"AppState"
{
	"appid"		"730"
	"name"		"Counter-Strike 2"
	"installdir"		"Counter-Strike Global Offensive"
	"SizeOnDisk"		"73963861286"
}
"#;
        let app = parse_steam_manifest(manifest, Path::new(r"E:\SteamLibrary"))
            .expect("Should parse manifest");
        assert_eq!(app.id, "steam|730");
        assert_eq!(app.name, "Counter-Strike 2");
        assert_eq!(app.source, "steam");
        assert_eq!(app.uninstall_support, "store_managed");
        assert_eq!(app.estimated_size, Some(73963861286));
        assert!(app.install_location.unwrap().contains("Counter-Strike Global Offensive"));
    }

    fn app(name: &str, hive: &str, location: &str) -> InstalledApp {
        InstalledApp {
            id: format!("{hive}|{name}"),
            name: name.to_string(),
            version: Some("1.0".into()),
            publisher: None,
            install_location: Some(location.into()),
            install_date: None,
            uninstall_string: None,
            quiet_uninstall_string: None,
            display_icon: None,
            estimated_size: None,
            hive: hive.to_string(),
            scope: if hive.starts_with("machine") {
                "machine"
            } else {
                "user"
            }
            .into(),
            architecture: if hive.ends_with("32") { "x86" } else { "x64" }.into(),
            is_protected: false,
            source: "win32".to_string(),
            uninstall_support: "standard".to_string(),
            confidence: "confirmed".to_string(),
            detection_reason: None,
            registry_key_name: Some(name.to_string()),
        }
    }

    #[test]
    fn dedupe_prefers_the_64_bit_machine_entry() {
        let apps = vec![
            app("Example", "machine-32", "C:\\Program Files (x86)\\Example"),
            app("Example", "machine-64", "C:\\Program Files (x86)\\Example"),
            app("Example", "user-64", "C:\\Users\\dev\\Example"),
            app("Another", "machine-64", "C:\\Program Files\\Another"),
        ];
        let deduped = dedupe(apps);
        assert_eq!(deduped.len(), 3);
        assert_eq!(
            deduped
                .iter()
                .find(|item| item.name == "Example" && item.hive == "machine-64")
                .map(|item| item.hive.as_str()),
            Some("machine-64")
        );
        assert_eq!(deduped[0].name, "Another");
    }
}
