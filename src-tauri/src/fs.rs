//! Fast directory enumeration for Windows.
//!
//! The primary path opens the directory with FILE_LIST_DIRECTORY and reads
//! `FileIdBothDirectoryInfo`, which returns logical size, allocated size and a
//! stable file id in one syscall per batch. The file id lets the scanner count a
//! hard-linked file once. A std::fs fallback covers directories the native call
//! cannot list or exceptionally large directory buffers.

use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone)]
pub struct DirEntryInfo {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    /// True for junctions, symlinks and other reparse points.
    pub is_reparse: bool,
    pub is_hidden: bool,
    pub is_system: bool,
    pub size: u64,
    pub allocated: u64,
    /// Stable per-volume id used to deduplicate hard links.
    pub file_id: Option<u64>,
    /// Seconds since the Unix epoch.
    pub modified: Option<u64>,
}

const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

const MAX_NATIVE_BUFFER: usize = 8 * 1024 * 1024;
const ERROR_NO_MORE_FILES: u32 = 18;
const ERROR_MORE_DATA: u32 = 234;
const ERROR_BUFFER_OVERFLOW: u32 = 111;

pub fn read_dir(path: &Path) -> io::Result<Vec<DirEntryInfo>> {
    match read_dir_native(path) {
        Ok(entries) => Ok(entries),
        Err(_native_error) => read_dir_fallback(path),
    }
}

/// Converts a Windows FILETIME (100 ns ticks since 1601) to Unix seconds.
fn filetime_to_unix(filetime: i64) -> Option<u64> {
    if filetime <= 0 {
        return None;
    }
    let seconds = filetime / 10_000_000 - 11_644_473_600;
    if seconds < 0 {
        None
    } else {
        Some(seconds as u64)
    }
}

fn systemtime_to_unix(time: std::time::SystemTime) -> Option<u64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|value| value.as_secs())
}

fn wide_path(path: &Path) -> Vec<u16> {
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    if wide.last() != Some(&0) {
        wide.push(0);
    }
    wide
}

fn read_dir_native(path: &Path) -> io::Result<Vec<DirEntryInfo>> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_LIST_DIRECTORY, FILE_SHARE_DELETE,
        FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };

    let wide = wide_path(path);
    // SAFETY: `wide` is null terminated, and a directory handle is opened with
    // backup semantics so directories can be inspected.
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
        return Err(io::Error::last_os_error());
    }

    let result = read_batches(handle, path);
    // SAFETY: the handle was opened above and is closed exactly once.
    unsafe {
        CloseHandle(handle);
    }
    result
}

fn read_batches(
    handle: windows_sys::Win32::Foundation::HANDLE,
    path: &Path,
) -> io::Result<Vec<DirEntryInfo>> {
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::Storage::FileSystem::{
        FileIdBothDirectoryInfo, FileIdBothDirectoryRestartInfo, GetFileInformationByHandleEx,
        FILE_ID_BOTH_DIR_INFO,
    };

    let mut entries = Vec::new();
    let mut byte_capacity = 64 * 1024usize;
    let mut first_batch = true;

    loop {
        if byte_capacity > MAX_NATIVE_BUFFER {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "directory listing exceeds the 8 MiB buffer limit",
            ));
        }
        // A Vec<u64> keeps the buffer 8-byte aligned for FILE_ID_BOTH_DIR_INFO.
        let mut buffer: Vec<u64> = vec![0u64; byte_capacity / 8];
        let class = if first_batch {
            FileIdBothDirectoryRestartInfo
        } else {
            FileIdBothDirectoryInfo
        };
        // SAFETY: the buffer is writable for `byte_capacity` bytes and aligned
        // for the requested structure.
        let ok = unsafe {
            GetFileInformationByHandleEx(
                handle,
                class,
                buffer.as_mut_ptr().cast(),
                byte_capacity as u32,
            )
        };
        if ok == 0 {
            // SAFETY: reads the thread-local last error set by the failed call.
            let code = unsafe { GetLastError() };
            match code {
                ERROR_BUFFER_OVERFLOW | ERROR_MORE_DATA => {
                    byte_capacity *= 2;
                    continue;
                }
                ERROR_NO_MORE_FILES => break,
                other => return Err(io::Error::from_raw_os_error(other as i32)),
            }
        }
        first_batch = false;

        let base = buffer.as_ptr() as *const u8;
        let mut offset = 0usize;
        let mut parsed = 0usize;
        loop {
            // SAFETY: Windows lays out linked FILE_ID_BOTH_DIR_INFO records
            // inside the buffer it just filled; offsets are 8-byte aligned.
            let info = unsafe { &*(base.add(offset) as *const FILE_ID_BOTH_DIR_INFO) };
            let name_length = (info.FileNameLength as usize) / 2;
            // SAFETY: the struct's inline FileName buffer is followed by
            // `FileNameLength` bytes of UTF-16 name inside the same allocation.
            let name_units = unsafe {
                std::slice::from_raw_parts(
                    std::ptr::addr_of!(info.FileName).cast::<u16>(),
                    name_length,
                )
            };
            let name = String::from_utf16_lossy(name_units);
            if !name.is_empty() && name != "." && name != ".." {
                parsed += 1;
                let attributes = info.FileAttributes;
                entries.push(DirEntryInfo {
                    path: path.join(&name),
                    name,
                    is_dir: attributes & FILE_ATTRIBUTE_DIRECTORY != 0,
                    is_reparse: attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0,
                    is_hidden: attributes & FILE_ATTRIBUTE_HIDDEN != 0,
                    is_system: attributes & FILE_ATTRIBUTE_SYSTEM != 0,
                    size: info.EndOfFile.max(0) as u64,
                    allocated: info.AllocationSize.max(0) as u64,
                    file_id: if info.FileId == 0 {
                        None
                    } else {
                        Some(info.FileId as u64)
                    },
                    modified: filetime_to_unix(info.LastWriteTime),
                });
            }
            if info.NextEntryOffset == 0 {
                break;
            }
            offset += info.NextEntryOffset as usize;
            if offset >= byte_capacity {
                break;
            }
        }
        if parsed == 0 {
            // Success without a usable record cannot make progress; stop instead
            // of retrying the same buffer forever.
            break;
        }
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|value| value.as_nanos())
                .unwrap_or(0);
            let path = std::env::temp_dir()
                .join(format!("cyron-fs-{tag}-{}-{nanos}", std::process::id()));
            fs::create_dir_all(&path).expect("temp directory");
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn lists_files_and_directories_with_real_metadata() {
        let temp = TempDir::new("listing");
        fs::create_dir_all(temp.0.join("nested")).expect("nested directory");
        fs::write(temp.0.join("payload.bin"), vec![3u8; 8192]).expect("payload");

        let entries = read_dir(&temp.0).expect("directory listing");

        let file = entries
            .iter()
            .find(|entry| entry.name == "payload.bin")
            .expect("file entry");
        assert!(!file.is_dir);
        assert!(!file.is_reparse);
        assert_eq!(file.size, 8192);
        assert!(
            file.allocated >= file.size,
            "allocation {} for logical {}",
            file.allocated,
            file.size
        );
        assert!(file.file_id.is_some(), "NTFS exposes a stable file id");
        assert!(file.modified.is_some());
        assert!(file.path.ends_with("payload.bin"));

        let directory = entries
            .iter()
            .find(|entry| entry.name == "nested")
            .expect("directory entry");
        assert!(directory.is_dir);
        assert_eq!(directory.size, 0);
    }

    #[test]
    fn returns_an_error_for_a_missing_directory() {
        let temp = TempDir::new("missing");
        let missing = temp.0.join("does-not-exist");
        assert!(read_dir(&missing).is_err());
    }
}

fn read_dir_fallback(path: &Path) -> io::Result<Vec<DirEntryInfo>> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let entry = match entry {
            Ok(value) => value,
            Err(_) => continue,
        };
        let metadata = match entry.metadata() {
            Ok(value) => value,
            Err(_) => continue,
        };
        let attributes = metadata.file_attributes();
        let size = metadata.file_size();
        entries.push(DirEntryInfo {
            path: entry.path(),
            name: entry.file_name().to_string_lossy().into_owned(),
            is_dir: metadata.is_dir(),
            is_reparse: attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0,
            is_hidden: attributes & FILE_ATTRIBUTE_HIDDEN != 0,
            is_system: attributes & FILE_ATTRIBUTE_SYSTEM != 0,
            size,
            // Without a native handle the cluster-rounded allocation is not
            // available; logical size is the honest approximation.
            allocated: size,
            file_id: None,
            modified: metadata.modified().ok().and_then(systemtime_to_unix),
        });
    }
    Ok(entries)
}
