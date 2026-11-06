use serde::Serialize;
use std::collections::HashMap;
use winreg::enums::*;
use winreg::RegKey;

/// One installed application discovered from the Windows uninstall registry.
#[derive(Debug, Clone, Serialize)]
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
    /// EstimatedSize from the registry converted to bytes.
    pub estimated_size: Option<u64>,
    /// Registry location: machine-64, machine-32, user-64, user-32.
    pub hive: String,
    /// machine or user.
    pub scope: String,
    /// x64 or x86.
    pub architecture: String,
    /// Located under the Windows directory or published as a Windows component.
    pub is_protected: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInventory {
    pub apps: Vec<InstalledApp>,
    pub warnings: Vec<String>,
}

const UNINSTALL_PATH: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall";
const UNINSTALL_WOW_PATH: &str =
    "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall";
const MAX_WARNINGS: usize = 10;

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

/// Enumerates the uninstall registry keys of the current machine.
pub fn discover_apps() -> AppInventory {
    let mut apps = Vec::new();
    let mut warnings = Vec::new();

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let view = KEY_READ | KEY_WOW64_64KEY;

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

    AppInventory {
        apps: dedupe(apps),
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
        // The view flag is not inherited by relative opens, so pass it again to
        // keep WOW6432Node enumeration in the 32-bit view.
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
    })
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

fn is_protected(
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
