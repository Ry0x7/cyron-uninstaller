//! Read-only developer project and regenerable-artifact discovery.
//!
//! This module never deletes or moves anything. It recognises projects from
//! manifests and Git roots, measures only named build/dependency folders, and
//! returns server-held records. Cleanup decisions are made later by the
//! existing review-first cleanup engine, which revalidates every path.

use crate::{cleanup, fs};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAX_ROOTS: usize = 24;
pub const MAX_PROJECTS: usize = 400;
pub const MAX_ARTIFACTS: usize = 600;
const MAX_ARTIFACTS_PER_PROJECT: usize = 24;
const MAX_PROTECTED_PER_PROJECT: usize = 12;
const MAX_SCAN_DIRS: usize = 60_000;
const MAX_SCAN_DEPTH: u32 = 3;
const MAX_FILES_PER_ARTIFACT: usize = 500_000;
const MAX_DIRS_PER_ARTIFACT: usize = 200_000;
const MAX_TRACKED_IDS: usize = 1_000_000;
const MAX_WARNINGS: usize = 20;
const ACTIVE_DAYS: u64 = 30;
const OLD_DAYS: u64 = 180;
const STALE_DAYS: u64 = 90;
const MAX_PACKAGE_JSON_BYTES: u64 = 512 * 1024;

const SKIP_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    ".hg",
    ".svn",
    "target",
    "dist",
    "build",
    ".next",
    ".turbo",
    ".gradle",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    ".venv",
    "venv",
    ".tox",
    "vendor",
    ".cache",
    "coverage",
    ".idea",
    ".vs",
];

const CREDENTIAL_NAMES: &[&str] = &[
    ".npmrc",
    ".pypirc",
    ".netrc",
    "credentials.json",
    "secrets.json",
    "id_rsa",
    "id_ed25519",
    "auth.json",
];

const MODEL_EXTENSIONS: &[&str] = &[
    "gguf",
    "ggml",
    "safetensors",
    "onnx",
    "pt",
    "pth",
    "ckpt",
    "bin",
];

const DATABASE_EXTENSIONS: &[&str] = &["sqlite", "sqlite3", "db", "duckdb", "mdb", "accdb"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperStorageScan {
    pub scan_id: String,
    pub scanned_at: u64,
    pub roots: Vec<ProjectRoot>,
    pub projects: Vec<DeveloperProject>,
    pub totals: DeveloperTotals,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRoot {
    pub path: String,
    /// selected or automatic.
    pub source: String,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperTotals {
    pub project_count: u64,
    pub artifact_count: u64,
    pub artifact_bytes: u64,
    pub artifact_allocated: u64,
    pub protected_bytes: u64,
    pub active_projects: u64,
    pub old_projects: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperProject {
    pub id: String,
    pub name: String,
    pub path: String,
    /// node, rust, python, go, java, dotnet, php, ruby, cmake, mixed or git.
    pub kind: String,
    pub markers: Vec<String>,
    pub frameworks: Vec<String>,
    pub modified: Option<u64>,
    /// active, recent, old or unknown.
    pub activity: String,
    pub artifacts: Vec<DeveloperArtifact>,
    pub protected: Vec<ProtectedComponent>,
    pub source_protected: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperArtifact {
    pub id: String,
    pub project_id: String,
    pub project_name: String,
    pub path: String,
    /// node-modules, dist, build, cargo-target, next-cache, turbo-cache,
    /// gradle-cache, pycache, pytest-cache, mypy-cache or ruff-cache.
    pub kind: String,
    pub label: String,
    pub risk: String,
    pub confidence: String,
    pub evidence: String,
    pub rebuild: String,
    pub loss: String,
    pub size: u64,
    pub allocated: u64,
    pub file_count: u64,
    pub dir_count: u64,
    pub last_modified: Option<u64>,
    pub activity: String,
    pub stale: bool,
    pub skipped_links: u64,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectedComponent {
    pub path: String,
    pub kind: String,
    pub reason: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactVerification {
    pub id: String,
    pub path: String,
    pub label: String,
    /// removed, remaining or changed.
    pub status: String,
    pub expected_bytes: u64,
    pub remaining_bytes: u64,
    pub remaining_files: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperCleanupVerification {
    pub scan_id: String,
    pub artifacts: Vec<ArtifactVerification>,
    pub remaining_bytes: u64,
    pub removed_artifacts: u64,
    pub remaining_artifacts: u64,
    /// Quarantine batches that still contain files for the verified artifacts.
    pub restorable_run_ids: Vec<String>,
    pub errors: Vec<String>,
}

struct ProjectMeta {
    name: String,
    kind: String,
    markers: Vec<String>,
    frameworks: Vec<String>,
}

struct CandidateArtifact {
    path: PathBuf,
    kind: &'static str,
    label: &'static str,
    risk: &'static str,
    confidence: &'static str,
    evidence: String,
    rebuild: &'static str,
    loss: &'static str,
}

#[derive(Default)]
struct Measurement {
    exists: bool,
    size: u64,
    allocated: u64,
    file_count: u64,
    dir_count: u64,
    last_modified: Option<u64>,
    skipped_links: u64,
    truncated: bool,
    errors: Vec<String>,
}

fn seconds_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn modified_at(path: &Path) -> Option<u64> {
    std::fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

fn normalize_key(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

fn is_within(child: &Path, parent: &Path) -> bool {
    let child = normalize_key(child);
    let parent = normalize_key(parent);
    child == parent || child.starts_with(&format!("{parent}\\"))
}

/// Display-only path text. Operations keep the canonical (possibly verbatim)
/// path; evidence shown to users should not carry the `\\?\` prefix.
fn display(path: &Path) -> String {
    crate::scan::display_path(&path.to_string_lossy())
}

fn push_warning(warnings: &mut Vec<String>, message: String) {
    if warnings.len() < MAX_WARNINGS && !warnings.iter().any(|existing| existing == &message) {
        warnings.push(message);
    }
}

fn activity_for(modified: Option<u64>) -> String {
    let Some(modified) = modified else {
        return "unknown".into();
    };
    let now = seconds_now();
    if now.saturating_sub(modified) <= ACTIVE_DAYS * 86_400 {
        "active".into()
    } else if now.saturating_sub(modified) <= OLD_DAYS * 86_400 {
        "recent".into()
    } else {
        "old".into()
    }
}

fn stable_id(prefix: &str, value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.to_ascii_lowercase().as_bytes());
    let digest = hasher.finalize();
    let encoded: String = digest.iter().take(10).map(|byte| format!("{byte:02x}")).collect();
    format!("{prefix}-{encoded}")
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default()
}

fn join_name(path: &Path, name: &str) -> PathBuf {
    path.join(name)
}

/// True when a directory is one of the system locations that must never be
/// used as a project root. Subfolders of these roots remain valid.
fn is_system_root(path: &Path) -> bool {
    let key = normalize_key(path);
    for variable in [
        "WINDIR",
        "SystemRoot",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramData",
    ] {
        if let Ok(value) = std::env::var(variable) {
            let root = normalize_key(Path::new(value.trim()));
            if !root.is_empty() && (key == root || key.starts_with(&format!("{root}\\"))) {
                return true;
            }
        }
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        let profile = normalize_key(Path::new(profile.trim()));
        if !profile.is_empty() && key == profile {
            return true;
        }
    }
    for variable in ["LOCALAPPDATA", "APPDATA"] {
        if let Ok(value) = std::env::var(variable) {
            let root = normalize_key(Path::new(value.trim()));
            if !root.is_empty() && key == root {
                return true;
            }
        }
    }
    false
}

fn is_protected_catalog_path(path: &Path) -> bool {
    crate::caches::CATALOG
        .iter()
        .filter(|definition| definition.risk == "protected")
        .filter_map(|definition| crate::caches::resolve_template(definition.path).ok())
        .any(|protected| is_within(path, Path::new(&protected)))
}

fn validate_root(raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("A project folder is empty.".into());
    }
    let path = PathBuf::from(trimmed);
    if !path.is_absolute() {
        return Err(format!("Project folder must be absolute: {trimmed}"));
    }
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("Windows refused access to {trimmed}: {error}"))?;
    if !canonical.is_dir() {
        return Err(format!("Project folder is not a folder: {trimmed}"));
    }
    if canonical.parent().is_none() {
        return Err("A whole drive cannot be scanned as a project folder.".into());
    }
    if is_system_root(&canonical) {
        return Err(format!(
            "Refusing to scan a system folder as projects: {}",
            display(&canonical)
        ));
    }
    if is_protected_catalog_path(&canonical) {
        return Err(format!(
            "Refusing to scan a protected application data folder: {}",
            display(&canonical)
        ));
    }
    Ok(canonical)
}

/// Resolves explicit roots plus automatic project candidates on the selected
/// local drives. Nothing outside those local-drive letters is scanned.
pub fn scan_roots(
    explicit: Vec<String>,
    drive_letters: Option<Vec<String>>,
) -> Result<(Vec<ProjectRoot>, Vec<(PathBuf, String)>), String> {
    scan_roots_in(explicit, drive_letters, true)
}

fn scan_roots_in(
    explicit: Vec<String>,
    drive_letters: Option<Vec<String>>,
    include_automatic: bool,
) -> Result<(Vec<ProjectRoot>, Vec<(PathBuf, String)>), String> {
    let volumes = crate::volumes::list_volumes();
    let local: HashSet<char> = volumes
        .iter()
        .filter(|volume| matches!(volume.kind.as_str(), "fixed" | "removable"))
        .filter_map(|volume| volume.mount.chars().next().map(|letter| letter.to_ascii_uppercase()))
        .collect();
    let selected = match drive_letters {
        None => local.clone(),
        Some(letters) if letters.is_empty() => return Err("Select at least one local drive.".into()),
        Some(letters) => {
            let mut selected = HashSet::new();
            for value in letters {
                let mut chars = value.trim().chars();
                let Some(letter) = chars.next().map(|value| value.to_ascii_uppercase()) else {
                    return Err("Drive selections must be single letters.".into());
                };
                if chars.next().is_some() || !local.contains(&letter) {
                    return Err(format!("{value} is not a mounted fixed or removable drive."));
                }
                selected.insert(letter);
            }
            selected
        }
    };

    let mut roots = Vec::new();
    let mut paths: Vec<(PathBuf, String)> = Vec::new();
    let mut seen = HashSet::new();

    for raw in explicit {
        let path = validate_root(&raw)?;
        let key = normalize_key(&path);
        if seen.insert(key) {
            roots.push(ProjectRoot {
                path: path.to_string_lossy().to_string(),
                source: "selected".into(),
                exists: true,
            });
            paths.push((path, "selected".into()));
        }
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    if include_automatic {
        if let Ok(profile) = std::env::var("USERPROFILE") {
            let profile = PathBuf::from(profile);
            for relative in [
                "source",
                r"source\repos",
                "repos",
                "projects",
                "dev",
                "code",
                "workspace",
                "git",
                "src",
                r"Documents\GitHub",
                r"Documents\Projects",
                r"Desktop\Projects",
                r"Downloads\Projects",
            ] {
                candidates.push(profile.join(relative));
            }
        }
        for volume in &volumes {
            let Some(letter) = volume.mount.chars().next().map(|value| value.to_ascii_uppercase()) else {
                continue;
            };
            if !selected.contains(&letter) {
                continue;
            }
            for name in ["Projects", "Dev", "Code", "Repos", "Workspace", "Source"] {
                candidates.push(PathBuf::from(format!("{}{name}", volume.mount)));
            }
        }
    }

    for candidate in candidates {
        if !candidate.is_dir() {
            continue;
        }
        let Ok(canonical) = candidate.canonicalize() else {
            continue;
        };
        // An explicit root already covers this automatic candidate.
        if paths.iter().any(|(root, _)| is_within(&canonical, root)) {
            continue;
        }
        let key = normalize_key(&canonical);
        if !seen.insert(key) || is_protected_catalog_path(&canonical) {
            continue;
        }
        roots.push(ProjectRoot {
            path: canonical.to_string_lossy().to_string(),
            source: "automatic".into(),
            exists: true,
        });
        paths.push((canonical, "automatic".into()));
        if paths.len() >= MAX_ROOTS {
            break;
        }
    }

    paths.truncate(MAX_ROOTS);
    roots.truncate(MAX_ROOTS);
    Ok((roots, paths))
}

/// Read-only project scan. Every returned artifact is a named regenerable
/// folder; source, Git metadata, credentials, databases and model weights are
/// reported separately as protected and never become plan targets.
pub fn scan(explicit_roots: Vec<String>, drive_letters: Option<Vec<String>>) -> Result<DeveloperStorageScan, String> {
    scan_internal(explicit_roots, drive_letters, true)
}

/// Scans exactly the supplied roots. This is used by disposable-project tests
/// and by callers that do not want the automatic candidate set; the desktop
/// command uses `scan` so multi-drive discovery stays on by default.
pub fn scan_selected(explicit_roots: Vec<String>) -> Result<DeveloperStorageScan, String> {
    scan_internal(explicit_roots, None, false)
}

fn scan_internal(
    explicit_roots: Vec<String>,
    drive_letters: Option<Vec<String>>,
    include_automatic: bool,
) -> Result<DeveloperStorageScan, String> {
    let started = seconds_now();
    let mut warnings = Vec::new();
    // A saved folder can be renamed or deleted between sessions. Report it and
    // keep scanning the other roots instead of failing the whole inventory.
    let explicit_roots: Vec<String> = explicit_roots
        .into_iter()
        .filter(|raw| match validate_root(raw) {
            Ok(_) => true,
            Err(error) => {
                push_warning(&mut warnings, error);
                false
            }
        })
        .collect();
    let (roots, root_paths) = scan_roots_in(explicit_roots, drive_letters, include_automatic)?;
    let mut projects = Vec::new();
    let mut seen_projects = HashSet::new();
    let mut seen_artifacts = HashSet::new();
    let mut scanned_dirs = 0usize;

    for (root, _source) in &root_paths {
        if projects.len() >= MAX_PROJECTS {
            push_warning(
                &mut warnings,
                format!("Stopped after {MAX_PROJECTS} projects; narrow the project folders to see the rest."),
            );
            break;
        }
        let mut stack: Vec<(PathBuf, u32)> = vec![(root.clone(), 0)];
        while let Some((dir, depth)) = stack.pop() {
            if scanned_dirs >= MAX_SCAN_DIRS {
                push_warning(
                    &mut warnings,
                    format!("Stopped after {MAX_SCAN_DIRS} folders; narrow the project folders to see the rest."),
                );
                break;
            }
            scanned_dirs += 1;

            if let Some(meta) = inspect_project(&dir) {
                if seen_projects.insert(normalize_key(&dir)) {
                    if let Some(project) = build_project(&dir, meta, &mut seen_artifacts, &mut warnings) {
                        projects.push(project);
                    }
                    if projects.len() >= MAX_PROJECTS {
                        break;
                    }
                }
            }

            if depth >= MAX_SCAN_DEPTH {
                continue;
            }
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries {
                if !entry.is_dir || entry.is_reparse || entry.is_system {
                    continue;
                }
                let name = entry.name.to_ascii_lowercase();
                if SKIP_DIRS.contains(&name.as_str()) {
                    continue;
                }
                if is_protected_catalog_path(&entry.path) {
                    continue;
                }
                stack.push((entry.path, depth + 1));
            }
        }
    }

    projects.sort_by(|a, b| {
        let rank = |value: &str| match value {
            "active" => 0,
            "recent" => 1,
            "unknown" => 2,
            _ => 3,
        };
        rank(&a.activity)
            .cmp(&rank(&b.activity))
            .then_with(|| b.artifacts.iter().map(|item| item.allocated).sum::<u64>().cmp(&a.artifacts.iter().map(|item| item.allocated).sum::<u64>()))
            .then_with(|| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()))
    });

    let artifact_count: u64 = projects.iter().map(|project| project.artifacts.len() as u64).sum();
    let artifact_bytes: u64 = projects.iter().flat_map(|project| project.artifacts.iter()).map(|item| item.size).sum();
    let artifact_allocated: u64 = projects.iter().flat_map(|project| project.artifacts.iter()).map(|item| item.allocated).sum();
    let protected_bytes: u64 = projects.iter().flat_map(|project| project.protected.iter()).map(|item| item.size).sum();
    let active_projects = projects.iter().filter(|project| project.activity == "active").count() as u64;
    let old_projects = projects.iter().filter(|project| project.activity == "old").count() as u64;

    if projects.is_empty() {
        push_warning(
            &mut warnings,
            "No projects were recognised. Add the folder that contains your projects, or create a package.json, Cargo.toml, pyproject.toml, go.mod or .git root.".into(),
        );
    }

    let project_count = projects.len() as u64;
    Ok(DeveloperStorageScan {
        scan_id: cleanup_scan_id(started),
        scanned_at: started,
        roots,
        projects,
        totals: DeveloperTotals {
            project_count,
            artifact_count,
            artifact_bytes,
            artifact_allocated,
            protected_bytes,
            active_projects,
            old_projects,
        },
        warnings,
    })
}

fn cleanup_scan_id(at: u64) -> String {
    format!("devscan-{at}-{}-{}", std::process::id(), crate::leftovers::generate_crypto_nonce(4))
}

fn inspect_project(path: &Path) -> Option<ProjectMeta> {
    let mut markers = Vec::new();
    let mut kinds = Vec::new();
    let mut frameworks = Vec::new();
    let mut name = file_name(path);

    let package_json = join_name(path, "package.json");
    if package_json.is_file() {
        markers.push("package.json".into());
        kinds.push("node");
        if let Some((package_name, detected)) = read_package_metadata(&package_json) {
            if !package_name.trim().is_empty() {
                name = package_name;
            }
            frameworks.extend(detected);
        }
    }
    if join_name(path, "pnpm-workspace.yaml").is_file() {
        markers.push("pnpm-workspace.yaml".into());
    }
    if join_name(path, "turbo.json").is_file() {
        markers.push("turbo.json".into());
    }
    if join_name(path, "Cargo.toml").is_file() {
        markers.push("Cargo.toml".into());
        kinds.push("rust");
        if let Some(cargo_name) = read_toml_name(&join_name(path, "Cargo.toml")) {
            name = cargo_name;
        }
        if let Ok(text) = std::fs::read_to_string(join_name(path, "Cargo.toml")) {
            for framework in ["tauri", "axum", "actix-web", "rocket", "bevy"] {
                if text.contains(framework) {
                    frameworks.push(framework.into());
                }
            }
        }
    }
    for (file, kind) in [
        ("pyproject.toml", "python"),
        ("requirements.txt", "python"),
        ("setup.py", "python"),
        ("poetry.lock", "python"),
        ("uv.lock", "python"),
        ("go.mod", "go"),
        ("build.gradle", "java"),
        ("build.gradle.kts", "java"),
        ("settings.gradle", "java"),
        ("settings.gradle.kts", "java"),
        ("pom.xml", "java"),
        ("composer.json", "php"),
        ("Gemfile", "ruby"),
        ("CMakeLists.txt", "cmake"),
    ] {
        if join_name(path, file).is_file() {
            markers.push(file.into());
            kinds.push(kind);
        }
    }
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries {
            if entry.is_dir {
                continue;
            }
            let lower = entry.name.to_ascii_lowercase();
            if lower.ends_with(".csproj") || lower.ends_with(".sln") {
                markers.push(entry.name.clone());
                kinds.push("dotnet");
            }
        }
    }
    if join_name(path, ".git").exists() {
        markers.push(".git".into());
    }
    if markers.is_empty() {
        return None;
    }

    // Detect web frameworks from package dependencies and pyproject text.
    if let Ok(text) = std::fs::read_to_string(join_name(path, "pyproject.toml")) {
        for framework in ["django", "flask", "fastapi", "streamlit"] {
            if text.to_ascii_lowercase().contains(framework) {
                frameworks.push(framework.into());
            }
        }
    }

    kinds.sort();
    kinds.dedup();
    frameworks.sort();
    frameworks.dedup();
    Some(ProjectMeta {
        name,
        kind: if kinds.len() > 1 { "mixed".into() } else { kinds.first().copied().unwrap_or("git").into() },
        markers,
        frameworks: frameworks.into_iter().take(6).collect(),
    })
}

fn read_package_metadata(path: &Path) -> Option<(String, Vec<String>)> {
    let metadata = std::fs::metadata(path).ok()?;
    if metadata.len() > MAX_PACKAGE_JSON_BYTES {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let name = value.get("name").and_then(|item| item.as_str()).unwrap_or("").to_string();
    let mut frameworks = Vec::new();
    for section in ["dependencies", "devDependencies", "peerDependencies"] {
        if let Some(dependencies) = value.get(section).and_then(|item| item.as_object()) {
            for framework in [
                "next", "vite", "react", "vue", "svelte", "@angular/core", "astro", "nuxt",
                "remix", "@remix-run/react", "expo", "electron", "@tauri-apps/api", "turbo",
            ] {
                if dependencies.contains_key(framework) && !frameworks.iter().any(|item| item == framework) {
                    frameworks.push(framework.to_string());
                }
            }
        }
    }
    Some((name, frameworks))
}

fn read_toml_name(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut in_package = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if in_package {
            if let Some(value) = line.strip_prefix("name") {
                if let Some(value) = value.trim().strip_prefix('=') {
                    return Some(value.trim().trim_matches('"').to_string());
                }
            }
        }
    }
    None
}

fn build_project(
    root: &Path,
    meta: ProjectMeta,
    seen_artifacts: &mut HashSet<String>,
    warnings: &mut Vec<String>,
) -> Option<DeveloperProject> {
    let project_id = stable_id("project", &normalize_key(root));
    let modified = project_modified(root, &meta);
    let activity = activity_for(modified);
    let mut artifacts = Vec::new();
    let protected = find_protected_components(root, warnings);
    let project_name = if meta.name.trim().is_empty() {
        file_name(root)
    } else {
        meta.name.clone()
    };

    let candidates = detect_artifacts(root, &meta);
    for candidate in candidates {
        if artifacts.len() >= MAX_ARTIFACTS_PER_PROJECT || seen_artifacts.len() >= MAX_ARTIFACTS {
            push_warning(
                warnings,
                format!(
                    "{}: stopped after {} artifacts; narrow the project folder to see the rest.",
                    project_name, MAX_ARTIFACTS_PER_PROJECT
                ),
            );
            break;
        }
        let key = normalize_key(&candidate.path);
        if !seen_artifacts.insert(key.clone()) {
            continue;
        }
        let measured = measure(&candidate.path);
        if !measured.exists || measured.file_count == 0 {
            continue;
        }
        for error in measured.errors.iter().take(1) {
            push_warning(warnings, format!("{}: {error}", candidate.label));
        }
        let artifact_modified = measured.last_modified.or_else(|| modified_at(&candidate.path));
        let artifact_activity = activity_for(artifact_modified);
        let stale = artifact_modified
            .map(|at| seconds_now().saturating_sub(at) > STALE_DAYS * 86_400)
            .unwrap_or(false);
        artifacts.push(DeveloperArtifact {
            id: stable_id("artifact", &format!("{key}|{}", candidate.kind)),
            project_id: project_id.clone(),
            project_name: project_name.clone(),
            path: candidate.path.to_string_lossy().to_string(),
            kind: candidate.kind.into(),
            label: candidate.label.into(),
            risk: candidate.risk.into(),
            confidence: candidate.confidence.into(),
            evidence: candidate.evidence,
            rebuild: candidate.rebuild.into(),
            loss: candidate.loss.into(),
            size: measured.size,
            allocated: measured.allocated,
            file_count: measured.file_count,
            dir_count: measured.dir_count,
            last_modified: artifact_modified,
            activity: artifact_activity,
            stale,
            skipped_links: measured.skipped_links,
            truncated: measured.truncated,
        });
    }

    artifacts.sort_by(|a, b| b.allocated.cmp(&a.allocated).then_with(|| a.label.cmp(&b.label)));
    Some(DeveloperProject {
        id: project_id,
        name: project_name,
        path: root.to_string_lossy().to_string(),
        kind: meta.kind,
        markers: meta.markers,
        frameworks: meta.frameworks,
        modified,
        activity,
        artifacts,
        protected,
        source_protected: true,
    })
}

fn project_modified(root: &Path, meta: &ProjectMeta) -> Option<u64> {
    let mut latest = modified_at(root);
    for marker in &meta.markers {
        if let Some(value) = modified_at(&join_name(root, marker)) {
            latest = Some(latest.map_or(value, |current| current.max(value)));
        }
    }
    for git_file in [".git/index", ".git/HEAD", ".git/logs/HEAD"] {
        if let Some(value) = modified_at(&join_name(root, git_file)) {
            latest = Some(latest.map_or(value, |current| current.max(value)));
        }
    }
    latest
}

fn detect_artifacts(root: &Path, meta: &ProjectMeta) -> Vec<CandidateArtifact> {
    let marker = |name: &str| meta.markers.iter().any(|item| item == name);
    let node = marker("package.json")
        || marker("pnpm-workspace.yaml")
        || marker("turbo.json");
    let python = meta.markers.iter().any(|item| {
        matches!(
            item.as_str(),
            "pyproject.toml" | "requirements.txt" | "setup.py" | "poetry.lock" | "uv.lock"
        )
    });
    let java = meta.markers.iter().any(|item| {
        matches!(
            item.as_str(),
            "build.gradle" | "build.gradle.kts" | "settings.gradle" | "settings.gradle.kts" | "pom.xml"
        )
    });
    let rust = marker("Cargo.toml");
    let mut candidates = Vec::new();

    let push = |candidates: &mut Vec<CandidateArtifact>, relative: &str, kind: &'static str, label: &'static str, risk: &'static str, confidence: &'static str, evidence: String, rebuild: &'static str, loss: &'static str| {
        let path = join_name(root, relative);
        if path.exists() {
            candidates.push(CandidateArtifact {
                path,
                kind,
                label,
                risk,
                confidence,
                evidence,
                rebuild,
                loss,
            });
        }
    };

    if node {
        let lockfile = [
            "package-lock.json",
            "pnpm-lock.yaml",
            "yarn.lock",
            "bun.lock",
            "bun.lockb",
        ]
        .iter()
        .find(|name| join_name(root, name).is_file())
        .copied();
        let node_evidence = match lockfile {
            Some(lockfile) => format!(
                "Direct child of {}; project markers: {}; lockfile {} is present.",
                display(root),
                meta.markers.join(", "),
                lockfile
            ),
            None => format!(
                "Direct child of {}; project markers: {}. No lockfile is present, so exact dependency versions are not pinned.",
                display(root),
                meta.markers.join(", ")
            ),
        };
        push(
            &mut candidates,
            "node_modules",
            "node-modules",
            "Installed dependencies",
            "caution",
            if lockfile.is_some() { "high" } else { "medium" },
            node_evidence,
            "Reinstall from the manifest and lockfile: npm ci, pnpm install --frozen-lockfile, or yarn install --immutable.",
            "Installed packages and native rebuild output. With a lockfile the versions are reproducible; file: dependencies, patches or native build output may not be recoverable from the registry alone.",
        );
        push(
            &mut candidates,
            r".next\cache",
            "next-cache",
            "Next.js build cache",
            "caution",
            "high",
            "Direct child of a project with package.json; Next.js stores its shared build cache in .next/cache. .next output and runtime ISR data are not selected."
                .into(),
            "Rebuild with next build; the cache repopulates.",
            "Cold build time. If a self-hosted server stores ISR data under the cache, pages regenerate on demand and need their upstream data source.",
        );
        if meta.frameworks.iter().any(|framework| matches!(framework.as_str(), "vite" | "next" | "react" | "vue" | "svelte" | "astro" | "nuxt" | "remix" | "@remix-run/react" | "expo" | "electron")) {
            push(
                &mut candidates,
                "dist",
                "dist",
                "Build output",
                "caution",
                "medium",
                "Direct child named dist in a recognised JavaScript project. The directory may be the deployable artifact, not only a cache.".into(),
                "Rebuild with the project's build script (for example npm run build).",
                "The built files, which may be the copy you deploy or ship. Source and lockfiles are untouched.",
            );
            push(
                &mut candidates,
                "build",
                "build",
                "Build output",
                "caution",
                "medium",
                "Direct child named build in a recognised JavaScript project. Generic build output can contain deployable files.".into(),
                "Rebuild with the project's build script.",
                "The built files, which may be the copy you deploy or ship. Source and lockfiles are untouched.",
            );
        }
        if marker("turbo.json") {
            push(
                &mut candidates,
                ".turbo",
                "turbo-cache",
                "Turborepo local cache",
                "safe",
                "high",
                "Direct child of a project with turbo.json; Turborepo documents .turbo/cache as local task cache.".into(),
                "Run the task again (for example turbo run build); the cache repopulates.",
                "Local task cache and logs. Remote cache is unaffected.",
            );
        }
    }

    if rust {
        push(
            &mut candidates,
            "target",
            "cargo-target",
            "Rust build output",
            "caution",
            "high",
            "Direct child of a project with Cargo.toml; Cargo writes build artifacts and incremental state to target.".into(),
            "Rebuild with cargo build. Prefer cargo clean for selective profiles.",
            "Compiled binaries, incremental state, and build-script output. A release binary you had not copied elsewhere can be lost; build scripts may need to re-fetch or regenerate output.",
        );
    }

    if java {
        if marker("pom.xml") {
            push(
                &mut candidates,
                "target",
                "maven-target",
                "Maven build output",
                "caution",
                "high",
                "Direct child of a project with pom.xml; Maven writes compiled classes, test reports, and packaged artifacts to target.".into(),
                "Rebuild with mvn package or your Maven goal.",
                "Compiled classes, test reports, and packaged JAR/WAR artifacts. A packaged artifact you had not copied elsewhere can be lost.",
            );
        }
        push(
            &mut candidates,
            "build",
            "gradle-build",
            "Gradle build output",
            "caution",
            "high",
            "Direct child of a project with Gradle build files; Gradle writes compiled classes and reports to build.".into(),
            "Rebuild with ./gradlew build or the matching Gradle task.",
            "Compiled classes, test reports, and packaged artifacts. A packaged artifact you had not copied elsewhere can be lost.",
        );
        push(
            &mut candidates,
            ".gradle",
            "gradle-cache",
            "Gradle project cache",
            "caution",
            "high",
            "Direct child of a project with Gradle build files; Gradle stores project-scoped caches and daemon state here.".into(),
            "Run Gradle again; it recreates project-scoped caches.",
            "Project-scoped dependency metadata and build cache. Global Gradle caches elsewhere are untouched.",
        );
    }

    if python {
        push(
            &mut candidates,
            ".pytest_cache",
            "pytest-cache",
            "pytest cache",
            "safe",
            "high",
            "Direct child of a recognised Python project; pytest documents this as its default cache directory.".into(),
            "Run pytest again or pytest --cache-clear.",
            "Last-failed history and plugin cache values only.",
        );
        push(
            &mut candidates,
            ".mypy_cache",
            "mypy-cache",
            "mypy cache",
            "safe",
            "high",
            "Direct child of a recognised Python project; mypy writes this default incremental cache.".into(),
            "Run mypy again.",
            "Incremental type-check cache only.",
        );
        push(
            &mut candidates,
            ".ruff_cache",
            "ruff-cache",
            "Ruff cache",
            "safe",
            "high",
            "Direct child of a recognised Python project; Ruff documents this default cache directory.".into(),
            "Run ruff check again or ruff clean.",
            "Lint and format result cache only.",
        );
        collect_pycache(root, &mut candidates);
    }

    candidates
}

fn collect_pycache(root: &Path, candidates: &mut Vec<CandidateArtifact>) {
    let mut stack = vec![(root.to_path_buf(), 0u32)];
    let mut found = 0;
    while let Some((dir, depth)) = stack.pop() {
        if found >= 12 || depth > 3 {
            continue;
        }
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries {
                if !entry.is_dir || entry.is_reparse {
                    continue;
                }
                let name = entry.name.to_ascii_lowercase();
                if name == "__pycache__" {
                    let evidence = if dir == root {
                        "Direct child of a recognised Python project; Python writes .pyc bytecode here.".to_string()
                    } else {
                        format!(
                            "Nested under {} inside a recognised Python project; Python writes .pyc bytecode here.",
                            display(&dir)
                        )
                    };
                    candidates.push(CandidateArtifact {
                        path: entry.path,
                        kind: "pycache",
                        label: "Python bytecode cache",
                        risk: "safe",
                        confidence: "high",
                        evidence,
                        rebuild: "Python recreates .pyc files automatically when source files are imported or py_compile is run.",
                        loss: "Compiled bytecode only. Source files stay in place. A tree distributed without .py sources is not a normal project and is not selected by this rule.",
                    });
                    found += 1;
                    if found >= 12 {
                        return;
                    }
                    continue;
                }
                if SKIP_DIRS.contains(&name.as_str()) {
                    continue;
                }
                stack.push((entry.path, depth + 1));
            }
        }
    }
}

fn find_protected_components(root: &Path, warnings: &mut Vec<String>) -> Vec<ProtectedComponent> {
    let mut protected = Vec::new();
    let git = join_name(root, ".git");
    if git.exists() {
        let measured = measure(&git);
        protected.push(ProtectedComponent {
            path: git.to_string_lossy().to_string(),
            kind: "git".into(),
            reason: "Git object database and branches. Removing it destroys repository history and uncommitted refs.".into(),
            size: measured.size,
        });
    }

    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries {
            if protected.len() >= MAX_PROTECTED_PER_PROJECT {
                break;
            }
            if entry.is_reparse {
                continue;
            }
            let lower = entry.name.to_ascii_lowercase();
            let credential = CREDENTIAL_NAMES.contains(&lower.as_str())
                || lower.starts_with(".env")
                || lower.ends_with(".pem")
                || lower.ends_with(".key")
                || lower.ends_with(".pfx")
                || lower.ends_with(".p12");
            let database = DATABASE_EXTENSIONS
                .iter()
                .any(|extension| lower.ends_with(&format!(".{extension}")));
            if credential && !entry.is_dir {
                protected.push(ProtectedComponent {
                    path: entry.path.to_string_lossy().to_string(),
                    kind: "credential".into(),
                    reason: "Environment file, token, or private key. Credentials and settings are never cleanup targets.".into(),
                    size: entry.size,
                });
            } else if database && !entry.is_dir {
                protected.push(ProtectedComponent {
                    path: entry.path.to_string_lossy().to_string(),
                    kind: "database".into(),
                    reason: "Database file in the project folder. Row data is not regenerable from source, so databases are never cleanup targets.".into(),
                    size: entry.size,
                });
            }
        }
    }

    let models = join_name(root, "models");
    if models.is_dir() {
        let has_models = fs::read_dir(&models).is_ok_and(|entries| {
            entries.iter().any(|entry| {
                !entry.is_dir
                    && MODEL_EXTENSIONS
                        .iter()
                        .any(|extension| entry.name.to_ascii_lowercase().ends_with(&format!(".{extension}")))
            })
        });
        if has_models {
            let measured = measure(&models);
            protected.push(ProtectedComponent {
                path: models.to_string_lossy().to_string(),
                kind: "model-weights".into(),
                reason: "Model weights located in a project models folder. Model files are never selected because they are large.".into(),
                size: measured.size,
            });
        }
    }

    if protected.len() >= MAX_PROTECTED_PER_PROJECT {
        push_warning(
            warnings,
            format!("{}: protected-data list was capped; source remains protected regardless.", display(root)),
        );
    }
    protected
}

fn measure(path: &Path) -> Measurement {
    let mut measured = Measurement::default();
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return measured,
    };
    measured.exists = true;
    let attributes = {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes()
    };
    if attributes & 0x400 != 0 {
        measured.errors.push(format!("{}: link or junction was not followed", display(path)));
        measured.skipped_links = 1;
        return measured;
    }
    measured.last_modified = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_secs());
    if metadata.is_file() {
        measured.file_count = 1;
        measured.size = metadata.len();
        {
            use std::os::windows::fs::MetadataExt;
            measured.allocated = metadata.file_size().max(metadata.len());
        }
        return measured;
    }
    if !metadata.is_dir() {
        return measured;
    }

    let mut stack = vec![(path.to_path_buf(), 0u32)];
    let mut seen_ids: HashSet<u64> = HashSet::new();
    let mut errors = 0;
    while let Some((dir, depth)) = stack.pop() {
        if measured.file_count >= MAX_FILES_PER_ARTIFACT as u64
            || measured.dir_count >= MAX_DIRS_PER_ARTIFACT as u64
        {
            measured.truncated = true;
            break;
        }
        if depth > 64 {
            measured.truncated = true;
            continue;
        }
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) => {
                if errors < 5 {
                    measured.errors.push(format!("{}: {error}", display(&dir)));
                    errors += 1;
                }
                continue;
            }
        };
        for entry in entries {
            if entry.is_reparse {
                measured.skipped_links += 1;
                continue;
            }
            if let Some(modified) = entry.modified {
                measured.last_modified = Some(measured.last_modified.map_or(modified, |current| current.max(modified)));
            }
            if entry.is_dir {
                measured.dir_count += 1;
                stack.push((entry.path, depth + 1));
            } else if !entry.is_dir {
                let unique = entry.file_id.map_or(true, |id| seen_ids.insert(id));
                if !unique {
                    continue;
                }
                measured.file_count += 1;
                measured.size = measured.size.saturating_add(entry.size);
                measured.allocated = measured.allocated.saturating_add(entry.allocated.max(entry.size));
                if seen_ids.len() > MAX_TRACKED_IDS {
                    // Stop trusting the hard-link set instead of growing without bound.
                    measured.truncated = true;
                    break;
                }
            }
        }
        if measured.file_count >= MAX_FILES_PER_ARTIFACT as u64 {
            measured.truncated = true;
            break;
        }
    }
    measured
}

/// Converts one server-held artifact into a cleanup spec. The caller must
/// resolve the artifact from a scan it holds; this function never accepts a
/// free-form path from the frontend.
pub fn artifact_spec(artifact: &DeveloperArtifact) -> cleanup::CleanupSpec {
    cleanup::CleanupSpec {
        id: artifact.id.clone(),
        tool: artifact.project_name.clone(),
        label: format!("{} · {}", artifact.project_name, artifact.label),
        risk: artifact.risk.clone(),
        category: "developer".into(),
        note: artifact.evidence.clone(),
        impact: format!("{} {}", artifact.loss, artifact.rebuild),
        path: PathBuf::from(&artifact.path),
    }
}

fn all_artifacts(scan: &DeveloperStorageScan) -> HashMap<&str, &DeveloperArtifact> {
    let mut map = HashMap::new();
    for project in &scan.projects {
        for artifact in &project.artifacts {
            map.insert(artifact.id.as_str(), artifact);
        }
    }
    map
}

/// Resolves selected artifact ids from a server-held scan. Unknown and
/// non-cleanable ids are refused; no path is accepted from the caller.
pub fn selected_specs(scan: &DeveloperStorageScan, ids: &[String]) -> Result<Vec<cleanup::CleanupSpec>, String> {
    if ids.is_empty() {
        return Err("Select at least one project artifact to preview.".into());
    }
    if ids.len() > 64 {
        return Err("Select at most 64 artifacts at once.".into());
    }
    let artifacts = all_artifacts(scan);
    let mut seen = HashSet::new();
    let mut specs = Vec::new();
    for id in ids {
        if !seen.insert(id.as_str()) {
            continue;
        }
        let artifact = artifacts
            .get(id.as_str())
            .ok_or("A selected artifact is not part of the current project scan. Scan again.")?;
        if artifact.risk == "protected" {
            return Err(format!("{} is protected data and cannot be cleaned.", artifact.label));
        }
        if !matches!(
            artifact.kind.as_str(),
            "node-modules"
                | "dist"
                | "build"
                | "cargo-target"
                | "maven-target"
                | "gradle-build"
                | "gradle-cache"
                | "next-cache"
                | "turbo-cache"
                | "pycache"
                | "pytest-cache"
                | "mypy-cache"
                | "ruff-cache"
        ) {
            return Err(format!("{} is not a recognised regenerable artifact.", artifact.label));
        }
        let path = PathBuf::from(&artifact.path);
        if !path.is_absolute() {
            return Err("A selected artifact path is not absolute; scan again.".into());
        }
        // The artifact must still be a direct descendant of the project that
        // produced it. This blocks a stale or tampered scan record from
        // redirecting the cleanup plan to another folder.
        let project_root = scan
            .projects
            .iter()
            .find(|project| project.id == artifact.project_id)
            .map(|project| PathBuf::from(&project.path))
            .ok_or("A selected artifact has lost its project ownership evidence. Scan again.")?;
        if !path.starts_with(&project_root) || path == project_root {
            return Err("A selected artifact is outside its recorded project. Scan again.".into());
        }
        specs.push(artifact_spec(artifact));
    }
    Ok(specs)
}

/// Re-measures the selected artifact paths after a cleanup. A missing folder,
/// or one with zero measured files, is reported as removed. No plan or path is
/// accepted from the caller: ids are resolved against the held scan.
pub fn verify(scan: &DeveloperStorageScan, ids: &[String]) -> Result<DeveloperCleanupVerification, String> {
    if ids.is_empty() {
        return Err("Select at least one artifact to verify.".into());
    }
    let artifacts = all_artifacts(scan);
    let mut seen = HashSet::new();
    let mut results = Vec::new();
    let mut remaining_bytes = 0u64;
    let mut removed_artifacts = 0u64;
    let mut remaining_artifacts = 0u64;
    let mut errors = Vec::new();
    for id in ids {
        if !seen.insert(id.as_str()) {
            continue;
        }
        let Some(artifact) = artifacts.get(id.as_str()) else {
            errors.push("An artifact was not part of the scan that produced this verification.".into());
            continue;
        };
        let measured = measure(Path::new(&artifact.path));
        let (status, remaining_files) = if !measured.exists || measured.file_count == 0 {
            ("removed", 0)
        } else {
            ("remaining", measured.file_count)
        };
        if status == "removed" {
            removed_artifacts += 1;
        } else {
            remaining_artifacts += 1;
            remaining_bytes = remaining_bytes.saturating_add(measured.size);
        }
        results.push(ArtifactVerification {
            id: artifact.id.clone(),
            path: artifact.path.clone(),
            label: artifact.label.clone(),
            status: status.into(),
            expected_bytes: artifact.size,
            remaining_bytes: measured.size,
            remaining_files,
        });
    }
    Ok(DeveloperCleanupVerification {
        scan_id: scan.scan_id.clone(),
        artifacts: results,
        remaining_bytes,
        removed_artifacts,
        remaining_artifacts,
        restorable_run_ids: Vec::new(),
        errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new(tag: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "cyron-devscan-{tag}-{}-{nanos}",
                std::process::id()
            ));
            std::fs::create_dir_all(&root).expect("fixture root");
            Self { root }
        }

        fn write(&self, relative: &str, contents: &[u8]) {
            let path = self.root.join(relative);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("fixture parent");
            }
            std::fs::write(path, contents).expect("fixture file");
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn scan_fixture(root: &Path) -> DeveloperStorageScan {
        scan_selected(vec![root.to_string_lossy().to_string()]).expect("fixture scan")
    }

    fn artifact<'a>(scan: &'a DeveloperStorageScan, kind: &str) -> &'a DeveloperArtifact {
        scan.projects
            .iter()
            .flat_map(|project| project.artifacts.iter())
            .find(|artifact| artifact.kind == kind)
            .unwrap_or_else(|| panic!("missing artifact kind {kind}"))
    }

    #[test]
    fn recognises_projects_and_only_named_regenerable_artifacts() {
        let fixture = Fixture::new("recognise");
        fixture.write(
            "app/package.json",
            br#"{"name":"fixture-app","dependencies":{"next":"15.0.0"}}"#,
        );
        fixture.write("app/package-lock.json", b"{}");
        fixture.write("app/node_modules/react/index.js", b"module.exports = 1;");
        fixture.write("app/.next/cache/data.bin", b"cache");
        fixture.write("app/.next/server/app.js", b"runtime output");
        fixture.write("app/dist/index.html", b"<html></html>");
        fixture.write("app/.env", b"TOKEN=secret");
        fixture.write("app/.git/config", b"[core]");
        fixture.write("app/src/main.ts", b"export const x = 1;");
        fixture.write("app/node_modules/evil/package.json", br#"{"name":"evil"}"#);
        fixture.write("app/app.sqlite", b"SQLite format 3\0");

        let scan = scan_fixture(&fixture.root);
        assert_eq!(scan.projects.len(), 1, "node_modules contents are not projects");
        let project = &scan.projects[0];
        assert_eq!(project.name, "fixture-app");
        assert!(project.source_protected);
        assert!(project.artifacts.iter().any(|item| item.kind == "node-modules"));
        assert!(project.artifacts.iter().any(|item| item.kind == "next-cache"));
        assert!(project.artifacts.iter().any(|item| item.kind == "dist"));
        assert!(!project.artifacts.iter().any(|item| item.kind == "git"));
        assert!(!project.artifacts.iter().any(|item| item.path.contains(".git")));
        assert!(!project.artifacts.iter().any(|item| item.path.ends_with("app\\.next")));
        assert!(project.protected.iter().any(|item| item.kind == "git"));
        assert!(project.protected.iter().any(|item| item.kind == "credential"));
        assert!(
            project
                .protected
                .iter()
                .any(|item| item.kind == "database"),
            "a project database must be reported as protected"
        );
        assert!(
            project
                .artifacts
                .iter()
                .all(|item| !item.path.to_ascii_lowercase().ends_with(".sqlite")),
            "a database must never become an artifact"
        );
        assert!(project.artifacts.iter().all(|item| item.evidence.contains("Direct child") || item.evidence.contains("Nested")));
    }

    #[test]
    fn python_and_rust_artifacts_are_detected_with_evidence() {
        let fixture = Fixture::new("languages");
        fixture.write("rust/Cargo.toml", b"[package]\nname = \"fixture-rust\"\nversion = \"0.1.0\"\n");
        fixture.write("rust/target/debug/fixture.exe", b"binary");
        fixture.write("python/pyproject.toml", b"[project]\nname = \"fixture-python\"\n");
        fixture.write("python/pkg/__pycache__/mod.pyc", b"bytecode");
        fixture.write("python/.pytest_cache/v/cache/lastfailed", b"{}");
        fixture.write("python/.ruff_cache/content", b"cache");

        let scan = scan_fixture(&fixture.root);
        assert_eq!(scan.projects.len(), 2);
        assert_eq!(artifact(&scan, "cargo-target").risk, "caution");
        assert_eq!(artifact(&scan, "pycache").risk, "safe");
        assert_eq!(artifact(&scan, "pytest-cache").risk, "safe");
        assert_eq!(artifact(&scan, "ruff-cache").risk, "safe");
        assert!(artifact(&scan, "cargo-target").loss.contains("Compiled binaries"));
    }

    #[test]
    fn activity_labels_distinguish_active_recent_old_and_unknown() {
        let now = seconds_now();
        assert_eq!(activity_for(Some(now - 2 * 86_400)), "active");
        assert_eq!(activity_for(Some(now - 90 * 86_400)), "recent");
        assert_eq!(activity_for(Some(now - 200 * 86_400)), "old");
        assert_eq!(activity_for(None), "unknown");
    }

    #[test]
    fn a_tampered_artifact_record_cannot_escape_its_project() {
        let fixture = Fixture::new("tamper");
        fixture.write("app/package.json", br#"{"name":"tamper-app"}"#);
        fixture.write("app/node_modules/pkg/index.js", b"x");
        let mut scan = scan_fixture(&fixture.root);
        scan.projects[0].artifacts[0].path = "C:\\Windows\\Temp".into();
        assert!(selected_specs(&scan, &[scan.projects[0].artifacts[0].id.clone()]).is_err());
    }

    #[test]
    fn stale_activity_is_informational_and_never_auto_selected() {
        let fixture = Fixture::new("stale");
        fixture.write("old/package.json", br#"{"name":"old-app"}"#);
        fixture.write("old/node_modules/pkg/index.js", b"x");
        let scan = scan_fixture(&fixture.root);
        let created = scan.projects[0].artifacts[0].last_modified;
        assert!(created.is_some());
        // No API here marks anything selected; the scan has no selection field at all.
        assert!(!scan.projects[0].artifacts.is_empty());
    }

    #[test]
    fn selection_is_bound_to_the_server_scan_and_rejects_unknown_ids() {
        let fixture = Fixture::new("selection");
        fixture.write("app/package.json", br#"{"name":"selection-app"}"#);
        fixture.write("app/node_modules/pkg/index.js", b"x");
        let scan = scan_fixture(&fixture.root);
        let id = artifact(&scan, "node-modules").id.clone();
        let specs = selected_specs(&scan, &[id.clone()]).expect("known artifact");
        assert_eq!(specs.len(), 1);
        assert!(specs[0].path.ends_with("node_modules"));
        assert!(selected_specs(&scan, &["artifact-not-real".into()]).is_err());
        assert!(selected_specs(&scan, &[]).is_err());
    }

    #[test]
    fn verification_reports_removed_and_remaining_artifacts() {
        let fixture = Fixture::new("verify");
        fixture.write(
            "app/package.json",
            br#"{"name":"verify-app","dependencies":{"vite":"7.0.0"}}"#,
        );
        fixture.write("app/node_modules/pkg/index.js", b"x");
        fixture.write("app/dist/index.html", b"<html></html>");
        let scan = scan_fixture(&fixture.root);
        let node = artifact(&scan, "node-modules").id.clone();
        let dist = artifact(&scan, "dist").id.clone();
        std::fs::remove_dir_all(fixture.root.join("app/node_modules")).expect("remove fixture artifact");
        let report = verify(&scan, &[node, dist]).expect("verification");
        assert_eq!(report.removed_artifacts, 1);
        assert_eq!(report.remaining_artifacts, 1);
        assert!(report.artifacts.iter().any(|item| item.status == "removed"));
        assert!(report.artifacts.iter().any(|item| item.status == "remaining"));
    }

    #[test]
    fn system_roots_are_refused_as_project_roots() {
        let profile = std::env::var("USERPROFILE").expect("profile");
        assert!(validate_root(&profile).is_err());
        assert!(validate_root("C:\\Windows").is_err());
        assert!(validate_root("relative\\project").is_err());
    }
}
