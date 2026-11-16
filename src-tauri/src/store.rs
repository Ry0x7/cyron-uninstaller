//! Native Microsoft Store / MSIX package inventory and current-user removal through
//! the Windows Runtime `PackageManager`. No shell wrappers or script interpreters.

use serde::{Deserialize, Serialize};
use windows::core::HSTRING;
use windows::ApplicationModel::PackageSignatureKind;
use windows::Management::Deployment::PackageManager;
use windows_sys::Win32::Foundation::{E_ACCESSDENIED, RPC_E_CHANGED_MODE};
use windows_sys::Win32::System::Com::{
    CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED,
};

const ERROR_PACKAGE_NOT_FOUND: u32 = 0x8007_3CF1;
const ERROR_PACKAGE_IN_USE: u32 = 0x8007_3D02;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreApp {
    pub full_name: String,
    pub family_name: String,
    pub name: String,
    pub display_name: Option<String>,
    pub publisher: Option<String>,
    pub publisher_id: Option<String>,
    pub install_location: Option<String>,
    pub version: Option<String>,
    pub is_framework: bool,
    pub is_resource: bool,
    pub is_bundle: bool,
    pub signature_kind: String,
    pub is_system: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreUninstallPlan {
    pub plan_id: String,
    pub app_id: String,
    pub app_name: String,
    pub package_full_name: String,
    pub package_family_name: String,
    pub publisher: Option<String>,
    pub publisher_id: Option<String>,
    pub install_location: Option<String>,
    pub expires_at: u64,
}

#[derive(Debug, Clone)]
pub struct StoredStoreUninstallPlan {
    pub plan_id: String,
    pub app_id: String,
    pub app_name: String,
    pub package_full_name: String,
    pub package_family_name: String,
    pub publisher: Option<String>,
    pub publisher_id: Option<String>,
    pub install_location: Option<String>,
    pub expires_at: u64,
}

impl StoredStoreUninstallPlan {
    pub fn to_public(&self) -> StoreUninstallPlan {
        StoreUninstallPlan {
            plan_id: self.plan_id.clone(),
            app_id: self.app_id.clone(),
            app_name: self.app_name.clone(),
            package_full_name: self.package_full_name.clone(),
            package_family_name: self.package_family_name.clone(),
            publisher: self.publisher.clone(),
            publisher_id: self.publisher_id.clone(),
            install_location: self.install_location.clone(),
            expires_at: self.expires_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreRemovalOutcome {
    pub success: bool,
    pub permission_denied: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreUninstallResult {
    pub app_id: String,
    pub status: String,
    pub package_full_name: String,
    pub message: String,
    pub elapsed_ms: u64,
}

/// Keeps a WinRT/MTA apartment alive for the duration of a blocking package call.
/// `RPC_E_CHANGED_MODE` means another part of the process already initialized COM,
/// so this guard must not uninitialize it.
struct ComGuard {
    initialized: bool,
}

impl ComGuard {
    fn new() -> Result<Self, String> {
        let hr = unsafe { CoInitializeEx(std::ptr::null(), COINIT_MULTITHREADED as u32) };
        if hr >= 0 {
            Ok(Self { initialized: true })
        } else if hr == RPC_E_CHANGED_MODE {
            Ok(Self { initialized: false })
        } else {
            Err(format!(
                "Windows Runtime initialization failed (0x{:08X}).",
                hr as u32
            ))
        }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.initialized {
            unsafe { CoUninitialize() };
        }
    }
}

/// Returns only main packages for the current user. Framework, resource and bundle
/// packages are dependency components and must never be offered for removal.
pub fn list_store_apps() -> Result<Vec<StoreApp>, String> {
    let _com = ComGuard::new()?;
    let manager = PackageManager::new()
        .map_err(|error| format!("Windows Package Manager is unavailable: {error}"))?;
    let packages = manager
        .FindPackagesByUserSecurityId(&HSTRING::new())
        .map_err(|error| format!("Could not enumerate Store packages: {error}"))?;
    let iterator = packages
        .First()
        .map_err(|error| format!("Could not read the Store package list: {error}"))?;

    let mut apps = Vec::new();
    loop {
        if !iterator
            .HasCurrent()
            .map_err(|error| format!("Could not advance the Store package list: {error}"))?
        {
            break;
        }
        let package = iterator
            .Current()
            .map_err(|error| format!("Could not read a Store package: {error}"))?;
        if let Some(app) = read_package(&package) {
            if should_list(&app) {
                apps.push(app);
            }
        }
        if !iterator
            .MoveNext()
            .map_err(|error| format!("Could not advance the Store package list: {error}"))?
        {
            break;
        }
    }

    apps.sort_by(|left, right| {
        left.display_name
            .as_deref()
            .unwrap_or(&left.name)
            .to_lowercase()
            .cmp(
                &right
                    .display_name
                    .as_deref()
                    .unwrap_or(&right.name)
                    .to_lowercase(),
            )
            .then_with(|| left.full_name.cmp(&right.full_name))
    });
    Ok(apps)
}

fn read_package(package: &windows::ApplicationModel::Package) -> Option<StoreApp> {
    let id = package.Id().ok()?;
    let full_name = id.FullName().ok()?.to_string_lossy();
    if full_name.trim().is_empty() {
        return None;
    }
    let family_name = id.FamilyName().map(|value| value.to_string_lossy()).unwrap_or_default();
    let name = id.Name().map(|value| value.to_string_lossy()).unwrap_or_default();
    let publisher = package
        .PublisherDisplayName()
        .ok()
        .map(|value| value.to_string_lossy())
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            id.Publisher()
                .ok()
                .map(|value| value.to_string_lossy())
                .filter(|value| !value.trim().is_empty())
        });
    let display_name = package
        .DisplayName()
        .ok()
        .map(|value| value.to_string_lossy())
        .filter(|value| !value.trim().is_empty());
    let install_location = package
        .InstalledPath()
        .ok()
        .map(|value| value.to_string_lossy())
        .filter(|value| !value.trim().is_empty());
    let version = id.Version().ok().map(|value| {
        format!(
            "{}.{}.{}.{}",
            value.Major, value.Minor, value.Build, value.Revision
        )
    });
    let signature = package
        .SignatureKind()
        .unwrap_or(PackageSignatureKind::None);
    let signature_kind = match signature {
        PackageSignatureKind::Developer => "developer",
        PackageSignatureKind::Enterprise => "enterprise",
        PackageSignatureKind::Store => "store",
        PackageSignatureKind::System => "system",
        _ => "none",
    }
    .to_string();
    let is_system = signature == PackageSignatureKind::System
        || install_location
            .as_deref()
            .map(|path| path.to_ascii_lowercase().contains(r"\windows\systemapps\"))
            .unwrap_or(false);

    Some(StoreApp {
        full_name,
        family_name,
        name,
        display_name,
        publisher,
        publisher_id: id.PublisherId().ok().map(|value| value.to_string_lossy()),
        install_location,
        version,
        is_framework: package.IsFramework().unwrap_or(false),
        is_resource: package.IsResourcePackage().unwrap_or(false),
        is_bundle: package.IsBundle().unwrap_or(false),
        signature_kind,
        is_system,
    })
}

fn should_list(app: &StoreApp) -> bool {
    !app.is_framework && !app.is_resource && !app.is_bundle && !app.is_system
}

/// Resolves the one current-user package for a stable family name. A plan always
/// binds to the live full name; an updated package is re-resolved by family.
pub fn package_for_family(family_name: &str) -> Result<Option<StoreApp>, String> {
    let family = family_name.trim();
    if family.is_empty() {
        return Err("Store package family name is missing.".into());
    }
    Ok(list_store_apps()?
        .into_iter()
        .find(|app| app.family_name.eq_ignore_ascii_case(family)))
}

/// Removes one package for the current user only. The caller must have re-verified the
/// exact package identity against a server-held plan. Machine-wide deprovisioning and
/// `RemoveForAllUsers` are deliberately never used.
pub fn remove_package(full_name: &str) -> StoreRemovalOutcome {
    let full_name = full_name.trim();
    if full_name.is_empty() {
        return failure("No Store package identity was provided.", false);
    }
    let _com = match ComGuard::new() {
        Ok(guard) => guard,
        Err(error) => return failure(&error, false),
    };
    let manager = match PackageManager::new() {
        Ok(manager) => manager,
        Err(error) => {
            return failure(
                &format!("Windows Package Manager is unavailable: {error}"),
                false,
            )
        }
    };

    let operation = match manager.RemovePackageAsync(&HSTRING::from(full_name)) {
        Ok(operation) => operation,
        Err(error) => return classify_error(&error),
    };
    let result = match operation.join() {
        Ok(result) => result,
        Err(error) => return classify_error(&error),
    };
    match result.ErrorText() {
        Ok(text) if !text.to_string_lossy().trim().is_empty() => {
            failure(text.to_string_lossy().trim(), false)
        }
        Ok(_) => StoreRemovalOutcome {
            success: true,
            permission_denied: false,
            error: None,
        },
        Err(error) => classify_error(&error),
    }
}

fn classify_error(error: &windows::core::Error) -> StoreRemovalOutcome {
    let code = error.code().0 as u32;
    let permission_denied = code == E_ACCESSDENIED as u32;
    let message = match code {
        ERROR_PACKAGE_NOT_FOUND => {
            "Windows reports that this package is no longer installed.".to_string()
        }
        ERROR_PACKAGE_IN_USE => {
            "Windows could not remove this package because it is currently in use. Close it and try again."
                .to_string()
        }
        _ if permission_denied => {
            "Windows denied permission to remove this package. Cyron only removes current-user packages and never elevates automatically."
                .to_string()
        }
        _ => format!("{} (0x{:08X})", error.message(), code),
    };
    failure(&message, permission_denied)
}

fn failure(message: &str, permission_denied: bool) -> StoreRemovalOutcome {
    StoreRemovalOutcome {
        success: false,
        permission_denied,
        error: Some(message.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase2_store_inventory_reads_current_user() {
        let apps = list_store_apps().expect("current-user Store inventory should be readable");
        if apps.is_empty() {
            eprintln!("No Store packages are registered for this user; inventory shape not exercised.");
            return;
        }
        assert!(
            apps.iter().all(|app| !app.is_framework && !app.is_resource && !app.is_bundle),
            "main-package inventory must never include dependencies"
        );
        assert!(
            apps.iter().all(|app| !app.full_name.is_empty() && !app.family_name.is_empty()),
            "every Store package needs a full and family name"
        );
        assert!(
            apps.iter().all(|app| !app.is_system),
            "System packages are not offered by inventory"
        );
    }

    #[test]
    fn phase2_store_filters_are_explicit() {
        let main = StoreApp {
            full_name: "Contoso.App_1.0.0.0_x64__abc".into(),
            family_name: "Contoso.App_abc".into(),
            name: "Contoso.App".into(),
            display_name: Some("Contoso".into()),
            publisher: Some("Contoso".into()),
            publisher_id: Some("CN=Contoso".into()),
            install_location: Some(r"C:\Program Files\WindowsApps\Contoso.App".into()),
            version: Some("1.0.0.0".into()),
            is_framework: false,
            is_resource: false,
            is_bundle: false,
            signature_kind: "store".into(),
            is_system: false,
        };
        assert!(should_list(&main));
        assert!(!should_list(&StoreApp {
            is_framework: true,
            ..main.clone()
        }));
        assert!(!should_list(&StoreApp {
            is_resource: true,
            ..main.clone()
        }));
        assert!(!should_list(&StoreApp {
            is_bundle: true,
            ..main.clone()
        }));
        assert!(!should_list(&StoreApp {
            is_system: true,
            ..main
        }));
    }
}
