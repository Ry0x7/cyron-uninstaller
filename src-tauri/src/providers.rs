use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: u32 = 1;

pub const SAFE: &str = "safe";
pub const CAUTION: &str = "caution";
pub const PROTECTED: &str = "protected";

pub const AI: &str = "ai";
pub const IDE: &str = "ide";
pub const PACKAGES: &str = "packages";
pub const CONTAINERS: &str = "containers";
pub const WINDOWS: &str = "windows";
pub const BROWSER: &str = "browser";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CleanupProviderDef {
    pub schema_version: u32,
    pub id: String,
    pub tool: String,
    pub label: String,
    pub path_template: String,
    pub risk: String,
    pub category: String,
    pub impact: String,
    pub app_detection: Option<ProviderAppDetection>,
    pub patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderAppDetection {
    pub env_var: Option<String>,
    pub reg_key: Option<String>,
    pub relative_indicator: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderValidationError {
    UnsupportedSchemaVersion(u32),
    InvalidId(String),
    MissingField(&'static str),
    InvalidRisk(String),
    InvalidCategory(String),
    DisallowedRoot(String),
    PathTraversal(String),
    WildcardInPathTemplate(String),
    ProtectedDataViolation(String),
    InvalidPattern(String),
}

impl std::fmt::Display for ProviderValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedSchemaVersion(v) => write!(f, "Unsupported schema version: {v}"),
            Self::InvalidId(id) => write!(f, "Invalid provider ID '{id}': must be alphanumeric and hyphens"),
            Self::MissingField(field) => write!(f, "Missing required field: {field}"),
            Self::InvalidRisk(risk) => write!(f, "Invalid risk level: '{risk}'. Must be safe, caution, or protected"),
            Self::InvalidCategory(cat) => write!(f, "Invalid category: '{cat}'"),
            Self::DisallowedRoot(path) => write!(f, "Disallowed root path template: '{path}'"),
            Self::PathTraversal(path) => write!(f, "Path traversal forbidden in: '{path}'"),
            Self::WildcardInPathTemplate(path) => write!(f, "Wildcards are not allowed in path template: '{path}'"),
            Self::ProtectedDataViolation(msg) => write!(f, "Protected data violation: {msg}"),
            Self::InvalidPattern(pat) => write!(f, "Invalid file pattern: '{pat}'"),
        }
    }
}

impl std::error::Error for ProviderValidationError {}

/// Validates a provider definition against strict security and schema rules.
pub fn validate_provider(provider: &CleanupProviderDef) -> Result<(), ProviderValidationError> {
    if provider.schema_version != SCHEMA_VERSION {
        return Err(ProviderValidationError::UnsupportedSchemaVersion(provider.schema_version));
    }

    if provider.id.is_empty()
        || provider.id.len() > 64
        || !provider.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(ProviderValidationError::InvalidId(provider.id.clone()));
    }

    if provider.tool.trim().is_empty() {
        return Err(ProviderValidationError::MissingField("tool"));
    }
    if provider.label.trim().is_empty() {
        return Err(ProviderValidationError::MissingField("label"));
    }
    if provider.impact.trim().is_empty() {
        return Err(ProviderValidationError::MissingField("impact"));
    }

    if provider.risk != SAFE && provider.risk != CAUTION && provider.risk != PROTECTED {
        return Err(ProviderValidationError::InvalidRisk(provider.risk.clone()));
    }

    if provider.category != AI
        && provider.category != IDE
        && provider.category != PACKAGES
        && provider.category != CONTAINERS
        && provider.category != WINDOWS
        && provider.category != BROWSER
    {
        return Err(ProviderValidationError::InvalidCategory(provider.category.clone()));
    }

    let template = provider.path_template.trim();
    if template.is_empty() {
        return Err(ProviderValidationError::MissingField("pathTemplate"));
    }

    // Path traversal check
    if template.contains("..") {
        return Err(ProviderValidationError::PathTraversal(template.to_string()));
    }

    // Wildcards in path_template are forbidden; wildcards belong in patterns
    if template.contains('*') || template.contains('?') {
        return Err(ProviderValidationError::WildcardInPathTemplate(template.to_string()));
    }

    // Must start with an approved environment root
    let upper = template.to_uppercase();
    let has_approved_root = upper == "%TEMP%"
        || upper.starts_with("%USERPROFILE%\\")
        || upper.starts_with("%LOCALAPPDATA%\\")
        || upper.starts_with("%APPDATA%\\")
        || upper.starts_with("%TEMP%\\")
        || upper.starts_with("%PROGRAMDATA%\\")
        || upper.starts_with("%SYSTEMROOT%\\")
        || upper.starts_with("%WINDIR%\\");

    if !has_approved_root {
        return Err(ProviderValidationError::DisallowedRoot(template.to_string()));
    }

    // Disallow targeting roots directly or sensitive user/system roots
    let forbidden_targets = [
        "%USERPROFILE%",
        "%LOCALAPPDATA%",
        "%APPDATA%",
        "%PROGRAMDATA%",
        "%SYSTEMROOT%",
        "%WINDIR%",
        "%USERPROFILE%\\DESKTOP",
        "%USERPROFILE%\\DOCUMENTS",
        "%USERPROFILE%\\PICTURES",
        "%USERPROFILE%\\VIDEOS",
        "%USERPROFILE%\\MUSIC",
        "%USERPROFILE%\\DOWNLOADS",
        "%SYSTEMROOT%\\SYSTEM32",
        "%WINDIR%\\SYSTEM32",
    ];

    let trimmed_upper = upper.trim_end_matches('\\');
    for forbidden in &forbidden_targets {
        if trimmed_upper == *forbidden {
            return Err(ProviderValidationError::DisallowedRoot(template.to_string()));
        }
    }

    // Protected data checks: models, history, credentials, settings, windows update
    let sensitive_markers = [
        "\\PROJECTS",
        "\\HISTORY",
        "\\SETTINGS",
        "\\.SSH",
        "\\.GNUPG",
        "\\CREDENTIALS",
        "\\CERTIFICATES",
        "\\SOFTWAREDISTRIBUTION",
        "\\DOCKER\\DATA",
        "\\WSL",
    ];

    if provider.risk == SAFE {
        for marker in &sensitive_markers {
            if upper.contains(marker) {
                return Err(ProviderValidationError::ProtectedDataViolation(format!(
                    "Location '{template}' matches sensitive pattern '{marker}' and cannot be marked safe"
                )));
            }
        }
    }

    // Validate patterns
    for pat in &provider.patterns {
        if pat.contains('/') || pat.contains('\\') || pat.contains("..") {
            return Err(ProviderValidationError::InvalidPattern(pat.clone()));
        }
    }

    Ok(())
}

/// Builds the comprehensive registry of cleanup providers, including built-in
/// catalogs and validated developer / AI additions.
pub fn get_default_providers() -> Vec<CleanupProviderDef> {
    let mut providers = Vec::with_capacity(64);

    // AI Providers
    providers.push(CleanupProviderDef {
        schema_version: SCHEMA_VERSION,
        id: "claude-code-projects".into(),
        tool: "Claude Code".into(),
        label: "Conversation history".into(),
        path_template: r"%USERPROFILE%\.claude\projects".into(),
        risk: PROTECTED.into(),
        category: AI.into(),
        impact: "Conversation history and resume state; never removed.".into(),
        app_detection: None,
        patterns: vec!["*".into()],
    });

    providers.push(CleanupProviderDef {
        schema_version: SCHEMA_VERSION,
        id: "claude-code-logs".into(),
        tool: "Claude Code".into(),
        label: "Session logs".into(),
        path_template: r"%USERPROFILE%\.claude\logs".into(),
        risk: SAFE.into(),
        category: AI.into(),
        impact: "Old session logs. Diagnostic only; safe to remove.".into(),
        app_detection: None,
        patterns: vec!["*".into()],
    });

    providers.push(CleanupProviderDef {
        schema_version: SCHEMA_VERSION,
        id: "ollama-models".into(),
        tool: "Ollama".into(),
        label: "Model weights cache".into(),
        path_template: r"%USERPROFILE%\.ollama\models\blobs".into(),
        risk: CAUTION.into(),
        category: AI.into(),
        impact: "Downloaded model weights. Cleaning frees multi-GB space but models must be pulled again.".into(),
        app_detection: None,
        patterns: vec!["*".into()],
    });

    providers.push(CleanupProviderDef {
        schema_version: SCHEMA_VERSION,
        id: "ollama-manifests".into(),
        tool: "Ollama".into(),
        label: "Model manifests".into(),
        path_template: r"%USERPROFILE%\.ollama\models\manifests".into(),
        risk: PROTECTED.into(),
        category: AI.into(),
        impact: "Model catalog declarations. Protected from automated deletion.".into(),
        app_detection: None,
        patterns: vec!["*".into()],
    });

    providers.push(CleanupProviderDef {
        schema_version: SCHEMA_VERSION,
        id: "huggingface-hub".into(),
        tool: "Hugging Face".into(),
        label: "Hub model cache".into(),
        path_template: r"%USERPROFILE%\.cache\huggingface\hub".into(),
        risk: CAUTION.into(),
        category: AI.into(),
        impact: "Cached model snapshots and tokenizer files. Re-downloaded on demand.".into(),
        app_detection: None,
        patterns: vec!["*".into()],
    });

    // Developer & Package Managers
    providers.push(CleanupProviderDef {
        schema_version: SCHEMA_VERSION,
        id: "npm-cache".into(),
        tool: "npm".into(),
        label: "Global tarball cache".into(),
        path_template: r"%LOCALAPPDATA%\npm-cache\_cacache".into(),
        risk: SAFE.into(),
        category: PACKAGES.into(),
        impact: "HTTP response and tarball cache. npm re-downloads packages as needed.".into(),
        app_detection: None,
        patterns: vec!["*".into()],
    });

    providers.push(CleanupProviderDef {
        schema_version: SCHEMA_VERSION,
        id: "pip-cache".into(),
        tool: "pip".into(),
        label: "Wheel download cache".into(),
        path_template: r"%LOCALAPPDATA%\pip\cache".into(),
        risk: SAFE.into(),
        category: PACKAGES.into(),
        impact: "Cached wheel distributions. Python packages will be re-downloaded when built.".into(),
        app_detection: None,
        patterns: vec!["*".into()],
    });

    providers.push(CleanupProviderDef {
        schema_version: SCHEMA_VERSION,
        id: "pypoetry-cache".into(),
        tool: "Poetry".into(),
        label: "Artifact cache".into(),
        path_template: r"%LOCALAPPDATA%\pypoetry\cache".into(),
        risk: SAFE.into(),
        category: PACKAGES.into(),
        impact: "Cached Python package archives. Automatically re-created.".into(),
        app_detection: None,
        patterns: vec!["*".into()],
    });

    providers.push(CleanupProviderDef {
        schema_version: SCHEMA_VERSION,
        id: "uv-cache".into(),
        tool: "uv".into(),
        label: "Package wheel cache".into(),
        path_template: r"%LOCALAPPDATA%\uv\cache".into(),
        risk: SAFE.into(),
        category: PACKAGES.into(),
        impact: "Fast Python wheel cache. Automatically repopulated on uv install.".into(),
        app_detection: None,
        patterns: vec!["*".into()],
    });

    providers.push(CleanupProviderDef {
        schema_version: SCHEMA_VERSION,
        id: "cargo-registry-cache".into(),
        tool: "Rust Cargo".into(),
        label: "Crate archive cache".into(),
        path_template: r"%USERPROFILE%\.cargo\registry\cache".into(),
        risk: SAFE.into(),
        category: PACKAGES.into(),
        impact: "Downloaded .crate archives. Re-downloaded during cargo build if missing.".into(),
        app_detection: None,
        patterns: vec!["*".into()],
    });

    providers.push(CleanupProviderDef {
        schema_version: SCHEMA_VERSION,
        id: "rustup-downloads".into(),
        tool: "Rustup".into(),
        label: "Toolchain installer downloads".into(),
        path_template: r"%USERPROFILE%\.rustup\downloads".into(),
        risk: SAFE.into(),
        category: PACKAGES.into(),
        impact: "Leftover toolchain installer archives. Safe to clean once rustup finishes installing.".into(),
        app_detection: None,
        patterns: vec!["*".into()],
    });

    providers.push(CleanupProviderDef {
        schema_version: SCHEMA_VERSION,
        id: "pnpm-store".into(),
        tool: "pnpm".into(),
        label: "Content-addressable store".into(),
        path_template: r"%LOCALAPPDATA%\pnpm\store".into(),
        risk: CAUTION.into(),
        category: PACKAGES.into(),
        impact: "Global content-addressable storage for hard-linked node modules.".into(),
        app_detection: None,
        patterns: vec!["*".into()],
    });

    providers.push(CleanupProviderDef {
        schema_version: SCHEMA_VERSION,
        id: "vcpkg-archives".into(),
        tool: "vcpkg".into(),
        label: "Binary package cache".into(),
        path_template: r"%LOCALAPPDATA%\vcpkg\archives".into(),
        risk: SAFE.into(),
        category: PACKAGES.into(),
        impact: "Prebuilt C/C++ library archives. Rebuilt by vcpkg on demand.".into(),
        app_detection: None,
        patterns: vec!["*".into()],
    });

    providers.push(CleanupProviderDef {
        schema_version: SCHEMA_VERSION,
        id: "gradle-cache".into(),
        tool: "Gradle".into(),
        label: "Dependency cache".into(),
        path_template: r"%USERPROFILE%\.gradle\caches".into(),
        risk: SAFE.into(),
        category: PACKAGES.into(),
        impact: "Downloaded JAR/AAR dependencies. Re-downloaded when building with Gradle.".into(),
        app_detection: None,
        patterns: vec!["*".into()],
    });

    // IDEs & Editors
    providers.push(CleanupProviderDef {
        schema_version: SCHEMA_VERSION,
        id: "vscode-workspace-storage".into(),
        tool: "VS Code".into(),
        label: "Workspace state cache".into(),
        path_template: r"%APPDATA%\Code\User\workspaceStorage".into(),
        risk: SAFE.into(),
        category: IDE.into(),
        impact: "Window UI state and search index for workspaces. Recreated on open.".into(),
        app_detection: None,
        patterns: vec!["*".into()],
    });

    providers.push(CleanupProviderDef {
        schema_version: SCHEMA_VERSION,
        id: "cursor-workspace-storage".into(),
        tool: "Cursor".into(),
        label: "Workspace state cache".into(),
        path_template: r"%APPDATA%\Cursor\User\workspaceStorage".into(),
        risk: SAFE.into(),
        category: IDE.into(),
        impact: "Workspace editor state and cached indexing. Recreated on open.".into(),
        app_detection: None,
        patterns: vec!["*".into()],
    });

    // Windows Caches
    providers.push(CleanupProviderDef {
        schema_version: SCHEMA_VERSION,
        id: "user-temp".into(),
        tool: "Windows".into(),
        label: "User temporary files".into(),
        path_template: r"%TEMP%".into(),
        risk: SAFE.into(),
        category: WINDOWS.into(),
        impact: "Temporary files created by running programs. Locked files are skipped.".into(),
        app_detection: None,
        patterns: vec!["*".into()],
    });

    providers.push(CleanupProviderDef {
        schema_version: SCHEMA_VERSION,
        id: "windows-software-distribution".into(),
        tool: "Windows Update".into(),
        label: "Update download staging".into(),
        path_template: r"%SYSTEMROOT%\SoftwareDistribution\Download".into(),
        risk: PROTECTED.into(),
        category: WINDOWS.into(),
        impact: "Windows Update internal cache. Protected from third-party cleaning.".into(),
        app_detection: None,
        patterns: vec!["*".into()],
    });

    providers
}

/// Resolves a path template into an absolute path on the current system.
pub fn resolve_path_template(template: &str) -> Option<PathBuf> {
    let mut resolved = template.to_string();
    let env_vars = [
        "USERPROFILE",
        "LOCALAPPDATA",
        "APPDATA",
        "TEMP",
        "PROGRAMDATA",
        "SYSTEMROOT",
        "WINDIR",
    ];

    for var in &env_vars {
        let pattern = format!("%{var}%");
        if resolved.contains(&pattern) {
            let val = std::env::var(var).ok()?;
            resolved = resolved.replace(&pattern, &val);
        }
    }

    let path = Path::new(&resolved);
    if path.is_absolute() {
        Some(path.to_path_buf())
    } else {
        None
    }
}

/// Matches a candidate directory against registered providers to determine
/// if it is a cleanable target, protected target, or unknown directory.
pub fn match_path_to_provider<'a>(
    providers: &'a [CleanupProviderDef],
    dir_path: &Path,
) -> Option<(&'a CleanupProviderDef, PathBuf)> {
    let dir_canon = std::fs::canonicalize(dir_path).unwrap_or_else(|_| dir_path.to_path_buf());
    let dir_norm = dir_canon.to_string_lossy().to_lowercase();

    for provider in providers {
        if let Some(resolved) = resolve_path_template(&provider.path_template) {
            let res_canon = std::fs::canonicalize(&resolved).unwrap_or(resolved.clone());
            let res_norm = res_canon.to_string_lossy().to_lowercase();

            if dir_norm == res_norm {
                return Some((provider, resolved));
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_all_default_providers() {
        let providers = get_default_providers();
        assert!(!providers.is_empty());
        for p in &providers {
            assert!(validate_provider(p).is_ok(), "Provider {} failed validation", p.id);
        }
    }

    #[test]
    fn rejects_unsupported_schema_version() {
        let mut p = get_default_providers()[0].clone();
        p.schema_version = 99;
        assert_eq!(
            validate_provider(&p),
            Err(ProviderValidationError::UnsupportedSchemaVersion(99))
        );
    }

    #[test]
    fn rejects_path_traversal() {
        let mut p = get_default_providers()[0].clone();
        p.path_template = r"%LOCALAPPDATA%\..\Windows\System32".into();
        assert!(matches!(
            validate_provider(&p),
            Err(ProviderValidationError::PathTraversal(_))
        ));
    }

    #[test]
    fn rejects_disallowed_roots() {
        let mut p = get_default_providers()[0].clone();
        p.path_template = r"C:\Windows\System32".into(); // No approved env var
        assert!(matches!(
            validate_provider(&p),
            Err(ProviderValidationError::DisallowedRoot(_))
        ));

        let mut p2 = get_default_providers()[0].clone();
        p2.path_template = r"%USERPROFILE%\Documents".into(); // Sensitive folder
        assert!(matches!(
            validate_provider(&p2),
            Err(ProviderValidationError::DisallowedRoot(_))
        ));
    }

    #[test]
    fn rejects_safe_risk_for_protected_data() {
        let mut p = get_default_providers()[0].clone();
        p.path_template = r"%USERPROFILE%\.claude\projects".into();
        p.risk = SAFE.into(); // Projects contains conversation history!
        assert!(matches!(
            validate_provider(&p),
            Err(ProviderValidationError::ProtectedDataViolation(_))
        ));
    }

    #[test]
    fn rejects_wildcards_in_path_template() {
        let mut p = get_default_providers()[0].clone();
        p.path_template = r"%LOCALAPPDATA%\Temp\*.tmp".into();
        assert!(matches!(
            validate_provider(&p),
            Err(ProviderValidationError::WildcardInPathTemplate(_))
        ));
    }
}
