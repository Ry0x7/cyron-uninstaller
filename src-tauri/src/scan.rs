use crate::fs::{self, DirEntryInfo};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScanOptions {
    /// Include files and folders flagged hidden or system.
    pub include_hidden: bool,
    /// Absolute path prefixes that must not be traversed.
    pub exclusions: Vec<String>,
    /// Build a tree for the treemap. False only returns totals.
    pub collect_tree: bool,
    /// Maximum directory recursion depth.
    pub max_depth: u32,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            include_hidden: true,
            exclusions: Vec::new(),
            collect_tree: true,
            max_depth: 48,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanNode {
    pub name: String,
    /// Empty for synthetic aggregate nodes.
    pub path: String,
    /// dir, file or group.
    pub kind: String,
    pub size: u64,
    pub allocated: u64,
    pub file_count: u64,
    pub dir_count: u64,
    pub modified: Option<u64>,
    pub children: Vec<ScanNode>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionStat {
    pub extension: String,
    pub file_count: u64,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStat {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub allocated: u64,
    pub extension: String,
    pub modified: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub job_id: String,
    pub root_path: String,
    pub root: ScanNode,
    pub total_size: u64,
    pub total_allocated: u64,
    pub file_count: u64,
    pub dir_count: u64,
    pub deduped_files: u64,
    pub skipped_reparse: u64,
    pub errors: Vec<String>,
    pub extension_stats: Vec<ExtensionStat>,
    pub largest_files: Vec<FileStat>,
    pub elapsed_ms: u64,
    pub cancelled: bool,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub job_id: String,
    pub files: u64,
    pub dirs: u64,
    pub bytes: u64,
    pub current_dir: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Measured {
    pub size: u64,
    pub allocated: u64,
    pub file_count: u64,
    pub dir_count: u64,
    pub elapsed_ms: u64,
    pub errors: Vec<String>,
}

const MAX_ERRORS: usize = 20;
const MAX_FILE_NODES: usize = 40;
const MAX_CHILDREN_PER_DIR: usize = 200;
const IPC_TREE_DEPTH: u32 = 8;
const MAX_EXTENSIONS: usize = 24;
const MAX_LARGEST_FILES: usize = 100;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(120);
/// Hard-link ids are the only per-file state the walker keeps. Cap it so a
/// multi-million-file scan cannot grow memory without limit; past the cap new
/// ids are no longer tracked and the result says so.
const MAX_TRACKED_FILE_IDS: usize = 1_000_000;
/// Poll progress inside very large single directories.
const PROGRESS_EVERY_FILES: u64 = 512;

/// Walks a directory and returns real file sizes. `cancel` is polled between
/// directories. `progress` is called at most a few times per second.
pub fn scan_directory(
    job_id: &str,
    path: &Path,
    options: &ScanOptions,
    cancel: Arc<AtomicBool>,
    progress: &(dyn Fn(ScanProgress) + Send + Sync),
) -> ScanResult {
    let started = Instant::now();
    let mut walker = Walker::new(job_id, options, Some(cancel), Some(progress), true);
    let mut root = walker.walk(path, 0);

    if walker.collect_tree && prune_tree(&mut root, 0) {
        walker.truncated = true;
    }
    walker.emit_progress(path, true);
    let extension_stats = walker.extension_stats();
    let largest_files = walker.largest_files();
    let errors = std::mem::take(&mut walker.errors);

    ScanResult {
        job_id: job_id.to_string(),
        root_path: path.to_string_lossy().into_owned(),
        root,
        total_size: walker.size,
        total_allocated: walker.allocated,
        file_count: walker.files,
        dir_count: walker.dirs,
        deduped_files: walker.deduped,
        skipped_reparse: walker.skipped_reparse,
        errors,
        extension_stats,
        largest_files,
        elapsed_ms: elapsed(started),
        cancelled: walker.cancelled,
        truncated: walker.truncated,
    }
}

/// Size of one folder without building a tree.
pub fn measure_path(path: &Path) -> Measured {
    let started = Instant::now();
    let mut walker = Walker::new("measure", &ScanOptions::default(), None, None, false);
    let _ = walker.walk(path, 0);
    Measured {
        size: walker.size,
        allocated: walker.allocated,
        file_count: walker.files,
        dir_count: walker.dirs,
        elapsed_ms: elapsed(started),
        errors: walker.errors,
    }
}

fn elapsed(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u64::MAX as u128) as u64
}

struct Walker<'a> {
    job_id: &'a str,
    include_hidden: bool,
    exclusions: Vec<String>,
    max_depth: u32,
    collect_tree: bool,
    cancel: Option<Arc<AtomicBool>>,
    progress: Option<&'a (dyn Fn(ScanProgress) + Send + Sync)>,
    last_progress: Option<Instant>,

    size: u64,
    allocated: u64,
    files: u64,
    dirs: u64,
    deduped: u64,
    skipped_reparse: u64,
    cancelled: bool,
    truncated: bool,

    seen_file_ids: HashSet<u64>,
    dedupe_limit_warned: bool,
    extensions: HashMap<String, (u64, u64)>,
    largest: Vec<FileStat>,
    errors: Vec<String>,
}

impl<'a> Walker<'a> {
    fn new(
        job_id: &'a str,
        options: &ScanOptions,
        cancel: Option<Arc<AtomicBool>>,
        progress: Option<&'a (dyn Fn(ScanProgress) + Send + Sync)>,
        collect_tree: bool,
    ) -> Self {
        Self {
            job_id,
            include_hidden: options.include_hidden,
            exclusions: options
                .exclusions
                .iter()
                .map(|value| normalize_path(value.trim()))
                .filter(|value| !value.is_empty())
                .collect(),
            max_depth: options.max_depth.clamp(1, 64),
            collect_tree: collect_tree && options.collect_tree,
            cancel,
            progress,
            last_progress: None,
            size: 0,
            allocated: 0,
            files: 0,
            dirs: 0,
            deduped: 0,
            skipped_reparse: 0,
            cancelled: false,
            truncated: false,
            seen_file_ids: HashSet::new(),
            dedupe_limit_warned: false,
            extensions: HashMap::new(),
            largest: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn walk(&mut self, path: &Path, depth: u32) -> ScanNode {
        let mut node = ScanNode {
            name: display_name(path),
            path: path.to_string_lossy().into_owned(),
            kind: "dir".into(),
            size: 0,
            allocated: 0,
            file_count: 0,
            dir_count: 0,
            modified: None,
            children: Vec::new(),
        };

        if self.is_cancelled() {
            return node;
        }
        let metadata = std::fs::metadata(path).ok();
        if !metadata.as_ref().is_some_and(|value| value.is_dir()) {
            self.record_error(format!("Not a directory: {}", path.display()));
            return node;
        }
        if self.collect_tree {
            node.modified = metadata
                .as_ref()
                .and_then(|value| value.modified().ok())
                .and_then(systemtime_to_unix);
        }

        let entries = match fs::read_dir(path) {
            Ok(entries) => entries,
            Err(error) => {
                self.record_error(format!("{}: {}", path.display(), error));
                self.dirs += 1;
                return node;
            }
        };
        self.dirs += 1;
        self.maybe_progress(path);

        let mut directory_children: Vec<ScanNode> = Vec::new();
        let mut file_nodes: Vec<ScanNode> = Vec::new();
        let mut own_files = 0u64;
        let mut overflow_count = 0u64;
        let mut overflow_size = 0u64;
        let mut overflow_allocated = 0u64;

        for entry in entries {
            if self.is_cancelled() {
                break;
            }
            if !self.include_hidden && (entry.is_hidden || entry.is_system) {
                continue;
            }
            if entry.is_reparse {
                self.skipped_reparse += 1;
                if self.skipped_reparse <= 3 {
                    self.record_error(format!(
                        "Skipped link or junction (not followed): {}",
                        entry.path.display()
                    ));
                }
                continue;
            }
            if entry.is_dir {
                if self.is_excluded(&entry.path) {
                    continue;
                }
                if depth >= self.max_depth {
                    continue;
                }
                let child = self.walk(&entry.path, depth + 1);
                node.size += child.size;
                node.allocated += child.allocated;
                directory_children.push(child);
                continue;
            }

            if self.is_excluded(&entry.path) {
                continue;
            }

            own_files += 1;
            self.files += 1;
            if own_files % PROGRESS_EVERY_FILES == 0 {
                self.maybe_progress(path);
            }
            self.size += entry.size;
            node.size += entry.size;

            let unique = match entry.file_id {
                Some(file_id) => self.track_file_id(file_id),
                None => true,
            };
            if unique {
                self.allocated += entry.allocated;
                node.allocated += entry.allocated;
            } else {
                self.deduped += 1;
            }

            self.track_extension(&entry, unique);
            self.track_largest(&entry);

            if self.collect_tree {
                let file_node = ScanNode {
                    name: entry.name.clone(),
                    path: entry.path.to_string_lossy().into_owned(),
                    kind: "file".into(),
                    size: entry.size,
                    allocated: if unique { entry.allocated } else { 0 },
                    file_count: 1,
                    dir_count: 0,
                    modified: entry.modified,
                    children: Vec::new(),
                };
                insert_file_node(&mut file_nodes, file_node);
                if file_nodes.len() > MAX_FILE_NODES {
                    let dropped = file_nodes.remove(0);
                    overflow_count += 1;
                    overflow_size += dropped.size;
                    overflow_allocated += dropped.allocated;
                }
            }
        }

        let mut sub_files = 0u64;
        let mut sub_dirs = 0u64;
        for child in &directory_children {
            sub_files += child.file_count;
            sub_dirs += child.dir_count + 1;
        }
        node.file_count = own_files + sub_files;
        node.dir_count = sub_dirs;

        if self.collect_tree {
            node.children.extend(directory_children);
            if overflow_count > 0 {
                node.children.push(ScanNode {
                    name: format!("{} smaller files", format_count(overflow_count)),
                    path: String::new(),
                    kind: "group".into(),
                    size: overflow_size,
                    allocated: overflow_allocated,
                    file_count: overflow_count,
                    dir_count: 0,
                    modified: None,
                    children: Vec::new(),
                });
            }
            node.children.extend(file_nodes);
            node.children
                .sort_by_key(|child| std::cmp::Reverse(child.size));
        }

        node
    }

    /// Inserts one file id, returning false for a second link to the same file.
    fn track_file_id(&mut self, file_id: u64) -> bool {
        if self.seen_file_ids.len() >= MAX_TRACKED_FILE_IDS {
            if self.seen_file_ids.contains(&file_id) {
                return false;
            }
            self.record_dedupe_limit();
            return true;
        }
        self.seen_file_ids.insert(file_id)
    }

    fn record_dedupe_limit(&mut self) {
        if self.dedupe_limit_warned {
            return;
        }
        self.dedupe_limit_warned = true;
        self.record_error(format!(
            "Hard-link tracking stopped after {} files; later links may count twice.",
            format_count(MAX_TRACKED_FILE_IDS as u64)
        ));
    }

    fn is_cancelled(&mut self) -> bool {
        if let Some(flag) = &self.cancel {
            if flag.load(Ordering::Relaxed) {
                self.cancelled = true;
            }
        }
        self.cancelled
    }

    fn is_excluded(&self, path: &Path) -> bool {
        if self.exclusions.is_empty() {
            return false;
        }
        let candidate = normalize_path(&path.to_string_lossy());
        self.exclusions.iter().any(|excluded| {
            candidate == *excluded
                || (candidate.len() > excluded.len()
                    && candidate.starts_with(excluded.as_str())
                    && candidate.as_bytes()[excluded.len()] == b'\\')
        })
    }

    fn track_extension(&mut self, entry: &DirEntryInfo, unique: bool) {
        let extension = extension_of(&entry.name);
        let slot = self.extensions.entry(extension).or_insert((0, 0));
        slot.0 += 1;
        // Extension totals follow the same hard-link rule as allocated space:
        // a file counted twice for logical size is counted once for storage.
        if unique {
            slot.1 += entry.size;
        }
    }

    fn track_largest(&mut self, entry: &DirEntryInfo) {
        let candidate = FileStat {
            name: entry.name.clone(),
            path: entry.path.to_string_lossy().into_owned(),
            size: entry.size,
            allocated: entry.allocated,
            extension: extension_of(&entry.name),
            modified: entry.modified,
        };
        if self.largest.len() < MAX_LARGEST_FILES {
            self.largest.push(candidate);
            return;
        }
        let smallest = self
            .largest
            .iter()
            .enumerate()
            .min_by_key(|(_, item)| item.size)
            .map(|(index, _)| index);
        if let Some(index) = smallest {
            if self.largest[index].size < candidate.size {
                self.largest[index] = candidate;
            }
        }
    }

    fn extension_stats(&self) -> Vec<ExtensionStat> {
        let mut stats: Vec<ExtensionStat> = self
            .extensions
            .iter()
            .map(|(extension, (file_count, size))| ExtensionStat {
                extension: extension.clone(),
                file_count: *file_count,
                size: *size,
            })
            .collect();
        stats.sort_by_key(|stat| std::cmp::Reverse(stat.size));
        stats.truncate(MAX_EXTENSIONS);
        stats
    }

    fn largest_files(&mut self) -> Vec<FileStat> {
        self.largest
            .sort_by_key(|file| std::cmp::Reverse(file.size));
        self.largest.truncate(MAX_LARGEST_FILES);
        self.largest.clone()
    }

    fn record_error(&mut self, message: String) {
        if self.errors.len() < MAX_ERRORS {
            self.errors.push(message);
        }
    }

    fn maybe_progress(&mut self, current: &Path) {
        let Some(callback) = self.progress else {
            return;
        };
        let due = match self.last_progress {
            None => true,
            Some(last) => last.elapsed() >= PROGRESS_INTERVAL,
        };
        if !due {
            return;
        }
        callback(ScanProgress {
            job_id: self.job_id.to_string(),
            files: self.files,
            dirs: self.dirs,
            bytes: self.size,
            current_dir: current.to_string_lossy().into_owned(),
        });
        self.last_progress = Some(Instant::now());
    }

    fn emit_progress(&self, current: &Path, force: bool) {
        let Some(callback) = self.progress else {
            return;
        };
        if !force && self.last_progress.is_some() {
            return;
        }
        callback(ScanProgress {
            job_id: self.job_id.to_string(),
            files: self.files,
            dirs: self.dirs,
            bytes: self.size,
            current_dir: current.to_string_lossy().into_owned(),
        });
    }
}

/// Keeps the largest file nodes first and drops the smallest when full.
fn insert_file_node(nodes: &mut Vec<ScanNode>, node: ScanNode) {
    let position = nodes.iter().position(|existing| existing.size > node.size);
    match position {
        Some(index) => nodes.insert(index, node),
        None => nodes.push(node),
    }
}

/// Caps the tree that crosses the IPC boundary and reports whether anything was
/// dropped. Aggregated nodes keep the dropped sizes visible.
fn prune_tree(node: &mut ScanNode, depth: u32) -> bool {
    if node.children.is_empty() {
        return false;
    }
    if depth >= IPC_TREE_DEPTH {
        node.children.clear();
        return true;
    }

    let mut dropped = false;
    if node.children.len() > MAX_CHILDREN_PER_DIR {
        node.children
            .sort_by_key(|child| std::cmp::Reverse(child.size));
        let removed = node.children.split_off(MAX_CHILDREN_PER_DIR);
        let group = ScanNode {
            name: format!("{} more items", format_count(removed.len() as u64)),
            path: String::new(),
            kind: "group".into(),
            size: removed.iter().map(|child| child.size).sum(),
            allocated: removed.iter().map(|child| child.allocated).sum(),
            file_count: removed.iter().map(|child| child.file_count).sum(),
            dir_count: removed.iter().map(|child| child.dir_count).sum(),
            modified: None,
            children: Vec::new(),
        };
        node.children.push(group);
        dropped = true;
    }
    for child in &mut node.children {
        if prune_tree(child, depth + 1) {
            dropped = true;
        }
    }
    dropped
}

fn systemtime_to_unix(value: SystemTime) -> Option<u64> {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

fn display_name(path: &Path) -> String {
    match path.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => path.display().to_string(),
    }
}

fn extension_of(name: &str) -> String {
    let lowered = name.to_ascii_lowercase();
    let dot = lowered.rfind('.');
    match dot {
        Some(index) if index > 0 && index + 1 < lowered.len() => lowered[index + 1..].to_string(),
        _ => String::new(),
    }
}

fn normalize_path(value: &str) -> String {
    value
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

fn format_count(value: u64) -> String {
    let digits = value.to_string();
    let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, character) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            formatted.push(',');
        }
        formatted.push(character);
    }
    formatted
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(tag: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|value| value.as_nanos())
                .unwrap_or(0);
            let path = std::env::temp_dir()
                .join(format!("cyron-scan-{tag}-{}-{nanos}", std::process::id()));
            fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn write_file(path: &Path, bytes: usize) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, vec![7u8; bytes]).expect("write fixture");
    }

    fn scan(root: &Path, options: ScanOptions) -> ScanResult {
        scan_directory(
            "test-job",
            root,
            &options,
            Arc::new(AtomicBool::new(false)),
            &|_| {},
        )
    }

    #[test]
    fn totals_and_tree_match_the_fixture() {
        let temp = TempDir::new("totals");
        write_file(&temp.path().join("a.bin"), 1000);
        write_file(&temp.path().join("nested").join("b.bin"), 2500);
        write_file(
            &temp.path().join("nested").join("deeper").join("c.txt"),
            500,
        );

        let result = scan(temp.path(), ScanOptions::default());

        assert_eq!(result.total_size, 4000);
        assert_eq!(result.file_count, 3);
        assert!(result.dir_count >= 3, "root plus two nested dirs");
        assert_eq!(result.root.size, 4000);
        assert_eq!(result.root.file_count, 3);
        assert!(result
            .root
            .children
            .iter()
            .any(|child| child.name == "nested"));
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(!result.cancelled);
    }

    #[test]
    fn small_files_are_aggregated_without_losing_totals() {
        let temp = TempDir::new("aggregate");
        for index in 0..60u64 {
            write_file(&temp.path().join(format!("f{index:02}.bin")), 4096);
        }

        let result = scan(temp.path(), ScanOptions::default());

        assert_eq!(result.total_size, 60 * 4096);
        assert_eq!(result.root.file_count, 60);
        assert!(result.root.children.len() <= MAX_FILE_NODES + 1);
        let group = result
            .root
            .children
            .iter()
            .find(|child| child.kind == "group")
            .expect("overflow group is present");
        assert_eq!(group.file_count, 60 - MAX_FILE_NODES as u64);
        assert_eq!(
            group.size,
            (60 - MAX_FILE_NODES as u64) * 4096,
            "aggregate keeps the dropped bytes"
        );
        assert!(group.path.is_empty(), "groups are not openable paths");
    }

    #[test]
    fn ipc_pruning_keeps_the_largest_directories() {
        let temp = TempDir::new("prune");
        for index in 0..210u64 {
            let directory = temp.path().join(format!("d{index:03}"));
            write_file(&directory.join("payload.bin"), (index as usize + 1) * 16);
        }

        let result = scan(temp.path(), ScanOptions::default());

        assert!(result.truncated, "dropping tree detail must be reported");
        assert!(
            result.root.children.len() <= MAX_CHILDREN_PER_DIR + 1,
            "{} children",
            result.root.children.len()
        );
        assert!(
            result.root.children.iter().any(|child| child.name == "d209"),
            "largest directory stays as an individual tile"
        );
        assert!(
            result
                .root
                .children
                .iter()
                .all(|child| child.name != "d000"),
            "smallest directory was folded into the group"
        );
        let group = result
            .root
            .children
            .iter()
            .find(|child| child.kind == "group")
            .expect("overflow group is present");
        assert_eq!(group.kind, "group");
        assert!(group.path.is_empty());
        assert_eq!(
            group.file_count,
            210 - MAX_CHILDREN_PER_DIR as u64,
            "group accounts for every folded directory"
        );
        assert_eq!(result.total_size, (1..=210u64).map(|value| value * 16).sum::<u64>());
    }

    #[test]
    fn ipc_depth_cap_trims_deep_trees_without_losing_totals() {
        let temp = TempDir::new("ipcdepth");
        let mut cursor = temp.path().to_path_buf();
        for index in 0..12 {
            cursor = cursor.join(format!("level{index:02}"));
            write_file(&cursor.join("note.txt"), 32);
        }

        let result = scan(temp.path(), ScanOptions::default());

        assert!(result.truncated, "depth trimming must be reported");
        assert_eq!(result.total_size, 12 * 32);
        let mut node = &result.root;
        let mut depth = 0;
        while let Some(child) = node
            .children
            .iter()
            .find(|child| child.kind == "dir")
        {
            node = child;
            depth += 1;
        }
        assert!(
            depth <= IPC_TREE_DEPTH as usize + 1,
            "visible tree stopped at depth {depth}"
        );
    }

    #[test]
    fn hard_links_are_counted_once_for_allocated_space() {
        let temp = TempDir::new("hardlink");
        let source = temp.path().join("original.bin");
        write_file(&source, 300_000);
        let link = temp.path().join("link.bin");
        if fs::hard_link(&source, &link).is_err() {
            return; // Filesystem without hard link support.
        }

        let result = scan(temp.path(), ScanOptions::default());

        assert_eq!(result.file_count, 2);
        assert_eq!(result.total_size, 600_000);
        assert_eq!(result.deduped_files, 1);
        assert!(
            result.total_allocated < result.total_size,
            "allocated {} should be smaller than logical {}",
            result.total_allocated,
            result.total_size
        );
    }

    #[test]
    fn exclusions_skip_a_subtree() {
        let temp = TempDir::new("exclude");
        write_file(&temp.path().join("keep.bin"), 100);
        let excluded = temp.path().join("skip-me");
        write_file(&excluded.join("big.bin"), 9000);

        let result = scan(
            temp.path(),
            ScanOptions {
                exclusions: vec![excluded.to_string_lossy().into_owned()],
                ..ScanOptions::default()
            },
        );

        assert_eq!(result.total_size, 100);
        assert!(result
            .root
            .children
            .iter()
            .all(|child| child.name != "skip-me"));
    }

    #[test]
    fn max_depth_stops_recursion() {
        let temp = TempDir::new("depth");
        write_file(&temp.path().join("top.bin"), 10);
        write_file(&temp.path().join("one").join("one.bin"), 20);
        write_file(&temp.path().join("one").join("two").join("two.bin"), 4000);

        let result = scan(
            temp.path(),
            ScanOptions {
                max_depth: 1,
                ..ScanOptions::default()
            },
        );

        assert_eq!(result.total_size, 30);
        assert_eq!(result.file_count, 2);
    }

    #[test]
    fn cancellation_returns_partial_results() {
        let temp = TempDir::new("cancel");
        write_file(&temp.path().join("a.bin"), 1000);
        let cancel = Arc::new(AtomicBool::new(true));

        let result = scan_directory(
            "cancel-job",
            temp.path(),
            &ScanOptions::default(),
            cancel,
            &|_| {},
        );

        assert!(result.cancelled);
        assert_eq!(result.file_count, 0);
    }

    #[test]
    fn progress_is_reported_with_real_counts() {
        let temp = TempDir::new("progress");
        write_file(&temp.path().join("a.bin"), 1000);
        write_file(&temp.path().join("b").join("b.bin"), 1000);
        let events: Mutex<Vec<ScanProgress>> = Mutex::new(Vec::new());

        let result = scan_directory(
            "progress-job",
            temp.path(),
            &ScanOptions::default(),
            Arc::new(AtomicBool::new(false)),
            &|event| events.lock().unwrap().push(event),
        );

        let events = events.into_inner().unwrap();
        assert!(!events.is_empty(), "expected at least one progress event");
        assert!(events.iter().all(|event| event.job_id == "progress-job"));
        assert_eq!(events.last().unwrap().files, result.file_count);
    }

    #[test]
    fn extension_stats_and_largest_files_are_reported() {
        let temp = TempDir::new("stats");
        write_file(&temp.path().join("video.mp4"), 5000);
        write_file(&temp.path().join("notes.md"), 300);
        write_file(&temp.path().join("noextension"), 100);

        let result = scan(temp.path(), ScanOptions::default());

        let video = result
            .extension_stats
            .iter()
            .find(|stat| stat.extension == "mp4")
            .expect("mp4 stat");
        assert_eq!(video.size, 5000);
        assert_eq!(video.file_count, 1);
        assert!(result
            .extension_stats
            .iter()
            .any(|stat| stat.extension.is_empty()));
        assert_eq!(result.largest_files[0].name, "video.mp4");
    }

    #[test]
    fn junctions_are_never_followed() {
        let temp = TempDir::new("junction");
        write_file(&temp.path().join("real.bin"), 100);
        let outside = TempDir::new("junction-target");
        write_file(&outside.path().join("outside.bin"), 2_000_000);
        let link = temp.path().join("junction");
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(outside.path())
            .status();
        if !matches!(status, Ok(value) if value.success()) {
            return; // Junction creation is unavailable in this environment.
        }

        let result = scan(temp.path(), ScanOptions::default());

        assert_eq!(result.total_size, 100);
        assert!(result.skipped_reparse >= 1);
        let _ = fs::remove_dir(&link);
    }

    #[test]
    fn measure_path_reports_totals_without_a_tree() {
        let temp = TempDir::new("measure");
        write_file(&temp.path().join("one.bin"), 2048);

        let measured = measure_path(temp.path());

        assert_eq!(measured.size, 2048);
        assert_eq!(measured.file_count, 1);
        assert!(measured.errors.is_empty());
    }

    #[test]
    fn missing_path_reports_an_error_instead_of_panicking() {
        let temp = TempDir::new("missing");
        let missing = temp.path().join("does-not-exist");

        let result = scan(&missing, ScanOptions::default());

        assert_eq!(result.total_size, 0);
        assert!(!result.errors.is_empty());
    }
}
