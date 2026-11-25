//! Native, read-only file index with Everything-compatible search.
//!
//! Cyron builds its own index from directory metadata (name, size, timestamps,
//! attributes) across fixed local volumes. It never reads file contents, never
//! follows reparse points, and never modifies the indexed filesystem. The index
//! lives in memory for the session, is persisted to the application data
//! directory for instant startup, and is kept current with
//! `ReadDirectoryChangesW` watchers.
//!
//! The query language follows Everything's conventions: whitespace is AND,
//! `|` is OR, `!` is NOT, quotes form a phrase, `*` and `?` are wildcards, and
//! field filters such as `ext:`, `size:`, `dm:`, `type:`, `path:` and
//! `attrib:` narrow the result. Search is read-only and local; no result path
//! is ever executed from this module.

use crate::fs;
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering as CmpOrdering;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::File;
use std::hash::{BuildHasherDefault, Hasher};
use std::io::{BufReader, BufWriter, Read, Write};
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const NONE: u32 = u32::MAX;
/// Hard cap on records so a hostile or unusual volume cannot exhaust memory.
pub const MAX_ENTRIES: usize = 3_000_000;
/// Largest accepted query; keeps parsing and matching bounded.
const MAX_QUERY_CHARS: usize = 512;
const MAX_QUERY_TERMS: usize = 32;
const MAX_VOLUME_RECORDS: usize = 64;
const MAX_POOL_BYTES: usize = 256 * 1024 * 1024;
const DEFAULT_LIMIT: u32 = 100;
const MAX_LIMIT: u32 = 500;
const MAX_PATH_DEPTH: usize = 64;
const MAX_WARNINGS: usize = 8;
const INDEX_FILE: &str = "search-index.bin";
const INDEX_MAGIC: &[u8; 8] = b"CYRSIDX1";
const INDEX_VERSION: u32 = 1;
const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
const FILE_ATTRIBUTE_ARCHIVE: u32 = 0x20;

const FLAG_DIR: u8 = 1;
const FLAG_DELETED: u8 = 2;
const FLAG_ROOT: u8 = 4;

/* ------------------------------------------------------------------ types */

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SearchQueryOptions {
    pub query: String,
    pub match_case: bool,
    pub whole_word: bool,
    pub match_path: bool,
    pub regex: bool,
    /// all, files, folders, audio, video, image, document, executable, compressed
    pub filter: String,
    /// name, path, size, modified, created, extension, type
    pub sort: String,
    pub descending: bool,
    pub offset: u32,
    pub limit: u32,
}

impl Default for SearchQueryOptions {
    fn default() -> Self {
        Self {
            query: String::new(),
            match_case: false,
            whole_word: false,
            match_path: false,
            regex: false,
            filter: "all".into(),
            sort: "name".into(),
            descending: false,
            offset: 0,
            limit: DEFAULT_LIMIT,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchIndexVolumeStatus {
    pub mount: String,
    pub label: String,
    /// queued, indexing, done, error
    pub state: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchIndexProgress {
    pub volume: String,
    pub entries: u64,
    pub current_dir: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchIndexStatus {
    /// empty, loading, building, ready, error
    pub state: String,
    pub building: bool,
    pub ready: bool,
    pub entries: u64,
    pub files: u64,
    pub folders: u64,
    pub total_size: u64,
    pub indexed_at: Option<u64>,
    pub build_ms: Option<u64>,
    pub volumes: Vec<SearchIndexVolumeStatus>,
    pub progress: Option<SearchIndexProgress>,
    pub warnings: Vec<String>,
    pub watching: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchIndexItem {
    pub name: String,
    pub parent_path: String,
    pub full_path: String,
    pub is_folder: bool,
    pub size: u64,
    pub modified: Option<u64>,
    pub created: Option<u64>,
    pub extension: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchIndexPage {
    pub items: Vec<SearchIndexItem>,
    pub total: u32,
    pub files: u32,
    pub folders: u32,
    pub total_size: u64,
    pub offset: u32,
    pub limit: u32,
    pub has_more: bool,
    pub elapsed_ms: u64,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchExportReport {
    pub path: String,
    pub rows: u64,
    pub skipped: u64,
    pub truncated: bool,
}

/* ------------------------------------------------------------------ index */

#[derive(Debug, Clone)]
struct Meta {
    is_dir: bool,
    size: u64,
    modified: u32,
    created: u32,
    accessed: u32,
    attrs: u32,
}

#[derive(Clone, Copy)]
struct Entry {
    name_off: u32,
    name_len: u16,
    lower_off: u32,
    lower_len: u16,
    parent: u32,
    size: u64,
    modified: u32,
    created: u32,
    accessed: u32,
    attrs: u32,
    flags: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VolumeRecord {
    mount: String,
    label: String,
}

type PathMap = HashMap<u64, u32, BuildHasherDefault<U64Hasher>>;

/// The path key is already a 64-bit hash, so re-hashing it with SipHash only
/// burns CPU. This hasher returns the key unchanged.
#[derive(Default)]
struct U64Hasher(u64);

impl Hasher for U64Hasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for byte in bytes {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        self.0 = hash;
    }

    fn write_u64(&mut self, value: u64) {
        self.0 = value;
    }
}

struct FileIndex {
    names: String,
    lower: String,
    entries: Vec<Entry>,
    first_child: Vec<u32>,
    next_sibling: Vec<u32>,
    /// Directory paths only. File lookups resolve their parent directory and
    /// then scan its children, which keeps the map small on multi-million-file
    /// volumes while preserving watcher rename/add/remove correctness.
    paths: PathMap,
    /// Entry ids sorted by lowercase name, rebuilt after a build or load and
    /// cleared by watcher mutations. Name-sorted queries walk this order and
    /// stop after the requested page instead of sorting every match.
    name_order: Vec<u32>,
    volumes: Vec<VolumeRecord>,
    files: u64,
    folders: u64,
    total_size: u64,
    deleted: u64,
    indexed_at: u64,
    build_ms: u64,
    complete: bool,
    truncated: bool,
    warnings: Vec<String>,
}

impl FileIndex {
    fn new() -> Self {
        Self {
            names: String::new(),
            lower: String::new(),
            entries: Vec::new(),
            first_child: Vec::new(),
            next_sibling: Vec::new(),
            paths: PathMap::default(),
            name_order: Vec::new(),
            volumes: Vec::new(),
            files: 0,
            folders: 0,
            total_size: 0,
            deleted: 0,
            indexed_at: 0,
            build_ms: 0,
            complete: false,
            truncated: false,
            warnings: Vec::new(),
        }
    }

    fn count(&self) -> u64 {
        self.entries.len() as u64 - self.deleted
    }

    fn is_empty(&self) -> bool {
        self.count() == 0
    }

    fn entry(&self, id: u32) -> &Entry {
        &self.entries[id as usize]
    }

    fn name_of(&self, entry: &Entry) -> &str {
        let start = entry.name_off as usize;
        let end = start + entry.name_len as usize;
        &self.names[start..end]
    }

    fn lower_of(&self, entry: &Entry) -> &str {
        let start = entry.lower_off as usize;
        let end = start + entry.lower_len as usize;
        &self.lower[start..end]
    }

    fn name(&self, id: u32) -> &str {
        self.name_of(self.entry(id))
    }

    fn lower_name(&self, id: u32) -> &str {
        self.lower_of(self.entry(id))
    }

    fn is_dir(&self, id: u32) -> bool {
        self.entry(id).flags & FLAG_DIR != 0
    }

    fn is_deleted(&self, id: u32) -> bool {
        self.entry(id).flags & FLAG_DELETED != 0
    }

    fn extension(&self, id: u32) -> &str {
        extension_of(self.name(id))
    }

    fn add_root(&mut self, mount: &str, label: &str) -> u32 {
        let lower = mount.to_lowercase();
        if let Some(id) = self.find_dir_path(&lower) {
            return id;
        }
        let meta = Meta {
            is_dir: true,
            size: 0,
            modified: 0,
            created: 0,
            accessed: 0,
            attrs: FILE_ATTRIBUTE_DIRECTORY,
        };
        self.volumes.push(VolumeRecord {
            mount: mount.to_string(),
            label: label.to_string(),
        });
        match self.add(mount, NONE, &meta) {
            Some(id) => {
                self.entries[id as usize].flags |= FLAG_ROOT;
                self.paths.insert(path_hash(&lower), id);
                id
            }
            None => NONE,
        }
    }

    fn add(&mut self, name: &str, parent: u32, meta: &Meta) -> Option<u32> {
        if self.entries.len() >= MAX_ENTRIES || name.is_empty() {
            return None;
        }
        self.name_order.clear();
        if name.len() > u16::MAX as usize {
            return None;
        }
        // Keep both persisted pools inside the loader's sanity bound. A volume
        // with unusually long names stops cleanly instead of producing a cache
        // this build cannot read back.
        if self.names.len() + name.len() > MAX_POOL_BYTES {
            return None;
        }
        let name_off = self.names.len() as u32;
        self.names.push_str(name);
        let lower_off = self.lower.len() as u32;
        append_lower(&mut self.lower, name);
        let lower_len = self.lower.len() - lower_off as usize;
        if lower_len > u16::MAX as usize || self.lower.len() > MAX_POOL_BYTES {
            self.names.truncate(name_off as usize);
            self.lower.truncate(lower_off as usize);
            return None;
        }

        let id = self.entries.len() as u32;
        let mut flags = 0u8;
        if meta.is_dir {
            flags |= FLAG_DIR;
        }
        self.entries.push(Entry {
            name_off,
            name_len: name.len() as u16,
            lower_off,
            lower_len: lower_len as u16,
            parent,
            size: if meta.is_dir { 0 } else { meta.size },
            modified: meta.modified,
            created: meta.created,
            accessed: meta.accessed,
            attrs: meta.attrs,
            flags,
        });
        self.first_child.push(NONE);
        self.next_sibling.push(if parent == NONE {
            NONE
        } else {
            self.first_child[parent as usize]
        });
        if parent != NONE {
            self.first_child[parent as usize] = id;
        }
        if meta.is_dir {
            self.folders += 1;
        } else {
            self.files += 1;
            self.total_size = self.total_size.saturating_add(meta.size);
        }
        Some(id)
    }

    fn path_of(&self, mut id: u32) -> String {
        let mut parts: Vec<&str> = Vec::new();
        let mut depth = 0usize;
        loop {
            let entry = self.entry(id);
            parts.push(self.name_of(entry));
            if entry.parent == NONE || depth >= MAX_PATH_DEPTH {
                break;
            }
            id = entry.parent;
            depth += 1;
        }
        parts.reverse();
        let mut out = String::with_capacity(parts.iter().map(|part| part.len() + 1).sum());
        for part in parts {
            append_part(&mut out, part);
        }
        out
    }

    fn path_of_cached(&self, id: u32, cache: &mut HashMap<u32, String>) -> String {
        let entry = self.entry(id);
        let mut out = if entry.parent == NONE {
            String::new()
        } else if let Some(parent) = cache.get(&entry.parent) {
            parent.clone()
        } else {
            self.path_of_cached(entry.parent, cache)
        };
        append_part(&mut out, self.name_of(entry));
        if entry.flags & FLAG_DIR != 0 && entry.parent != NONE {
            cache.insert(id, out.clone());
        }
        out
    }

    fn parent_path(&self, id: u32) -> String {
        let full = self.path_of(id);
        let name = self.name(id);
        let Some(base) = full.strip_suffix(name) else {
            return String::new();
        };
        if base.len() == 3 && base.as_bytes().get(1) == Some(&b':') {
            return base.to_string();
        }
        base.trim_end_matches(['\\', '/']).to_string()
    }

    fn find_dir_path(&self, lower: &str) -> Option<u32> {
        let id = *self.paths.get(&path_hash(lower))?;
        if id as usize >= self.entries.len() || self.is_deleted(id) {
            return None;
        }
        (self.path_of(id).to_lowercase() == lower).then_some(id)
    }

    fn find_child(&self, parent: u32, lower_name: &str) -> Option<u32> {
        let mut child = self.first_child[parent as usize];
        while child != NONE {
            if !self.is_deleted(child) && self.lower_name(child) == lower_name {
                return Some(child);
            }
            child = self.next_sibling[child as usize];
        }
        None
    }

    /// Resolves a full path. Directories hit the map; files resolve their
    /// parent directory and then scan that directory's children. This keeps the
    /// persisted map small while watcher events stay exact.
    fn find_path(&self, lower: &str) -> Option<u32> {
        if let Some(id) = self.find_dir_path(lower) {
            return Some(id);
        }
        let split = lower.rfind(|character| character == '\\' || character == '/')?;
        let parent = &lower[..split];
        let name = &lower[split + 1..];
        if name.is_empty() {
            return None;
        }
        let parent_id = if parent.len() == 2 && parent.as_bytes()[1] == b':' {
            self.find_dir_path(&format!("{parent}\\"))?
        } else {
            self.find_dir_path(parent)?
        };
        self.find_child(parent_id, name)
    }

    fn tombstone(&mut self, id: u32) {
        self.name_order.clear();
        let mut stack = vec![id];
        while let Some(current) = stack.pop() {
            let entry = self.entry(current);
            if entry.flags & FLAG_DELETED != 0 {
                continue;
            }
            let entry = self.entries[current as usize];
            if entry.flags & FLAG_DIR != 0 {
                let path = self.path_of(current).to_lowercase();
                self.paths.remove(&path_hash(&path));
                self.folders = self.folders.saturating_sub(1);
            } else {
                self.files = self.files.saturating_sub(1);
                self.total_size = self.total_size.saturating_sub(entry.size);
            }
            self.deleted += 1;
            self.entries[current as usize].flags |= FLAG_DELETED;
            let mut child = self.first_child[current as usize];
            while child != NONE {
                stack.push(child);
                child = self.next_sibling[child as usize];
            }
        }
    }

    /// Updates an entry's name and parent in place. Children keep pointing at
    /// this entry, so a moved directory carries its subtree with it.
    fn rename(&mut self, id: u32, name: &str, new_parent: u32) {
        if name.is_empty() || name.len() > u16::MAX as usize {
            return;
        }
        self.name_order.clear();
        let is_dir = self.is_dir(id);
        let old_path = if is_dir {
            Some(self.path_of(id).to_lowercase())
        } else {
            None
        };
        let lower_start = self.lower.len();
        append_lower(&mut self.lower, name);
        let lower_len = self.lower.len() - lower_start;
        if lower_len > u16::MAX as usize || self.lower.len() > MAX_POOL_BYTES {
            self.lower.truncate(lower_start);
            return;
        }
        if let Some(old_path) = &old_path {
            self.paths.remove(&path_hash(old_path));
        }

        let old_parent = self.entries[id as usize].parent;
        if old_parent != NONE {
            let mut current = self.first_child[old_parent as usize];
            let mut previous = NONE;
            while current != NONE {
                if current == id {
                    let next = self.next_sibling[current as usize];
                    if previous == NONE {
                        self.first_child[old_parent as usize] = next;
                    } else {
                        self.next_sibling[previous as usize] = next;
                    }
                    break;
                }
                previous = current;
                current = self.next_sibling[current as usize];
            }
        }

        let name_off = self.names.len() as u32;
        self.names.push_str(name);
        let lower_off = lower_start as u32;
        let entry = &mut self.entries[id as usize];
        entry.name_off = name_off;
        entry.name_len = name.len() as u16;
        entry.lower_off = lower_off;
        entry.lower_len = lower_len as u16;
        entry.parent = new_parent;

        if new_parent != NONE {
            self.next_sibling[id as usize] = self.first_child[new_parent as usize];
            self.first_child[new_parent as usize] = id;
        } else {
            self.next_sibling[id as usize] = NONE;
        }

        if is_dir {
            let prefix = self.path_of(id);
            self.paths.insert(path_hash(&prefix.to_lowercase()), id);
            self.reseed_paths(id, &prefix);
        }
    }

    fn reseed_paths(&mut self, dir: u32, prefix: &str) {
        let mut child = self.first_child[dir as usize];
        while child != NONE {
            if self.is_dir(child) {
                let name = self.name(child).to_string();
                let mut path = prefix.to_string();
                append_part(&mut path, &name);
                self.paths.insert(path_hash(&path.to_lowercase()), child);
                self.reseed_paths(child, &path);
            }
            child = self.next_sibling[child as usize];
        }
    }

    fn sort_name_order(&mut self) {
        let mut order: Vec<u32> = (0..self.entries.len() as u32)
            .filter(|id| !self.is_deleted(*id))
            .collect();
        order.sort_unstable_by(|left, right| {
            self.lower_name(*left)
                .cmp(self.lower_name(*right))
                .then(left.cmp(right))
        });
        self.name_order = order;
    }

    fn seed_paths(&mut self) {
        self.paths.clear();
        self.paths.reserve(self.folders as usize + self.volumes.len());
        let roots: Vec<u32> = (0..self.entries.len() as u32)
            .filter(|id| self.entry(*id).parent == NONE && !self.is_deleted(*id))
            .collect();
        for root in roots {
            let mut prefix = self.name(root).to_string();
            self.paths.insert(path_hash(&prefix.to_lowercase()), root);
            self.seed_children(root, &mut prefix);
        }
    }

    fn seed_children(&mut self, dir: u32, prefix: &mut String) {
        let mut child = self.first_child[dir as usize];
        while child != NONE {
            if self.is_dir(child) {
                let name = self.name(child).to_string();
                let before = prefix.len();
                append_part(prefix, &name);
                self.paths.insert(path_hash(&prefix.to_lowercase()), child);
                self.seed_children(child, prefix);
                prefix.truncate(before);
            }
            child = self.next_sibling[child as usize];
        }
    }

    fn save(&self, path: &Path) -> Result<(), String> {
        let temp = path.with_file_name(format!("{INDEX_FILE}.tmp"));
        let file = File::create(&temp)
            .map_err(|error| format!("The search index could not be written: {error}"))?;
        let mut writer = BufWriter::with_capacity(1024 * 1024, file);
        writer
            .write_all(INDEX_MAGIC)
            .map_err(|error| format!("The search index could not be written: {error}"))?;
        write_u32(&mut writer, INDEX_VERSION)?;
        let mut flags = 0u32;
        if self.complete {
            flags |= 1;
        }
        if self.truncated {
            flags |= 2;
        }
        write_u32(&mut writer, flags)?;
        write_u64(&mut writer, self.indexed_at)?;
        write_u64(&mut writer, self.build_ms)?;
        write_u32(&mut writer, self.entries.len() as u32)?;
        write_u32(&mut writer, self.volumes.len() as u32)?;
        write_u32(&mut writer, self.names.len() as u32)?;
        write_u32(&mut writer, self.lower.len() as u32)?;
        write_u32(&mut writer, 0)?;
        for volume in &self.volumes {
            write_string16(&mut writer, &volume.mount)?;
            write_string16(&mut writer, &volume.label)?;
        }
        writer
            .write_all(self.names.as_bytes())
            .map_err(|error| format!("The search index could not be written: {error}"))?;
        writer
            .write_all(self.lower.as_bytes())
            .map_err(|error| format!("The search index could not be written: {error}"))?;
        for entry in &self.entries {
            write_u32(&mut writer, entry.name_off)?;
            write_u16(&mut writer, entry.name_len)?;
            write_u32(&mut writer, entry.lower_off)?;
            write_u16(&mut writer, entry.lower_len)?;
            write_u32(&mut writer, entry.parent)?;
            write_u64(&mut writer, entry.size)?;
            write_u32(&mut writer, entry.modified)?;
            write_u32(&mut writer, entry.created)?;
            write_u32(&mut writer, entry.accessed)?;
            write_u32(&mut writer, entry.attrs)?;
            writer
                .write_all(&[entry.flags])
                .map_err(|error| format!("The search index could not be written: {error}"))?;
        }
        writer
            .flush()
            .map_err(|error| format!("The search index could not be written: {error}"))?;
        let file = writer
            .into_inner()
            .map_err(|error| format!("The search index could not be flushed: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("The search index could not be synced: {error}"))?;
        drop(file);
        std::fs::rename(&temp, path)
            .map_err(|error| format!("The search index could not be replaced: {error}"))
    }
}

fn append_part(out: &mut String, part: &str) {
    if !out.is_empty() && !out.ends_with(['\\', '/']) {
        out.push('\\');
    }
    out.push_str(part);
}

/// Appends the lowercase form without allocating an intermediate String. Most
/// Windows names are ASCII, so the common path is a byte-wise lowercase copy;
/// Unicode names fall back to the standard case mapping.
fn append_lower(out: &mut String, name: &str) {
    if name.is_ascii() {
        out.reserve(name.len());
        for byte in name.bytes() {
            out.push(byte.to_ascii_lowercase() as char);
        }
    } else {
        for character in name.chars() {
            for lower in character.to_lowercase() {
                out.push(lower);
            }
        }
    }
}

fn extension_of(name: &str) -> &str {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() => ext,
        _ => "",
    }
}

fn path_hash(value: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in value.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

fn filetime_to_unix(ticks: u64) -> u32 {
    const EPOCH: u64 = 11_644_473_600 * 10_000_000;
    if ticks < EPOCH {
        return 0;
    }
    ((ticks - EPOCH) / 10_000_000).min(u32::MAX as u64) as u32
}

/* -------------------------------------------------------------- persistence */

fn write_u16(writer: &mut impl Write, value: u16) -> Result<(), String> {
    writer
        .write_all(&value.to_le_bytes())
        .map_err(|error| format!("The search index could not be written: {error}"))
}

fn write_u32(writer: &mut impl Write, value: u32) -> Result<(), String> {
    writer
        .write_all(&value.to_le_bytes())
        .map_err(|error| format!("The search index could not be written: {error}"))
}

fn write_u64(writer: &mut impl Write, value: u64) -> Result<(), String> {
    writer
        .write_all(&value.to_le_bytes())
        .map_err(|error| format!("The search index could not be written: {error}"))
}

fn write_string16(writer: &mut impl Write, value: &str) -> Result<(), String> {
    let bytes = value.as_bytes();
    if bytes.len() > u16::MAX as usize {
        return Err("A volume name is too long for the search index.".into());
    }
    write_u16(writer, bytes.len() as u16)?;
    writer
        .write_all(bytes)
        .map_err(|error| format!("The search index could not be written: {error}"))
}

fn read_u16(reader: &mut impl Read) -> Result<u16, String> {
    let mut bytes = [0u8; 2];
    reader
        .read_exact(&mut bytes)
        .map_err(|_| "The saved search index is truncated.".to_string())?;
    Ok(u16::from_le_bytes(bytes))
}

fn read_u32(reader: &mut impl Read) -> Result<u32, String> {
    let mut bytes = [0u8; 4];
    reader
        .read_exact(&mut bytes)
        .map_err(|_| "The saved search index is truncated.".to_string())?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64(reader: &mut impl Read) -> Result<u64, String> {
    let mut bytes = [0u8; 8];
    reader
        .read_exact(&mut bytes)
        .map_err(|_| "The saved search index is truncated.".to_string())?;
    Ok(u64::from_le_bytes(bytes))
}

fn read_string16(reader: &mut impl Read) -> Result<String, String> {
    let length = read_u16(reader)? as usize;
    if length > 4096 {
        return Err("The saved search index has an oversized volume name.".into());
    }
    let mut bytes = vec![0u8; length];
    reader
        .read_exact(&mut bytes)
        .map_err(|_| "The saved search index is truncated.".to_string())?;
    String::from_utf8(bytes).map_err(|_| "The saved search index has invalid text.".into())
}

fn load_index(path: &Path) -> Result<FileIndex, String> {
    let file =
        File::open(path).map_err(|error| format!("The saved index is unavailable: {error}"))?;
    let mut reader = BufReader::with_capacity(1024 * 1024, file);
    let mut magic = [0u8; 8];
    reader
        .read_exact(&mut magic)
        .map_err(|_| "The saved search index is truncated.".to_string())?;
    if &magic != INDEX_MAGIC {
        return Err("The saved search index was not written by this version of Cyron.".into());
    }
    let version = read_u32(&mut reader)?;
    if version != INDEX_VERSION {
        return Err("The saved search index uses an unsupported format.".into());
    }
    let flags = read_u32(&mut reader)?;
    let indexed_at = read_u64(&mut reader)?;
    let build_ms = read_u64(&mut reader)?;
    let entry_count = read_u32(&mut reader)? as usize;
    let volume_count = read_u32(&mut reader)? as usize;
    let names_len = read_u32(&mut reader)? as usize;
    let lower_len = read_u32(&mut reader)? as usize;
    let _reserved = read_u32(&mut reader)?;
    if entry_count > MAX_ENTRIES {
        return Err("The saved search index is larger than this version supports.".into());
    }
    if volume_count > MAX_VOLUME_RECORDS {
        return Err("The saved search index has too many volumes.".into());
    }
    if names_len > MAX_POOL_BYTES || lower_len > MAX_POOL_BYTES {
        return Err("The saved search index has an oversized name table.".into());
    }

    let mut volumes = Vec::with_capacity(volume_count);
    for _ in 0..volume_count {
        volumes.push(VolumeRecord {
            mount: read_string16(&mut reader)?,
            label: read_string16(&mut reader)?,
        });
    }
    let mut names = vec![0u8; names_len];
    reader
        .read_exact(&mut names)
        .map_err(|_| "The saved search index is truncated.".to_string())?;
    let names = String::from_utf8(names)
        .map_err(|_| "The saved search index has invalid file names.".to_string())?;
    let mut lower = vec![0u8; lower_len];
    reader
        .read_exact(&mut lower)
        .map_err(|_| "The saved search index is truncated.".to_string())?;
    let lower = String::from_utf8(lower)
        .map_err(|_| "The saved search index has invalid file names.".to_string())?;

    let mut entries = Vec::with_capacity(entry_count);
    for _ in 0..entry_count {
        let name_off = read_u32(&mut reader)?;
        let name_len = read_u16(&mut reader)?;
        let lower_off = read_u32(&mut reader)?;
        let lower_len = read_u16(&mut reader)?;
        let parent = read_u32(&mut reader)?;
        let size = read_u64(&mut reader)?;
        let modified = read_u32(&mut reader)?;
        let created = read_u32(&mut reader)?;
        let accessed = read_u32(&mut reader)?;
        let attrs = read_u32(&mut reader)?;
        let mut flag = [0u8; 1];
        reader
            .read_exact(&mut flag)
            .map_err(|_| "The saved search index is truncated.".to_string())?;
        let name_end = name_off as usize + name_len as usize;
        let lower_end = lower_off as usize + lower_len as usize;
        if name_end > names.len()
            || lower_end > lower.len()
            || !names.is_char_boundary(name_off as usize)
            || !names.is_char_boundary(name_end)
            || !lower.is_char_boundary(lower_off as usize)
            || !lower.is_char_boundary(lower_end)
        {
            return Err("The saved search index has invalid name offsets.".into());
        }
        if parent != NONE && parent as usize >= entry_count {
            return Err("The saved search index has an invalid parent link.".into());
        }
        entries.push(Entry {
            name_off,
            name_len,
            lower_off,
            lower_len,
            parent,
            size,
            modified,
            created,
            accessed,
            attrs,
            flags: flag[0],
        });
    }

    let mut index = FileIndex {
        names,
        lower,
        entries,
        first_child: vec![NONE; entry_count],
        next_sibling: vec![NONE; entry_count],
        paths: PathMap::default(),
        name_order: Vec::new(),
        volumes,
        files: 0,
        folders: 0,
        total_size: 0,
        deleted: 0,
        indexed_at,
        build_ms,
        complete: flags & 1 != 0,
        truncated: flags & 2 != 0,
        warnings: Vec::new(),
    };

    for id in 0..entry_count {
        let parent = index.entries[id].parent;
        if parent == NONE {
            continue;
        }
        index.next_sibling[id] = index.first_child[parent as usize];
        index.first_child[parent as usize] = id as u32;
    }

    for id in 0..entry_count {
        let entry = index.entries[id];
        if entry.flags & FLAG_DELETED != 0 {
            index.deleted += 1;
        } else if entry.flags & FLAG_DIR != 0 {
            index.folders += 1;
        } else {
            index.files += 1;
            index.total_size = index.total_size.saturating_add(entry.size);
        }
    }
    index.seed_paths();
    index.sort_name_order();
    Ok(index)
}

/* ----------------------------------------------------------------- runtime */

#[derive(Default)]
struct Runtime {
    loading: bool,
    building: bool,
    message: String,
    error: Option<String>,
    progress: Option<SearchIndexProgress>,
    volumes: Vec<SearchIndexVolumeStatus>,
    warnings: Vec<String>,
}

pub struct SearchState {
    index: RwLock<FileIndex>,
    runtime: Mutex<Runtime>,
    cancel: AtomicBool,
    building: AtomicBool,
    load_started: AtomicBool,
    watched: Mutex<HashSet<String>>,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            index: RwLock::new(FileIndex::new()),
            runtime: Mutex::new(Runtime::default()),
            cancel: AtomicBool::new(false),
            building: AtomicBool::new(false),
            load_started: AtomicBool::new(false),
            watched: Mutex::new(HashSet::new()),
        }
    }
}

impl SearchState {
    pub fn status(&self) -> SearchIndexStatus {
        let runtime = self
            .runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let index = self
            .index
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let entries = index.count();
        let state = if runtime.building {
            "building"
        } else if runtime.loading {
            "loading"
        } else if entries == 0 {
            if runtime.error.is_some() {
                "error"
            } else {
                "empty"
            }
        } else {
            "ready"
        };
        let watching = !self
            .watched
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_empty();
        SearchIndexStatus {
            state: state.to_string(),
            building: runtime.building,
            ready: entries > 0 && !runtime.loading,
            entries,
            files: index.files,
            folders: index.folders,
            total_size: index.total_size,
            indexed_at: (index.indexed_at > 0).then_some(index.indexed_at),
            build_ms: (index.build_ms > 0).then_some(index.build_ms),
            volumes: if runtime.volumes.is_empty() {
                index
                    .volumes
                    .iter()
                    .map(|volume| SearchIndexVolumeStatus {
                        mount: volume.mount.clone(),
                        label: volume.label.clone(),
                        state: "done".into(),
                        error: None,
                    })
                    .collect()
            } else {
                runtime.volumes.clone()
            },
            progress: runtime.progress.clone(),
            warnings: merged_warnings(&runtime, &index),
            watching,
            message: runtime.message.clone(),
        }
    }

    pub fn start_load(self: &Arc<Self>, data_dir: PathBuf) {
        if self.load_started.swap(true, Ordering::SeqCst) {
            return;
        }
        {
            let mut runtime = self
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            runtime.loading = true;
            runtime.message = "Loading the saved search index…".into();
        }
        let state = Arc::clone(self);
        std::thread::spawn(move || {
            let loaded = load_index(&data_dir.join(INDEX_FILE));
            let mut runtime = state
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            runtime.loading = false;
            match loaded {
                Ok(index) if !index.is_empty() && !runtime.building => {
                    runtime.volumes = index
                        .volumes
                        .iter()
                        .map(|volume| SearchIndexVolumeStatus {
                            mount: volume.mount.clone(),
                            label: volume.label.clone(),
                            state: "done".into(),
                            error: None,
                        })
                        .collect();
                    runtime.message = if index.complete {
                        "Index ready. Watching for file changes.".into()
                    } else {
                        "The saved index is incomplete; rebuild it to cover every file.".into()
                    };
                    runtime.error = None;
                    drop(runtime);
                    *state
                        .index
                        .write()
                        .unwrap_or_else(|poisoned| poisoned.into_inner()) = index;
                    state.start_watchers();
                }
                Ok(_) => {
                    if !runtime.building {
                        runtime.message = "No local index yet. Build it to search this PC.".into();
                    }
                }
                Err(_) => {
                    if !runtime.building {
                        runtime.message =
                            "No readable saved index. Build a new index to search this PC.".into();
                    }
                }
            }
        });
    }

    pub fn start_build(self: &Arc<Self>, data_dir: PathBuf) -> Result<(), String> {
        {
            let runtime = self
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if runtime.building || self.building.load(Ordering::SeqCst) {
                return Err("A search index build is already running.".into());
            }
        }
        if self.building.swap(true, Ordering::SeqCst) {
            return Err("A search index build is already running.".into());
        }
        self.cancel.store(false, Ordering::SeqCst);
        {
            let mut runtime = self
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            runtime.building = true;
            runtime.error = None;
            runtime.warnings.clear();
            runtime.progress = Some(SearchIndexProgress {
                volume: String::new(),
                entries: 0,
                current_dir: String::new(),
            });
            runtime.message = "Preparing the index…".into();
        }
        let state = Arc::clone(self);
        std::thread::spawn(move || state.run_build(data_dir));
        Ok(())
    }

    pub fn cancel_build(&self) {
        if self.building.load(Ordering::SeqCst) {
            self.cancel.store(true, Ordering::SeqCst);
            let mut runtime = self
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            runtime.message = "Stopping the index build…".into();
        }
    }

    pub fn query(&self, options: &SearchQueryOptions) -> Result<SearchIndexPage, String> {
        let index = self
            .index
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if index.is_empty() {
            return Err("The search index is empty. Build the index first.".into());
        }
        query_index(&index, options)
    }

    pub fn export(
        &self,
        options: &SearchQueryOptions,
        format: &str,
        destination: &str,
        selected: Option<&[String]>,
    ) -> Result<SearchExportReport, String> {
        let index = self
            .index
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if index.is_empty() {
            return Err("The search index is empty. Build the index first.".into());
        }
        export_index(&index, options, format, destination, selected)
    }

    fn run_build(self: Arc<Self>, data_dir: PathBuf) {
        let definitions: Vec<(String, String)> = crate::volumes::list_volumes()
            .into_iter()
            .filter(|volume| volume.kind == "fixed")
            .map(|volume| (volume.mount, volume.label))
            .collect();
        if definitions.is_empty() {
            let mut runtime = self
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            runtime.building = false;
            runtime.progress = None;
            runtime.error = Some("No fixed local drive was available to index.".into());
            runtime.message = "No fixed local drive was available to index.".into();
            self.building.store(false, Ordering::SeqCst);
            return;
        }
        {
            let mut runtime = self
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            runtime.volumes = definitions
                .iter()
                .map(|(mount, label)| SearchIndexVolumeStatus {
                    mount: mount.clone(),
                    label: label.clone(),
                    state: "queued".into(),
                    error: None,
                })
                .collect();
            runtime.message = "Indexing local drives…".into();
        }

        let started = Instant::now();
        let state = Arc::clone(&self);
        let mut progress = move |mount: &str, entries: u64, current: &Path| {
            let mut runtime = state
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            for volume in &mut runtime.volumes {
                if volume.mount.eq_ignore_ascii_case(mount) {
                    volume.state = "indexing".into();
                } else if volume.state == "indexing" {
                    volume.state = "done".into();
                }
            }
            runtime.progress = Some(SearchIndexProgress {
                volume: mount.to_string(),
                entries,
                current_dir: current.to_string_lossy().into_owned(),
            });
            runtime.message = format!(
                "Indexing {mount} — {} entries so far. Searches use the previous index until this build finishes.",
                format_number(entries)
            );
        };

        let mut warnings = Vec::new();
        let mut index = build_index(&definitions, &self.cancel, &mut progress, &mut warnings);
        let cancelled = self.cancel.load(Ordering::SeqCst);
        index.complete = !cancelled && !index.truncated;
        index.indexed_at = now_unix();
        index.build_ms = started.elapsed().as_millis() as u64;
        if index.truncated {
            warnings.push(format!(
                "The index stopped at its {} entry limit; some files are not indexed.",
                format_number(MAX_ENTRIES as u64)
            ));
        }
        if cancelled && !index.is_empty() {
            warnings.push(
                "The build was stopped before it finished; rebuild to cover every file.".into(),
            );
        }
        if !warnings.is_empty() {
            warnings = dedup_warnings(warnings);
        }
        index.warnings = warnings.clone();

        let mut save_error = None;
        if index.files > 0 {
            if let Err(error) = std::fs::create_dir_all(&data_dir) {
                save_error = Some(format!("The index folder could not be created: {error}"));
            } else if let Err(error) = index.save(&data_dir.join(INDEX_FILE)) {
                save_error = Some(error);
            }
        }

        // A build that produced only volume roots could not read any file. Keep
        // the previous index (if any) and report the failure instead of calling
        // an empty index ready.
        let empty = index.files == 0;
        let files = index.files;
        let folders = index.folders;
        let entries = index.count();
        let indexed_at = index.indexed_at;

        let mut runtime = self
            .runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        runtime.building = false;
        runtime.progress = None;
        if let Some(error) = save_error {
            warnings.push(error);
        }
        runtime.warnings = warnings;
        if empty {
            runtime.error =
                Some("The index found no files. Check the drive list and try again.".into());
            runtime.message = "The index found no files. No files were changed.".into();
            runtime.volumes.clear();
            self.building.store(false, Ordering::SeqCst);
            return;
        }
        for volume in &mut runtime.volumes {
            volume.state = "done".into();
        }
        runtime.error = None;
        runtime.message = if cancelled {
            format!(
                "Index stopped after {} entries. Rebuild to complete it.",
                format_number(entries)
            )
        } else {
            format!(
                "Index ready: {} files and {} folders ({}) across {} drives.",
                format_number(files),
                format_number(folders),
                format_size(index.total_size),
                runtime.volumes.len()
            )
        };
        if indexed_at == 0 {
            runtime.error = None;
        }
        drop(runtime);
        *self
            .index
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = index;
        self.building.store(false, Ordering::SeqCst);
        self.start_watchers();
    }

    fn start_watchers(self: &Arc<Self>) {
        let mounts: Vec<String> = self
            .index
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .volumes
            .iter()
            .map(|volume| volume.mount.clone())
            .collect();
        for mount in mounts {
            {
                let mut watched = self
                    .watched
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if !watched.insert(mount.clone()) {
                    continue;
                }
            }
            let state = Arc::clone(self);
            std::thread::spawn(move || watch_volume(state, mount));
        }
    }
}

fn merged_warnings(runtime: &Runtime, index: &FileIndex) -> Vec<String> {
    let mut warnings = runtime.warnings.clone();
    for warning in &index.warnings {
        if !warnings.iter().any(|value| value == warning) {
            warnings.push(warning.clone());
        }
    }
    warnings.truncate(MAX_WARNINGS);
    warnings
}

fn dedup_warnings(warnings: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for warning in warnings {
        if seen.insert(warning.clone()) {
            result.push(warning);
        }
        if result.len() >= MAX_WARNINGS {
            break;
        }
    }
    result
}

fn format_number(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, character) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(character);
    }
    out
}

fn format_size(value: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = value as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{value} B")
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

/* ------------------------------------------------------------------- build */

struct BuildJob {
    path: PathBuf,
    dir_id: u32,
    dir_lower: String,
    volume: String,
}

struct BuildQueue {
    jobs: VecDeque<BuildJob>,
    pending: usize,
    stopped: bool,
}

struct BuildShared {
    queue: Mutex<BuildQueue>,
    signal: Condvar,
    index: Mutex<FileIndex>,
    entries: AtomicU64,
    current: Mutex<String>,
    volume: Mutex<String>,
    warnings: Mutex<Vec<String>>,
    truncated: AtomicBool,
}

/// Enumerates directories on a few workers while inserting each directory's
/// children under one short index lock. Four workers is enough to keep an SSD
/// busy without turning a background index build into a CPU or I/O hog.
fn build_index(
    definitions: &[(String, String)],
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(&str, u64, &Path),
    warnings: &mut Vec<String>,
) -> FileIndex {
    if cancel.load(Ordering::SeqCst) {
        return FileIndex::new();
    }

    let mut index = FileIndex::new();
    let mut root_jobs = VecDeque::new();
    for (mount, label) in definitions {
        progress(mount, index.count(), Path::new(mount));
        let root_id = index.add_root(mount, label);
        if root_id == NONE {
            index.truncated = true;
            break;
        }
        root_jobs.push_back(BuildJob {
            path: PathBuf::from(mount),
            dir_id: root_id,
            dir_lower: mount.to_lowercase(),
            volume: mount.clone(),
        });
    }
    if root_jobs.is_empty() {
        return index;
    }

    let pending = root_jobs.len();
    let shared = BuildShared {
        queue: Mutex::new(BuildQueue {
            jobs: root_jobs,
            pending,
            stopped: false,
        }),
        signal: Condvar::new(),
        index: Mutex::new(index),
        entries: AtomicU64::new(0),
        current: Mutex::new(definitions[0].0.clone()),
        volume: Mutex::new(definitions[0].0.clone()),
        warnings: Mutex::new(Vec::new()),
        truncated: AtomicBool::new(false),
    };
    let workers = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(1)
        .clamp(1, 4);

    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| build_worker(&shared, cancel));
        }
        let mut last_report = Instant::now();
        loop {
            {
                let state = shared
                    .queue
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if state.stopped || state.pending == 0 {
                    break;
                }
            }
            if cancel.load(Ordering::SeqCst) || shared.truncated.load(Ordering::SeqCst) {
                break;
            }
            if last_report.elapsed() >= Duration::from_millis(150) {
                let volume = shared
                    .volume
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .clone();
                let current = shared
                    .current
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .clone();
                progress(
                    &volume,
                    shared.entries.load(Ordering::SeqCst),
                    Path::new(&current),
                );
                last_report = Instant::now();
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let mut state = shared
            .queue
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.stopped = true;
        shared.signal.notify_all();
    });

    let mut index = shared
        .index
        .into_inner()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    index.truncated |= shared.truncated.load(Ordering::SeqCst);
    warnings.extend(
        shared
            .warnings
            .into_inner()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
    );
    if !index.entries.is_empty() {
        index.folders = index.folders.max(1);
    }
    index.sort_name_order();
    for (mount, _) in definitions {
        progress(mount, index.count(), Path::new(mount));
    }
    index
}

fn build_worker(shared: &BuildShared, cancel: &AtomicBool) {
    loop {
        let job = {
            let mut state = shared
                .queue
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            loop {
                if cancel.load(Ordering::SeqCst)
                    || shared.truncated.load(Ordering::SeqCst)
                    || state.stopped
                {
                    return;
                }
                if let Some(job) = state.jobs.pop_front() {
                    break job;
                }
                if state.pending == 0 {
                    return;
                }
                state = shared
                    .signal
                    .wait(state)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
        };
        process_build_job(shared, cancel, job);
        let mut state = shared
            .queue
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.pending = state.pending.saturating_sub(1);
        if state.pending == 0 {
            state.stopped = true;
            shared.signal.notify_all();
        }
    }
}

fn process_build_job(shared: &BuildShared, cancel: &AtomicBool, job: BuildJob) {
    if cancel.load(Ordering::SeqCst) || shared.truncated.load(Ordering::SeqCst) {
        return;
    }
    {
        *shared
            .current
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = job.path.to_string_lossy().into_owned();
        *shared
            .volume
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = job.volume.clone();
    }
    let children = match fs::read_dir(&job.path) {
        Ok(children) => children,
        Err(error) => {
            shared
                .warnings
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(format!(
                    "{} could not be indexed: {error}",
                    job.path.display()
                ));
            return;
        }
    };

    let mut directories = Vec::new();
    let mut added = 0u64;
    {
        let mut index = shared
            .index
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for child in children {
            if index.entries.len() >= MAX_ENTRIES {
                shared.truncated.store(true, Ordering::SeqCst);
                break;
            }
            let meta = Meta {
                is_dir: child.is_dir,
                size: if child.is_dir { 0 } else { child.size },
                modified: child.modified.unwrap_or(0) as u32,
                created: child.created.unwrap_or(0) as u32,
                accessed: child.accessed.unwrap_or(0) as u32,
                attrs: child.attributes,
            };
            let Some(id) = index.add(&child.name, job.dir_id, &meta) else {
                shared.truncated.store(true, Ordering::SeqCst);
                break;
            };
            added += 1;
            if child.is_dir && !child.is_reparse {
                let mut child_lower = job.dir_lower.clone();
                append_part(&mut child_lower, &child.name.to_lowercase());
                index.paths.insert(path_hash(&child_lower), id);
                directories.push(BuildJob {
                    path: child.path,
                    dir_id: id,
                    dir_lower: child_lower,
                    volume: job.volume.clone(),
                });
            }
        }
    }
    shared.entries.fetch_add(added, Ordering::SeqCst);
    if shared.truncated.load(Ordering::SeqCst) {
        return;
    }
    if !directories.is_empty() {
        let mut state = shared
            .queue
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.stopped {
            return;
        }
        state.pending += directories.len();
        for directory in directories {
            state.jobs.push_back(directory);
        }
        shared.signal.notify_all();
    }
}

/* ----------------------------------------------------------------- watcher */

fn watch_volume(state: Arc<SearchState>, mount: String) {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, ReadDirectoryChangesW, FILE_FLAG_BACKUP_SEMANTICS, FILE_LIST_DIRECTORY,
        FILE_NOTIFY_CHANGE_ATTRIBUTES, FILE_NOTIFY_CHANGE_CREATION, FILE_NOTIFY_CHANGE_DIR_NAME,
        FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_LAST_WRITE, FILE_NOTIFY_CHANGE_SIZE,
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };

    let mut wide: Vec<u16> = mount.encode_utf16().collect();
    if wide.last() != Some(&0) {
        wide.push(0);
    }
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_LIST_DIRECTORY,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        state
            .watched
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&mount);
        note_watch_stopped(&state, &mount);
        return;
    }

    let filter = FILE_NOTIFY_CHANGE_FILE_NAME
        | FILE_NOTIFY_CHANGE_DIR_NAME
        | FILE_NOTIFY_CHANGE_ATTRIBUTES
        | FILE_NOTIFY_CHANGE_SIZE
        | FILE_NOTIFY_CHANGE_LAST_WRITE
        | FILE_NOTIFY_CHANGE_CREATION;
    let mut buffer: Vec<u64> = vec![0; 16 * 1024];
    loop {
        let mut returned = 0u32;
        let ok = unsafe {
            ReadDirectoryChangesW(
                handle,
                buffer.as_mut_ptr().cast(),
                (buffer.len() * 8) as u32,
                1,
                filter,
                &mut returned,
                std::ptr::null_mut(),
                None,
            )
        };
        if ok == 0 || returned == 0 {
            if ok == 0 {
                note_watch_stopped(&state, &mount);
                break;
            }
            continue;
        }
        let bytes =
            unsafe { std::slice::from_raw_parts(buffer.as_ptr() as *const u8, returned as usize) };
        apply_notifications(&state, &mount, bytes);
    }
    unsafe {
        CloseHandle(handle);
    }
    state
        .watched
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&mount);
}

fn note_watch_stopped(state: &SearchState, mount: &str) {
    let mut runtime = state
        .runtime
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let warning = format!("Live updates stopped for {mount}; rebuild the index to refresh it.");
    if !runtime.warnings.iter().any(|value| value == &warning) {
        runtime.warnings.push(warning);
        runtime.warnings.truncate(MAX_WARNINGS);
    }
}

fn apply_notifications(state: &SearchState, mount: &str, bytes: &[u8]) {
    let mut offset = 0usize;
    let mut rename_from: Option<PathBuf> = None;
    while offset + 12 <= bytes.len() {
        let next = u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) as usize;
        let action = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]);
        let name_length = u32::from_le_bytes([
            bytes[offset + 8],
            bytes[offset + 9],
            bytes[offset + 10],
            bytes[offset + 11],
        ]) as usize;
        let name_start = offset + 12;
        if name_length % 2 != 0 || name_start + name_length > bytes.len() {
            break;
        }
        let units: Vec<u16> = bytes[name_start..name_start + name_length]
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        let relative = String::from_utf16_lossy(&units);
        // `join` handles a volume mount with or without its trailing slash.
        let full = Path::new(mount).join(relative);
        match action {
            1 => add_path(state, &full),
            2 => remove_path(state, &full),
            3 => refresh_path(state, &full),
            4 => rename_from = Some(full),
            5 => {
                if let Some(from) = rename_from.take() {
                    rename_path(state, &from, &full);
                }
            }
            _ => {}
        }
        if next == 0 {
            break;
        }
        offset += next;
    }
}

fn metadata_for(path: &Path) -> Option<Meta> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    let attrs = metadata.file_attributes();
    Some(Meta {
        is_dir: metadata.is_dir(),
        size: if metadata.is_dir() {
            0
        } else {
            metadata.file_size()
        },
        modified: filetime_to_unix(metadata.last_write_time()),
        created: filetime_to_unix(metadata.creation_time()),
        accessed: filetime_to_unix(metadata.last_access_time()),
        attrs,
    })
}

fn add_path(state: &SearchState, path: &Path) {
    let Some(meta) = metadata_for(path) else {
        return;
    };
    let lower = path.to_string_lossy().to_lowercase();
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return;
    };
    let Some(parent) = path.parent() else {
        return;
    };
    let parent_lower = parent.to_string_lossy().to_lowercase();
    let mut index = state
        .index
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if index.find_path(&lower).is_some() {
        return;
    }
    let Some(parent_id) = index.find_dir_path(&parent_lower) else {
        return;
    };
    if let Some(id) = index.add(name, parent_id, &meta) {
        if meta.is_dir {
            index.paths.insert(path_hash(&lower), id);
        }
    }
}

fn remove_path(state: &SearchState, path: &Path) {
    let lower = path.to_string_lossy().to_lowercase();
    let mut index = state
        .index
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(id) = index.find_path(&lower) {
        index.tombstone(id);
    }
}

fn refresh_path(state: &SearchState, path: &Path) {
    let Some(meta) = metadata_for(path) else {
        return;
    };
    let lower = path.to_string_lossy().to_lowercase();
    let mut index = state
        .index
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(id) = index.find_path(&lower) else {
        return;
    };
    if index.is_dir(id) != meta.is_dir {
        return;
    }
    let previous_size = index.entries[id as usize].size;
    if !meta.is_dir {
        index.total_size = index
            .total_size
            .saturating_sub(previous_size)
            .saturating_add(meta.size);
    }
    let entry = &mut index.entries[id as usize];
    entry.size = if meta.is_dir { 0 } else { meta.size };
    entry.modified = meta.modified;
    entry.created = meta.created;
    entry.accessed = meta.accessed;
    entry.attrs = meta.attrs;
}

fn rename_path(state: &SearchState, from: &Path, to: &Path) {
    let from_lower = from.to_string_lossy().to_lowercase();
    let Some(name) = to.file_name().and_then(|value| value.to_str()) else {
        return;
    };
    let Some(parent) = to.parent() else {
        return;
    };
    let parent_lower = parent.to_string_lossy().to_lowercase();
    let mut index = state
        .index
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(id) = index.find_path(&from_lower) else {
        return;
    };
    let Some(parent_id) = index.find_dir_path(&parent_lower) else {
        return;
    };
    index.rename(id, name, parent_id);
}

/* ------------------------------------------------------------ query parser */

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Word(String),
    Phrase(String),
    Or,
    Not,
    LParen,
    RParen,
}

#[derive(Debug, Clone, Copy)]
struct Flags {
    match_case: bool,
    whole_word: bool,
    regex: bool,
}

#[derive(Debug)]
enum Node {
    And(Vec<Node>),
    Or(Vec<Node>),
    Not(Box<Node>),
    Term(Term),
}

#[derive(Debug)]
enum Term {
    Name { pattern: Pattern, path_too: bool },
    Path { pattern: Pattern },
    Ext(Vec<String>),
    Size(SizeRange),
    Date { field: DateField, range: (u64, u64) },
    Attr { mask: u32, set: bool },
    Type(&'static [&'static str]),
    IsDir(bool),
    Dupe,
    Empty,
}

#[derive(Debug)]
struct Pattern {
    regex: Option<Regex>,
    needle: String,
    whole_word: bool,
    case_sensitive: bool,
}

#[derive(Debug, Clone, Copy)]
enum DateField {
    Modified,
    Created,
    Accessed,
}

#[derive(Debug)]
enum SizeRange {
    Lt(u64),
    Le(u64),
    Gt(u64),
    Ge(u64),
    Eq(u64),
    Between(u64, u64),
}

#[derive(Debug, Clone, Copy)]
enum SortField {
    Name,
    Path,
    Size,
    Modified,
    Created,
    Extension,
    Type,
}

#[derive(Debug)]
struct ParsedQuery {
    root: Node,
    needs_path: bool,
    needs_lower_path: bool,
    has_dupe: bool,
    sort: SortField,
    descending: bool,
}

fn parse_query(options: &SearchQueryOptions) -> Result<ParsedQuery, String> {
    let query = options.query.trim();
    if query.is_empty() {
        return Err("Enter a search term.".into());
    }
    if query.chars().count() > MAX_QUERY_CHARS {
        return Err(format!(
            "Search terms are limited to {MAX_QUERY_CHARS} characters."
        ));
    }
    let mut flags = Flags {
        match_case: options.match_case,
        whole_word: options.whole_word,
        regex: options.regex,
    };
    let tokens = tokenize(query)?;
    let tokens = take_global_tokens(tokens, &mut flags);
    let mut parser = Parser {
        tokens,
        position: 0,
        flags,
        options,
        terms: 0,
    };
    let root = parser.parse_expression()?;
    if parser.position < parser.tokens.len() {
        return Err("The search has an unexpected closing parenthesis.".into());
    }
    let filter = filter_node(&options.filter);
    let root = match filter {
        Some(filter) => Node::And(vec![filter, root]),
        None => root,
    };
    let needs_path = node_needs_path(&root);
    let has_dupe = node_has_dupe(&root);
    Ok(ParsedQuery {
        root,
        needs_path,
        needs_lower_path: needs_path && !flags.match_case,
        has_dupe,
        sort: sort_field(&options.sort),
        descending: options.descending,
    })
}

fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let characters: Vec<char> = input.chars().collect();
    let mut tokens = Vec::new();
    let mut index = 0usize;
    while index < characters.len() {
        let character = characters[index];
        if character.is_whitespace() {
            index += 1;
            continue;
        }
        match character {
            '|' => {
                tokens.push(Token::Or);
                index += 1;
            }
            '!' => {
                tokens.push(Token::Not);
                index += 1;
            }
            '(' => {
                tokens.push(Token::LParen);
                index += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                index += 1;
            }
            '"' => {
                index += 1;
                let mut phrase = String::new();
                while index < characters.len() && characters[index] != '"' {
                    phrase.push(characters[index]);
                    index += 1;
                }
                if index >= characters.len() {
                    return Err("A quoted phrase is missing its closing quote.".into());
                }
                index += 1;
                tokens.push(Token::Phrase(phrase));
            }
            _ => {
                let mut word = String::new();
                while index < characters.len() {
                    let current = characters[index];
                    if current.is_whitespace() || matches!(current, '|' | '(' | ')') {
                        break;
                    }
                    if current == '"' {
                        index += 1;
                        while index < characters.len() && characters[index] != '"' {
                            word.push(characters[index]);
                            index += 1;
                        }
                        if index >= characters.len() {
                            return Err("A quoted phrase is missing its closing quote.".into());
                        }
                        index += 1;
                        continue;
                    }
                    word.push(current);
                    index += 1;
                }
                if !word.is_empty() {
                    tokens.push(Token::Word(word));
                }
            }
        }
    }
    Ok(tokens)
}

fn take_global_tokens(tokens: Vec<Token>, flags: &mut Flags) -> Vec<Token> {
    tokens
        .into_iter()
        .filter(|token| {
            let Token::Word(word) = token else {
                return true;
            };
            let Some((field, value)) = split_field(word) else {
                return true;
            };
            if !value.trim().is_empty() {
                return true;
            }
            match field.to_ascii_lowercase().as_str() {
                "case" => {
                    flags.match_case = true;
                    false
                }
                "nocase" => {
                    flags.match_case = false;
                    false
                }
                "wholeword" | "ww" => {
                    flags.whole_word = true;
                    false
                }
                "nowholeword" | "noww" => {
                    flags.whole_word = false;
                    false
                }
                "regex" => {
                    flags.regex = true;
                    false
                }
                _ => true,
            }
        })
        .collect()
}

struct Parser<'a> {
    tokens: Vec<Token>,
    position: usize,
    flags: Flags,
    options: &'a SearchQueryOptions,
    terms: usize,
}

impl Parser<'_> {
    fn parse_expression(&mut self) -> Result<Node, String> {
        let mut nodes = vec![self.parse_and()?];
        while matches!(self.tokens.get(self.position), Some(Token::Or)) {
            self.position += 1;
            nodes.push(self.parse_and()?);
        }
        Ok(if nodes.len() == 1 {
            nodes.pop().unwrap_or(Node::Term(Term::Empty))
        } else {
            Node::Or(nodes)
        })
    }

    fn parse_and(&mut self) -> Result<Node, String> {
        let mut nodes = Vec::new();
        while self.starts_unary() {
            nodes.push(self.parse_unary()?);
        }
        if nodes.is_empty() {
            return Err("The search is missing a term.".into());
        }
        Ok(if nodes.len() == 1 {
            nodes.pop().unwrap_or(Node::Term(Term::Empty))
        } else {
            Node::And(nodes)
        })
    }

    fn starts_unary(&self) -> bool {
        matches!(
            self.tokens.get(self.position),
            Some(Token::Word(_)) | Some(Token::Phrase(_)) | Some(Token::Not) | Some(Token::LParen)
        )
    }

    fn parse_unary(&mut self) -> Result<Node, String> {
        match self.tokens.get(self.position).cloned() {
            Some(Token::Not) => {
                self.position += 1;
                if !self.starts_unary() {
                    return Err("The search has a `!` with nothing to exclude.".into());
                }
                Ok(Node::Not(Box::new(self.parse_unary()?)))
            }
            Some(Token::LParen) => {
                self.position += 1;
                let node = self.parse_expression()?;
                if !matches!(self.tokens.get(self.position), Some(Token::RParen)) {
                    return Err("A search group is missing its closing parenthesis.".into());
                }
                self.position += 1;
                Ok(node)
            }
            Some(Token::Phrase(phrase)) => {
                self.position += 1;
                self.name_node(&phrase, true)
            }
            Some(Token::Word(word)) => {
                self.position += 1;
                self.word_node(&word)
            }
            _ => Err("The search is missing a term.".into()),
        }
    }

    fn name_node(&mut self, text: &str, phrase: bool) -> Result<Node, String> {
        self.terms += 1;
        if self.terms > MAX_QUERY_TERMS {
            return Err(format!(
                "Search terms are limited to {MAX_QUERY_TERMS} per query."
            ));
        }
        if text.trim().is_empty() {
            return Err("The search contains an empty phrase.".into());
        }
        let pattern = compile_pattern(text, &self.flags)?;
        if phrase {
            return Ok(Node::Term(Term::Name {
                pattern,
                path_too: self.options.match_path,
            }));
        }
        Ok(Node::Term(Term::Name {
            pattern,
            path_too: self.options.match_path,
        }))
    }

    fn word_node(&mut self, word: &str) -> Result<Node, String> {
        let Some((field, raw_value)) = split_field(word) else {
            return self.name_node(word, false);
        };
        let field = field.to_ascii_lowercase();
        let value = unquote(raw_value.trim());
        match field.as_str() {
            "case" | "nocase" | "wholeword" | "ww" | "nowholeword" | "noww" if value.is_empty() => {
                self.name_node(word, false)
            }
            "ext" => {
                let extensions = parse_extensions(value)?;
                self.terms += 1;
                Ok(Node::Term(Term::Ext(extensions)))
            }
            "file" | "files" => {
                self.terms += 1;
                if value.is_empty() {
                    Ok(Node::Term(Term::IsDir(false)))
                } else {
                    Ok(Node::And(vec![
                        Node::Term(Term::IsDir(false)),
                        self.name_node(value, false)?,
                    ]))
                }
            }
            "folder" | "folders" | "dir" | "dirs" => {
                self.terms += 1;
                if value.is_empty() {
                    Ok(Node::Term(Term::IsDir(true)))
                } else {
                    Ok(Node::And(vec![
                        Node::Term(Term::IsDir(true)),
                        self.name_node(value, false)?,
                    ]))
                }
            }
            "path" => {
                if value.is_empty() {
                    return Err("`path:` needs a path fragment.".into());
                }
                self.terms += 1;
                Ok(Node::Term(Term::Path {
                    pattern: compile_pattern(value, &self.flags)?,
                }))
            }
            "size" => {
                if value.is_empty() {
                    return Err("`size:` needs a value such as 10mb or 1kb..10mb.".into());
                }
                self.terms += 1;
                Ok(Node::Term(Term::Size(parse_size_filter(value)?)))
            }
            "dm" | "datemodified" | "modified" => {
                self.terms += 1;
                Ok(Node::Term(Term::Date {
                    field: DateField::Modified,
                    range: parse_date_range(value)?,
                }))
            }
            "dc" | "datecreated" | "created" => {
                self.terms += 1;
                Ok(Node::Term(Term::Date {
                    field: DateField::Created,
                    range: parse_date_range(value)?,
                }))
            }
            "da" | "dateaccessed" | "accessed" => {
                self.terms += 1;
                Ok(Node::Term(Term::Date {
                    field: DateField::Accessed,
                    range: parse_date_range(value)?,
                }))
            }
            "attrib" | "attributes" => {
                let (mask, set) = parse_attributes(value)?;
                self.terms += 1;
                Ok(Node::Term(Term::Attr { mask, set }))
            }
            "type" => {
                let list = file_type_extensions(value)
                    .ok_or_else(|| format!("Unknown type filter: {value}"))?;
                self.terms += 1;
                Ok(Node::Term(Term::Type(list)))
            }
            "dupe" | "duplicates" => {
                self.terms += 1;
                Ok(Node::Term(Term::Dupe))
            }
            "empty" => {
                self.terms += 1;
                Ok(Node::Term(Term::Empty))
            }
            "regex" => {
                if value.is_empty() {
                    return Err("`regex:` needs a regular expression.".into());
                }
                self.terms += 1;
                let mut regex_flags = self.flags;
                regex_flags.regex = true;
                Ok(Node::Term(Term::Name {
                    pattern: compile_pattern(value, &regex_flags)?,
                    path_too: self.options.match_path,
                }))
            }
            "audio" | "video" | "image" | "images" | "pic" | "pics" | "doc" | "docs"
            | "document" | "documents" | "exe" | "executable" | "executables" | "zip"
            | "compressed" | "archive" => {
                let list = file_type_extensions(&field)
                    .ok_or_else(|| format!("Unknown file type filter: {field}"))?;
                self.terms += 1;
                if value.is_empty() {
                    Ok(Node::Term(Term::Type(list)))
                } else {
                    Ok(Node::And(vec![
                        Node::Term(Term::Type(list)),
                        self.name_node(value, false)?,
                    ]))
                }
            }
            _ => self.name_node(word, false),
        }
    }
}

fn split_field(word: &str) -> Option<(&str, &str)> {
    let (field, value) = word.split_once(':')?;
    if field.is_empty()
        || !field
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return None;
    }
    Some((field, value))
}

fn unquote(value: &str) -> &str {
    let bytes = value.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
    {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

fn compile_pattern(text: &str, flags: &Flags) -> Result<Pattern, String> {
    if flags.regex {
        let regex = RegexBuilder::new(text)
            .case_insensitive(!flags.match_case)
            .size_limit(4 * 1024 * 1024)
            .dfa_size_limit(4 * 1024 * 1024)
            .build()
            .map_err(|error| format!("Invalid regular expression: {error}"))?;
        return Ok(Pattern {
            regex: Some(regex),
            needle: String::new(),
            whole_word: false,
            case_sensitive: flags.match_case,
        });
    }
    if text.contains('*') || text.contains('?') {
        let mut expression = String::from("^");
        for character in text.chars() {
            match character {
                '*' => expression.push_str(".*"),
                '?' => expression.push('.'),
                _ => expression.push_str(&regex::escape(&character.to_string())),
            }
        }
        expression.push('$');
        let regex = RegexBuilder::new(&expression)
            .case_insensitive(!flags.match_case)
            .size_limit(1024 * 1024)
            .build()
            .map_err(|error| format!("Invalid wildcard search: {error}"))?;
        return Ok(Pattern {
            regex: Some(regex),
            needle: String::new(),
            whole_word: false,
            case_sensitive: flags.match_case,
        });
    }
    Ok(Pattern {
        regex: None,
        needle: if flags.match_case {
            text.to_string()
        } else {
            text.to_lowercase()
        },
        whole_word: flags.whole_word,
        case_sensitive: flags.match_case,
    })
}

fn pattern_matches(pattern: &Pattern, original: &str, lower: &str) -> bool {
    if let Some(regex) = &pattern.regex {
        return regex.is_match(original);
    }
    let haystack = if pattern.case_sensitive {
        original
    } else {
        lower
    };
    if pattern.whole_word {
        contains_whole_word(haystack, &pattern.needle)
    } else {
        haystack.contains(&pattern.needle)
    }
}

fn contains_whole_word(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let mut from = 0usize;
    while from <= haystack.len() {
        let Some(position) = haystack[from..].find(needle) else {
            return false;
        };
        let begin = from + position;
        let end = begin + needle.len();
        let before = haystack[..begin].chars().next_back();
        let after = haystack[end..].chars().next();
        if before.map_or(true, |character| !is_word_character(character))
            && after.map_or(true, |character| !is_word_character(character))
        {
            return true;
        }
        from = begin
            + haystack[begin..]
                .chars()
                .next()
                .map(|character| character.len_utf8())
                .unwrap_or(1);
    }
    false
}

fn is_word_character(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

fn parse_extensions(value: &str) -> Result<Vec<String>, String> {
    let mut extensions = Vec::new();
    for part in value.split([',', ';']) {
        for item in part.split_whitespace() {
            let cleaned = item
                .trim_start_matches('*')
                .trim_start_matches('.')
                .to_ascii_lowercase();
            if !cleaned.is_empty() && cleaned.len() <= 32 {
                extensions.push(cleaned);
            }
        }
    }
    if extensions.is_empty() {
        return Err("`ext:` needs at least one extension.".into());
    }
    extensions.truncate(32);
    Ok(extensions)
}

fn file_type_extensions(kind: &str) -> Option<&'static [&'static str]> {
    match kind.to_ascii_lowercase().as_str() {
        "audio" => Some(AUDIO),
        "video" => Some(VIDEO),
        "image" | "images" | "pic" | "pics" => Some(IMAGE),
        "doc" | "docs" | "document" | "documents" => Some(DOCUMENT),
        "exe" | "executable" | "executables" => Some(EXECUTABLE),
        "zip" | "compressed" | "archive" => Some(COMPRESSED),
        "folder" | "folders" | "dir" | "dirs" => Some(&[]),
        "file" | "files" => Some(&[]),
        _ => None,
    }
}

fn filter_node(filter: &str) -> Option<Node> {
    match filter.to_ascii_lowercase().as_str() {
        "files" => Some(Node::Term(Term::IsDir(false))),
        "folders" => Some(Node::Term(Term::IsDir(true))),
        "audio" | "video" | "image" | "document" | "executable" | "compressed" => {
            file_type_extensions(filter).map(|list| Node::Term(Term::Type(list)))
        }
        _ => None,
    }
}

fn sort_field(value: &str) -> SortField {
    match value.to_ascii_lowercase().as_str() {
        "path" => SortField::Path,
        "size" => SortField::Size,
        "modified" | "datemodified" => SortField::Modified,
        "created" | "datecreated" => SortField::Created,
        "extension" | "ext" => SortField::Extension,
        "type" => SortField::Type,
        _ => SortField::Name,
    }
}

fn node_needs_path(node: &Node) -> bool {
    match node {
        Node::And(nodes) | Node::Or(nodes) => nodes.iter().any(node_needs_path),
        Node::Not(inner) => node_needs_path(inner),
        Node::Term(Term::Path { .. }) => true,
        Node::Term(Term::Name { path_too, .. }) => *path_too,
        _ => false,
    }
}

fn node_has_dupe(node: &Node) -> bool {
    match node {
        Node::And(nodes) | Node::Or(nodes) => nodes.iter().any(node_has_dupe),
        Node::Not(inner) => node_has_dupe(inner),
        Node::Term(Term::Dupe) => true,
        _ => false,
    }
}

/* --------------------------------------------------------------- matching */

fn query_index(index: &FileIndex, options: &SearchQueryOptions) -> Result<SearchIndexPage, String> {
    let parsed = parse_query(options)?;
    let started = Instant::now();
    let mut matches: Vec<u32> = Vec::new();
    let mut cache: HashMap<u32, String> = HashMap::new();
    // The overwhelmingly common query is one plain name term. Skip the AST
    // walk, option checks and path work for that case.
    let fast_name = match &parsed.root {
        Node::Term(Term::Name {
            pattern,
            path_too: false,
        }) if pattern.regex.is_none() && !pattern.whole_word => Some(pattern),
        _ => None,
    };

    // Name order is maintained by build/load and dropped after a live change.
    // Walking it avoids materialising and sorting every match for the common
    // name-sorted query.
    if !parsed.needs_path
        && !parsed.has_dupe
        && matches!(parsed.sort, SortField::Name)
        && !index.name_order.is_empty()
    {
        let limit = options.limit.clamp(1, MAX_LIMIT) as usize;
        let requested = (options.offset as usize).saturating_add(limit);
        let mut page_ids: Vec<u32> = Vec::with_capacity(requested.min(4096));
        let mut total = 0usize;
        let mut files = 0u32;
        let mut folders = 0u32;
        let mut total_size = 0u64;
        let mut visit = |id: u32| {
            let entry = index.entry(id);
            if entry.flags & FLAG_DELETED != 0 {
                return;
            }
            let matched = if let Some(pattern) = fast_name {
                let haystack = if pattern.case_sensitive {
                    index.name_of(entry)
                } else {
                    index.lower_of(entry)
                };
                haystack.contains(&pattern.needle)
            } else {
                matches_node(&parsed.root, index, id, None, None)
            };
            if !matched {
                return;
            }
            total += 1;
            if index.is_dir(id) {
                folders += 1;
            } else {
                files += 1;
                total_size = total_size.saturating_add(entry.size);
            }
            if page_ids.len() < requested {
                page_ids.push(id);
            }
        };
        if parsed.descending {
            for id in index.name_order.iter().rev() {
                visit(*id);
            }
        } else {
            for id in &index.name_order {
                visit(*id);
            }
        }
        let offset = (options.offset as usize).min(total);
        let end = (offset + limit).min(total);
        let mut items = Vec::with_capacity(end.saturating_sub(offset));
        for id in &page_ids[offset..end] {
            let entry = index.entry(*id);
            let is_folder = entry.flags & FLAG_DIR != 0;
            items.push(SearchIndexItem {
                name: index.name_of(entry).to_string(),
                parent_path: index.parent_path(*id),
                full_path: index.path_of(*id),
                is_folder,
                size: if is_folder { 0 } else { entry.size },
                modified: (entry.modified > 0).then_some(entry.modified as u64),
                created: (entry.created > 0).then_some(entry.created as u64),
                extension: index.extension(*id).to_string(),
            });
        }
        return Ok(SearchIndexPage {
            items,
            total: total.min(u32::MAX as usize) as u32,
            files,
            folders,
            total_size,
            offset: offset as u32,
            limit: limit as u32,
            has_more: end < total,
            elapsed_ms: started.elapsed().as_millis() as u64,
            truncated: index.truncated,
        });
    }

    for id in 0..index.entries.len() as u32 {
        let entry = index.entry(id);
        if entry.flags & FLAG_DELETED != 0 {
            continue;
        }
        if let Some(pattern) = fast_name {
            let haystack = if pattern.case_sensitive {
                index.name_of(entry)
            } else {
                index.lower_of(entry)
            };
            if haystack.contains(&pattern.needle) {
                matches.push(id);
            }
            continue;
        }
        let (path, lower_path) = if parsed.needs_path {
            let value = index.path_of_cached(id, &mut cache);
            let lower = if parsed.needs_lower_path {
                Some(value.to_lowercase())
            } else {
                None
            };
            (Some(value), lower)
        } else {
            (None, None)
        };
        if matches_node(
            &parsed.root,
            index,
            id,
            path.as_deref(),
            lower_path.as_deref(),
        ) {
            matches.push(id);
        }
    }

    if parsed.has_dupe {
        let mut counts: HashMap<u64, u32> = HashMap::new();
        for id in &matches {
            let key = path_hash(index.lower_name(*id));
            *counts.entry(key).or_insert(0) += 1;
        }
        matches.retain(|id| {
            counts
                .get(&path_hash(index.lower_name(*id)))
                .copied()
                .unwrap_or(0)
                > 1
        });
    }

    let total = matches.len().min(u32::MAX as usize) as u32;
    let mut files = 0u32;
    let mut folders = 0u32;
    let mut total_size = 0u64;
    for id in &matches {
        if index.is_dir(*id) {
            folders += 1;
        } else {
            files += 1;
            total_size = total_size.saturating_add(index.entry(*id).size);
        }
    }

    let offset = (options.offset as usize).min(matches.len());
    let limit = options.limit.clamp(1, MAX_LIMIT) as usize;
    let end = (offset + limit).min(matches.len());
    let has_more = end < matches.len();
    sort_matches(
        index,
        &mut matches,
        parsed.sort,
        parsed.descending,
        Some(end),
    );
    let mut items = Vec::with_capacity(end.saturating_sub(offset));
    for id in &matches[offset..end] {
        let entry = index.entry(*id);
        let is_folder = entry.flags & FLAG_DIR != 0;
        items.push(SearchIndexItem {
            name: index.name_of(entry).to_string(),
            parent_path: index.parent_path(*id),
            full_path: index.path_of(*id),
            is_folder,
            size: if is_folder { 0 } else { entry.size },
            modified: (entry.modified > 0).then_some(entry.modified as u64),
            created: (entry.created > 0).then_some(entry.created as u64),
            extension: index.extension(*id).to_string(),
        });
    }

    Ok(SearchIndexPage {
        items,
        total,
        files,
        folders,
        total_size,
        offset: offset as u32,
        limit: limit as u32,
        has_more,
        elapsed_ms: started.elapsed().as_millis() as u64,
        truncated: index.truncated,
    })
}

/// Hard cap for one export so a broad query cannot create an unbounded file
/// or hold the CPU for an unbounded time.
pub const MAX_EXPORT_ROWS: usize = 1_000_000;

fn collect_matches(index: &FileIndex, parsed: &ParsedQuery) -> Vec<u32> {
    let fast_name = match &parsed.root {
        Node::Term(Term::Name {
            pattern,
            path_too: false,
        }) if pattern.regex.is_none() && !pattern.whole_word => Some(pattern),
        _ => None,
    };
    let mut matches = Vec::new();
    let mut cache: HashMap<u32, String> = HashMap::new();
    for id in 0..index.entries.len() as u32 {
        let entry = index.entry(id);
        if entry.flags & FLAG_DELETED != 0 {
            continue;
        }
        if let Some(pattern) = fast_name {
            let haystack = if pattern.case_sensitive {
                index.name_of(entry)
            } else {
                index.lower_of(entry)
            };
            if haystack.contains(&pattern.needle) {
                matches.push(id);
            }
            continue;
        }
        let (path, lower_path) = if parsed.needs_path {
            let value = index.path_of_cached(id, &mut cache);
            let lower = if parsed.needs_lower_path {
                Some(value.to_lowercase())
            } else {
                None
            };
            (Some(value), lower)
        } else {
            (None, None)
        };
        if matches_node(
            &parsed.root,
            index,
            id,
            path.as_deref(),
            lower_path.as_deref(),
        ) {
            matches.push(id);
        }
    }
    if parsed.has_dupe {
        let mut counts: HashMap<u64, u32> = HashMap::new();
        for id in &matches {
            *counts
                .entry(path_hash(index.lower_name(*id)))
                .or_insert(0) += 1;
        }
        matches.retain(|id| {
            counts
                .get(&path_hash(index.lower_name(*id)))
                .copied()
                .unwrap_or(0)
                > 1
        });
    }
    matches
}

fn export_index(
    index: &FileIndex,
    options: &SearchQueryOptions,
    format: &str,
    destination: &str,
    selected: Option<&[String]>,
) -> Result<SearchExportReport, String> {
    let format = format.trim().to_ascii_lowercase();
    if format != "csv" && format != "txt" {
        return Err("The export format must be CSV or TXT.".into());
    }
    if destination.trim().is_empty() {
        return Err("Choose where to save the export.".into());
    }
    if destination.len() > 32_767 {
        return Err("The export path is too long.".into());
    }
    let destination_path = PathBuf::from(destination);
    if !destination_path.is_absolute() {
        return Err("Choose an absolute folder for the export.".into());
    }
    if destination_path.is_dir() {
        return Err("The export destination is a folder, not a file.".into());
    }
    let extension = destination_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    if !extension.eq_ignore_ascii_case(&format) {
        return Err(format!("The export file must end in .{format}."));
    }
    let Some(parent) = destination_path.parent() else {
        return Err("Choose an existing folder for the export.".into());
    };
    if !parent.is_dir() {
        return Err("The export folder no longer exists.".into());
    }

    let mut skipped = 0u64;
    let mut truncated = false;
    let ids: Vec<u32> = if let Some(paths) = selected {
        if paths.len() > MAX_EXPORT_ROWS {
            return Err(format!(
                "Select at most {MAX_EXPORT_ROWS} rows for one export."
            ));
        }
        let mut seen = HashSet::new();
        let mut ids = Vec::with_capacity(paths.len());
        for path in paths {
            let lower = path.to_lowercase();
            if lower.trim().is_empty() {
                skipped += 1;
                continue;
            }
            match index.find_path(&lower) {
                Some(id) if seen.insert(id) => ids.push(id),
                _ => skipped += 1,
            }
        }
        ids
    } else {
        let parsed = parse_query(options)?;
        let mut matches = collect_matches(index, &parsed);
        truncated = matches.len() > MAX_EXPORT_ROWS;
        sort_matches(
            index,
            &mut matches,
            parsed.sort,
            parsed.descending,
            Some(MAX_EXPORT_ROWS),
        );
        matches.truncate(MAX_EXPORT_ROWS);
        matches
    };

    let file = File::create(&destination_path)
        .map_err(|error| format!("The export could not be created: {error}"))?;
    let mut writer = BufWriter::with_capacity(256 * 1024, file);
    if format == "csv" {
        writer
            .write_all(b"Name,Path,Type,SizeBytes,Created,Modified\r\n")
            .map_err(|error| format!("The export could not be written: {error}"))?;
    }
    let mut rows = 0u64;
    for id in &ids {
        let entry = index.entry(*id);
        if entry.flags & FLAG_DELETED != 0 {
            skipped += 1;
            continue;
        }
        let is_folder = entry.flags & FLAG_DIR != 0;
        let path = index.path_of(*id);
        if format == "csv" {
            let name = index.name_of(entry);
            let extension = index.extension(*id);
            let kind = if is_folder {
                "Folder".to_string()
            } else if extension.is_empty() {
                "File".to_string()
            } else {
                extension.to_string()
            };
            write_csv_field(&mut writer, name)?;
            writer
                .write_all(b",")
                .map_err(|error| format!("The export could not be written: {error}"))?;
            write_csv_field(&mut writer, &path)?;
            writer
                .write_all(b",")
                .map_err(|error| format!("The export could not be written: {error}"))?;
            write_csv_field(&mut writer, &kind)?;
            writer
                .write_all(b",")
                .map_err(|error| format!("The export could not be written: {error}"))?;
            if !is_folder {
                writer
                    .write_all(entry.size.to_string().as_bytes())
                    .map_err(|error| format!("The export could not be written: {error}"))?;
            }
            writer
                .write_all(b",")
                .map_err(|error| format!("The export could not be written: {error}"))?;
            writer
                .write_all(format_export_time(entry.created).as_bytes())
                .map_err(|error| format!("The export could not be written: {error}"))?;
            writer
                .write_all(b",")
                .map_err(|error| format!("The export could not be written: {error}"))?;
            writer
                .write_all(format_export_time(entry.modified).as_bytes())
                .map_err(|error| format!("The export could not be written: {error}"))?;
            writer
                .write_all(b"\r\n")
                .map_err(|error| format!("The export could not be written: {error}"))?;
        } else {
            writer
                .write_all(path.as_bytes())
                .and_then(|()| writer.write_all(b"\r\n"))
                .map_err(|error| format!("The export could not be written: {error}"))?;
        }
        rows += 1;
    }
    writer
        .flush()
        .map_err(|error| format!("The export could not be flushed: {error}"))?;

    Ok(SearchExportReport {
        path: destination.to_string(),
        rows,
        skipped,
        truncated,
    })
}

fn write_csv_field(writer: &mut impl Write, value: &str) -> Result<(), String> {
    if value.contains([',', '"', '\r', '\n']) {
        let escaped = value.replace('"', "\"\"");
        writer
            .write_all(format!("\"{escaped}\"").as_bytes())
            .map_err(|error| format!("The export could not be written: {error}"))?;
    } else {
        writer
            .write_all(value.as_bytes())
            .map_err(|error| format!("The export could not be written: {error}"))?;
    }
    Ok(())
}

fn format_export_time(value: u32) -> String {
    if value == 0 {
        return String::new();
    }
    let seconds = value as i64;
    let days = seconds.div_euclid(86_400);
    let remainder = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        remainder / 3600,
        (remainder % 3600) / 60,
        remainder % 60
    )
}

fn matches_node(
    node: &Node,
    index: &FileIndex,
    id: u32,
    path: Option<&str>,
    lower_path: Option<&str>,
) -> bool {
    match node {
        Node::And(nodes) => nodes
            .iter()
            .all(|node| matches_node(node, index, id, path, lower_path)),
        Node::Or(nodes) => nodes
            .iter()
            .any(|node| matches_node(node, index, id, path, lower_path)),
        Node::Not(inner) => !matches_node(inner, index, id, path, lower_path),
        Node::Term(term) => term_matches(term, index, id, path, lower_path),
    }
}

fn term_matches(
    term: &Term,
    index: &FileIndex,
    id: u32,
    path: Option<&str>,
    lower_path: Option<&str>,
) -> bool {
    let entry = index.entry(id);
    match term {
        Term::Name { pattern, path_too } => {
            if pattern_matches(pattern, index.name_of(entry), index.lower_of(entry)) {
                return true;
            }
            if *path_too {
                if let Some(value) = path {
                    return pattern_matches(pattern, value, lower_path.unwrap_or(""));
                }
            }
            false
        }
        Term::Path { pattern } => path
            .map(|value| pattern_matches(pattern, value, lower_path.unwrap_or("")))
            .unwrap_or(false),
        Term::Ext(extensions) => {
            let extension = index.extension(id);
            extensions
                .iter()
                .any(|value| value.eq_ignore_ascii_case(extension))
        }
        Term::Size(range) => {
            if entry.flags & FLAG_DIR != 0 {
                return false;
            }
            match range {
                SizeRange::Lt(value) => entry.size < *value,
                SizeRange::Le(value) => entry.size <= *value,
                SizeRange::Gt(value) => entry.size > *value,
                SizeRange::Ge(value) => entry.size >= *value,
                SizeRange::Eq(value) => entry.size == *value,
                SizeRange::Between(low, high) => entry.size >= *low && entry.size <= *high,
            }
        }
        Term::Date { field, range } => {
            let value = match field {
                DateField::Modified => entry.modified as u64,
                DateField::Created => entry.created as u64,
                DateField::Accessed => entry.accessed as u64,
            };
            value >= range.0 && value < range.1
        }
        Term::Attr { mask, set } => {
            if *set {
                entry.attrs & mask == *mask
            } else {
                entry.attrs & mask == 0
            }
        }
        Term::Type(extensions) => {
            let extension = index.extension(id);
            extensions
                .iter()
                .any(|value| value.eq_ignore_ascii_case(extension))
        }
        Term::IsDir(directory) => (entry.flags & FLAG_DIR != 0) == *directory,
        Term::Dupe => true,
        Term::Empty => {
            if entry.flags & FLAG_DIR != 0 {
                index.first_child[id as usize] == NONE
            } else {
                entry.size == 0
            }
        }
    }
}

fn compare_matches(
    index: &FileIndex,
    field: SortField,
    left: u32,
    right: u32,
) -> CmpOrdering {
    let order = match field {
        SortField::Path => CmpOrdering::Equal,
        SortField::Name => index.lower_name(left).cmp(index.lower_name(right)),
        SortField::Extension => index
            .extension(left)
            .cmp(index.extension(right))
            .then(index.lower_name(left).cmp(index.lower_name(right))),
        SortField::Size => index
            .entry(left)
            .size
            .cmp(&index.entry(right).size)
            .then(index.lower_name(left).cmp(index.lower_name(right))),
        SortField::Modified => index
            .entry(left)
            .modified
            .cmp(&index.entry(right).modified)
            .then(index.lower_name(left).cmp(index.lower_name(right))),
        SortField::Created => index
            .entry(left)
            .created
            .cmp(&index.entry(right).created)
            .then(index.lower_name(left).cmp(index.lower_name(right))),
        SortField::Type => type_rank(index.extension(left))
            .cmp(&type_rank(index.extension(right)))
            .then(index.lower_name(left).cmp(index.lower_name(right))),
    };
    order.then(left.cmp(&right))
}

/// Sorts matches and, when `keep` is smaller than the result set, selects only
/// the page that will be rendered before sorting it. A broad query can match
/// millions of entries; ordering the whole set per keystroke is wasted work.
fn sort_matches(
    index: &FileIndex,
    matches: &mut Vec<u32>,
    field: SortField,
    descending: bool,
    keep: Option<usize>,
) {
    if matches!(field, SortField::Path) {
        let mut cache: HashMap<u32, String> = HashMap::new();
        let mut keyed: Vec<(String, u32)> = matches
            .iter()
            .map(|id| (index.path_of_cached(*id, &mut cache).to_lowercase(), *id))
            .collect();
        keyed.sort_by(|left, right| {
            let order = left.0.cmp(&right.0).then(left.1.cmp(&right.1));
            if descending {
                order.reverse()
            } else {
                order
            }
        });
        *matches = keyed.into_iter().map(|(_, id)| id).collect();
        return;
    }

    let keep = keep.unwrap_or(matches.len()).min(matches.len());
    if keep > 0 && keep < matches.len() {
        matches.select_nth_unstable_by(keep - 1, |left, right| {
            let order = compare_matches(index, field, *left, *right);
            if descending {
                order.reverse()
            } else {
                order
            }
        });
        matches.truncate(keep);
    }
    matches.sort_by(|left, right| {
        let order = compare_matches(index, field, *left, *right);
        if descending {
            order.reverse()
        } else {
            order
        }
    });
}

fn type_rank(extension: &str) -> u8 {
    if AUDIO.contains(&extension) {
        1
    } else if VIDEO.contains(&extension) {
        2
    } else if IMAGE.contains(&extension) {
        3
    } else if DOCUMENT.contains(&extension) {
        4
    } else if EXECUTABLE.contains(&extension) {
        5
    } else if COMPRESSED.contains(&extension) {
        6
    } else {
        7
    }
}

const AUDIO: &[&str] = &[
    "mp3", "flac", "wav", "m4a", "aac", "ogg", "oga", "opus", "wma", "aif", "aiff", "mid", "midi",
    "ape", "mka", "wv",
];
const VIDEO: &[&str] = &[
    "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", "ts", "m2ts", "3gp",
    "ogv", "vob",
];
const IMAGE: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "bmp", "webp", "tif", "tiff", "svg", "ico", "heic", "avif", "raw",
    "cr2", "nef", "psd", "jxl",
];
const DOCUMENT: &[&str] = &[
    "pdf", "doc", "docx", "txt", "rtf", "odt", "xls", "xlsx", "csv", "ppt", "pptx", "md", "epub",
    "pages", "numbers", "key", "log", "json", "xml", "html", "htm",
];
const EXECUTABLE: &[&str] = &[
    "exe", "dll", "com", "bat", "cmd", "msi", "ps1", "msix", "appx", "sys", "scr", "jar",
];
const COMPRESSED: &[&str] = &[
    "zip", "rar", "7z", "tar", "gz", "bz2", "xz", "iso", "cab", "tgz", "zst", "lz", "lzma",
];

/* -------------------------------------------------------------- size/date */

fn parse_size_filter(value: &str) -> Result<SizeRange, String> {
    let (operator, rest) = split_operator(value);
    if let Some((low, high)) = rest.split_once("..") {
        let low = parse_size(low)?;
        let high = parse_size(high)?;
        if low > high {
            return Err("A size range must start below its end.".into());
        }
        return Ok(SizeRange::Between(low, high));
    }
    let number = if rest.eq_ignore_ascii_case("empty") {
        0
    } else {
        parse_size(rest)?
    };
    match operator {
        "" | "=" => Ok(SizeRange::Eq(number)),
        ">" => Ok(SizeRange::Gt(number)),
        ">=" => Ok(SizeRange::Ge(number)),
        "<" => Ok(SizeRange::Lt(number)),
        "<=" => Ok(SizeRange::Le(number)),
        other => Err(format!("Unknown size comparison `{other}`.")),
    }
}

fn parse_size(value: &str) -> Result<u64, String> {
    let cleaned = value.trim().to_ascii_lowercase();
    let split = cleaned
        .find(|character: char| !character.is_ascii_digit() && character != '.')
        .unwrap_or(cleaned.len());
    let (number_text, suffix) = cleaned.split_at(split);
    if number_text.is_empty() {
        return Err(format!("`{value}` is not a size."));
    }
    let number: f64 = number_text
        .parse()
        .map_err(|_| format!("`{value}` is not a size."))?;
    let multiplier = match suffix.trim() {
        "" | "b" | "byte" | "bytes" => 1f64,
        "k" | "kb" | "kib" => 1024f64,
        "m" | "mb" | "mib" => 1024f64.powi(2),
        "g" | "gb" | "gib" => 1024f64.powi(3),
        "t" | "tb" | "tib" => 1024f64.powi(4),
        other => return Err(format!("Unknown size unit `{other}`.")),
    };
    Ok((number * multiplier).min(u64::MAX as f64) as u64)
}

fn split_operator(value: &str) -> (&str, &str) {
    let trimmed = value.trim_start();
    for operator in [">=", "<=", ">", "<", "="] {
        if let Some(rest) = trimmed.strip_prefix(operator) {
            return (operator, rest.trim_start());
        }
    }
    ("", trimmed)
}

fn parse_date_range(value: &str) -> Result<(u64, u64), String> {
    if value.trim().is_empty() {
        return Err("A date filter needs a date such as today or 2026-01-31.".into());
    }
    let (operator, rest) = split_operator(value);
    if let Some((start_text, end_text)) = rest.split_once("..") {
        let start = parse_date_point(start_text)?;
        let end = parse_date_point(end_text)?;
        return Ok((start.0, end.1));
    }
    let point = parse_date_point(rest)?;
    Ok(match operator {
        "" | "=" => point,
        ">" => (point.1, u64::MAX),
        ">=" => (point.0, u64::MAX),
        "<" => (0, point.0),
        "<=" => (0, point.1),
        other => return Err(format!("Unknown date comparison `{other}`.")),
    })
}

fn parse_date_point(value: &str) -> Result<(u64, u64), String> {
    let cleaned = value.trim().to_ascii_lowercase();
    match cleaned.as_str() {
        "today" => day_range(0),
        "yesterday" => day_range(-1),
        "tomorrow" => day_range(1),
        "thisweek" => week_range(0),
        "lastweek" => week_range(-1),
        "thismonth" => month_range(0),
        "lastmonth" => month_range(-1),
        "thisyear" => year_range(0),
        "lastyear" => year_range(-1),
        _ => absolute_date_point(&cleaned),
    }
}

fn day_range(offset: i64) -> Result<(u64, u64), String> {
    let (year, month, day) =
        local_today().ok_or_else(|| "The local date is unavailable.".to_string())?;
    let days = days_from_civil(year, month, day) + offset;
    let (start_year, start_month, start_day) = civil_from_days(days);
    let (end_year, end_month, end_day) = civil_from_days(days + 1);
    let start = local_midnight(start_year, start_month, start_day)
        .ok_or_else(|| "The local date could not be converted.".to_string())?;
    let end = local_midnight(end_year, end_month, end_day)
        .ok_or_else(|| "The local date could not be converted.".to_string())?;
    Ok((start, end))
}

fn month_start_days(year: i64, month: i64) -> i64 {
    days_from_civil(year, month, 1)
}

fn add_months(year: i64, month: i64, delta: i64) -> (i64, i64) {
    let total = (year * 12) + (month - 1) + delta;
    (total.div_euclid(12), total.rem_euclid(12) + 1)
}

fn week_range(offset: i64) -> Result<(u64, u64), String> {
    let (year, month, day) =
        local_today().ok_or_else(|| "The local date is unavailable.".to_string())?;
    let today = days_from_civil(year, month, day);
    let weekday = (today + 4).rem_euclid(7); // 1970-01-01 was a Thursday.
    let monday = today - (weekday + 6).rem_euclid(7) + offset * 7;
    let (start_year, start_month, start_day) = civil_from_days(monday);
    let (end_year, end_month, end_day) = civil_from_days(monday + 7);
    Ok((
        local_midnight(start_year, start_month, start_day)
            .ok_or_else(|| "The local date could not be converted.".to_string())?,
        local_midnight(end_year, end_month, end_day)
            .ok_or_else(|| "The local date could not be converted.".to_string())?,
    ))
}

fn month_range(offset: i64) -> Result<(u64, u64), String> {
    let (year, month, _) =
        local_today().ok_or_else(|| "The local date is unavailable.".to_string())?;
    let (year, month) = add_months(year, month, offset);
    let (next_year, next_month) = add_months(year, month, 1);
    let start = civil_from_days(month_start_days(year, month));
    let end = civil_from_days(month_start_days(next_year, next_month));
    Ok((
        local_midnight(start.0, start.1, start.2)
            .ok_or_else(|| "The local date could not be converted.".to_string())?,
        local_midnight(end.0, end.1, end.2)
            .ok_or_else(|| "The local date could not be converted.".to_string())?,
    ))
}

fn year_range(offset: i64) -> Result<(u64, u64), String> {
    let (year, _, _) = local_today().ok_or_else(|| "The local date is unavailable.".to_string())?;
    let year = year + offset;
    Ok((
        local_midnight(year, 1, 1)
            .ok_or_else(|| "The local date could not be converted.".to_string())?,
        local_midnight(year + 1, 1, 1)
            .ok_or_else(|| "The local date could not be converted.".to_string())?,
    ))
}

fn absolute_date_point(value: &str) -> Result<(u64, u64), String> {
    let (date_text, time_text) = match value.split_once('t') {
        Some((date, time)) => (date, Some(time)),
        None => (value, None),
    };
    let parts: Vec<&str> = date_text.split(['-', '/']).collect();
    if parts.len() != 3 {
        return Err(format!(
            "`{value}` is not a date. Use today, yesterday, thisweek, thismonth, thisyear, or yyyy-mm-dd."
        ));
    }
    let year: i64 = parts[0]
        .parse()
        .map_err(|_| format!("`{value}` has an invalid year."))?;
    let month: i64 = parts[1]
        .parse()
        .map_err(|_| format!("`{value}` has an invalid month."))?;
    let day: i64 = parts[2]
        .parse()
        .map_err(|_| format!("`{value}` has an invalid day."))?;
    if !(1601..=9999).contains(&year) || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(format!("`{value}` is outside the supported date range."));
    }
    let (check_year, check_month, check_day) = civil_from_days(days_from_civil(year, month, day));
    if (check_year, check_month, check_day) != (year, month, day) {
        return Err(format!("`{value}` is not a real calendar date."));
    }
    let start = local_midnight(year, month, day)
        .ok_or_else(|| "The local date could not be converted.".to_string())?;
    if let Some(time) = time_text {
        let clock: Vec<&str> = time.split(':').collect();
        if clock.len() < 2 || clock.len() > 3 {
            return Err(format!("`{value}` has an invalid time."));
        }
        let hour: u64 = clock[0]
            .parse()
            .map_err(|_| format!("`{value}` has an invalid hour."))?;
        let minute: u64 = clock[1]
            .parse()
            .map_err(|_| format!("`{value}` has an invalid minute."))?;
        if hour > 23 || minute > 59 {
            return Err(format!("`{value}` has an invalid time."));
        }
        let specific = start + hour * 3600 + minute * 60;
        return Ok((specific, specific + 60));
    }
    let next = civil_from_days(days_from_civil(year, month, day) + 1);
    let end = local_midnight(next.0, next.1, next.2)
        .ok_or_else(|| "The local date could not be converted.".to_string())?;
    Ok((start, end))
}

fn parse_attributes(value: &str) -> Result<(u32, bool), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("`attrib:` needs flags such as h, s, r or a.".into());
    }
    let set = !trimmed.starts_with(['-', '!']);
    let cleaned = trimmed.trim_start_matches(['-', '!']);
    let mut mask = 0u32;
    for character in cleaned.chars() {
        mask |= match character.to_ascii_lowercase() {
            'r' => 0x1,
            'h' => FILE_ATTRIBUTE_HIDDEN,
            's' => FILE_ATTRIBUTE_SYSTEM,
            'a' => FILE_ATTRIBUTE_ARCHIVE,
            'd' => FILE_ATTRIBUTE_DIRECTORY,
            other => return Err(format!("Unknown attribute flag `{other}`.")),
        };
    }
    if mask == 0 {
        return Err("`attrib:` needs at least one flag.".into());
    }
    Ok((mask, set))
}

fn local_today() -> Option<(i64, i64, i64)> {
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    let mut time: windows_sys::Win32::Foundation::SYSTEMTIME = unsafe { std::mem::zeroed() };
    unsafe {
        GetLocalTime(&mut time);
    }
    Some((time.wYear as i64, time.wMonth as i64, time.wDay as i64))
}

fn local_midnight(year: i64, month: i64, day: i64) -> Option<u64> {
    use windows_sys::Win32::Foundation::{FILETIME, SYSTEMTIME};
    use windows_sys::Win32::System::Time::{SystemTimeToFileTime, TzSpecificLocalTimeToSystemTime};
    if !(1601..=9999).contains(&year) {
        return None;
    }
    let local = SYSTEMTIME {
        wYear: year as u16,
        wMonth: month as u16,
        wDayOfWeek: 0,
        wDay: day as u16,
        wHour: 0,
        wMinute: 0,
        wSecond: 0,
        wMilliseconds: 0,
    };
    let mut utc: SYSTEMTIME = unsafe { std::mem::zeroed() };
    let ok = unsafe { TzSpecificLocalTimeToSystemTime(std::ptr::null(), &local, &mut utc) };
    if ok == 0 {
        return None;
    }
    let mut file_time: FILETIME = unsafe { std::mem::zeroed() };
    let ok = unsafe { SystemTimeToFileTime(&utc, &mut file_time) };
    if ok == 0 {
        return None;
    }
    const EPOCH: u64 = 11_644_473_600 * 10_000_000;
    let ticks = ((file_time.dwHighDateTime as u64) << 32) | file_time.dwLowDateTime as u64;
    if ticks < EPOCH {
        return None;
    }
    Some((ticks - EPOCH) / 10_000_000)
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_prime = (month + 9).rem_euclid(12);
    let day_of_year = (153 * month_prime + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/* --------------------------------------------------------------- startup */

/// Loads the persisted index in the background during application startup.
pub fn start_cached_load(app: tauri::AppHandle, state: Arc<SearchState>) {
    use tauri::Manager;
    let Ok(directory) = app.path().app_data_dir() else {
        return;
    };
    state.start_load(directory);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn civil_dates_round_trip_and_convert_to_local_midnight() {
        for days in [-10_000i64, 0, 1, 19_000, 20_000, 50_000] {
            let (year, month, day) = civil_from_days(days);
            assert_eq!(days_from_civil(year, month, day), days);
        }
        let today = local_today().expect("local today");
        assert!(local_midnight(today.0, today.1, today.2).is_some());
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|value| value.as_nanos())
                .unwrap_or(0);
            let path = std::env::temp_dir()
                .join(format!("cyron-search-{tag}-{}-{nanos}", std::process::id()));
            fs::create_dir_all(&path).expect("temp directory");
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn meta(is_dir: bool, size: u64, modified: u64) -> Meta {
        meta_with(
            is_dir,
            size,
            modified,
            if is_dir {
                FILE_ATTRIBUTE_DIRECTORY
            } else {
                FILE_ATTRIBUTE_ARCHIVE
            },
        )
    }

    fn meta_with(is_dir: bool, size: u64, modified: u64, attrs: u32) -> Meta {
        Meta {
            is_dir,
            size,
            modified: modified as u32,
            created: modified as u32,
            accessed: modified as u32,
            attrs,
        }
    }

    fn sample() -> FileIndex {
        let mut index = FileIndex::new();
        let root = index.add_root("C:\\", "System");
        assert_ne!(root, NONE);
        let users = index
            .add("Users", root, &meta(true, 0, 1_700_000_000))
            .expect("users");
        let docs = index
            .add("Docs", users, &meta(true, 0, 1_700_000_100))
            .expect("docs");
        index
            .add("report.pdf", docs, &meta(false, 4096, 1_700_000_200))
            .expect("report");
        index
            .add("notes.txt", docs, &meta(false, 100, 1_600_000_000))
            .expect("notes");
        index
            .add("photo.JPG", users, &meta(false, 2_000_000, 1_700_000_300))
            .expect("photo");
        index
            .add("archive.zip", root, &meta(false, 3_000_000, 1_650_000_000))
            .expect("archive");
        let copy = index
            .add("Copy", root, &meta(true, 0, 1_700_000_000))
            .expect("copy");
        index
            .add("notes.txt", copy, &meta(false, 100, 1_600_000_000))
            .expect("copy notes");
        index
            .add("empty", root, &meta(true, 0, 1_700_000_000))
            .expect("empty");
        index
            .add(
                "secret.dat",
                root,
                &meta_with(
                    false,
                    42,
                    1_700_000_000,
                    FILE_ATTRIBUTE_ARCHIVE | FILE_ATTRIBUTE_HIDDEN,
                ),
            )
            .expect("secret");
        index
            .add("recent.log", root, &meta(false, 10, now_unix()))
            .expect("recent");
        index.seed_paths();
        index
    }

    fn options(text: &str) -> SearchQueryOptions {
        SearchQueryOptions {
            query: text.to_string(),
            ..Default::default()
        }
    }

    fn run(index: &FileIndex, text: &str) -> SearchIndexPage {
        query_index(index, &options(text)).expect("query")
    }

    #[test]
    fn reconstructs_paths_and_finds_names() {
        let index = sample();
        let page = run(&index, "report");
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].name, "report.pdf");
        assert_eq!(page.items[0].full_path, "C:\\Users\\Docs\\report.pdf");
        assert_eq!(page.items[0].parent_path, "C:\\Users\\Docs");
        assert_eq!(page.items[0].size, 4096);
        assert!(!page.items[0].is_folder);
        assert_eq!(page.files, 1);
        assert_eq!(page.folders, 0);
    }

    #[test]
    fn supports_boolean_phrases_negation_and_wildcards() {
        let index = sample();
        assert_eq!(run(&index, "report|notes").total, 3);
        assert_eq!(run(&index, "report notes").total, 0);
        assert_eq!(run(&index, "!report ext:txt").total, 2);
        assert_eq!(run(&index, "\"report.pdf\"").total, 1);
        assert_eq!(run(&index, "*.pdf").total, 1);
        assert_eq!(run(&index, "*OTE*").total, 2);
        assert_eq!(run(&index, "(report|notes) !copy").total, 3);
    }

    #[test]
    fn respects_case_and_whole_word_options() {
        let index = sample();
        let mut case_sensitive = options("NOTE");
        case_sensitive.match_case = true;
        assert_eq!(query_index(&index, &case_sensitive).unwrap().total, 0);

        let mut whole_note = options("note");
        whole_note.whole_word = true;
        assert_eq!(query_index(&index, &whole_note).unwrap().total, 0);

        let mut whole_notes = options("notes");
        whole_notes.whole_word = true;
        assert_eq!(query_index(&index, &whole_notes).unwrap().total, 2);
    }

    #[test]
    fn filters_by_extension_size_kind_attributes_and_type() {
        let index = sample();
        assert_eq!(run(&index, "ext:PDF").total, 1);
        assert_eq!(run(&index, "size:>1mb").total, 2);
        assert_eq!(run(&index, "size:1kb..5kb").total, 1);
        assert_eq!(run(&index, "folder:").total, 5);
        assert_eq!(run(&index, "type:image").total, 1);
        assert_eq!(run(&index, "attrib:h").total, 1);
        assert_eq!(run(&index, "attrib:-h size:>0").total, 6);
        assert_eq!(run(&index, "dupe:").total, 2);
        assert_eq!(run(&index, "empty:").total, 1);
    }

    #[test]
    fn filters_by_date_keywords_and_ranges() {
        let index = sample();
        assert_eq!(run(&index, "dm:today").total, 1);
        assert_eq!(run(&index, "dc:today").total, 1);
        assert!(run(&index, "dm:>2020-01-01").total > 0);
        assert_eq!(run(&index, "dm:2000-01-01").total, 0);
        assert_eq!(run(&index, "dm:2000-01-01..2000-01-02").total, 0);
    }

    #[test]
    fn honors_the_type_and_file_kind_chips() {
        let index = sample();
        let mut kind_options = options("txt");
        kind_options.filter = "files".into();
        assert_eq!(query_index(&index, &kind_options).unwrap().total, 2);
        kind_options.filter = "folders".into();
        assert_eq!(query_index(&index, &kind_options).unwrap().total, 0);

        let mut folders = options("users");
        folders.filter = "folders".into();
        assert_eq!(query_index(&index, &folders).unwrap().total, 1);
    }

    #[test]
    fn matches_paths_only_when_requested() {
        let index = sample();
        assert_eq!(run(&index, "docs").total, 1);
        let mut path_options = options("docs");
        path_options.match_path = true;
        assert_eq!(query_index(&index, &path_options).unwrap().total, 3);
        assert_eq!(run(&index, "path:docs").total, 3);
    }

    #[test]
    fn sorts_and_paginates_stable_results() {
        let index = sample();
        let mut options = options("size:>1mb");
        options.sort = "size".into();
        options.descending = true;
        let page = query_index(&index, &options).unwrap();
        assert_eq!(page.total, 2);
        assert_eq!(page.items[0].name, "archive.zip");
        assert_eq!(page.items[1].name, "photo.JPG");

        options.limit = 1;
        options.offset = 0;
        let first = query_index(&index, &options).unwrap();
        assert_eq!(first.items.len(), 1);
        assert!(first.has_more);
        options.offset = 1;
        let second = query_index(&index, &options).unwrap();
        assert_eq!(second.items[0].name, "photo.JPG");
        assert!(!second.has_more);
    }

    #[test]
    fn builds_a_real_directory_tree() {
        let temp = TempDir::new("build");
        fs::create_dir_all(temp.0.join("alpha").join("beta")).expect("nested");
        fs::write(temp.0.join("root.txt"), b"root").expect("root file");
        fs::write(temp.0.join("alpha").join("one.bin"), vec![0u8; 2048]).expect("bin");
        fs::write(temp.0.join("alpha").join("beta").join("deep.txt"), b"deep").expect("deep file");

        let mount = temp.0.to_string_lossy().to_string();
        let definitions = vec![(mount.clone(), "Fixture".to_string())];
        let index = build_index(
            &definitions,
            &AtomicBool::new(false),
            &mut |_, _, _| {},
            &mut Vec::new(),
        );
        assert!(index.files >= 3);
        assert!(index.folders >= 3);
        let page = run(&index, "deep");
        assert_eq!(page.total, 1);
        assert_eq!(
            page.items[0].full_path,
            format!("{mount}\\alpha\\beta\\deep.txt")
        );
        let deep = index
            .find_path(&format!("{mount}\\alpha\\beta\\deep.txt").to_lowercase())
            .expect("indexed deep file");
        assert!(index.entry(deep).created > 0);
        assert!(run(&index, "size:>1kb").total >= 1);
    }

    #[test]
    fn persists_and_reloads_the_index() {
        let temp = TempDir::new("persist");
        let path = temp.0.join("index.bin");
        let index = sample();
        index.save(&path).expect("save");
        let loaded = load_index(&path).expect("load");
        assert_eq!(loaded.count(), index.count());
        assert_eq!(loaded.files, index.files);
        assert_eq!(loaded.folders, index.folders);
        assert_eq!(
            run(&loaded, "report").items[0].full_path,
            "C:\\Users\\Docs\\report.pdf"
        );
        assert_eq!(run(&loaded, "dupe:").total, 2);
        assert_eq!(loaded.find_path("c:\\copy\\notes.txt").is_some(), true);
    }

    #[test]
    fn rejects_a_corrupt_saved_index() {
        let temp = TempDir::new("corrupt");
        let path = temp.0.join("index.bin");
        fs::write(&path, b"not a cyron index").expect("fixture");
        assert!(load_index(&path).is_err());
    }

    #[test]
    fn rename_and_delete_keep_descendants_consistent() {
        let mut index = sample();
        let docs = index.find_path("c:\\users\\docs").expect("docs");
        let users = index.find_path("c:\\users").expect("users");
        index.rename(docs, "Papers", users);
        assert!(index.find_path("c:\\users\\docs").is_none());
        let notes = index
            .find_path("c:\\users\\papers\\notes.txt")
            .expect("renamed child");
        assert_eq!(index.path_of(notes), "C:\\Users\\Papers\\notes.txt");
        assert_eq!(index.parent_path(notes), "C:\\Users\\Papers");

        index.tombstone(docs);
        assert!(index.find_path("c:\\users\\papers\\notes.txt").is_none());
        assert!(index.find_path("c:\\users\\papers\\report.pdf").is_none());
        assert_eq!(index.files, 5);
        assert_eq!(index.folders, 4);
    }

    #[test]
    fn rejects_invalid_or_hostile_queries() {
        let index = sample();
        for query in [
            "",
            "   ",
            "ext:",
            "size:1xb",
            "dm:notadate",
            "type:unknown",
            "regex:[",
            "\"unclosed",
            "(alpha",
            "alpha )",
            "!",
            "size:10gb..1kb",
        ] {
            assert!(
                query_index(&index, &options(query)).is_err(),
                "{query} should be rejected"
            );
        }
        let long = "x".repeat(MAX_QUERY_CHARS + 1);
        assert!(query_index(&index, &options(&long)).is_err());
    }

    fn notification_bytes(action: u32, name: &str, next: u32) -> Vec<u8> {
        let units: Vec<u16> = name.encode_utf16().collect();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&next.to_le_bytes());
        bytes.extend_from_slice(&action.to_le_bytes());
        bytes.extend_from_slice(&((units.len() * 2) as u32).to_le_bytes());
        for unit in units {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        while bytes.len() % 4 != 0 {
            bytes.push(0);
        }
        bytes
    }

    #[test]
    fn watcher_updates_add_remove_and_rename_indexed_paths() {
        let temp = TempDir::new("watch");
        fs::create_dir_all(temp.0.join("sub")).expect("sub");
        fs::write(temp.0.join("sub").join("old.txt"), b"old").expect("old file");
        let mount = temp.0.to_string_lossy().to_string();
        let definitions = vec![(mount.clone(), "Fixture".to_string())];
        let index = build_index(
            &definitions,
            &AtomicBool::new(false),
            &mut |_, _, _| {},
            &mut Vec::new(),
        );
        let state = SearchState::default();
        *state
            .index
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = index;

        fs::write(temp.0.join("sub").join("new.txt"), b"new").expect("new file");
        apply_notifications(&state, &mount, &notification_bytes(1, "sub\\new.txt", 0));
        let new_path = format!("{mount}\\sub\\new.txt").to_lowercase();
        assert!(state
            .index
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .find_path(&new_path)
            .is_some());

        fs::rename(
            temp.0.join("sub").join("new.txt"),
            temp.0.join("sub").join("renamed.txt"),
        )
        .expect("rename");
        let mut buffer = notification_bytes(4, "sub\\new.txt", 0);
        let second = notification_bytes(5, "sub\\renamed.txt", 0);
        let first_length = buffer.len() as u32;
        buffer[0..4].copy_from_slice(&first_length.to_le_bytes());
        buffer.extend(second);
        apply_notifications(&state, &mount, &buffer);
        let renamed_path = format!("{mount}\\sub\\renamed.txt").to_lowercase();
        {
            let index = state
                .index
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            assert!(index.find_path(&new_path).is_none());
            assert!(index.find_path(&renamed_path).is_some());
        }

        fs::remove_file(temp.0.join("sub").join("renamed.txt")).expect("remove");
        apply_notifications(
            &state,
            &mount,
            &notification_bytes(2, "sub\\renamed.txt", 0),
        );
        assert!(state
            .index
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .find_path(&renamed_path)
            .is_none());
    }

    #[test]
    fn cancel_before_the_walk_leaves_no_partial_index() {
        let temp = TempDir::new("cancel");
        fs::write(temp.0.join("file.txt"), b"x").expect("fixture");
        let definitions = vec![(temp.0.to_string_lossy().to_string(), "Fixture".to_string())];
        let cancel = AtomicBool::new(true);
        let index = build_index(&definitions, &cancel, &mut |_, _, _| {}, &mut Vec::new());
        assert!(index.is_empty());
    }

    #[test]
    fn status_starts_empty_and_query_requires_an_index() {
        let state = SearchState::default();
        let status = state.status();
        assert_eq!(status.state, "empty");
        assert!(!status.ready);
        assert!(state.query(&options("anything")).is_err());
    }

    #[test]
    fn searchindex_export_writes_csv_txt_and_validates() {
        let index = sample();
        let temp = TempDir::new("export");
        let csv = temp.0.join("results.csv");
        let csv_text = csv.to_string_lossy().to_string();
        let report = export_index(&index, &options("notes"), "csv", &csv_text, None)
            .expect("csv export");
        assert_eq!(report.rows, 2);
        assert_eq!(report.skipped, 0);
        let content = fs::read_to_string(&csv).expect("read csv");
        assert!(content.starts_with("Name,Path,Type,SizeBytes,Created,Modified\r\n"));
        assert_eq!(content.lines().count(), 3);
        assert!(content.contains("C:\\Users\\Docs\\notes.txt"));

        let txt = temp.0.join("results.txt");
        let txt_text = txt.to_string_lossy().to_string();
        let selected = vec![
            "C:\\Users\\Docs\\notes.txt".to_string(),
            "C:\\missing.txt".to_string(),
        ];
        let report = export_index(&index, &options("ignored"), "txt", &txt_text, Some(&selected))
            .expect("txt export");
        assert_eq!(report.rows, 1);
        assert_eq!(report.skipped, 1);
        assert_eq!(
            fs::read_to_string(&txt).expect("read txt").trim(),
            "C:\\Users\\Docs\\notes.txt"
        );

        assert!(export_index(&index, &options("notes"), "pdf", &csv_text, None).is_err());
        let wrong_extension = temp.0.join("results.txt");
        assert!(export_index(
            &index,
            &options("notes"),
            "csv",
            &wrong_extension.to_string_lossy(),
            None
        )
        .is_err());
        assert!(export_index(&index, &options("notes"), "csv", "relative.csv", None).is_err());

        let mut weird = FileIndex::new();
        let root = weird.add_root("C:\\", "Weird");
        weird
            .add("a,\"b\".txt", root, &meta(false, 1, 1_700_000_000))
            .expect("weird name");
        weird.seed_paths();
        weird.sort_name_order();
        let escaped = temp.0.join("escaped.csv");
        export_index(
            &weird,
            &options("a"),
            "csv",
            &escaped.to_string_lossy(),
            None,
        )
        .expect("escaped csv");
        let escaped = fs::read_to_string(&escaped).expect("read escaped");
        assert!(escaped.contains("\"a,\"\"b\"\".txt\""));
    }

    /// Repeatable local benchmark. Run explicitly in release mode:
    /// cargo test --release --lib searchindex_perf -- --ignored --nocapture
    #[test]
    #[ignore = "manual performance benchmark"]
    fn searchindex_perf_fixture() {
        const FILES: usize = 20_000;
        const DIRS: usize = 200;
        const QUERY_ENTRIES: usize = 500_000;

        let temp = TempDir::new("perf");
        for directory in 0..DIRS {
            let path = temp.0.join(format!("dir-{directory:03}"));
            fs::create_dir_all(&path).expect("perf directory");
            for file in 0..(FILES / DIRS) {
                fs::write(path.join(format!("document-{directory}-{file}.txt")), b"x")
                    .expect("perf file");
            }
        }
        let definitions = vec![(temp.0.to_string_lossy().to_string(), "Fixture".to_string())];
        let started = Instant::now();
        let fixture = build_index(
            &definitions,
            &AtomicBool::new(false),
            &mut |_, _, _| {},
            &mut Vec::new(),
        );
        let build_ms = started.elapsed().as_millis();
        assert_eq!(fixture.entries.len(), FILES + DIRS + 1);

        let mut index = FileIndex::new();
        let root = index.add_root("C:\\", "Synthetic");
        assert_ne!(root, NONE);
        for value in 0..QUERY_ENTRIES {
            let name = format!("document-{value}.txt");
            let meta = meta(
                false,
                ((value as u64 * 7919) % 1_000_000) + 1,
                1_700_000_000 + value as u64,
            );
            assert!(index.add(&name, root, &meta).is_some());
        }
        index.sort_name_order();
        let pool_bytes = index.names.len() + index.lower.len();
        let entry_bytes = index.entries.len() * std::mem::size_of::<Entry>();
        let mut request = options("document");
        request.limit = 100;
        let query_started = Instant::now();
        let page = query_index(&index, &request).expect("perf query");
        let query_ms = query_started.elapsed().as_millis();
        assert_eq!(page.total as usize, QUERY_ENTRIES);
        assert_eq!(page.items.len(), 100);

        let mut size_request = options("document");
        size_request.sort = "size".into();
        size_request.descending = true;
        size_request.limit = 100;
        let size_started = Instant::now();
        let size_page = query_index(&index, &size_request).expect("perf size query");
        let size_query_ms = size_started.elapsed().as_millis();
        assert_eq!(size_page.total as usize, QUERY_ENTRIES);
        assert_eq!(size_page.items.len(), 100);

        let mut rare_request = options("document-499999.txt");
        rare_request.limit = 100;
        let rare_started = Instant::now();
        let rare_page = query_index(&index, &rare_request).expect("perf rare query");
        let rare_query_ms = rare_started.elapsed().as_millis();
        assert_eq!(rare_page.total, 1);

        // Pre-change baseline on this machine: build 355 ms, name query 21 ms.
        // These ceilings are deliberately generous but still fail that baseline.
        assert!(build_ms <= 300, "index build regressed to {build_ms} ms");
        assert!(query_ms <= 18, "name query regressed to {query_ms} ms");
        assert!(size_query_ms <= 60, "size query regressed to {size_query_ms} ms");
        assert!(rare_query_ms <= 30, "rare query regressed to {rare_query_ms} ms");

        println!(
            "SEARCH_INDEX_PERF build_ms={build_ms} query_ms={query_ms} size_query_ms={size_query_ms} rare_query_ms={rare_query_ms} fixture_entries={} query_entries={QUERY_ENTRIES} pools_bytes={pool_bytes} entry_bytes={entry_bytes} path_entries={}",
            fixture.entries.len(),
            index.paths.len()
        );
        println!("SEARCH_INDEX_PERF_OK");
    }
}
