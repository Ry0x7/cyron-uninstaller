//! NTFS Master File Table reader.
//!
//! Reads `$MFT` directly through the volume handle and parses FILE records
//! instead of walking directories. This is the fast path for an elevated
//! process on an NTFS volume; every failure returns a user-facing reason so the
//! caller can fall back to the directory walker.
//!
//! The reader is strictly read-only: it opens `$MFT`, reads records, and never
//! writes to the volume.

use crate::scan::{display_path, ExtensionStat, FileStat, ScanNode, ScanOptions, ScanProgress};
use crate::storage::Accumulator;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const FILE_RECORD_SIZE: usize = 1024;
const ATTR_STANDARD_INFORMATION: u32 = 0x10;
const ATTR_FILE_NAME: u32 = 0x30;
const ATTR_DATA: u32 = 0x80;
const ATTR_END: u32 = 0xFFFF_FFFF;

const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

const MAX_ERRORS: usize = 20;
const MAX_EXTENSIONS: usize = 24;
const MAX_LARGEST_FILES: usize = 100;
const MAX_CHILDREN_PER_DIR: usize = 200;
const IPC_TREE_DEPTH: u32 = 8;
const MAX_EXTENSION_KEYS: usize = 4096;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(120);

/// Memory the parsed table may occupy. Everything the reader keeps per record
/// is covered by `ESTIMATED_BYTES_PER_RECORD`, so the record limit below is a
/// derived, checkable budget rather than a guess. Volumes above it are refused
/// with a reason and the caller falls back to the directory walk.
const MAX_RECORD_MEMORY_BYTES: u64 = 768 * 1024 * 1024;

/// Parsed record plus its child index entry, hash-map bucket and bitset bit.
/// Measured from the types so a future field change cannot silently double the
/// real footprint without the budget noticing.
const ESTIMATED_BYTES_PER_RECORD: u64 =
    std::mem::size_of::<Option<RawRecord>>() as u64 + std::mem::size_of::<u32>() as u64 + 24;

const fn record_limit() -> u64 {
    MAX_RECORD_MEMORY_BYTES / ESTIMATED_BYTES_PER_RECORD
}

/// Hard cap on records we are willing to hold; a volume with more records than
/// this is reported as unsupported instead of exhausting memory.
const MAX_RECORDS: u64 = record_limit();

/// Validates the record count reported by the `$MFT` size before anything is
/// allocated. Kept separate from the reader so the limits are testable.
fn validate_record_count(count: u64) -> Result<(), String> {
    if count == 0 {
        return Err("The NTFS master file table is empty.".into());
    }
    if count > MAX_RECORDS {
        return Err(format!(
            "This volume has {count} MFT records, above the supported limit of {MAX_RECORDS} \
             (about {} MiB of parsed records); using the directory walk instead.",
            MAX_RECORD_MEMORY_BYTES / (1024 * 1024)
        ));
    }
    Ok(())
}

/// Bitset of already-counted record numbers. Replaces a `HashSet<u64>` so the
/// hard-link bookkeeping costs one bit per record instead of a hash entry.
struct SeenRecords {
    bits: Vec<u64>,
}

impl SeenRecords {
    fn new(records: usize) -> Self {
        Self {
            bits: vec![0; records.div_ceil(64)],
        }
    }

    /// True when `number` was already visited during this traversal.
    fn is_seen(&self, number: u64) -> bool {
        let index = number as usize;
        let word = index / 64;
        word < self.bits.len() && self.bits[word] & (1u64 << (index % 64)) != 0
    }

    /// Returns true the first time `number` is seen. Record numbers come from
    /// the table index, so growth past the initial size is defensive only and
    /// still capped by `MAX_RECORDS`.
    fn mark(&mut self, number: u64) -> bool {
        let index = number as usize;
        let word = index / 64;
        if word >= self.bits.len() {
            if number >= MAX_RECORDS {
                return true;
            }
            self.bits.resize(word + 1, 0);
        }
        let mask = 1u64 << (index % 64);
        let fresh = self.bits[word] & mask == 0;
        self.bits[word] |= mask;
        fresh
    }
}

pub struct MftOutcome {
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
    pub categories: Vec<crate::storage::StorageCategory>,
    pub age_buckets: Vec<crate::storage::AgeBucket>,
    pub insights: Vec<crate::storage::StorageInsight>,
    pub cancelled: bool,
    pub truncated: bool,
}

#[derive(Debug, Clone)]
struct NameEntry {
    parent: u64,
    name: String,
    namespace: u8,
    modified: Option<u64>,
    allocated: u64,
    real: u64,
    attributes: u32,
}

#[derive(Debug, Clone)]
struct RawRecord {
    number: u64,
    in_use: bool,
    is_dir: bool,
    link_count: u16,
    base_record: u64,
    name: Option<NameEntry>,
    size: u64,
    allocated: u64,
    modified: Option<u64>,
    attributes: u32,
    is_reparse: bool,
}

pub fn is_elevated() -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::Security::{
        GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation: TOKEN_ELEVATION = std::mem::zeroed();
        let mut returned = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut std::ffi::c_void,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        );
        let _ = CloseHandle(token);
        ok != 0 && elevation.TokenIsElevated != 0
    }
}

fn volume_root(root: &Path) -> Result<PathBuf, String> {
    match root.components().next() {
        Some(Component::Prefix(prefix)) => {
            let mut text = prefix.as_os_str().to_string_lossy().into_owned();
            if !text.ends_with('\\') {
                text.push('\\');
            }
            Ok(PathBuf::from(text))
        }
        _ => Err("The path is not on a local drive.".into()),
    }
}

fn volume_is_ntfs(root: &Path) -> Result<bool, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetVolumeInformationW;

    let volume = volume_root(root)?;
    let wide: Vec<u16> = volume
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut filesystem = [0u16; 32];
    let mut serial = 0u32;
    let mut max_component = 0u32;
    let mut flags = 0u32;
    let ok = unsafe {
        GetVolumeInformationW(
            wide.as_ptr(),
            std::ptr::null_mut(),
            0,
            &mut serial,
            &mut max_component,
            &mut flags,
            filesystem.as_mut_ptr(),
            filesystem.len() as u32,
        )
    };
    if ok == 0 {
        return Err(format!(
            "The volume for {} could not be queried.",
            volume.display()
        ));
    }
    let length = filesystem
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(filesystem.len());
    let name = String::from_utf16_lossy(&filesystem[..length]);
    Ok(name.eq_ignore_ascii_case("NTFS"))
}

/// Checks whether the MFT path can serve this root, without reading the table.
pub fn support(root: &Path) -> Result<(), String> {
    if !root.is_dir() {
        return Err("The selected path is not a folder.".into());
    }
    if !volume_is_ntfs(root)? {
        return Err("MFT scanning needs an NTFS volume.".into());
    }
    if !is_elevated() {
        return Err(
            "MFT scanning needs administrator rights to read the NTFS master file table.".into(),
        );
    }
    Ok(())
}

/// Windows denies opening `C:\$MFT` by name even for an elevated process, so
/// the table is read from the raw volume device and located through the boot
/// sector. The handle is opened read-only.
fn volume_device(root: &Path) -> Result<PathBuf, String> {
    let volume = volume_root(root)?;
    let text = volume.to_string_lossy();
    let letter = text
        .chars()
        .next()
        .filter(|value| value.is_ascii_alphabetic())
        .ok_or_else(|| "The drive letter is unavailable.".to_string())?;
    Ok(PathBuf::from(format!("\\\\.\\{letter}:")))
}

/// Lets an elevated process open the volume device itself.
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;

/// A read-only raw volume handle plus the layout facts needed to address the
/// master file table.
struct Volume {
    reader: BufReader<File>,
    cluster_size: u64,
    mft_offset: u64,
}

fn open_volume(root: &Path) -> Result<Volume, String> {
    let device = volume_device(root)?;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(&device)
        .map_err(|error| {
            format!(
                "The volume {device:?} could not be opened read-only ({error}); administrator rights may be required."
            )
        })?;
    let mut reader = BufReader::with_capacity(1024 * 1024, file);
    let mut boot = [0u8; 512];
    reader
        .seek(SeekFrom::Start(0))
        .and_then(|_| reader.read_exact(&mut boot))
        .map_err(|error| format!("The volume boot sector could not be read ({error})."))?;
    if &boot[3..11] != b"NTFS    " {
        return Err("The volume boot sector is not NTFS.".into());
    }
    let bytes_per_sector = u16::from_le_bytes([boot[0x0B], boot[0x0C]]) as u64;
    let sectors_per_cluster = boot[0x0D] as u64;
    let cluster_size = bytes_per_sector * sectors_per_cluster;
    if cluster_size < FILE_RECORD_SIZE as u64 {
        return Err(format!(
            "The volume cluster size ({cluster_size} bytes) is smaller than one MFT record."
        ));
    }
    let mft_lcn = u64::from_le_bytes([
        boot[0x30], boot[0x31], boot[0x32], boot[0x33], boot[0x34], boot[0x35], boot[0x36],
        boot[0x37],
    ]);
    if mft_lcn == 0 {
        return Err("The volume does not report a master file table location.".into());
    }
    Ok(Volume {
        reader,
        cluster_size,
        mft_offset: mft_lcn * cluster_size,
    })
}

fn read_at(volume: &mut Volume, offset: u64, buffer: &mut [u8]) -> Result<(), String> {
    volume
        .reader
        .seek(SeekFrom::Start(offset))
        .and_then(|_| volume.reader.read_exact(buffer))
        .map_err(|error| format!("The master file table could not be read ({error})."))
}

fn load_records(
    root: &Path,
    cancel: &AtomicBool,
) -> Result<Vec<Option<RawRecord>>, String> {
    let mut volume = open_volume(root)?;
    let mut first = [0u8; FILE_RECORD_SIZE];
    let mft_offset = volume.mft_offset;
    read_at(&mut volume, mft_offset, &mut first)?;
    let (data_size, runs) = data_runs(&first).ok_or_else(|| {
        "The master file table data runs could not be parsed; the volume layout is unsupported."
            .to_string()
    })?;
    let count = data_size / FILE_RECORD_SIZE as u64;
    // The size comes from the volume, so validate before reserving anything.
    validate_record_count(count)?;

    // One fixed record buffer on the stack: parsing does not allocate per
    // record, and memory stays proportional to the validated record count.
    let mut buffer = [0u8; FILE_RECORD_SIZE];
    let mut records: Vec<Option<RawRecord>> = Vec::with_capacity(count as usize);
    let mut parsed = 0u64;
    let mut failed = 0u64;
    let cluster_size = volume.cluster_size;

    'runs: for (lcn, clusters) in runs {
        if records.len() as u64 >= count {
            break;
        }
        if lcn == SPARSE_RUN {
            // Sparse extents keep their record numbers but hold no data.
            let sparse_records = clusters * cluster_size / FILE_RECORD_SIZE as u64;
            for _ in 0..sparse_records {
                if records.len() as u64 >= count {
                    break 'runs;
                }
                records.push(None);
            }
            continue;
        }
        let start = lcn * cluster_size;
        let run_records = clusters * cluster_size / FILE_RECORD_SIZE as u64;
        volume
            .reader
            .seek(SeekFrom::Start(start))
            .map_err(|error| format!("The master file table could not be read ({error})."))?;
        for _ in 0..run_records {
            if records.len() as u64 >= count {
                break 'runs;
            }
            let number = records.len() as u64;
            if number % 4096 == 0 && cancel.load(Ordering::Relaxed) {
                return Err("Scan cancelled while reading the master file table.".into());
            }
            if volume.reader.read_exact(&mut buffer).is_err() {
                break 'runs;
            }
            match parse_record(&buffer[..], number) {
                Some(record) => {
                    parsed += 1;
                    records.push(Some(record));
                }
                None => {
                    failed += 1;
                    records.push(None);
                }
            }
            if failed > 100 && failed * 5 > parsed {
                return Err(
                    "Most master file table records could not be parsed; the volume layout is unsupported."
                        .into(),
                );
            }
        }
    }
    if parsed == 0 {
        return Err("No master file table records could be read.".into());
    }
    Ok(records)
}

/// Marker for a sparse run in the MFT data attribute.
const SPARSE_RUN: u64 = u64::MAX;

/// Extracts the unnamed non-resident `$DATA` attribute from the MFT's own
/// FILE record: the logical size plus its cluster runs.
fn data_runs(record: &[u8]) -> Option<(u64, Vec<(u64, u64)>)> {
    if record.len() < 0x38 || &record[0..4] != b"FILE" {
        return None;
    }
    let mut offset = u16::from_le_bytes([record[0x14], record[0x15]]) as usize;
    while offset + 8 <= record.len() {
        let kind = u32::from_le_bytes(record[offset..offset + 4].try_into().ok()?);
        if kind == 0xFFFF_FFFF {
            return None;
        }
        let length = u32::from_le_bytes(record[offset + 4..offset + 8].try_into().ok()?) as usize;
        if length < 16 || offset + length > record.len() {
            return None;
        }
        let non_resident = record[offset + 8] != 0;
        if kind == 0x80 && non_resident && record[offset + 9] == 0 && length >= 64 {
            let run_offset =
                u16::from_le_bytes([record[offset + 32], record[offset + 33]]) as usize;
            let data_size = u64::from_le_bytes(record[offset + 48..offset + 56].try_into().ok()?);
            if run_offset < 64 || offset + run_offset >= offset + length {
                return None;
            }
            let runs = parse_runlist(&record[offset + run_offset..offset + length])?;
            return Some((data_size, runs));
        }
        offset += length;
    }
    None
}

/// Decodes an NTFS data run list into (LCN, cluster count) pairs. Sparse runs
/// use [`SPARSE_RUN`] because they have no logical cluster number.
fn parse_runlist(bytes: &[u8]) -> Option<Vec<(u64, u64)>> {
    let mut runs = Vec::new();
    let mut position = 0usize;
    let mut lcn: i64 = 0;
    while position < bytes.len() {
        let header = bytes[position];
        position += 1;
        if header == 0 {
            break;
        }
        let length_bytes = (header & 0x0F) as usize;
        let offset_bytes = (header >> 4) as usize;
        if length_bytes == 0
            || length_bytes > 8
            || offset_bytes > 8
            || position + length_bytes + offset_bytes > bytes.len()
        {
            return None;
        }
        let mut clusters = 0u64;
        for index in 0..length_bytes {
            clusters |= (bytes[position + index] as u64) << (8 * index);
        }
        position += length_bytes;
        if clusters == 0 {
            return None;
        }
        if offset_bytes == 0 {
            runs.push((SPARSE_RUN, clusters));
            continue;
        }
        let mut delta = 0i64;
        for index in 0..offset_bytes {
            delta |= (bytes[position + index] as i64) << (8 * index);
        }
        let shift = 64 - 8 * offset_bytes;
        let delta = (delta << shift) >> shift;
        position += offset_bytes;
        lcn = lcn.checked_add(delta)?;
        if lcn < 0 {
            return None;
        }
        runs.push((lcn as u64, clusters));
    }
    if runs.is_empty() {
        None
    } else {
        Some(runs)
    }
}

/// Applies the NTFS update-sequence fixup in place. Returns false when the
/// record is truncated or the fixup array is inconsistent.
fn apply_fixup(record: &mut [u8]) -> bool {
    if record.len() < 0x30 || &record[0..4] != b"FILE" {
        return false;
    }
    let usa_offset = u16::from_le_bytes([record[0x04], record[0x05]]) as usize;
    let usa_count = u16::from_le_bytes([record[0x06], record[0x07]]) as usize;
    if usa_count < 2 || usa_offset + usa_count * 2 > record.len() {
        return false;
    }
    let expected = record.len() / 512 + 1;
    if usa_count != expected {
        return false;
    }
    // A 1024-byte record has exactly three update-sequence slots; allow a few
    // more so larger record sizes still work, but never allocate here.
    const MAX_USA_SLOTS: usize = 16;
    if usa_count > MAX_USA_SLOTS {
        return false;
    }
    let mut values = [[0u8; 2]; MAX_USA_SLOTS];
    for (slot, index) in (1..usa_count).enumerate() {
        let start = usa_offset + index * 2;
        values[slot] = [record[start], record[start + 1]];
    }
    for (index, value) in values[..usa_count - 1].iter().enumerate() {
        let end = (index + 1) * 512;
        if end > record.len() {
            return false;
        }
        record[end - 2] = value[0];
        record[end - 1] = value[1];
    }
    true
}

fn parse_record(record: &[u8], number: u64) -> Option<RawRecord> {
    if record.len() < FILE_RECORD_SIZE {
        return None;
    }
    let mut buffer = [0u8; FILE_RECORD_SIZE];
    buffer.copy_from_slice(&record[..FILE_RECORD_SIZE]);
    if !apply_fixup(&mut buffer) {
        return None;
    }
    let link_count = u16::from_le_bytes([buffer[0x12], buffer[0x13]]);
    let flags = u16::from_le_bytes([buffer[0x16], buffer[0x17]]);
    let first_attr = u16::from_le_bytes([buffer[0x14], buffer[0x15]]) as usize;
    let bytes_in_use = u32::from_le_bytes([
        buffer[0x18],
        buffer[0x19],
        buffer[0x1A],
        buffer[0x1B],
    ]) as usize;
    let base_record = u64::from_le_bytes([
        buffer[0x20],
        buffer[0x21],
        buffer[0x22],
        buffer[0x23],
        buffer[0x24],
        buffer[0x25],
        buffer[0x26],
        buffer[0x27],
    ]) & 0x0000_FFFF_FFFF_FFFF;

    let mut result = RawRecord {
        number,
        in_use: flags & 0x0001 != 0,
        is_dir: flags & 0x0002 != 0,
        link_count,
        base_record,
        name: None,
        size: 0,
        allocated: 0,
        modified: None,
        attributes: 0,
        is_reparse: false,
    };
    if !result.in_use || result.base_record != 0 {
        return Some(result);
    }

    let limit = bytes_in_use.min(buffer.len());
    let mut offset = first_attr;
    while offset + 8 <= limit {
        let attr_type = u32::from_le_bytes([
            buffer[offset],
            buffer[offset + 1],
            buffer[offset + 2],
            buffer[offset + 3],
        ]);
        if attr_type == ATTR_END {
            break;
        }
        let attr_len = u32::from_le_bytes([
            buffer[offset + 4],
            buffer[offset + 5],
            buffer[offset + 6],
            buffer[offset + 7],
        ]) as usize;
        if attr_len < 8 || offset + attr_len > limit {
            break;
        }
        let non_resident = buffer[offset + 8] != 0;
        let name_len = buffer[offset + 9] as usize;
        let value = &buffer[offset..offset + attr_len];

        match attr_type {
            ATTR_STANDARD_INFORMATION if !non_resident && attr_len >= 60 => {
                // File attributes live at offset 32 of the resident value, and
                // the resident value starts 24 bytes into the attribute.
                let attrs = u32::from_le_bytes([value[56], value[57], value[58], value[59]]);
                result.attributes |= attrs;
                if attrs & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                    result.is_reparse = true;
                }
            }
            ATTR_FILE_NAME if !non_resident && attr_len >= 66 => {
                let value_offset =
                    u16::from_le_bytes([value[20], value[21]]) as usize;
                let value_len = u32::from_le_bytes([
                    value[16],
                    value[17],
                    value[18],
                    value[19],
                ]) as usize;
                if value_offset + 66 <= attr_len && value_len >= 66 {
                    let data = &value[value_offset..attr_len];
                    if data.len() >= 66 {
                        let parent = u64::from_le_bytes([
                            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
                        ]) & 0x0000_FFFF_FFFF_FFFF;
                        let modified = filetime_to_unix(i64::from_le_bytes([
                            data[16], data[17], data[18], data[19], data[20], data[21], data[22],
                            data[23],
                        ]));
                        let allocated = u64::from_le_bytes([
                            data[40], data[41], data[42], data[43], data[44], data[45], data[46],
                            data[47],
                        ]);
                        let real = u64::from_le_bytes([
                            data[48], data[49], data[50], data[51], data[52], data[53], data[54],
                            data[55],
                        ]);
                        let name_len = data[64] as usize;
                        let namespace = data[65];
                        let name_end = 66 + name_len * 2;
                        if name_end <= data.len() {
                            let units: Vec<u16> = data[66..name_end]
                                .chunks_exact(2)
                                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                                .collect();
                            let name = String::from_utf16_lossy(&units);
                            let candidate = NameEntry {
                                parent,
                                name,
                                namespace,
                                modified,
                                allocated,
                                real,
                                attributes: 0,
                            };
                            let better = match &result.name {
                                None => namespace != 2,
                                Some(existing) => {
                                    namespace_rank(namespace) > namespace_rank(existing.namespace)
                                }
                            };
                            if better && !candidate.name.is_empty() {
                                if result.modified.is_none() {
                                    result.modified = modified;
                                }
                                result.name = Some(candidate);
                            }
                        }
                    }
                }
            }
            ATTR_DATA if name_len == 0 => {
                if non_resident && attr_len >= 64 {
                    let allocated = u64::from_le_bytes([
                        value[40], value[41], value[42], value[43], value[44], value[45],
                        value[46], value[47],
                    ]);
                    let real = u64::from_le_bytes([
                        value[48], value[49], value[50], value[51], value[52], value[53],
                        value[54], value[55],
                    ]);
                    result.allocated = allocated;
                    result.size = real;
                } else if attr_len >= 24 {
                    let value_len = u32::from_le_bytes([
                        value[16],
                        value[17],
                        value[18],
                        value[19],
                    ]) as u64;
                    result.size = value_len;
                    result.allocated = value_len;
                }
                if let Some(name) = &result.name {
                    result.modified = result.modified.or(name.modified);
                }
            }
            _ => {}
        }

        offset += attr_len;
    }

    if let Some(name) = &result.name {
        result.modified = result.modified.or(name.modified);
        result.attributes |= name.attributes;
        if result.size == 0 && name.real > 0 {
            result.size = name.real;
        }
        if result.allocated == 0 && name.allocated > 0 {
            result.allocated = name.allocated;
        }
    }
    Some(result)
}

fn namespace_rank(namespace: u8) -> u8 {
    match namespace {
        3 => 4, // Win32 & DOS name
        1 => 3, // Win32
        0 => 2, // POSIX
        2 => 1, // DOS 8.3 name
        _ => 0,
    }
}

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

fn extension_of(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() && !extension.is_empty() => {
            extension.to_ascii_lowercase()
        }
        _ => "(none)".into(),
    }
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn normalize_path(value: &str) -> String {
    value
        .trim()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

#[derive(Clone)]
struct RecordSnapshot {
    number: u64,
    in_use: bool,
    is_dir: bool,
    is_reparse: bool,
    hidden: bool,
    link_count: u16,
    base_record: u64,
    name: Option<String>,
    size: u64,
    allocated: u64,
    modified: Option<u64>,
}

fn snapshot(records: &[Option<RawRecord>], index: u32) -> Option<RecordSnapshot> {
    let record = records.get(index as usize)?.as_ref()?;
    Some(RecordSnapshot {
        number: record.number,
        in_use: record.in_use,
        is_dir: record.is_dir,
        is_reparse: record.is_reparse,
        hidden: record.attributes & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0,
        link_count: record.link_count,
        base_record: record.base_record,
        name: record.name.as_ref().map(|name| name.name.clone()),
        size: record.size,
        allocated: record.allocated,
        modified: record.modified,
    })
}

struct Builder<'a> {
    records: &'a [Option<RawRecord>],
    children: HashMap<u64, Vec<u32>>,
    options: &'a ScanOptions,
    exclusions: Vec<String>,
    cancel: Arc<AtomicBool>,
    progress: &'a (dyn Fn(ScanProgress) + Send + Sync),
    job_id: &'a str,

    total_size: u64,
    total_allocated: u64,
    files: u64,
    dirs: u64,
    deduped: u64,
    skipped_reparse: u64,
    cancelled: bool,
    truncated: bool,
    errors: Vec<String>,
    extensions: HashMap<String, (u64, u64)>,
    other_extensions: (u64, u64),
    largest: Vec<FileStat>,
    seen_records: SeenRecords,
    /// Every record may be reached only once. NTFS can hold orphaned or
    /// stale parent links that form cycles; without this guard a whole-volume
    /// walk revisits subtrees and inflates totals.
    visited: SeenRecords,
    categories: Accumulator,
    last_progress: Option<Instant>,
}

impl<'a> Builder<'a> {
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

    fn is_cancelled(&mut self) -> bool {
        if self.cancel.load(Ordering::Relaxed) {
            self.cancelled = true;
        }
        self.cancelled
    }

    fn error(&mut self, message: String) {
        if self.errors.len() < MAX_ERRORS {
            self.errors.push(message);
        }
    }

    fn maybe_progress(&mut self, current: &Path) {
        let due = match self.last_progress {
            None => true,
            Some(last) => last.elapsed() >= PROGRESS_INTERVAL,
        };
        if !due {
            return;
        }
        (self.progress)(ScanProgress {
            job_id: self.job_id.to_string(),
            files: self.files,
            dirs: self.dirs,
            bytes: self.total_size,
            current_dir: current.to_string_lossy().into_owned(),
        });
        self.last_progress = Some(Instant::now());
    }

    fn track_extension(&mut self, name: &str, size: u64, unique: bool) {
        let extension = extension_of(name);
        if self.extensions.len() >= MAX_EXTENSION_KEYS && !self.extensions.contains_key(&extension)
        {
            self.other_extensions.0 += 1;
            if unique {
                self.other_extensions.1 += size;
            }
            return;
        }
        let slot = self.extensions.entry(extension).or_insert((0, 0));
        slot.0 += 1;
        if unique {
            slot.1 += size;
        }
    }

    fn track_largest(&mut self, file: FileStat) {
        if self.largest.len() < MAX_LARGEST_FILES {
            self.largest.push(file);
            return;
        }
        let smallest = self
            .largest
            .iter()
            .enumerate()
            .min_by_key(|(_, item)| item.size)
            .map(|(index, _)| index);
        if let Some(index) = smallest {
            if self.largest[index].size < file.size {
                self.largest[index] = file;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn add_file(
        &mut self,
        number: u64,
        size: u64,
        allocated: u64,
        modified: Option<u64>,
        link_count: u16,
        path: &Path,
        name: &str,
    ) {
        self.files += 1;
        self.total_size += size;
        let unique = self.seen_records.mark(number);
        if unique {
            self.total_allocated += allocated;
        } else {
            self.deduped += 1;
        }
        if link_count > 1 && unique {
            // Extra names for the same record are not emitted separately, but
            // they are real links; report them instead of pretending they do
            // not exist.
            self.deduped += link_count.saturating_sub(1) as u64;
        }

        self.track_extension(name, size, unique);
        self.track_largest(FileStat {
            name: name.to_string(),
            path: display_path(&path.to_string_lossy()),
            size,
            allocated,
            extension: extension_of(name),
            modified,
        });
        self.categories.add_file(path, size, allocated, modified);
    }

    fn child_count(&self, parent: u64) -> usize {
        self.children.get(&parent).map_or(0, Vec::len)
    }

    /// Reads one child index without cloning the parent's vector. Records are
    /// never removed from `children` while building, so indexing position by
    /// position is stable and keeps peak memory free of recursive copies.
    fn child_at(&self, parent: u64, position: usize) -> Option<u32> {
        self.children
            .get(&parent)
            .and_then(|values| values.get(position))
            .copied()
    }

    /// Adds totals for a subtree without creating nodes. Used below the IPC
    /// tree depth so parent totals stay correct without a huge tree.
    fn accumulate_subtree(&mut self, index: u32, path: &Path, depth: u32) {
        if self.is_cancelled() {
            return;
        }
        if !self.visited.mark(index as u64) {
            return;
        }
        let Some(current) = snapshot(self.records, index) else {
            return;
        };
        if !self.options.include_hidden && current.hidden {
            return;
        }
        if current.is_reparse {
            self.skipped_reparse += 1;
            return;
        }
        if current.is_dir {
            self.dirs += 1;
            if depth >= self.options.max_depth {
                return;
            }
            let count = self.child_count(current.number);
            for position in 0..count {
                let Some(child) = self.child_at(current.number, position) else {
                    continue;
                };
                if self.visited.is_seen(child as u64) {
                    continue;
                }
                if let Some(child_snapshot) = snapshot(self.records, child) {
                    if let Some(child_name) = &child_snapshot.name {
                        let child_path = path.join(child_name);
                        self.accumulate_subtree(child, &child_path, depth + 1);
                    }
                }
            }
            return;
        }
        let Some(name) = current.name.clone() else {
            return;
        };
        self.add_file(
            current.number,
            current.size,
            current.allocated,
            current.modified,
            current.link_count,
            path,
            &name,
        );
    }

    fn build_dir(&mut self, index: u32, path: &Path, depth: u32) -> ScanNode {
        let modified = snapshot(self.records, index).and_then(|record| record.modified);
        self.visited.mark(index as u64);
        let mut node = ScanNode {
            name: display_name(path),
            path: display_path(&path.to_string_lossy()),
            kind: "dir".into(),
            size: 0,
            allocated: 0,
            file_count: 0,
            dir_count: 0,
            modified,
            children: Vec::new(),
        };
        self.dirs += 1;
        self.maybe_progress(path);
        if self.is_cancelled() {
            return node;
        }

        let mut retained: Vec<ScanNode> = Vec::new();
        let mut overflow_count = 0u64;
        let mut overflow_size = 0u64;
        let mut overflow_allocated = 0u64;
        let mut own_files = 0u64;
        let mut sub_dirs = 0u64;
        let mut sub_files = 0u64;

        let child_total = self.child_count(index as u64);
        for position in 0..child_total {
            if self.is_cancelled() {
                break;
            }
            let Some(child) = self.child_at(index as u64, position) else {
                continue;
            };
            let Some(record) = snapshot(self.records, child) else {
                continue;
            };
            if !record.in_use || record.base_record != 0 {
                continue;
            }
            // A record reached twice would be duplicated in the tree and in the
            // totals; stale parent links on a real volume make that reachable.
            // Directories are marked by the recursive call itself, files here.
            if record.is_dir {
                if self.visited.is_seen(child as u64) {
                    continue;
                }
            } else if !self.visited.mark(child as u64) {
                continue;
            }
            let Some(name) = record.name.clone() else {
                continue;
            };
            if !self.options.include_hidden && record.hidden {
                continue;
            }
            let child_path = path.join(&name);
            if self.is_excluded(&child_path) {
                continue;
            }
            if record.is_reparse {
                self.skipped_reparse += 1;
                if self.skipped_reparse <= 3 {
                    self.error(format!(
                        "Skipped link or junction (not followed): {}",
                        child_path.display()
                    ));
                }
                continue;
            }

            if record.is_dir {
                if depth + 1 > self.options.max_depth {
                    continue;
                }
                let child_node = if depth + 1 >= IPC_TREE_DEPTH {
                    let before_size = self.total_size;
                    let before_allocated = self.total_allocated;
                    let before_files = self.files;
                    self.accumulate_subtree(child, &child_path, depth + 1);
                    self.truncated = true;
                    ScanNode {
                        name: name.clone(),
                        path: String::new(),
                        kind: "group".into(),
                        size: self.total_size - before_size,
                        allocated: self.total_allocated - before_allocated,
                        file_count: self.files - before_files,
                        dir_count: 0,
                        modified: None,
                        children: Vec::new(),
                    }
                } else {
                    self.build_dir(child, &child_path, depth + 1)
                };
                sub_dirs += 1 + child_node.dir_count;
                sub_files += child_node.file_count;
                node.size += child_node.size;
                node.allocated += child_node.allocated;
                retain(
                    &mut retained,
                    child_node,
                    &mut overflow_count,
                    &mut overflow_size,
                    &mut overflow_allocated,
                );
                continue;
            }

            if record.size == 0 && record.allocated == 0 && record.name.is_none() {
                continue;
            }
            own_files += 1;
            self.add_file(
                record.number,
                record.size,
                record.allocated,
                record.modified,
                record.link_count,
                &child_path,
                &name,
            );
            node.size += record.size;
            node.allocated += record.allocated;
            let file_node = ScanNode {
                name: name.clone(),
                path: display_path(&child_path.to_string_lossy()),
                kind: "file".into(),
                size: record.size,
                allocated: record.allocated,
                file_count: 1,
                dir_count: 0,
                modified: record.modified,
                children: Vec::new(),
            };
            retain(
                &mut retained,
                file_node,
                &mut overflow_count,
                &mut overflow_size,
                &mut overflow_allocated,
            );
        }

        node.file_count = own_files + sub_files;
        node.dir_count = sub_dirs;
        retained.sort_by_key(|child| std::cmp::Reverse(child.size));
        node.children = retained;
        if overflow_count > 0 {
            self.truncated = true;
            node.children.push(ScanNode {
                name: format!("{overflow_count} smaller items"),
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
        node
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
        if self.other_extensions.0 > 0 {
            stats.push(ExtensionStat {
                extension: "(other)".into(),
                file_count: self.other_extensions.0,
                size: self.other_extensions.1,
            });
        }
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
}

fn retain(
    retained: &mut Vec<ScanNode>,
    node: ScanNode,
    overflow_count: &mut u64,
    overflow_size: &mut u64,
    overflow_allocated: &mut u64,
) {
    if retained.len() < MAX_CHILDREN_PER_DIR {
        retained.push(node);
        return;
    }
    let smallest = retained
        .iter()
        .enumerate()
        .min_by_key(|(_, item)| item.size)
        .map(|(index, _)| index);
    match smallest {
        Some(index) if retained[index].size < node.size => {
            let dropped = std::mem::replace(&mut retained[index], node);
            *overflow_count += 1;
            *overflow_size += dropped.size;
            *overflow_allocated += dropped.allocated;
        }
        _ => {
            *overflow_count += 1;
            *overflow_size += node.size;
            *overflow_allocated += node.allocated;
        }
    }
}

fn resolve_target(
    records: &[Option<RawRecord>],
    children: &HashMap<u64, Vec<u32>>,
    root: &Path,
) -> Result<u32, String> {
    let mut current: u64 = 5; // $Root
    for component in root.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::CurDir => continue,
            Component::ParentDir => return Err("Parent path segments are not allowed.".into()),
            Component::Normal(value) => {
                let wanted = value.to_string_lossy();
                let found = children.get(&current).and_then(|indices| {
                    indices.iter().copied().find(|index| {
                        records
                            .get(*index as usize)
                            .and_then(|record| record.as_ref())
                            .and_then(|record| record.name.as_ref())
                            .is_some_and(|name| name.name.eq_ignore_ascii_case(&wanted))
                    })
                });
                match found {
                    Some(index) => current = index as u64,
                    None => {
                        return Err(format!(
                            "The master file table has no record for {}.",
                            root.display()
                        ))
                    }
                }
            }
        }
    }
    Ok(current as u32)
}

pub fn scan(
    job_id: &str,
    root: &Path,
    options: &ScanOptions,
    cancel: Arc<AtomicBool>,
    progress: &(dyn Fn(ScanProgress) + Send + Sync),
) -> Result<MftOutcome, String> {
    support(root)?;
    if cancel.load(Ordering::Relaxed) {
        return Err("Scan cancelled before the master file table was read.".into());
    }
    let records = load_records(root, cancel.as_ref())?;

    let mut children: HashMap<u64, Vec<u32>> = HashMap::with_capacity(records.len() / 4);
    for (index, record) in records.iter().enumerate() {
        let Some(record) = record else {
            continue;
        };
        if !record.in_use || record.is_dir || record.base_record != 0 {
            continue;
        }
        let Some(name) = &record.name else {
            continue;
        };
        children
            .entry(name.parent)
            .or_default()
            .push(index as u32);
    }
    for (index, record) in records.iter().enumerate() {
        let Some(record) = record else {
            continue;
        };
        if !record.in_use || !record.is_dir || record.base_record != 0 {
            continue;
        }
        let Some(name) = &record.name else {
            continue;
        };
        children
            .entry(name.parent)
            .or_default()
            .push(index as u32);
    }

    let target = resolve_target(&records, &children, root)?;
    let mut builder = Builder {
        records: &records,
        children,
        options,
        exclusions: options
            .exclusions
            .iter()
            .map(|value| normalize_path(value))
            .filter(|value| !value.is_empty())
            .collect(),
        cancel,
        progress,
        job_id,
        total_size: 0,
        total_allocated: 0,
        files: 0,
        dirs: 0,
        deduped: 0,
        skipped_reparse: 0,
        cancelled: false,
        truncated: false,
        errors: Vec::new(),
        extensions: HashMap::new(),
        other_extensions: (0, 0),
        largest: Vec::new(),
        seen_records: SeenRecords::new(records.len()),
        visited: SeenRecords::new(records.len()),
        categories: Accumulator::new(root),
        last_progress: None,
    };

    let mut node = if options.collect_tree {
        builder.build_dir(target, root, 0)
    } else {
        let mut placeholder = ScanNode {
            name: display_name(root),
            path: display_path(&root.to_string_lossy()),
            kind: "dir".into(),
            size: 0,
            allocated: 0,
            file_count: 0,
            dir_count: 0,
            modified: None,
            children: Vec::new(),
        };
        let before_files = builder.files;
        builder.accumulate_subtree(target, root, 0);
        placeholder.file_count = builder.files - before_files;
        placeholder
    };

    if node.size == 0 && builder.files > 0 {
        // Totals live on the builder; reflect them on the root for empty or
        // aggregated roots.
        node.size = builder.total_size;
        node.allocated = builder.total_allocated;
        node.file_count = builder.files;
    }

    if builder.files == 0 {
        return Err(
            "The master file table scan found no files; falling back to the directory walk.".into(),
        );
    }

    (progress)(ScanProgress {
        job_id: job_id.to_string(),
        files: builder.files,
        dirs: builder.dirs,
        bytes: builder.total_size,
        current_dir: root.to_string_lossy().into_owned(),
    });

    let extension_stats = builder.extension_stats();
    let largest_files = builder.largest_files();
    let total_size = builder.total_size;
    let total_allocated = builder.total_allocated;
    let file_count = builder.files;
    let dir_count = builder.dirs;
    let deduped_files = builder.deduped;
    let skipped_reparse = builder.skipped_reparse;
    let errors: Vec<String> = builder
        .errors
        .iter()
        .map(|value| value.replace("\\\\?\\", ""))
        .collect();
    let cancelled = builder.cancelled;
    let truncated = builder.truncated;
    let (categories, age_buckets, insights) = builder.categories.finish();

    Ok(MftOutcome {
        root: node,
        total_size,
        total_allocated,
        file_count,
        dir_count,
        deduped_files,
        skipped_reparse,
        errors,
        extension_stats,
        largest_files,
        categories,
        age_buckets,
        insights,
        cancelled,
        truncated,
    })
}

/// Exposed for tests and diagnostics: number of records a path resolves to.
pub fn resolve_record_count(root: &Path) -> Result<u64, String> {
    support(root)?;
    let cancel = AtomicBool::new(false);
    Ok(load_records(root, &cancel)?.len() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_attr(buffer: &mut [u8], offset: usize, kind: u32, resident: bool, value: &[u8]) -> usize {
        let header = 24;
        let length = header + value.len();
        buffer[offset..offset + 4].copy_from_slice(&kind.to_le_bytes());
        buffer[offset + 4..offset + 8].copy_from_slice(&(length as u32).to_le_bytes());
        buffer[offset + 8] = if resident { 0 } else { 1 };
        if resident {
            buffer[offset + 16..offset + 20].copy_from_slice(&(value.len() as u32).to_le_bytes());
            buffer[offset + 20..offset + 22].copy_from_slice(&(header as u16).to_le_bytes());
        }
        buffer[offset + header..offset + header + value.len()].copy_from_slice(value);
        length
    }

    fn make_record(number: u64, is_dir: bool, link_count: u16, attrs: u32) -> Vec<u8> {
        let mut buffer = vec![0u8; FILE_RECORD_SIZE];
        buffer[0..4].copy_from_slice(b"FILE");
        buffer[0x04..0x06].copy_from_slice(&0x30u16.to_le_bytes());
        buffer[0x06..0x08].copy_from_slice(&3u16.to_le_bytes());
        buffer[0x12..0x14].copy_from_slice(&link_count.to_le_bytes());
        buffer[0x14..0x16].copy_from_slice(&0x38u16.to_le_bytes());
        let flags: u16 = 0x0001 | if is_dir { 0x0002 } else { 0 };
        buffer[0x16..0x18].copy_from_slice(&flags.to_le_bytes());
        buffer[0x18..0x1C].copy_from_slice(&(FILE_RECORD_SIZE as u32).to_le_bytes());

        let mut offset = 0x38;
        if attrs != 0 {
            let mut value = vec![0u8; 48];
            value[32..36].copy_from_slice(&attrs.to_le_bytes());
            offset += write_attr(&mut buffer, offset, ATTR_STANDARD_INFORMATION, true, &value);
        }
        let name = "example.txt";
        let units: Vec<u16> = name.encode_utf16().collect();
        let mut value = vec![0u8; 66 + units.len() * 2];
        value[0..8].copy_from_slice(&5u64.to_le_bytes());
        value[48..56].copy_from_slice(&4096u64.to_le_bytes());
        value[64] = units.len() as u8;
        value[65] = 1;
        for (index, unit) in units.iter().enumerate() {
            value[66 + index * 2..68 + index * 2].copy_from_slice(&unit.to_le_bytes());
        }
        offset += write_attr(&mut buffer, offset, ATTR_FILE_NAME, true, &value);

        if !is_dir {
            // Non-resident sizes are attribute-relative; the value starts 24
            // bytes in, so allocated/real sit at value 16/24.
            let mut data = [0u8; 64];
            data[16..24].copy_from_slice(&8192u64.to_le_bytes());
            data[24..32].copy_from_slice(&9000u64.to_le_bytes());
            offset += write_attr(&mut buffer, offset, ATTR_DATA, false, &data);
        }
        buffer[offset..offset + 4].copy_from_slice(&ATTR_END.to_le_bytes());
        buffer[0x18..0x1C].copy_from_slice(&((offset + 8) as u32).to_le_bytes());

        // Update sequence array: sequence value plus one replacement per sector.
        let usa = 0x30;
        buffer[usa..usa + 2].copy_from_slice(&7u16.to_le_bytes());
        let sector_count = FILE_RECORD_SIZE / 512;
        for index in 0..sector_count {
            let end = (index + 1) * 512;
            // On disk the sector tail holds the update-sequence value; the
            // original bytes are parked in the USA array.
            buffer[usa + 2 + index * 2..usa + 4 + index * 2]
                .copy_from_slice(&[0xAA, 0xBB]);
            buffer[end - 2..end].copy_from_slice(&7u16.to_le_bytes());
        }
        let _ = number;
        buffer
    }

    #[test]
    fn applies_update_sequence_fixup() {
        let mut record = make_record(7, false, 1, 0);
        assert_eq!(&record[510..512], &7u16.to_le_bytes());
        assert!(apply_fixup(&mut record));
        assert_eq!(&record[510..512], &[0xAA, 0xBB]);
        assert_eq!(&record[1022..1024], &[0xAA, 0xBB]);
    }

    #[test]
    fn rejects_corrupted_magic() {
        let mut record = make_record(1, false, 1, 0);
        record[0] = b'X';
        assert!(parse_record(&record, 0).is_none());
        assert!(!apply_fixup(&mut record));
    }

    #[test]
    fn reads_name_data_and_directory_flag() {
        let file = parse_record(&make_record(2, false, 1, 0), 2).expect("file record");
        assert!(!file.is_dir);
        assert_eq!(file.name.as_ref().map(|name| name.name.as_str()), Some("example.txt"));
        assert_eq!(file.name.as_ref().map(|name| name.parent), Some(5));
        assert_eq!(file.size, 9000);
        assert_eq!(file.allocated, 8192);

        let dir = parse_record(&make_record(3, true, 1, 0), 3).expect("dir record");
        assert!(dir.is_dir);
        assert_eq!(dir.size, 4096, "directories fall back to the file-name size");
        assert_eq!(dir.allocated, 0);
    }

    #[test]
    fn skips_dos_names_and_detects_hidden() {
        let mut record = make_record(4, false, 1, FILE_ATTRIBUTE_HIDDEN);
        // Add a second $FILE_NAME with namespace 2 (DOS) after the Win32 one.
        let record_len = FILE_RECORD_SIZE;
        let mut offset = 0x38;
        loop {
            let kind = u32::from_le_bytes([
                record[offset],
                record[offset + 1],
                record[offset + 2],
                record[offset + 3],
            ]);
            if kind == ATTR_END || offset + 8 > record_len {
                break;
            }
            let length = u32::from_le_bytes([
                record[offset + 4],
                record[offset + 5],
                record[offset + 6],
                record[offset + 7],
            ]) as usize;
            if length == 0 {
                break;
            }
            offset += length;
        }
        let name = "EXAMPL~1.TXT";
        let units: Vec<u16> = name.encode_utf16().collect();
        let mut value = vec![0u8; 66 + units.len() * 2];
        value[0..8].copy_from_slice(&5u64.to_le_bytes());
        value[64] = units.len() as u8;
        value[65] = 2;
        for (index, unit) in units.iter().enumerate() {
            value[66 + index * 2..68 + index * 2].copy_from_slice(&unit.to_le_bytes());
        }
        let written = write_attr(&mut record, offset, ATTR_FILE_NAME, true, &value);
        record[0x18..0x1C].copy_from_slice(&((offset + written + 8) as u32).to_le_bytes());

        let parsed = parse_record(&record, 4).expect("record");
        assert_eq!(parsed.name.as_ref().map(|name| name.name.as_str()), Some("example.txt"));
        assert!(parsed.attributes & FILE_ATTRIBUTE_HIDDEN != 0);
    }

    #[test]
    fn record_limit_stays_inside_the_memory_budget() {
        assert!(MAX_RECORDS > 1_000_000, "a real volume must still fit");
        assert!(MAX_RECORDS * ESTIMATED_BYTES_PER_RECORD <= MAX_RECORD_MEMORY_BYTES);
    }

    #[test]
    fn record_count_is_rejected_before_allocating() {
        assert!(validate_record_count(0).is_err());
        assert!(validate_record_count(MAX_RECORDS).is_ok());
        let error = validate_record_count(MAX_RECORDS + 1).expect_err("over budget");
        assert!(error.contains(&MAX_RECORDS.to_string()), "{error}");
        assert!(error.contains("directory walk"), "{error}");
    }

    #[test]
    fn seen_records_bitset_reports_first_visit_only() {
        let mut seen = SeenRecords::new(1);
        assert!(seen.mark(5));
        assert!(!seen.mark(5));
        assert!(seen.mark(3));
        // Growth beyond the pre-sized bitset is defensive but must still work.
        assert!(seen.mark(128));
        assert!(!seen.mark(128));
        assert!(seen.mark(MAX_RECORDS));
        assert!(seen.mark(MAX_RECORDS), "out-of-range ids are never stored");
    }

    #[test]
    fn retain_keeps_the_largest_children_and_accounts_overflow() {
        let mut retained = Vec::new();
        let (mut count, mut size, mut allocated) = (0u64, 0u64, 0u64);
        for value in 1..=250u64 {
            retain(
                &mut retained,
                ScanNode {
                    name: format!("item-{value}"),
                    path: String::new(),
                    kind: "file".into(),
                    size: value,
                    allocated: value,
                    file_count: 1,
                    dir_count: 0,
                    modified: None,
                    children: Vec::new(),
                },
                &mut count,
                &mut size,
                &mut allocated,
            );
        }
        assert_eq!(retained.len(), MAX_CHILDREN_PER_DIR);
        assert_eq!(count, 50);
        assert_eq!(size, (1..=50).sum::<u64>());
        assert_eq!(allocated, size);
        assert!(retained.iter().any(|child| child.size == 250));
        assert_eq!(
            retained.iter().map(|child| child.size).min(),
            Some(51),
            "the 50 smallest children are folded into the overflow bucket"
        );
    }

    #[test]
    fn rejects_out_of_bounds_attribute() {
        let mut record = make_record(5, false, 1, 0);
        record[0x38 + 4..0x38 + 8].copy_from_slice(&0xFFFF_FF00u32.to_le_bytes());
        let parsed = parse_record(&record, 5).expect("record still parses");
        assert!(parsed.name.is_none());
    }

    #[test]
    fn runlist_applies_signed_relative_offsets() {
        // Two runs: 2 clusters at LCN 16, then 3 clusters at LCN 0 (16 - 16).
        let bytes = [0x11, 0x02, 0x10, 0x11, 0x03, 0xF0, 0x00];
        let runs = parse_runlist(&bytes).expect("runlist parses");
        assert_eq!(runs, vec![(16, 2), (0, 3)]);
    }

    #[test]
    fn runlist_marks_sparse_runs() {
        let bytes = [0x01, 0x04, 0x11, 0x02, 0x20, 0x00];
        let runs = parse_runlist(&bytes).expect("runlist parses");
        assert_eq!(runs, vec![(SPARSE_RUN, 4), (0x20, 2)]);
    }

    #[test]
    fn rejects_truncated_or_zero_length_runs() {
        assert!(parse_runlist(&[0x11, 0x02]).is_none());
        assert!(parse_runlist(&[0x11, 0x00, 0x10]).is_none());
        assert!(parse_runlist(&[]).is_none());
    }

    #[test]
    fn reads_the_mft_data_attribute() {
        let mut record = vec![0u8; FILE_RECORD_SIZE];
        record[0..4].copy_from_slice(b"FILE");
        record[0x14..0x16].copy_from_slice(&0x38u16.to_le_bytes());
        let at = 0x38;
        record[at..at + 4].copy_from_slice(&0x80u32.to_le_bytes());
        record[at + 4..at + 8].copy_from_slice(&72u32.to_le_bytes());
        record[at + 8] = 1;
        record[at + 32..at + 34].copy_from_slice(&64u16.to_le_bytes());
        record[at + 48..at + 56].copy_from_slice(&(2 * FILE_RECORD_SIZE as u64).to_le_bytes());
        record[at + 64..at + 68].copy_from_slice(&[0x11, 0x02, 0x20, 0x00]);
        record[at + 72..at + 76].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());

        let (size, runs) = data_runs(&record).expect("data attribute parsed");
        assert_eq!(size, 2 * FILE_RECORD_SIZE as u64);
        assert_eq!(runs, vec![(0x20, 2)]);
    }
}
