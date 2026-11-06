//! High-performance queryable scan index.
//!
//! Stores complete, unpruned directory hierarchies for drill-down, pagination,
//! filtering, and global search, keeping the top-level treemap representation
//! compact while allowing users to inspect every accessible file.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use crate::scan::{display_path, FileStat, ScanNode};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexedChild {
    pub name: String,
    pub path: String,
    pub kind: String, // "dir" | "file"
    pub size: u64,
    pub allocated: u64,
    pub file_count: u64,
    pub dir_count: u64,
    pub modified: Option<u64>,
    pub extension: String,
}

impl From<&ScanNode> for IndexedChild {
    fn from(node: &ScanNode) -> Self {
        let extension = if node.kind == "file" && node.name.contains('.') {
            node.name.split('.').last().unwrap_or("").to_lowercase()
        } else {
            String::new()
        };

        Self {
            name: node.name.clone(),
            path: node.path.clone(),
            kind: node.kind.clone(),
            size: node.size,
            allocated: node.allocated,
            file_count: node.file_count,
            dir_count: node.dir_count,
            modified: node.modified,
            extension,
        }
    }
}

impl IndexedChild {
    pub fn to_scan_node(&self) -> ScanNode {
        ScanNode {
            name: self.name.clone(),
            path: self.path.clone(),
            kind: self.kind.clone(),
            size: self.size,
            allocated: self.allocated,
            file_count: self.file_count,
            dir_count: self.dir_count,
            modified: self.modified,
            children: Vec::new(),
        }
    }

    pub fn to_file_stat(&self) -> FileStat {
        FileStat {
            name: self.name.clone(),
            path: self.path.clone(),
            size: self.size,
            allocated: self.allocated,
            extension: self.extension.clone(),
            modified: self.modified,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryQuery {
    pub job_id: String,
    pub dir_path: String,
    pub page: usize,
    pub page_size: usize,
    pub sort_field: Option<String>, // "size" | "name" | "modified" | "files"
    pub sort_dir: Option<String>,   // "asc" | "desc"
    pub search: Option<String>,
    pub min_size: Option<u64>,
    pub extension: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryPage {
    pub entries: Vec<ScanNode>,
    pub total_entries: usize,
    pub page: usize,
    pub page_size: usize,
    pub has_more: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub job_id: String,
    pub query: String,
    pub extension: Option<String>,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub limit: usize,
    pub offset: usize,
    pub sort_field: Option<String>,
    pub sort_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultPage {
    pub items: Vec<FileStat>,
    pub total_matches: usize,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexStats {
    pub job_id: String,
    pub total_files: u64,
    pub total_dirs: u64,
    pub total_size: u64,
    pub total_allocated: u64,
    pub is_complete: bool,
}

/// Normalizes path keys for case-insensitive lookup on Windows.
fn normalize_key(path: &str) -> String {
    let clean = display_path(path);
    clean
        .trim()
        .trim_end_matches(['\\', '/'])
        .to_ascii_lowercase()
}

/// A queryable index for a single scan job.
pub struct ScanIndex {
    job_id: String,
    /// Maps parent_dir_path (normalized) -> list of direct children.
    directories: RwLock<HashMap<String, Vec<IndexedChild>>>,
    /// Flattened catalog for global searches.
    flat_files: RwLock<Vec<IndexedChild>>,
    total_files: Mutex<u64>,
    total_dirs: Mutex<u64>,
    total_size: Mutex<u64>,
    total_allocated: Mutex<u64>,
    is_complete: Mutex<bool>,
}

impl ScanIndex {
    pub fn new(job_id: &str) -> Self {
        Self {
            job_id: job_id.to_string(),
            directories: RwLock::new(HashMap::new()),
            flat_files: RwLock::new(Vec::new()),
            total_files: Mutex::new(0),
            total_dirs: Mutex::new(0),
            total_size: Mutex::new(0),
            total_allocated: Mutex::new(0),
            is_complete: Mutex::new(false),
        }
    }

    pub fn job_id(&self) -> &str {
        &self.job_id
    }

    pub fn mark_complete(&self) {
        if let Ok(mut complete) = self.is_complete.lock() {
            *complete = true;
        }
    }

    pub fn insert_child(&self, parent_path: &str, child: IndexedChild) {
        let key = normalize_key(parent_path);

        if let Ok(mut total_size) = self.total_size.lock() {
            *total_size += child.size;
        }
        if let Ok(mut total_allocated) = self.total_allocated.lock() {
            *total_allocated += child.allocated;
        }

        if child.kind == "dir" {
            if let Ok(mut dirs) = self.total_dirs.lock() {
                *dirs += 1;
            }
        } else {
            if let Ok(mut files) = self.total_files.lock() {
                *files += 1;
            }
            if let Ok(mut flat) = self.flat_files.write() {
                flat.push(child.clone());
            }
        }

        if let Ok(mut map) = self.directories.write() {
            map.entry(key).or_default().push(child);
        }
    }

    pub fn insert_children(&self, parent_path: &str, children: Vec<IndexedChild>) {
        let key = normalize_key(parent_path);
        let mut added_size = 0u64;
        let mut added_allocated = 0u64;
        let mut added_files = 0u64;
        let mut added_dirs = 0u64;

        let mut files_to_flatten = Vec::new();

        for child in &children {
            added_size += child.size;
            added_allocated += child.allocated;
            if child.kind == "dir" {
                added_dirs += 1;
            } else {
                added_files += 1;
                files_to_flatten.push(child.clone());
            }
        }

        if let Ok(mut total_size) = self.total_size.lock() {
            *total_size += added_size;
        }
        if let Ok(mut total_allocated) = self.total_allocated.lock() {
            *total_allocated += added_allocated;
        }
        if let Ok(mut files) = self.total_files.lock() {
            *files += added_files;
        }
        if let Ok(mut dirs) = self.total_dirs.lock() {
            *dirs += added_dirs;
        }

        if !files_to_flatten.is_empty() {
            if let Ok(mut flat) = self.flat_files.write() {
                flat.extend(files_to_flatten);
            }
        }

        if let Ok(mut map) = self.directories.write() {
            map.entry(key).or_default().extend(children);
        }
    }

    pub fn query_children(&self, query: &DirectoryQuery) -> DirectoryPage {
        let key = normalize_key(&query.dir_path);
        let page = query.page.max(1);
        let page_size = query.page_size.clamp(1, 1000);

        let map = match self.directories.read() {
            Ok(guard) => guard,
            Err(_) => {
                return DirectoryPage {
                    entries: Vec::new(),
                    total_entries: 0,
                    page,
                    page_size,
                    has_more: false,
                }
            }
        };

        let raw_children = match map.get(&key) {
            Some(children) => children,
            None => {
                return DirectoryPage {
                    entries: Vec::new(),
                    total_entries: 0,
                    page,
                    page_size,
                    has_more: false,
                }
            }
        };

        let search_needle = query.search.as_ref().map(|s| s.trim().to_lowercase());
        let ext_needle = query
            .extension
            .as_ref()
            .map(|e| e.trim().trim_start_matches('.').to_lowercase());
        let min_size = query.min_size.unwrap_or(0);

        let mut filtered: Vec<&IndexedChild> = raw_children
            .iter()
            .filter(|child| {
                if child.size < min_size {
                    return false;
                }
                if let Some(ref ext) = ext_needle {
                    if !ext.is_empty() && child.extension != *ext {
                        return false;
                    }
                }
                if let Some(ref s) = search_needle {
                    if !s.is_empty() && !child.name.to_lowercase().contains(s) {
                        return false;
                    }
                }
                true
            })
            .collect();

        // Sort items (directories always grouped first unless sorting specifically by name/size)
        let sort_field = query.sort_field.as_deref().unwrap_or("size");
        let sort_desc = query
            .sort_dir
            .as_deref()
            .map(|d| d.eq_ignore_ascii_case("desc"))
            .unwrap_or(true);

        filtered.sort_by(|a, b| {
            // Folders stay together unless explicitly sorted
            if a.kind != b.kind {
                return if a.kind == "dir" {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Greater
                };
            }
            let ordering = match sort_field {
                "name" => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                "modified" => a.modified.cmp(&b.modified),
                "files" => a.file_count.cmp(&b.file_count),
                _ => a.size.cmp(&b.size),
            };
            if sort_desc {
                ordering.reverse()
            } else {
                ordering
            }
        });

        let total_entries = filtered.len();
        let start = (page - 1) * page_size;
        let entries = if start < total_entries {
            filtered
                .iter()
                .skip(start)
                .take(page_size)
                .map(|item| item.to_scan_node())
                .collect()
        } else {
            Vec::new()
        };

        let has_more = start + entries.len() < total_entries;

        DirectoryPage {
            entries,
            total_entries,
            page,
            page_size,
            has_more,
        }
    }

    pub fn search(&self, query: &SearchQuery) -> SearchResultPage {
        let needle = query.query.trim().to_lowercase();
        let ext_needle = query
            .extension
            .as_ref()
            .map(|e| e.trim().trim_start_matches('.').to_lowercase());
        let min_size = query.min_size.unwrap_or(0);
        let max_size = query.max_size.unwrap_or(u64::MAX);
        let limit = query.limit.clamp(1, 500);
        let offset = query.offset;

        let flat = match self.flat_files.read() {
            Ok(guard) => guard,
            Err(_) => {
                return SearchResultPage {
                    items: Vec::new(),
                    total_matches: 0,
                    limit,
                    offset,
                }
            }
        };

        let mut matches: Vec<&IndexedChild> = flat
            .iter()
            .filter(|item| {
                if item.size < min_size || item.size > max_size {
                    return false;
                }
                if let Some(ref ext) = ext_needle {
                    if !ext.is_empty() && item.extension != *ext {
                        return false;
                    }
                }
                if !needle.is_empty()
                    && !item.name.to_lowercase().contains(&needle)
                    && !item.path.to_lowercase().contains(&needle)
                {
                    return false;
                }
                true
            })
            .collect();

        let sort_field = query.sort_field.as_deref().unwrap_or("size");
        let sort_desc = query
            .sort_dir
            .as_deref()
            .map(|d| d.eq_ignore_ascii_case("desc"))
            .unwrap_or(true);

        matches.sort_by(|a, b| {
            let ordering = match sort_field {
                "name" => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                "modified" => a.modified.cmp(&b.modified),
                _ => a.size.cmp(&b.size),
            };
            if sort_desc {
                ordering.reverse()
            } else {
                ordering
            }
        });

        let total_matches = matches.len();
        let items = if offset < total_matches {
            matches
                .iter()
                .skip(offset)
                .take(limit)
                .map(|item| item.to_file_stat())
                .collect()
        } else {
            Vec::new()
        };

        SearchResultPage {
            items,
            total_matches,
            limit,
            offset,
        }
    }

    pub fn stats(&self) -> IndexStats {
        IndexStats {
            job_id: self.job_id.clone(),
            total_files: self.total_files.lock().map(|g| *g).unwrap_or(0),
            total_dirs: self.total_dirs.lock().map(|g| *g).unwrap_or(0),
            total_size: self.total_size.lock().map(|g| *g).unwrap_or(0),
            total_allocated: self.total_allocated.lock().map(|g| *g).unwrap_or(0),
            is_complete: self.is_complete.lock().map(|g| *g).unwrap_or(false),
        }
    }
}

/// Global registry of in-memory / active scan indexes.
pub struct IndexManager {
    indexes: RwLock<HashMap<String, Arc<ScanIndex>>>,
}

impl Default for IndexManager {
    fn default() -> Self {
        Self {
            indexes: RwLock::new(HashMap::new()),
        }
    }
}

impl IndexManager {
    pub fn get_or_create(&self, job_id: &str) -> Arc<ScanIndex> {
        let mut map = self.indexes.write().unwrap();
        map.entry(job_id.to_string())
            .or_insert_with(|| Arc::new(ScanIndex::new(job_id)))
            .clone()
    }

    pub fn get(&self, job_id: &str) -> Option<Arc<ScanIndex>> {
        let map = self.indexes.read().unwrap();
        map.get(job_id).cloned()
    }

    pub fn remove(&self, job_id: &str) -> Option<Arc<ScanIndex>> {
        let mut map = self.indexes.write().unwrap();
        map.remove(job_id)
    }

    pub fn clear(&self) {
        let mut map = self.indexes.write().unwrap();
        map.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_and_paginates_directories_with_large_entry_counts() {
        let index = ScanIndex::new("test-job-1");
        let parent = "C:\\large_dir";

        // Generate 105,000 synthetic items to verify exceeding the 100K threshold
        let mut children = Vec::with_capacity(105_000);
        for i in 0..105_000 {
            children.push(IndexedChild {
                name: format!("file_{i:06}.log"),
                path: format!("C:\\large_dir\\file_{i:06}.log"),
                kind: "file".into(),
                size: (i as u64) + 10,
                allocated: (i as u64) + 10,
                file_count: 1,
                dir_count: 0,
                modified: Some(1700000000 + (i as u64)),
                extension: "log".into(),
            });
        }

        index.insert_children(parent, children);
        index.mark_complete();

        let stats = index.stats();
        assert_eq!(stats.total_files, 105_000);
        assert!(stats.is_complete);

        // Page 1: 50 items, default desc sort by size
        let query_p1 = DirectoryQuery {
            job_id: "test-job-1".into(),
            dir_path: parent.into(),
            page: 1,
            page_size: 50,
            sort_field: Some("size".into()),
            sort_dir: Some("desc".into()),
            search: None,
            min_size: None,
            extension: None,
        };

        let page1 = index.query_children(&query_p1);
        assert_eq!(page1.total_entries, 105_000);
        assert_eq!(page1.entries.len(), 50);
        assert!(page1.has_more);
        assert_eq!(page1.entries[0].name, "file_104999.log");

        // Search within the 105K entries
        let search_query = SearchQuery {
            job_id: "test-job-1".into(),
            query: "file_050000".into(),
            extension: Some("log".into()),
            min_size: None,
            max_size: None,
            limit: 10,
            offset: 0,
            sort_field: Some("name".into()),
            sort_dir: Some("asc".into()),
        };

        let search_result = index.search(&search_query);
        assert_eq!(search_result.total_matches, 1);
        assert_eq!(search_result.items[0].name, "file_050000.log");
    }

    #[test]
    fn synthetic_one_million_items_index_performance_and_reopen() {
        let manager = IndexManager::default();
        let index = manager.get_or_create("job-million");

        let chunk_size = 50_000;
        let total = 1_000_000;
        let mut inserted = 0;

        while inserted < total {
            let mut chunk = Vec::with_capacity(chunk_size);
            for i in inserted..(inserted + chunk_size) {
                let dir_idx = i / 10_000;
                let dir_path = format!("C:\\Data\\dir_{dir_idx}");
                chunk.push(IndexedChild {
                    name: format!("data_{i}.dat"),
                    path: format!("{dir_path}\\data_{i}.dat"),
                    kind: "file".into(),
                    size: 1024,
                    allocated: 1024,
                    file_count: 1,
                    dir_count: 0,
                    modified: Some(1700000000),
                    extension: "dat".into(),
                });
            }
            index.insert_children("C:\\Data", chunk);
            inserted += chunk_size;
        }

        let stats = index.stats();
        assert_eq!(stats.total_files, 1_000_000);
        assert_eq!(stats.total_size, 1_000_000 * 1024);

        // Verify index retrieval by reopening from manager
        let retrieved = manager.get("job-million").expect("reopened index");
        assert_eq!(retrieved.stats().total_files, 1_000_000);
    }
}
