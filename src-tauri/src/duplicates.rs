//! Read-only duplicate-file detection.
//!
//! Three stages keep the work proportional to real candidates: exact-size
//! grouping, a head+tail SHA-256 fingerprint, then a full streaming SHA-256.
//! Nothing here deletes, moves or opens a file for writing; hard links are
//! reported but never counted as recoverable space.

use crate::fs::read_dir;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, UNIX_EPOCH};

/// Bytes read from the start and (for larger files) the end for the first pass.
const PARTIAL_BYTES: u64 = 4 * 1024;
const HASH_BUFFER: usize = 1024 * 1024;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(150);
const MAX_ERRORS: usize = 20;
const MAX_GROUPS: usize = 200;
const MAX_FILES_PER_GROUP: usize = 50;
const MAX_DEPTH: u32 = 64;
/// Bound the candidate list so a drive-sized scan cannot exhaust memory.
const MAX_CANDIDATES: usize = 1_000_000;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DuplicateOptions {
    /// Files smaller than this are not candidates. Defaults to 1 MiB.
    pub min_size: u64,
    pub include_hidden: bool,
    /// Absolute path prefixes that must not be traversed.
    pub exclusions: Vec<String>,
}

impl Default for DuplicateOptions {
    fn default() -> Self {
        Self {
            min_size: 1024 * 1024,
            include_hidden: true,
            exclusions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateFile {
    pub path: String,
    pub size: u64,
    pub allocated: u64,
    pub modified: Option<u64>,
    /// True when this path is another hard link to a file already in the group.
    pub hard_link: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub id: String,
    pub size: u64,
    /// Recoverable bytes: size × (non-hard-link copies − 1).
    pub wasted: u64,
    pub files: Vec<DuplicateFile>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateReport {
    pub job_id: String,
    pub root_path: String,
    pub groups: Vec<DuplicateGroup>,
    pub total_groups: u64,
    pub total_files: u64,
    pub total_wasted: u64,
    pub candidate_files: u64,
    pub hashed_files: u64,
    pub scanned_files: u64,
    pub errors: Vec<String>,
    pub elapsed_ms: u64,
    pub cancelled: bool,
    pub truncated: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateProgress {
    pub job_id: String,
    /// index, partial or full.
    pub phase: String,
    pub files: u64,
    pub bytes_hashed: u64,
    pub current_path: String,
}

pub const DUPLICATE_EVENT: &str = "cyron-duplicate-progress";

#[derive(Debug, Clone)]
struct Candidate {
    path: PathBuf,
    size: u64,
    allocated: u64,
    modified: Option<u64>,
    file_id: Option<u64>,
    hard_link: bool,
    digest: Option<[u8; 32]>,
}

struct Context<'a> {
    job_id: &'a str,
    cancel: &'a AtomicBool,
    progress: &'a (dyn Fn(DuplicateProgress) + Send + Sync),
    last_progress: Option<Instant>,
    cancelled: bool,
    hashed_files: u64,
    bytes_hashed: u64,
    errors: Vec<String>,
}

impl<'a> Context<'a> {
    fn new(
        job_id: &'a str,
        cancel: &'a AtomicBool,
        progress: &'a (dyn Fn(DuplicateProgress) + Send + Sync),
    ) -> Self {
        Self {
            job_id,
            cancel,
            progress,
            last_progress: None,
            cancelled: false,
            hashed_files: 0,
            bytes_hashed: 0,
            errors: Vec::new(),
        }
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled || self.cancel.load(Ordering::Relaxed)
    }

    fn push_error(&mut self, message: String) {
        if self.errors.len() < MAX_ERRORS {
            self.errors.push(message);
        }
    }

    fn emit(&mut self, phase: &str, files: u64, current_path: &Path, force: bool) {
        let now = Instant::now();
        if !force
            && self
                .last_progress
                .is_some_and(|last| now.duration_since(last) < PROGRESS_INTERVAL)
        {
            return;
        }
        self.last_progress = Some(now);
        (self.progress)(DuplicateProgress {
            job_id: self.job_id.to_string(),
            phase: phase.to_string(),
            files,
            bytes_hashed: self.bytes_hashed,
            current_path: current_path.to_string_lossy().into_owned(),
        });
    }
}

/// Finds duplicate files below `root`. The scan is read-only and cancellation
/// is polled while indexing and between every hashing chunk.
pub fn find_duplicates(
    job_id: &str,
    root: &Path,
    options: &DuplicateOptions,
    cancel: Arc<AtomicBool>,
    progress: &(dyn Fn(DuplicateProgress) + Send + Sync),
) -> DuplicateReport {
    let started = Instant::now();
    let mut ctx = Context::new(job_id, cancel.as_ref(), progress);
    let mut index = Indexer {
        include_hidden: options.include_hidden,
        exclusions: options
            .exclusions
            .iter()
            .map(|value| normalize_path(value))
            .filter(|value| !value.is_empty())
            .collect(),
        min_size: options.min_size,
        candidates: Vec::new(),
        seen_ids: HashMap::new(),
        scanned_files: 0,
        truncated: false,
    };

    index.walk(root, 0, &mut ctx);
    ctx.emit("index", index.scanned_files, root, true);

    let mut report = DuplicateReport {
        job_id: job_id.to_string(),
        root_path: root.to_string_lossy().into_owned(),
        groups: Vec::new(),
        total_groups: 0,
        total_files: 0,
        total_wasted: 0,
        candidate_files: index.candidates.len() as u64,
        hashed_files: 0,
        scanned_files: index.scanned_files,
        errors: ctx.errors.clone(),
        elapsed_ms: 0,
        cancelled: false,
        truncated: index.truncated,
    };

    if ctx.is_cancelled() {
        report.cancelled = true;
        report.errors = ctx.errors;
        report.elapsed_ms = elapsed_ms(started);
        return report;
    }

    let mut groups = size_groups(index.candidates);
    if groups.is_empty() {
        report.errors = ctx.errors;
        report.elapsed_ms = elapsed_ms(started);
        return report;
    }

    if !hash_stage(&mut groups, false, &mut ctx) {
        report.cancelled = true;
        report.errors = ctx.errors;
        report.elapsed_ms = elapsed_ms(started);
        return report;
    }
    groups = regroup(groups);
    // The full pass must hash bytes again: keeping the partial digest would
    // make the second stage skip every candidate.
    for group in groups.iter_mut() {
        for candidate in group.iter_mut() {
            candidate.digest = None;
        }
    }

    if !hash_stage(&mut groups, true, &mut ctx) {
        report.cancelled = true;
        report.errors = ctx.errors;
        report.elapsed_ms = elapsed_ms(started);
        return report;
    }
    let verified = regroup(groups);
    report.hashed_files = ctx.hashed_files;
    report.truncated |= verified.len() > MAX_GROUPS;

    let mut all_groups: Vec<DuplicateGroup> = verified
        .into_iter()
        .filter_map(|mut files| {
            // Hard links are the same bytes on disk. A group needs at least two
            // independent copies before any space is actually recoverable.
            let independent = files.iter().filter(|file| !file.hard_link).count();
            if independent < 2 {
                return None;
            }
            let size = files[0].size;
            files.sort_by(|a, b| {
                a.hard_link
                    .cmp(&b.hard_link)
                    .then_with(|| a.path.cmp(&b.path))
            });
            let wasted = size.saturating_mul((independent - 1) as u64);
            let digest = files[0].digest.unwrap_or([0u8; 32]);
            Some(DuplicateGroup {
                id: format!("dup-{:x}-{}", size, hex_prefix(&digest)),
                size,
                wasted,
                files: files
                    .into_iter()
                    .take(MAX_FILES_PER_GROUP)
                    .map(|file| DuplicateFile {
                        path: file.path.to_string_lossy().into_owned(),
                        size: file.size,
                        allocated: file.allocated,
                        modified: file.modified,
                        hard_link: file.hard_link,
                    })
                    .collect(),
            })
        })
        .collect();

    all_groups.sort_by(|a, b| b.wasted.cmp(&a.wasted).then_with(|| a.id.cmp(&b.id)));
    report.total_groups = all_groups.len() as u64;
    report.total_files = all_groups.iter().map(|group| group.files.len() as u64).sum();
    report.total_wasted = all_groups.iter().map(|group| group.wasted).sum();
    all_groups.truncate(MAX_GROUPS);
    report.groups = all_groups;
    report.has_files_per_group_capped();
    report.errors = ctx.errors;
    report.elapsed_ms = elapsed_ms(started);
    report
}

impl DuplicateReport {
    fn has_files_per_group_capped(&mut self) {
        // The cap is applied while building each group; mark the report so the
        // UI can say that only the first paths per group are listed.
        if self.groups.iter().any(|group| group.files.len() == MAX_FILES_PER_GROUP) {
            self.truncated = true;
        }
    }
}

struct Indexer {
    include_hidden: bool,
    exclusions: Vec<String>,
    min_size: u64,
    candidates: Vec<Candidate>,
    seen_ids: HashMap<u64, PathBuf>,
    scanned_files: u64,
    truncated: bool,
}

impl Indexer {
    fn walk(&mut self, path: &Path, depth: u32, ctx: &mut Context<'_>) {
        if ctx.is_cancelled() {
            return;
        }
        if depth > MAX_DEPTH {
            self.truncated = true;
            ctx.push_error(format!(
                "Stopped at {} because the folder nesting is deeper than {MAX_DEPTH} levels.",
                path.display()
            ));
            return;
        }
        let entries = match read_dir(path) {
            Ok(entries) => entries,
            Err(error) => {
                ctx.push_error(format!("{}: {error}", path.display()));
                return;
            }
        };

        for entry in entries {
            if ctx.is_cancelled() {
                return;
            }
            if entry.is_reparse {
                continue;
            }
            if !self.include_hidden && (entry.is_hidden || entry.is_system) {
                continue;
            }
            if self.is_excluded(&entry.path) {
                continue;
            }
            if entry.is_dir {
                self.walk(&entry.path, depth + 1, ctx);
                continue;
            }

            self.scanned_files += 1;
            ctx.emit("index", self.scanned_files, &entry.path, false);
            if entry.size == 0 || entry.size < self.min_size {
                continue;
            }
            if self.candidates.len() >= MAX_CANDIDATES {
                self.truncated = true;
                ctx.push_error(format!(
                    "Stopped after {MAX_CANDIDATES} candidate files; narrow the folder or raise the minimum size."
                ));
                return;
            }

            let hard_link = match entry.file_id {
                Some(id) => {
                    if self.seen_ids.contains_key(&id) {
                        true
                    } else {
                        self.seen_ids.insert(id, entry.path.clone());
                        false
                    }
                }
                None => false,
            };

            self.candidates.push(Candidate {
                path: entry.path,
                size: entry.size,
                allocated: entry.allocated,
                modified: entry.modified,
                file_id: entry.file_id,
                hard_link,
                digest: None,
            });
        }
    }

    fn is_excluded(&self, path: &Path) -> bool {
        if self.exclusions.is_empty() {
            return false;
        }
        let normalized = normalize_path(&path.to_string_lossy());
        self.exclusions.iter().any(|exclusion| {
            normalized == *exclusion
                || normalized
                    .strip_prefix(exclusion)
                    .is_some_and(|rest| rest.starts_with('\\'))
        })
    }
}

fn size_groups(candidates: Vec<Candidate>) -> Vec<Vec<Candidate>> {
    let mut by_size: HashMap<u64, Vec<Candidate>> = HashMap::new();
    for candidate in candidates {
        by_size.entry(candidate.size).or_default().push(candidate);
    }
    by_size
        .into_values()
        .filter(|group| group.len() > 1)
        .collect()
}

/// Hashes every group with either the head+tail fingerprint or the full file.
/// Returns false when the user cancelled.
fn hash_stage(groups: &mut [Vec<Candidate>], full: bool, ctx: &mut Context<'_>) -> bool {
    for group in groups.iter_mut() {
        if ctx.is_cancelled() {
            return false;
        }
        // A hard link shares bytes with its primary, so reuse that digest when
        // the primary appears in the same size bucket.
        let mut digests: HashMap<u64, [u8; 32]> = HashMap::new();
        for candidate in group.iter() {
            if let (Some(id), Some(digest)) = (candidate.file_id, candidate.digest) {
                digests.entry(id).or_insert(digest);
            }
        }

        for candidate in group.iter_mut() {
            if ctx.is_cancelled() {
                return false;
            }
            if candidate.digest.is_some() {
                continue;
            }
            if let Some(id) = candidate.file_id {
                if let Some(digest) = digests.get(&id) {
                    candidate.digest = Some(*digest);
                    continue;
                }
            }

            let before = file_signature(&candidate.path);
            let digest = if full {
                full_digest(&candidate.path, candidate.size, ctx, &candidate.path)
            } else {
                partial_digest(&candidate.path, candidate.size)
            };

            match digest {
                Ok(digest) => {
                    candidate.digest = Some(digest);
                    if let Some(id) = candidate.file_id {
                        digests.insert(id, digest);
                    }
                }
                Err(error) => {
                    ctx.push_error(format!("{}: {error}", candidate.path.display()));
                    continue;
                }
            }

            ctx.hashed_files += 1;
            if full {
                let after = file_signature(&candidate.path);
                if before.is_some() && after.is_some() && before != after {
                    ctx.push_error(format!(
                        "{} changed while it was being read; its group was skipped.",
                        candidate.path.display()
                    ));
                    candidate.digest = None;
                }
            }
            ctx.emit(
                if full { "full" } else { "partial" },
                ctx.hashed_files,
                &candidate.path,
                false,
            );
        }
    }
    true
}

fn regroup(groups: Vec<Vec<Candidate>>) -> Vec<Vec<Candidate>> {
    let mut result = Vec::new();
    for group in groups {
        let mut by_digest: HashMap<[u8; 32], Vec<Candidate>> = HashMap::new();
        for candidate in group {
            if let Some(digest) = candidate.digest {
                by_digest.entry(digest).or_default().push(candidate);
            }
        }
        result.extend(by_digest.into_values().filter(|group| group.len() > 1));
    }
    result
}

fn partial_digest(path: &Path, size: u64) -> std::io::Result<[u8; 32]> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let head = size.min(PARTIAL_BYTES);
    let mut buffer = vec![0u8; head as usize];
    file.read_exact(&mut buffer)?;
    hasher.update(&buffer);
    if size > PARTIAL_BYTES * 2 {
        file.seek(SeekFrom::Start(size - PARTIAL_BYTES))?;
        let mut tail = vec![0u8; PARTIAL_BYTES as usize];
        file.read_exact(&mut tail)?;
        hasher.update(&tail);
    }
    Ok(hasher.finalize().into())
}

fn full_digest(
    path: &Path,
    expected_size: u64,
    ctx: &mut Context<'_>,
    display_path: &Path,
) -> std::io::Result<[u8; 32]> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; HASH_BUFFER.min(expected_size.max(1) as usize)];
    let mut total: u64 = 0;
    loop {
        if ctx.is_cancelled() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "the scan was cancelled",
            ));
        }
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        total += read as u64;
        ctx.bytes_hashed = ctx.bytes_hashed.saturating_add(read as u64);
        ctx.emit("full", ctx.hashed_files, display_path, false);
    }
    if total != expected_size {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "the file size changed while it was being read",
        ));
    }
    Ok(hasher.finalize().into())
}

fn file_signature(path: &Path) -> Option<(u64, Option<u64>)> {
    let metadata = std::fs::metadata(path).ok()?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_secs());
    Some((metadata.len(), modified))
}

fn normalize_path(value: &str) -> String {
    let mut normalized = value.trim().replace('/', "\\").to_lowercase();
    while normalized.len() > 3 && normalized.ends_with('\\') {
        normalized.pop();
    }
    normalized
}

fn hex_prefix(digest: &[u8; 32]) -> String {
    digest[..6].iter().map(|byte| format!("{byte:02x}")).collect()
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u64::MAX as u128) as u64
}

/// Unique temporary directory for the tests; removed by the caller.
#[cfg(test)]
fn temp_dir(name: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!("cyron-dup-{name}-{stamp}"));
    std::fs::create_dir_all(&path).expect("temp dir");
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(path: &Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent");
        }
        fs::write(path, bytes).expect("write fixture");
    }

    fn scan(root: &Path, min_size: u64) -> DuplicateReport {
        let cancel = Arc::new(AtomicBool::new(false));
        find_duplicates(
            "test",
            root,
            &DuplicateOptions {
                min_size,
                include_hidden: true,
                exclusions: Vec::new(),
            },
            cancel,
            &|_| {},
        )
    }

    #[test]
    fn groups_identical_files() {
        let root = temp_dir("identical");
        let payload = vec![7u8; 64 * 1024];
        write(&root.join("a.bin"), &payload);
        write(&root.join("nested/b.bin"), &payload);
        write(&root.join("nested/c.bin"), &payload);
        write(&root.join("unique.bin"), &vec![9u8; 64 * 1024]);

        let report = scan(&root, 4096);
        assert_eq!(report.groups.len(), 1);
        assert_eq!(report.total_files, 3);
        assert_eq!(report.total_wasted, 2 * 64 * 1024);
        assert!(report.groups[0].files.iter().all(|file| !file.hard_link));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn same_size_different_content_is_not_a_group() {
        let root = temp_dir("different");
        write(&root.join("a.bin"), &vec![1u8; 32 * 1024]);
        write(&root.join("b.bin"), &vec![2u8; 32 * 1024]);
        let report = scan(&root, 4096);
        assert!(report.groups.is_empty(), "{:?}", report.groups);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn full_hash_separates_files_with_equal_edges() {
        let root = temp_dir("edges");
        let size = 64 * 1024;
        let mut first = vec![3u8; size];
        let mut second = first.clone();
        // Same first and last 4 KiB, different middle: only the full hash can tell.
        first[size / 2] = 11;
        second[size / 2] = 12;
        write(&root.join("a.bin"), &first);
        write(&root.join("b.bin"), &second);
        let report = scan(&root, 4096);
        assert!(report.groups.is_empty());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn minimum_size_filters_small_files() {
        let root = temp_dir("min-size");
        write(&root.join("a.bin"), &vec![5u8; 1024]);
        write(&root.join("b.bin"), &vec![5u8; 1024]);
        assert!(scan(&root, 4096).groups.is_empty());
        assert_eq!(scan(&root, 512).groups.len(), 1);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn hard_links_do_not_inflate_recoverable_space() {
        let root = temp_dir("hard-links");
        let payload = vec![4u8; 48 * 1024];
        write(&root.join("original.bin"), &payload);
        fs::hard_link(root.join("original.bin"), root.join("link.bin")).expect("hard link");
        let alone = scan(&root, 4096);
        assert!(alone.groups.is_empty(), "a hard link is not a duplicate");
        assert_eq!(alone.total_wasted, 0);

        write(&root.join("copy.bin"), &payload);
        let report = scan(&root, 4096);
        assert_eq!(report.groups.len(), 1);
        assert_eq!(report.total_wasted, 48 * 1024);
        assert_eq!(report.groups[0].files.len(), 3);
        assert_eq!(
            report.groups[0]
                .files
                .iter()
                .filter(|file| file.hard_link)
                .count(),
            1
        );
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn cancellation_stops_without_groups() {
        let root = temp_dir("cancel");
        write(&root.join("a.bin"), &vec![6u8; 8192]);
        write(&root.join("b.bin"), &vec![6u8; 8192]);
        let cancel = Arc::new(AtomicBool::new(true));
        let report = find_duplicates(
            "test",
            &root,
            &DuplicateOptions::default(),
            cancel,
            &|_| {},
        );
        assert!(report.cancelled);
        assert!(report.groups.is_empty());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn missing_root_reports_an_error_instead_of_panicking() {
        let missing = std::env::temp_dir().join("cyron-dup-does-not-exist-xyz");
        let report = scan(&missing, 4096);
        assert!(report.groups.is_empty());
        assert!(!report.errors.is_empty());
        assert_eq!(report.scanned_files, 0);
    }

    #[test]
    fn exclusions_are_skipped() {
        let root = temp_dir("excluded");
        write(&root.join("keep/a.bin"), &vec![8u8; 8192]);
        write(&root.join("keep/b.bin"), &vec![8u8; 8192]);
        write(&root.join("skip/c.bin"), &vec![8u8; 8192]);
        write(&root.join("skip/d.bin"), &vec![8u8; 8192]);
        let cancel = Arc::new(AtomicBool::new(false));
        let report = find_duplicates(
            "test",
            &root,
            &DuplicateOptions {
                min_size: 4096,
                include_hidden: true,
                exclusions: vec![root.join("skip").to_string_lossy().into_owned()],
            },
            cancel,
            &|_| {},
        );
        assert_eq!(report.groups.len(), 1);
        assert_eq!(report.total_files, 2);
        assert!(report.groups[0]
            .files
            .iter()
            .all(|file| !file.path.to_lowercase().contains("skip")));
        fs::remove_dir_all(&root).ok();
    }
}
