//! Storage intelligence: classifies scanned files into human categories, age
//! buckets and a small set of size insights.
//!
//! The classification is deliberately path- and extension-based only. A `known`
//! category matched a documented path rule (Program Files, Downloads, a model
//! directory); an inferred category (media by extension, generic build folders)
//! is reported as `known: false` so the UI never presents a guess as a fact.

use serde::Serialize;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageCategory {
    pub key: String,
    pub label: String,
    pub bytes: u64,
    pub allocated: u64,
    pub file_count: u64,
    pub known: bool,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgeBucket {
    pub key: String,
    pub label: String,
    pub bytes: u64,
    pub file_count: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageInsight {
    pub key: String,
    pub title: String,
    pub detail: String,
    pub bytes: u64,
    pub file_count: u64,
    pub sample_path: Option<String>,
}

/// Fixed display order. Every report contains all ten keys, even when zero.
pub const CATEGORY_ORDER: [&str; 10] = [
    "applications",
    "games",
    "downloads",
    "documents",
    "media",
    "system",
    "developer",
    "ai",
    "caches",
    "other",
];

const AGE_KEYS: [&str; 5] = ["0-30", "31-90", "91-365", "365+", "unknown"];
const AGE_LABELS: [&str; 5] = [
    "Modified in the last 30 days",
    "31–90 days old",
    "91–365 days old",
    "Older than a year",
    "Modified date unknown",
];

const OLD_LARGE_BYTES: u64 = 1024 * 1024 * 1024;
const OLD_LARGE_DAYS: u64 = 365;
const MAX_INSIGHTS: usize = 6;

/// One sentence per category, used by the UI so a category never appears
/// without an explanation of what it holds.
pub fn category_meta(key: &str) -> (&'static str, &'static str) {
    match key {
        "applications" => (
            "Applications",
            "Installed program files under Program Files, ProgramData, WindowsApps and user program folders.",
        ),
        "games" => (
            "Games",
            "Game installations in Steam, Epic, GOG and Xbox game folders.",
        ),
        "downloads" => (
            "Downloads",
            "Files in the user's Downloads folder; some are the only copy unless they can be fetched again.",
        ),
        "documents" => (
            "Documents",
            "Documents, Desktop and cloud-synced folders.",
        ),
        "media" => (
            "Media",
            "Photos, video and audio, whether detected by folder or by file extension.",
        ),
        "system" => (
            "Windows system",
            "Windows itself, restore points and page/hibernation files. Not user data, but not free space to reclaim casually.",
        ),
        "developer" => (
            "Developer artifacts",
            "Build output, dependency folders and repositories: node_modules, target, .venv, .git and similar.",
        ),
        "ai" => (
            "AI models",
            "Local model weights and model caches from Ollama, Hugging Face, LM Studio and similar tools.",
        ),
        "caches" => (
            "Caches and temp",
            "Regenerable temporary, browser and package caches; some need a re-download or a slower first run.",
        ),
        _ => (
            "Other",
            "Files that no path or extension rule classified.",
        ),
    }
}

fn lower_components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_lowercase()),
            _ => None,
        })
        .collect()
}

/// Wrapping separator makes component matches unambiguous.
fn joined(components: &[String]) -> String {
    format!("\\{}\\", components.join("\\"))
}

fn has_component(components: &[String], name: &str) -> bool {
    components.iter().any(|component| component == name)
}

fn has_any_component(components: &[String], names: &[&str]) -> bool {
    components
        .iter()
        .any(|component| names.iter().any(|name| component == name))
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn is_old_large(size: u64, modified: Option<u64>, now: u64) -> bool {
    size >= OLD_LARGE_BYTES
        && modified
            .map(|stamp| now.saturating_sub(stamp) > OLD_LARGE_DAYS * 86_400)
            .unwrap_or(false)
}

/// Classify one path relative to the scan root. `is_dir` suppresses
/// extension-based inference for folders that happen to end in `.jpg`.
pub fn classify(root: &Path, path: &Path, is_dir: bool) -> &'static str {
    classify_detail(root, path, is_dir).0
}

/// Same rules as [`classify`], also reporting whether a documented path rule
/// matched (`true`) or the result is an inference (`false`).
pub fn classify_detail(root: &Path, path: &Path, is_dir: bool) -> (&'static str, bool) {
    let absolute = lower_components(path);
    let relative = path
        .strip_prefix(root)
        .map(lower_components)
        .unwrap_or_default();
    // When the path is outside the scan root (another volume in tests, a
    // junction target in practice) fall back to the absolute components so the
    // same path rules still apply.
    let scope = if relative.is_empty() {
        absolute.clone()
    } else {
        relative
    };
    let scope_joined = joined(&scope);
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let extension = path
        .extension()
        .map(|value| value.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    // Windows itself and volume metadata.
    if absolute.first().map(|part| part == "windows").unwrap_or(false)
        || has_component(&absolute, "$recycle.bin")
        || has_component(&absolute, "system volume information")
        || matches!(
            file_name.as_str(),
            "pagefile.sys" | "hiberfil.sys" | "swapfile.sys"
        )
    {
        return ("system", true);
    }

    // Model weights and model directories. These are never caches: removing
    // them costs a re-download and can break offline use.
    if contains_any(
        &scope_joined,
        &[
            "\\.ollama\\models\\",
            "\\huggingface\\hub\\",
            "\\huggingface\\models\\",
            "\\lm-studio\\models\\",
            "\\lmstudio\\models\\",
            "\\gpt4all\\",
            "\\comfyui\\models\\",
            "\\stable-diffusion-webui\\models\\",
        ],
    ) {
        return ("ai", true);
    }
    if !is_dir
        && matches!(
            extension.as_str(),
            "gguf" | "safetensors" | "onnx" | "pt" | "pth" | "ckpt"
        )
    {
        return ("ai", false);
    }
    if !is_dir
        && extension == "bin"
        && contains_any(&scope_joined, &["\\models\\", "\\model\\"])
    {
        return ("ai", false);
    }

    // Game libraries.
    if has_any_component(&scope, &["steamapps", "epic games", "gog games", "xboxgames"]) {
        return ("games", true);
    }

    // Media folders win over generic document folders (Pictures is not Documents).
    if has_any_component(&scope, &["pictures", "videos", "music"]) {
        return ("media", true);
    }

    if has_component(&scope, "downloads") {
        return ("downloads", true);
    }

    if has_any_component(&scope, &["documents", "desktop", "onedrive"]) {
        return ("documents", true);
    }

    // Temporary and cache locations. Named system/package caches are trusted;
    // a generic folder called "temp" is still a cache by intent.
    if has_component(&scope, "temp")
        || has_component(&scope, "inetcache")
        || has_any_component(
            &scope,
            &[
                "npm-cache",
                "pnpm-store",
                ".pnpm-store",
                "yarn-cache",
                "cache2",
                "codecache",
                "gpucache",
                "shadercache",
            ],
        )
        || contains_any(
            &scope_joined,
            &[
                "\\yarn\\cache\\",
                "\\pip\\cache\\",
                "\\gradle\\caches\\",
                "\\google\\chrome\\user data\\",
                "\\microsoft\\edge\\user data\\",
                "\\mozilla\\firefox\\profiles\\",
            ],
        )
    {
        return ("caches", true);
    }

    // Program locations are checked before generic developer markers so an
    // installed application that happens to contain a `vendor` folder is still
    // reported as an application.
    if absolute
        .first()
        .map(|part| {
            matches!(
                part.as_str(),
                "program files" | "program files (x86)" | "programdata" | "windowsapps"
            )
        })
        .unwrap_or(false)
        || contains_any(
            &scope_joined,
            &[
                "\\appdata\\local\\programs\\",
                "\\appdata\\local\\microsoft\\windowsapps\\",
            ],
        )
    {
        return ("applications", true);
    }

    if has_any_component(
        &scope,
        &[
            "node_modules",
            ".git",
            "target",
            ".venv",
            "venv",
            "__pycache__",
            ".next",
            ".nuxt",
            ".turbo",
            ".cargo",
            ".m2",
            ".gradle",
            ".idea",
            "vendor",
        ],
    ) {
        return ("developer", true);
    }

    // Generic build folders are inferred, not trusted path rules.
    if has_any_component(&scope, &["dist", "build", "obj", "out"]) {
        return ("developer", false);
    }

    if !is_dir
        && matches!(
            extension.as_str(),
            "jpg" | "jpeg"
                | "png"
                | "gif"
                | "webp"
                | "bmp"
                | "tif"
                | "tiff"
                | "heic"
                | "raw"
                | "mp4"
                | "mkv"
                | "avi"
                | "mov"
                | "webm"
                | "mp3"
                | "flac"
                | "wav"
                | "m4a"
                | "aac"
                | "ogg"
        )
    {
        return ("media", false);
    }

    ("other", false)
}

#[derive(Debug, Default)]
struct Entry {
    bytes: u64,
    allocated: u64,
    files: u64,
    /// Files that only matched an inference rather than a documented path rule.
    inferred: u64,
    sample: Option<(u64, String)>,
}

impl Entry {
    fn add(&mut self, path: &Path, bytes: u64, allocated: u64, known: bool) {
        self.bytes = self.bytes.saturating_add(bytes);
        self.allocated = self.allocated.saturating_add(allocated);
        self.files = self.files.saturating_add(1);
        if !known {
            self.inferred = self.inferred.saturating_add(1);
        }
        if self
            .sample
            .as_ref()
            .map(|(largest, _)| bytes > *largest)
            .unwrap_or(true)
        {
            self.sample = Some((bytes, path.to_string_lossy().into_owned()));
        }
    }
}

#[derive(Debug)]
pub struct Accumulator {
    root: PathBuf,
    now: u64,
    categories: HashMap<&'static str, Entry>,
    buckets: [Entry; 5],
    old_large: Entry,
    unknown_modified: u64,
}

impl Accumulator {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            now: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|value| value.as_secs())
                .unwrap_or(0),
            categories: HashMap::new(),
            buckets: Default::default(),
            old_large: Entry::default(),
            unknown_modified: 0,
        }
    }

    pub fn add_file(&mut self, path: &Path, size: u64, allocated: u64, modified: Option<u64>) {
        let (key, known) = classify_detail(&self.root, path, false);
        self.categories
            .entry(key)
            .or_default()
            .add(path, size, allocated, known);

        let bucket = age_index(modified, self.now);
        self.buckets[bucket].add(path, size, allocated, known);
        if modified.is_none() {
            self.unknown_modified = self.unknown_modified.saturating_add(1);
        }

        if is_old_large(size, modified, self.now) {
            self.old_large.add(path, size, allocated, known);
        }
    }

    pub fn categories(&self) -> Vec<StorageCategory> {
        CATEGORY_ORDER
            .iter()
            .map(|key| {
                let entry = self.categories.get(key);
                let (label, note) = category_meta(key);
                StorageCategory {
                    key: (*key).to_string(),
                    label: label.to_string(),
                    bytes: entry.map(|value| value.bytes).unwrap_or(0),
                    allocated: entry.map(|value| value.allocated).unwrap_or(0),
                    file_count: entry.map(|value| value.files).unwrap_or(0),
                    // A category is only reported as known when every file in it
                    // came from a documented path rule, not an extension guess.
                    known: entry
                        .map(|value| value.files > 0 && value.inferred == 0)
                        .unwrap_or(false),
                    note: note.to_string(),
                }
            })
            .collect()
    }

    pub fn age_buckets(&self) -> Vec<AgeBucket> {
        AGE_KEYS
            .iter()
            .zip(AGE_LABELS.iter())
            .enumerate()
            .map(|(index, (key, label))| AgeBucket {
                key: (*key).to_string(),
                label: (*label).to_string(),
                bytes: self.buckets[index].bytes,
                file_count: self.buckets[index].files,
            })
            .collect()
    }

    pub fn insights(&self) -> Vec<StorageInsight> {
        let mut insights = Vec::new();
        let mut push = |key: &str, title: &str, detail: &str, entry: Option<&Entry>| {
            if let Some(entry) = entry {
                if entry.bytes > 0 && entry.files > 0 {
                    insights.push(StorageInsight {
                        key: key.to_string(),
                        title: title.to_string(),
                        detail: detail.to_string(),
                        bytes: entry.bytes,
                        file_count: entry.files,
                        sample_path: entry.sample.as_ref().map(|(_, path)| path.clone()),
                    });
                }
            }
        };

        push(
            "developer",
            "Developer artifacts",
            "Dependency and build folders that regenerating requires reinstalling packages and rebuilding, which can take a long time or a large download.",
            self.categories.get("developer"),
        );
        push(
            "ai",
            "AI model storage",
            "Local model weights; removing them means downloading the model again and breaks offline use for that tool.",
            self.categories.get("ai"),
        );
        push(
            "caches",
            "Caches and temporary data",
            "Regenerable caches and temp data, though some applications re-download or run more slowly on the next start.",
            self.categories.get("caches"),
        );
        push(
            "downloads",
            "Downloads folder",
            "Downloaded files that may be the only local copy unless the source can be fetched again.",
            self.categories.get("downloads"),
        );
        push(
            "games",
            "Installed games",
            "Game data that must be re-downloaded or reinstalled before it can be played again.",
            self.categories.get("games"),
        );
        push(
            "old-large",
            "Old, large files",
            "Files at least 1 GiB and untouched for more than a year; review them individually, since they may be archives, backups or project data.",
            Some(&self.old_large),
        );

        insights.sort_by(|left, right| right.bytes.cmp(&left.bytes));
        insights.truncate(MAX_INSIGHTS);
        insights
    }

    pub fn finish(self) -> (Vec<StorageCategory>, Vec<AgeBucket>, Vec<StorageInsight>) {
        let categories = self.categories();
        let buckets = self.age_buckets();
        let insights = self.insights();
        (categories, buckets, insights)
    }

    /// Files whose modification time was unavailable, for an honest UI note.
    pub fn unknown_modified_files(&self) -> u64 {
        self.unknown_modified
    }
}

fn age_index(modified: Option<u64>, now: u64) -> usize {
    let Some(stamp) = modified else {
        return 4;
    };
    let age_days = now.saturating_sub(stamp) / 86_400;
    if age_days <= 30 {
        0
    } else if age_days <= 90 {
        1
    } else if age_days <= 365 {
        2
    } else {
        3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        PathBuf::from("C:\\")
    }

    #[test]
    fn known_locations_are_classified_with_confidence() {
        let cases = [
            ("C:\\Program Files\\Vendor\\app.exe", "applications", true),
            ("C:\\ProgramData\\Vendor\\settings.json", "applications", true),
            ("C:\\Users\\Ada\\Downloads\\setup.exe", "downloads", true),
            ("C:\\Users\\Ada\\Documents\\notes.txt", "documents", true),
            ("C:\\Users\\Ada\\Pictures\\trip.jpg", "media", true),
            ("C:\\Users\\Ada\\project\\node_modules\\react\\index.js", "developer", true),
            ("C:\\Users\\Ada\\project\\.git\\config", "developer", true),
            ("C:\\Users\\Ada\\.ollama\\models\\blobs\\sha256-abc", "ai", true),
            ("C:\\Users\\Ada\\AppData\\Local\\Temp\\tmp123.tmp", "caches", true),
            ("C:\\Windows\\System32\\drivers\\etc\\hosts", "system", true),
            ("D:\\SteamLibrary\\steamapps\\common\\Game\\game.exe", "games", true),
        ];
        for (path, expected, known) in cases {
            let (key, is_known) = classify_detail(&root(), Path::new(path), false);
            assert_eq!(key, expected, "{path}");
            assert_eq!(is_known, known, "{path}");
        }
    }

    #[test]
    fn extension_inference_is_marked_as_inferred() {
        let (key, known) = classify_detail(
            &root(),
            Path::new("C:\\Users\\Ada\\misc\\holiday.mp4"),
            false,
        );
        assert_eq!(key, "media");
        assert!(!known);

        let (key, known) =
            classify_detail(&root(), Path::new("C:\\Users\\Ada\\models\\weights.gguf"), false);
        assert_eq!(key, "ai");
        assert!(!known);
    }

    #[test]
    fn directories_are_not_classified_by_extension() {
        let (key, known) =
            classify_detail(&root(), Path::new("C:\\Users\\Ada\\misc\\holiday.jpg"), true);
        assert_eq!(key, "other");
        assert!(!known);
    }

    #[test]
    fn relative_roots_still_find_known_folders() {
        let scan_root = Path::new("C:\\Users\\Ada");
        let (key, known) = classify_detail(
            scan_root,
            Path::new("C:\\Users\\Ada\\AppData\\Local\\Programs\\Tool\\tool.exe"),
            false,
        );
        assert_eq!(key, "applications");
        assert!(known);
        assert_eq!(
            classify(scan_root, Path::new("C:\\Users\\Ada\\Downloads\\a.zip"), false),
            "downloads"
        );
    }

    #[test]
    fn every_category_is_always_reported_in_order() {
        let mut accumulator = Accumulator::new(&root());
        accumulator.add_file(Path::new("C:\\misc\\a.bin"), 10, 10, None);
        let (categories, _, _) = accumulator.finish();
        let keys: Vec<_> = categories.iter().map(|entry| entry.key.as_str()).collect();
        assert_eq!(keys, CATEGORY_ORDER);
        assert!(categories.iter().all(|entry| !entry.note.is_empty()));
    }

    #[test]
    fn age_buckets_use_inclusive_boundaries() {
        let now = 1_800_000_000u64;
        let day = 86_400u64;
        assert_eq!(age_index(Some(now - 30 * day), now), 0);
        assert_eq!(age_index(Some(now - 31 * day), now), 1);
        assert_eq!(age_index(Some(now - 90 * day), now), 1);
        assert_eq!(age_index(Some(now - 91 * day), now), 2);
        assert_eq!(age_index(Some(now - 365 * day), now), 2);
        assert_eq!(age_index(Some(now - 366 * day), now), 3);
        assert_eq!(age_index(None, now), 4);
    }

    #[test]
    fn mixed_confidence_categories_are_not_fully_known() {
        let mut accumulator = Accumulator::new(&root());
        accumulator.add_file(Path::new("C:\\Users\\Ada\\Pictures\\a.jpg"), 10, 10, None);
        accumulator.add_file(Path::new("C:\\Users\\Ada\\misc\\b.jpg"), 10, 10, None);
        let (categories, _, _) = accumulator.finish();
        let media = categories
            .iter()
            .find(|entry| entry.key == "media")
            .expect("media category");
        assert_eq!(media.file_count, 2);
        assert!(!media.known, "one inferred file makes the category inferred");

        let mut known_only = Accumulator::new(&root());
        known_only.add_file(Path::new("C:\\Users\\Ada\\Downloads\\a.zip"), 10, 10, None);
        let (categories, _, _) = known_only.finish();
        let downloads = categories
            .iter()
            .find(|entry| entry.key == "downloads")
            .expect("downloads category");
        assert!(downloads.known);
    }

    #[test]
    fn accumulator_totals_and_insights_are_real() {
        let mut accumulator = Accumulator::new(&root());
        accumulator.add_file(Path::new("C:\\Users\\Ada\\Downloads\\a.iso"), 2_000, 2_000, None);
        accumulator.add_file(Path::new("C:\\Users\\Ada\\Downloads\\b.iso"), 3_000, 3_000, None);
        accumulator.add_file(Path::new("C:\\Windows\\System32\\a.dll"), 1_000, 1_000, None);
        let (categories, buckets, insights) = accumulator.finish();

        let downloads = categories
            .iter()
            .find(|entry| entry.key == "downloads")
            .expect("downloads category");
        assert_eq!(downloads.bytes, 5_000);
        assert_eq!(downloads.file_count, 2);
        assert_eq!(
            buckets.iter().map(|bucket| bucket.file_count).sum::<u64>(),
            3
        );
        let download_insight = insights
            .iter()
            .find(|entry| entry.key == "downloads")
            .expect("downloads insight");
        assert_eq!(download_insight.bytes, 5_000);
        assert!(download_insight.detail.to_lowercase().contains("download"));
        assert!(!download_insight.detail.to_lowercase().contains("safe to delete"));
    }

    #[test]
    fn old_large_requires_both_thresholds() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_secs();
        let old = now - 400 * 86_400;
        let gib = 1024 * 1024 * 1024;
        let mut accumulator = Accumulator::new(&root());
        accumulator.add_file(Path::new("C:\\Users\\Ada\\archive.iso"), gib, gib, Some(old));
        accumulator.add_file(
            Path::new("C:\\Users\\Ada\\almost.iso"),
            gib - 1,
            gib - 1,
            Some(old),
        );
        accumulator.add_file(
            Path::new("C:\\Users\\Ada\\recent.iso"),
            gib * 2,
            gib * 2,
            Some(now - 10 * 86_400),
        );
        let (_, _, insights) = accumulator.finish();
        let insight = insights
            .iter()
            .find(|entry| entry.key == "old-large")
            .expect("old large insight");
        assert_eq!(insight.file_count, 1);
        assert_eq!(insight.bytes, gib);
    }
}
