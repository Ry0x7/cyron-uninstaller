use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Cache folders can hold hundreds of thousands of files (npm, %TEMP%).
/// Measuring a handful of them in parallel shortens the first inventory without
/// changing what is reported.
const MAX_MEASURE_THREADS: usize = 4;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheTarget {
    pub id: String,
    pub tool: String,
    pub label: String,
    /// Resolved absolute path, empty when the environment variable was missing.
    pub path: String,
    pub exists: bool,
    pub size: u64,
    pub allocated: u64,
    pub file_count: u64,
    pub dir_count: u64,
    /// safe, caution or protected.
    pub risk: String,
    /// ai, ide, packages, containers or windows.
    pub category: String,
    pub note: String,
    pub last_modified: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheInventory {
    pub targets: Vec<CacheTarget>,
    pub total_size: u64,
    pub total_safe: u64,
    pub total_caution: u64,
    pub total_protected: u64,
    pub scanned_at: u64,
    pub warnings: Vec<String>,
}

pub struct TargetDef {
    pub id: &'static str,
    pub tool: &'static str,
    pub label: &'static str,
    /// Supports %VARIABLE% segments. Every entry must be a real Windows location.
    pub path: &'static str,
    pub risk: &'static str,
    pub category: &'static str,
    pub note: &'static str,
}

const SAFE: &str = "safe";
const CAUTION: &str = "caution";
const PROTECTED: &str = "protected";

const AI: &str = "ai";
const IDE: &str = "ide";
const PACKAGES: &str = "packages";
const CONTAINERS: &str = "containers";
const WINDOWS: &str = "windows";

const MAX_WARNINGS: usize = 12;

/// Locations are intentionally non-overlapping so that summing every target
/// does not double count bytes.
pub const CATALOG: &[TargetDef] = &[
    // ---------------------------------------------------------------- AI tools
    TargetDef {
        id: "claude-code-projects",
        tool: "Claude Code",
        label: "Conversation history",
        path: "%USERPROFILE%\\.claude\\projects",
        risk: PROTECTED,
        category: AI,
        note: "Claude Code conversation history. Cyron never offers it for removal.",
    },
    TargetDef {
        id: "claude-code-shell-snapshots",
        tool: "Claude Code",
        label: "Shell snapshots",
        path: "%USERPROFILE%\\.claude\\shell-snapshots",
        risk: SAFE,
        category: AI,
        note: "Shell environment snapshots that Claude Code regenerates each session.",
    },
    TargetDef {
        id: "claude-code-debug",
        tool: "Claude Code",
        label: "Debug logs",
        path: "%USERPROFILE%\\.claude\\debug",
        risk: SAFE,
        category: AI,
        note: "Debug logs written only while logging is enabled.",
    },
    TargetDef {
        id: "claude-code-todos",
        tool: "Claude Code",
        label: "Session todo scratch files",
        path: "%USERPROFILE%\\.claude\\todos",
        risk: CAUTION,
        category: AI,
        note: "Leaves of unfinished session checklists; conversation history stays untouched.",
    },
    TargetDef {
        id: "claude-code-cache",
        tool: "Claude Code",
        label: "CLI cache",
        path: "%LOCALAPPDATA%\\claude-cli-nodejs\\Cache",
        risk: SAFE,
        category: AI,
        note: "Local CLI cache, rebuilt on demand by Claude Code.",
    },
    TargetDef {
        id: "codex-sessions",
        tool: "Codex CLI",
        label: "Session rollouts",
        path: "%USERPROFILE%\\.codex\\sessions",
        risk: PROTECTED,
        category: AI,
        note: "Codex CLI conversation rollouts used for history and resuming work.",
    },
    TargetDef {
        id: "codex-log",
        tool: "Codex CLI",
        label: "CLI logs",
        path: "%USERPROFILE%\\.codex\\log",
        risk: SAFE,
        category: AI,
        note: "Codex CLI logs, recreated on the next run.",
    },
    TargetDef {
        id: "gemini-tmp",
        tool: "Gemini CLI",
        label: "Temporary working files",
        path: "%USERPROFILE%\\.gemini\\tmp",
        risk: SAFE,
        category: AI,
        note: "Temporary files Gemini CLI uses while running.",
    },
    TargetDef {
        id: "gemini-history",
        tool: "Gemini CLI",
        label: "Conversation history",
        path: "%USERPROFILE%\\.gemini\\history",
        risk: PROTECTED,
        category: AI,
        note: "Gemini CLI conversation history.",
    },
    TargetDef {
        id: "ollama-models",
        tool: "Ollama",
        label: "Model weights",
        path: "%USERPROFILE%\\.ollama\\models",
        risk: PROTECTED,
        category: AI,
        note: "Downloaded model weights; removing them forces a multi-gigabyte re-download.",
    },
    TargetDef {
        id: "ollama-logs",
        tool: "Ollama",
        label: "Server logs",
        path: "%LOCALAPPDATA%\\Ollama\\logs",
        risk: SAFE,
        category: AI,
        note: "Ollama server log files, recreated the next time the server runs.",
    },
    TargetDef {
        id: "lmstudio-models",
        tool: "LM Studio",
        label: "Model weights",
        path: "%USERPROFILE%\\.cache\\lm-studio\\models",
        risk: PROTECTED,
        category: AI,
        note: "Downloaded LM Studio model weights.",
    },
    TargetDef {
        id: "lmstudio-logs",
        tool: "LM Studio",
        label: "Application logs",
        path: "%APPDATA%\\LM Studio\\logs",
        risk: SAFE,
        category: AI,
        note: "LM Studio application logs, recreated on the next launch.",
    },
    // ----------------------------------------------------------- Editors and IDEs
    TargetDef {
        id: "cursor-cache",
        tool: "Cursor",
        label: "Renderer cache",
        path: "%APPDATA%\\Cursor\\Cache",
        risk: SAFE,
        category: IDE,
        note: "Electron renderer cache, rebuilt automatically on launch.",
    },
    TargetDef {
        id: "cursor-code-cache",
        tool: "Cursor",
        label: "Code cache",
        path: "%APPDATA%\\Cursor\\Code Cache",
        risk: SAFE,
        category: IDE,
        note: "Compiled JavaScript cache for the editor interface.",
    },
    TargetDef {
        id: "cursor-gpu",
        tool: "Cursor",
        label: "GPU cache",
        path: "%APPDATA%\\Cursor\\GPUCache",
        risk: SAFE,
        category: IDE,
        note: "GPU shader cache, rebuilt on launch.",
    },
    TargetDef {
        id: "cursor-logs",
        tool: "Cursor",
        label: "Logs",
        path: "%APPDATA%\\Cursor\\logs",
        risk: SAFE,
        category: IDE,
        note: "Cursor diagnostic logs; no editor settings or history.",
    },
    TargetDef {
        id: "cursor-workspace-storage",
        tool: "Cursor",
        label: "Workspace window state",
        path: "%APPDATA%\\Cursor\\User\\workspaceStorage",
        risk: CAUTION,
        category: IDE,
        note: "Window layout and local workspace state; clearing it resets recent editor state.",
    },
    TargetDef {
        id: "cursor-history",
        tool: "Cursor",
        label: "Local file history",
        path: "%APPDATA%\\Cursor\\User\\History",
        risk: PROTECTED,
        category: IDE,
        note: "Local edit history that may hold the only copy of an unsaved change.",
    },
    TargetDef {
        id: "antigravity-cache",
        tool: "Antigravity",
        label: "Renderer cache",
        path: "%APPDATA%\\Antigravity\\Cache",
        risk: SAFE,
        category: IDE,
        note: "Electron renderer cache, rebuilt automatically.",
    },
    TargetDef {
        id: "antigravity-code-cache",
        tool: "Antigravity",
        label: "Code cache",
        path: "%APPDATA%\\Antigravity\\Code Cache",
        risk: SAFE,
        category: IDE,
        note: "Compiled interface cache, rebuilt automatically on launch.",
    },
    TargetDef {
        id: "antigravity-gpu",
        tool: "Antigravity",
        label: "GPU cache",
        path: "%APPDATA%\\Antigravity\\GPUCache",
        risk: SAFE,
        category: IDE,
        note: "GPU shader cache, rebuilt on launch.",
    },
    TargetDef {
        id: "antigravity-logs",
        tool: "Antigravity",
        label: "Logs",
        path: "%APPDATA%\\Antigravity\\logs",
        risk: SAFE,
        category: IDE,
        note: "Editor diagnostic logs, safe to discard at any time.",
    },
    TargetDef {
        id: "antigravity-user",
        tool: "Antigravity",
        label: "Settings and agent history",
        path: "%APPDATA%\\Antigravity\\User",
        risk: PROTECTED,
        category: IDE,
        note: "Your settings, prompt history and agent state. Never a cleanup target.",
    },
    TargetDef {
        id: "vscode-cache",
        tool: "VS Code",
        label: "Renderer cache",
        path: "%APPDATA%\\Code\\Cache",
        risk: SAFE,
        category: IDE,
        note: "Electron renderer cache, rebuilt automatically.",
    },
    TargetDef {
        id: "vscode-code-cache",
        tool: "VS Code",
        label: "Code cache",
        path: "%APPDATA%\\Code\\Code Cache",
        risk: SAFE,
        category: IDE,
        note: "Compiled JavaScript cache for the editor interface.",
    },
    TargetDef {
        id: "vscode-gpu",
        tool: "VS Code",
        label: "GPU cache",
        path: "%APPDATA%\\Code\\GPUCache",
        risk: SAFE,
        category: IDE,
        note: "GPU shader cache, rebuilt on launch.",
    },
    TargetDef {
        id: "vscode-logs",
        tool: "VS Code",
        label: "Logs",
        path: "%APPDATA%\\Code\\logs",
        risk: SAFE,
        category: IDE,
        note: "VS Code diagnostic logs; no settings or extensions are touched.",
    },
    TargetDef {
        id: "vscode-workspace-storage",
        tool: "VS Code",
        label: "Workspace window state",
        path: "%APPDATA%\\Code\\User\\workspaceStorage",
        risk: CAUTION,
        category: IDE,
        note: "Window layout and local workspace state; clearing it resets recent editor state.",
    },
    TargetDef {
        id: "vscode-history",
        tool: "VS Code",
        label: "Local file history",
        path: "%APPDATA%\\Code\\User\\History",
        risk: PROTECTED,
        category: IDE,
        note: "Local edit history that may hold the only copy of an unsaved change.",
    },
    // -------------------------------------------------------- Package managers
    TargetDef {
        id: "npm-cache",
        tool: "npm",
        label: "Package cache",
        path: "%LOCALAPPDATA%\\npm-cache",
        risk: SAFE,
        category: PACKAGES,
        note: "npm's content-addressable package cache; rebuilt from the registry.",
    },
    TargetDef {
        id: "npm-cache-roaming",
        tool: "npm",
        label: "Package cache (Roaming)",
        path: "%APPDATA%\\npm-cache",
        risk: SAFE,
        category: PACKAGES,
        note: "npm's content-addressable package cache in Roaming AppData; rebuilt from the registry.",
    },
    TargetDef {
        id: "pnpm-store",
        tool: "pnpm",
        label: "Global content store",
        path: "%LOCALAPPDATA%\\pnpm\\store",
        risk: CAUTION,
        category: PACKAGES,
        note: "Packages re-link from this store. Removing it forces a fresh download.",
    },
    TargetDef {
        id: "yarn-cache",
        tool: "Yarn",
        label: "Package cache",
        path: "%LOCALAPPDATA%\\Yarn\\Cache",
        risk: CAUTION,
        category: PACKAGES,
        note: "Yarn archives that would be downloaded again on the next install.",
    },
    TargetDef {
        id: "pip-cache",
        tool: "pip",
        label: "Wheel cache",
        path: "%LOCALAPPDATA%\\pip\\Cache",
        risk: SAFE,
        category: PACKAGES,
        note: "Downloaded Python wheels and HTTP cache.",
    },
    TargetDef {
        id: "cargo-registry-cache",
        tool: "Cargo",
        label: "Crate archives",
        path: "%USERPROFILE%\\.cargo\\registry\\cache",
        risk: CAUTION,
        category: PACKAGES,
        note: "Downloaded crate archives. Removing them breaks offline builds until refreshed.",
    },
    TargetDef {
        id: "cargo-git-db",
        tool: "Cargo",
        label: "Git dependencies",
        path: "%USERPROFILE%\\.cargo\\git\\db",
        risk: CAUTION,
        category: PACKAGES,
        note: "Cloned git dependencies; Cargo re-fetches them when needed.",
    },
    TargetDef {
        id: "nuget-packages",
        tool: "NuGet",
        label: "Global package folder",
        path: "%USERPROFILE%\\.nuget\\packages",
        risk: CAUTION,
        category: PACKAGES,
        note: "Restored .NET packages. Removing them forces every project to restore again.",
    },
    // ------------------------------------------------------ Containers and WSL
    TargetDef {
        id: "docker-wsl",
        tool: "Docker Desktop",
        label: "Images, containers and volumes",
        path: "%LOCALAPPDATA%\\Docker\\wsl",
        risk: PROTECTED,
        category: CONTAINERS,
        note: "Docker images, containers and named volumes. Deleting this destroys container data.",
    },
    TargetDef {
        id: "docker-logs",
        tool: "Docker Desktop",
        label: "Logs",
        path: "%APPDATA%\\Docker\\logs",
        risk: SAFE,
        category: CONTAINERS,
        note: "Docker Desktop diagnostic logs.",
    },
    TargetDef {
        id: "wsl-distros",
        tool: "WSL",
        label: "Distribution disks",
        path: "%LOCALAPPDATA%\\wsl",
        risk: PROTECTED,
        category: CONTAINERS,
        note: "Linux distribution disks and their files. Removing them deletes the environments.",
    },
    // ------------------------------------------------------ Windows and browsers
    TargetDef {
        id: "user-temp",
        tool: "Windows",
        label: "User temporary files",
        path: "%TEMP%",
        risk: SAFE,
        category: WINDOWS,
        note: "Per-user temporary files. Open files stay locked and are skipped.",
    },
    TargetDef {
        id: "windows-temp",
        tool: "Windows",
        label: "System temporary files",
        path: "%WINDIR%\\Temp",
        risk: CAUTION,
        category: WINDOWS,
        note: "System-wide temporary files; services may hold some of them open.",
    },
    TargetDef {
        id: "crash-dumps",
        tool: "Windows",
        label: "Crash dumps",
        path: "%LOCALAPPDATA%\\CrashDumps",
        risk: SAFE,
        category: WINDOWS,
        note: "Dumps written when applications crash.",
    },
    TargetDef {
        id: "inet-cache",
        tool: "Windows",
        label: "Internet cache",
        path: "%LOCALAPPDATA%\\Microsoft\\Windows\\INetCache",
        risk: SAFE,
        category: WINDOWS,
        note: "Internet cache used by Windows apps and the system browser.",
    },
    TargetDef {
        id: "chrome-cache",
        tool: "Google Chrome",
        label: "Browser cache",
        path: "%LOCALAPPDATA%\\Google\\Chrome\\User Data\\Default\\Cache",
        risk: SAFE,
        category: WINDOWS,
        note: "Chrome's page cache; sites load fresh after it is cleared.",
    },
    TargetDef {
        id: "edge-cache",
        tool: "Microsoft Edge",
        label: "Browser cache",
        path: "%LOCALAPPDATA%\\Microsoft\\Edge\\User Data\\Default\\Cache",
        risk: SAFE,
        category: WINDOWS,
        note: "Edge's page cache; sites load fresh after it is cleared.",
    },
    TargetDef {
        id: "firefox-profiles",
        tool: "Mozilla Firefox",
        label: "Browser profiles",
        path: "%LOCALAPPDATA%\\Mozilla\\Firefox\\Profiles",
        risk: PROTECTED,
        category: WINDOWS,
        note: "Profiles contain history, cookies, saved passwords and caches; size is reported only.",
    },
    TargetDef {
        id: "windows-update-downloads",
        tool: "Windows Update",
        label: "Update download cache",
        path: "%WINDIR%\\SoftwareDistribution\\Download",
        risk: PROTECTED,
        category: WINDOWS,
        note: "Downloaded update payloads owned by Windows Update. Cyron measures this folder but does not move or delete it; use Windows Storage Sense or Disk Cleanup, which coordinate with the update service.",
    },
];

/// Looks up a catalog entry by its stable id.
pub fn find(id: &str) -> Option<&'static TargetDef> {
    CATALOG.iter().find(|entry| entry.id == id)
}

/// Resolves an environment-templated catalog path to an absolute path.
pub fn resolve_template(template: &str) -> Result<String, String> {
    expand_env(template)
}

/// Plain-language consequence of removing this location, used by the cleanup
/// planner and preview so the UI never invents its own wording.
pub fn impact_for(definition: &TargetDef) -> String {
    match definition.risk {
        PROTECTED => format!(
            "{} is protected data. Cyron measures it but never cleans it.",
            definition.label
        ),
        CAUTION => format!(
            "{} may re-download or rebuild these files, and recently cached state can be affected until it does.",
            definition.tool
        ),
        _ => format!(
            "{} recreates these files the next time it runs. Projects, settings and conversation history are not touched.",
            definition.tool
        ),
    }
}

/// Measures every known developer, AI and Windows cache location.
pub fn discover_caches() -> CacheInventory {
    let mut warnings = Vec::new();
    let mut seen = HashSet::new();
    let mut jobs = Vec::with_capacity(CATALOG.len());

    for def in CATALOG {
        let path = match expand_env(def.path) {
            Ok(path) if !path.is_empty() => path,
            Ok(_) => {
                push_warning(
                    &mut warnings,
                    format!("{}: resolved path is empty", def.label),
                );
                continue;
            }
            Err(error) => {
                push_warning(&mut warnings, format!("{}: {error}", def.label));
                continue;
            }
        };

        if !seen.insert(path.to_lowercase()) {
            continue;
        }
        jobs.push(Job { def, path });
    }

    let measurements = measure_jobs(&jobs);
    let mut targets = Vec::with_capacity(jobs.len());
    for (job, measurement) in jobs.iter().zip(measurements) {
        for error in &measurement.errors {
            // Skipping reparse points is the intended safe behaviour, not a
            // problem the user can act on. Permission failures still surface.
            if error.contains("Skipped link or junction") {
                continue;
            }
            push_warning(&mut warnings, format!("{}: {error}", job.def.label));
        }
        targets.push(CacheTarget {
            id: job.def.id.to_string(),
            tool: job.def.tool.to_string(),
            label: job.def.label.to_string(),
            path: job.path.clone(),
            exists: measurement.exists,
            size: measurement.size,
            allocated: measurement.allocated,
            file_count: measurement.file_count,
            dir_count: measurement.dir_count,
            risk: job.def.risk.to_string(),
            category: job.def.category.to_string(),
            note: job.def.note.to_string(),
            last_modified: measurement.last_modified,
        });
    }

    discover_secondary_drive_caches(&mut seen, &mut targets);

    targets.sort_by(|a, b| {
        category_rank(&a.category)
            .cmp(&category_rank(&b.category))
            .then_with(|| a.tool.to_lowercase().cmp(&b.tool.to_lowercase()))
            .then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
    });

    let total_size = targeted_sum(&targets, |_| true);
    CacheInventory {
        total_size,
        total_safe: targeted_sum(&targets, |target| target.risk == SAFE),
        total_caution: targeted_sum(&targets, |target| target.risk == CAUTION),
        total_protected: targeted_sum(&targets, |target| target.risk == PROTECTED),
        targets,
        scanned_at: seconds_now(),
        warnings,
    }
}

fn discover_secondary_drive_caches(seen: &mut HashSet<String>, targets: &mut Vec<CacheTarget>) {
    let volumes = crate::volumes::list_volumes();
    for vol in volumes {
        let mount = vol.mount.trim_end_matches('\\');
        let letter = mount.chars().next().unwrap_or('C').to_ascii_uppercase();
        let letter_lower = letter.to_ascii_lowercase();

        let candidates = [
            (
                format!("drive-{letter_lower}-recycle-bin"),
                "Windows",
                format!("Recycle Bin ({letter}:)"),
                format!("{mount}\\$Recycle.Bin"),
                SAFE,
                WINDOWS,
                format!("Deleted files held in the {letter}: volume Recycle Bin."),
            ),
            (
                format!("drive-{letter_lower}-temp"),
                "Temporary Files",
                format!("Volume Temp ({letter}:)"),
                format!("{mount}\\Temp"),
                SAFE,
                WINDOWS,
                format!("Temporary files and logs stored under {letter}:\\Temp."),
            ),
            (
                format!("drive-{letter_lower}-steam-downloading"),
                "Steam",
                format!("Steam Downloads ({letter}:)"),
                format!("{mount}\\SteamLibrary\\steamapps\\downloading"),
                SAFE,
                PACKAGES,
                format!("Staging and incomplete game downloads in {letter}: Steam Library."),
            ),
            (
                format!("drive-{letter_lower}-steam-shadercache"),
                "Steam",
                format!("Steam Shader Cache ({letter}:)"),
                format!("{mount}\\SteamLibrary\\steamapps\\shadercache"),
                SAFE,
                PACKAGES,
                format!("Pre-compiled graphics shaders for games on drive {letter}:."),
            ),
            (
                format!("drive-{letter_lower}-npm-cache"),
                "npm",
                format!("npm Cache ({letter}:)"),
                format!("{mount}\\npm-cache"),
                SAFE,
                PACKAGES,
                format!("npm package cache stored on drive {letter}:."),
            ),
        ];

        for (id, tool, label, path, risk, category, note) in candidates {
            if !seen.insert(path.to_lowercase()) {
                continue;
            }
            let p = Path::new(&path);
            if p.exists() {
                let m = measure_target(&path);
                if m.exists && m.file_count > 0 {
                    targets.push(CacheTarget {
                        id,
                        tool: tool.to_string(),
                        label,
                        path,
                        exists: true,
                        size: m.size,
                        allocated: m.allocated,
                        file_count: m.file_count,
                        dir_count: m.dir_count,
                        risk: risk.to_string(),
                        category: category.to_string(),
                        note,
                        last_modified: m.last_modified,
                    });
                }
            }
        }
    }
}

struct Job {
    def: &'static TargetDef,
    path: String,
}

struct Measurement {
    exists: bool,
    size: u64,
    allocated: u64,
    file_count: u64,
    dir_count: u64,
    last_modified: Option<u64>,
    errors: Vec<String>,
}

fn measure_jobs(jobs: &[Job]) -> Vec<Measurement> {
    if jobs.is_empty() {
        return Vec::new();
    }
    let workers = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(2)
        .clamp(1, MAX_MEASURE_THREADS)
        .min(jobs.len());
    let next = AtomicUsize::new(0);
    let collected = Mutex::new(Vec::with_capacity(jobs.len()));

    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(job) = jobs.get(index) else { break };
                let measurement = measure_target(&job.path);
                if let Ok(mut bucket) = collected.lock() {
                    bucket.push((index, measurement));
                }
            });
        }
    });

    let mut collected = collected.into_inner().unwrap_or_default();
    collected.sort_by_key(|(index, _)| *index);
    collected
        .into_iter()
        .map(|(_, measurement)| measurement)
        .collect()
}

fn measure_target(path: &str) -> Measurement {
    let path_ref = Path::new(path);
    if !path_ref.exists() {
        return Measurement {
            exists: false,
            size: 0,
            allocated: 0,
            file_count: 0,
            dir_count: 0,
            last_modified: None,
            errors: Vec::new(),
        };
    }
    let measured = crate::scan::measure_path(path_ref);
    Measurement {
        exists: true,
        size: measured.size,
        allocated: measured.allocated,
        file_count: measured.file_count,
        dir_count: measured.dir_count,
        last_modified: modified_at(path_ref),
        errors: measured.errors,
    }
}

fn targeted_sum(targets: &[CacheTarget], include: impl Fn(&CacheTarget) -> bool) -> u64 {
    targets
        .iter()
        .filter(|target| target.exists && include(target))
        .map(|target| target.size)
        .sum()
}

fn category_rank(category: &str) -> usize {
    match category {
        AI => 0,
        IDE => 1,
        PACKAGES => 2,
        CONTAINERS => 3,
        WINDOWS => 4,
        _ => 5,
    }
}

fn push_warning(warnings: &mut Vec<String>, message: String) {
    if warnings.len() < MAX_WARNINGS && !warnings.iter().any(|existing| existing == &message) {
        warnings.push(message);
    }
}

fn seconds_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn modified_at(path: &Path) -> Option<u64> {
    path.metadata()
        .ok()?
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

/// Expands `%VARIABLE%` segments and normalizes separators.
fn expand_env(input: &str) -> Result<String, String> {
    let mut output = String::with_capacity(input.len());
    let mut rest = input;

    while let Some(start) = rest.find('%') {
        output.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let end = after
            .find('%')
            .ok_or_else(|| format!("unterminated environment variable in {input}"))?;
        let name = &after[..end];
        if name.is_empty() {
            output.push_str("%%");
            rest = &after[end + 1..];
            continue;
        }
        let value =
            std::env::var(name).map_err(|_| format!("environment variable {name} is not set"))?;
        output.push_str(value.trim_end_matches(['\\', '/']));
        rest = &after[end + 1..];
    }
    output.push_str(rest);

    Ok(normalize_path(&output))
}

fn normalize_path(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches(['\\', '/']);
    if trimmed.is_empty() {
        return String::new();
    }
    let unc = trimmed.starts_with("\\\\") || trimmed.starts_with("//");
    let mut normalized = trimmed
        .split(['\\', '/'])
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\\");
    if unc && !normalized.starts_with("\\\\") {
        normalized.insert_str(0, "\\\\");
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog_entry(id: &str) -> &'static TargetDef {
        CATALOG
            .iter()
            .find(|entry| entry.id == id)
            .unwrap_or_else(|| panic!("missing catalog entry {id}"))
    }

    #[test]
    fn expands_known_environment_variables() {
        let expanded = expand_env("%USERPROFILE%\\cyron-expand-test").expect("USERPROFILE is set");
        assert!(expanded.to_lowercase().ends_with("cyron-expand-test"));
        assert!(!expanded.contains('%'));
        assert!(
            Path::new(&expanded).is_absolute(),
            "{expanded} is not absolute"
        );
    }

    #[test]
    fn reports_missing_environment_variables() {
        let error = expand_env("%CYRON_SURELY_MISSING_VAR%\\x").expect_err("must fail");
        assert!(error.contains("CYRON_SURELY_MISSING_VAR"), "{error}");
    }

    #[test]
    fn normalizes_separators_and_trailing_slashes() {
        assert_eq!(normalize_path("C:\\Users\\\\name\\"), "C:\\Users\\name");
        assert_eq!(normalize_path("C:/Users//name/"), "C:\\Users\\name");
        assert_eq!(normalize_path("  "), "");
    }

    #[test]
    fn catalog_is_internally_valid() {
        let mut ids = HashSet::new();
        let mut templates = HashSet::new();
        for entry in CATALOG {
            assert!(ids.insert(entry.id), "duplicate id {}", entry.id);
            assert!(
                templates.insert(entry.path),
                "duplicate path {}",
                entry.path
            );
            assert!(!entry.tool.is_empty());
            assert!(!entry.label.is_empty());
            assert!(
                entry.note.len() > 24,
                "note for {} is not informative",
                entry.id
            );
            assert!(entry.risk == SAFE || entry.risk == CAUTION || entry.risk == PROTECTED);
            assert!(
                entry.category == AI
                    || entry.category == IDE
                    || entry.category == PACKAGES
                    || entry.category == CONTAINERS
                    || entry.category == WINDOWS
            );
            assert!(
                entry.path.contains('%'),
                "{} must be environment relative",
                entry.id
            );
            assert!(
                !entry.path.ends_with('\\'),
                "{} has a trailing slash",
                entry.id
            );
        }
    }

    #[test]
    fn sensitive_locations_are_never_marked_safe() {
        for id in [
            "claude-code-projects",
            "codex-sessions",
            "gemini-history",
            "cursor-history",
            "vscode-history",
            "antigravity-user",
            "ollama-models",
            "lmstudio-models",
            "docker-wsl",
            "wsl-distros",
            "firefox-profiles",
        ] {
            assert_eq!(catalog_entry(id).risk, PROTECTED, "{id} must be protected");
        }
    }

    #[test]
    #[ignore = "diagnostic only; reports per-target measurement time"]
    fn profile_targets() {
        use std::time::Instant;
        for def in CATALOG {
            let path = match expand_env(def.path) {
                Ok(path) => path,
                Err(_) => continue,
            };
            if !Path::new(&path).exists() {
                continue;
            }
            let started = Instant::now();
            let measured = crate::scan::measure_path(Path::new(&path));
            eprintln!(
                "{:>8.2}s {:>10} {:>9} {}",
                started.elapsed().as_secs_f64(),
                measured.size,
                measured.file_count,
                def.id
            );
        }
    }

    #[test]
    fn discovery_reports_catalog_and_consistent_totals() {
        let inventory = discover_caches();
        assert!(
            inventory.targets.len() >= 40 && inventory.targets.len() <= CATALOG.len() + 32,
            "expected most of the catalog to resolve, got {}",
            inventory.targets.len()
        );
        assert!(inventory.scanned_at > 0);

        let mut ids = HashSet::new();
        for target in &inventory.targets {
            assert!(ids.insert(target.id.clone()), "duplicate discovered id");
            assert!(!target.path.is_empty());
            assert!(Path::new(&target.path).is_absolute());
            if !target.exists {
                assert_eq!(target.size, 0);
                assert!(target.last_modified.is_none());
            }
        }

        let existing_total: u64 = inventory
            .targets
            .iter()
            .filter(|target| target.exists)
            .map(|target| target.size)
            .sum();
        assert_eq!(inventory.total_size, existing_total);
        assert_eq!(
            inventory.total_size,
            inventory.total_safe + inventory.total_caution + inventory.total_protected
        );
        assert!(
            inventory.targets.iter().any(|target| target.exists),
            "at least the user temp folder should exist"
        );
        eprintln!(
            "targets={} existing={} total={} safe={} caution={} protected={} warnings={}",
            inventory.targets.len(),
            inventory
                .targets
                .iter()
                .filter(|target| target.exists)
                .count(),
            inventory.total_size,
            inventory.total_safe,
            inventory.total_caution,
            inventory.total_protected,
            inventory.warnings.len()
        );
        for warning in &inventory.warnings {
            eprintln!("warning: {warning}");
        }
    }
}
