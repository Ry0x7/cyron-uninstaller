//! Live resource monitor for the Performance page.
//!
//! A lightweight, read-only sampler meant to be polled about once a second,
//! in the spirit of Task Manager's Performance tab: total and per-processor
//! CPU, memory composition, per-physical-disk activity, per-adapter network
//! throughput and per-GPU engine load, plus an optional process table.
//!
//! One PDH query stays open between calls so rate counters are measured over
//! the real interval between two polls instead of a blocking sleep. Nothing
//! here changes system state; process handles are opened with
//! `PROCESS_QUERY_LIMITED_INFORMATION` only and closed on every sample.

use crate::perf::{self, CpuTimes};
use crate::process;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::c_void;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use winreg::enums::HKEY_LOCAL_MACHINE;
use winreg::RegKey;

const PDH_FMT_DOUBLE: u32 = 0x0000_0200;
const PDH_FMT_NOCAP100: u32 = 0x0000_8000;
const PDH_MORE_DATA: u32 = 0x8000_07D2;
const PDH_CSTATUS_VALID_DATA: u32 = 0x0000_0000;
const PDH_CSTATUS_NEW_DATA: u32 = 0x0000_0001;
/// Processes are capped so one sample cannot grow without bound.
const MAX_LIVE_PROCESSES: usize = 2048;

/* ---------------------------------------------------------------- payload */

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveCpu {
    pub utilization_percent: f64,
    pub user_percent: f64,
    pub kernel_percent: f64,
    /// Per logical processor, in processor order. Empty when counters are unavailable.
    pub per_core_percent: Vec<f64>,
    /// Current effective speed in MHz, when Windows reports it.
    pub speed_mhz: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveMemory {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    pub used_percent: f64,
    pub commit_used_bytes: u64,
    pub commit_limit_bytes: u64,
    pub cached_bytes: u64,
    pub paged_pool_bytes: u64,
    pub nonpaged_pool_bytes: u64,
    pub modified_bytes: Option<u64>,
    pub standby_bytes: Option<u64>,
    pub free_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveDisk {
    /// Physical disk index as Windows numbers it ("Disk 0").
    pub index: u32,
    /// Drive letters on this physical disk, e.g. ["C:", "D:"].
    pub volumes: Vec<String>,
    pub active_percent: f64,
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
    /// Average response time in milliseconds.
    pub response_ms: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveNetwork {
    pub name: String,
    /// "wifi", "ethernet" or "other", guessed from the adapter description.
    pub kind: String,
    pub received_bytes_per_sec: f64,
    pub sent_bytes_per_sec: f64,
    /// Link speed in bits per second.
    pub link_bits_per_sec: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveGpuEngine {
    pub name: String,
    pub percent: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveGpu {
    pub index: u32,
    pub name: Option<String>,
    pub luid: String,
    /// Busiest engine type, the way Task Manager reports GPU utilization.
    pub utilization_percent: f64,
    pub engines: Vec<LiveGpuEngine>,
    pub dedicated_used_bytes: u64,
    pub shared_used_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveProcess {
    pub pid: u32,
    pub name: String,
    pub path: Option<String>,
    pub cpu_percent: f64,
    pub working_set_bytes: u64,
    pub private_bytes: u64,
    pub io_bytes_per_sec: f64,
    /// Owns a visible top-level window (an "app" in Task Manager's grouping).
    pub has_window: bool,
    pub is_critical: bool,
    pub is_cyron: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveSystem {
    pub process_count: u32,
    pub thread_count: u32,
    pub handle_count: u32,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveStatic {
    pub cpu_name: String,
    pub base_speed_mhz: Option<f64>,
    pub sockets: u32,
    pub cores: u32,
    pub logical_processors: u32,
    pub l1_cache_bytes: u64,
    pub l2_cache_bytes: u64,
    pub l3_cache_bytes: u64,
    pub installed_memory_bytes: Option<u64>,
    pub gpu_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LivePerformance {
    pub captured_at: u64,
    /// Milliseconds the rate counters cover; 0 on the first sample.
    pub interval_ms: u64,
    /// False on the first call: rate counters need two samples.
    pub warm: bool,
    pub cpu: LiveCpu,
    pub memory: LiveMemory,
    pub disks: Vec<LiveDisk>,
    pub networks: Vec<LiveNetwork>,
    pub gpus: Vec<LiveGpu>,
    pub system: LiveSystem,
    pub processes: Option<Vec<LiveProcess>>,
    pub hardware: LiveStatic,
    pub notes: Vec<String>,
}

/* --------------------------------------------------------------- PDH query */

struct Counter {
    handle: *mut c_void,
}

struct LiveQuery {
    query: *mut c_void,
    cpu_total: Option<Counter>,
    cpu_cores: Option<Counter>,
    cpu_performance: Option<Counter>,
    cpu_frequency: Option<Counter>,
    disk_idle: Option<Counter>,
    disk_read: Option<Counter>,
    disk_write: Option<Counter>,
    disk_response: Option<Counter>,
    net_received: Option<Counter>,
    net_sent: Option<Counter>,
    net_bandwidth: Option<Counter>,
    gpu_engine: Option<Counter>,
    gpu_dedicated: Option<Counter>,
    gpu_shared: Option<Counter>,
    mem_modified: Option<Counter>,
    mem_standby_core: Option<Counter>,
    mem_standby_normal: Option<Counter>,
    mem_standby_reserve: Option<Counter>,
    mem_free_zero: Option<Counter>,
}

// The PDH handles are only touched while the sampler mutex is held.
unsafe impl Send for LiveQuery {}

impl Drop for LiveQuery {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::System::Performance::PdhCloseQuery(self.query);
        }
    }
}

impl LiveQuery {
    fn open() -> Option<Self> {
        use windows_sys::Win32::System::Performance::PdhOpenQueryW;
        let mut query: *mut c_void = std::ptr::null_mut();
        if unsafe { PdhOpenQueryW(std::ptr::null(), 0, &mut query) } != 0 {
            return None;
        }
        let add = |path: &str| -> Option<Counter> {
            use windows_sys::Win32::System::Performance::PdhAddEnglishCounterW;
            let wide = perf::wide(path);
            let mut handle: *mut c_void = std::ptr::null_mut();
            let status = unsafe { PdhAddEnglishCounterW(query, wide.as_ptr(), 0, &mut handle) };
            (status == 0).then_some(Counter { handle })
        };
        let cpu_total = add(r"\Processor Information(_Total)\% Processor Utility")
            .or_else(|| add(r"\Processor(_Total)\% Processor Time"));
        let cpu_cores = add(r"\Processor Information(*)\% Processor Utility")
            .or_else(|| add(r"\Processor(*)\% Processor Time"));
        Some(Self {
            query,
            cpu_total,
            cpu_cores,
            cpu_performance: add(r"\Processor Information(_Total)\% Processor Performance"),
            cpu_frequency: add(r"\Processor Information(_Total)\Processor Frequency"),
            disk_idle: add(r"\PhysicalDisk(*)\% Idle Time"),
            disk_read: add(r"\PhysicalDisk(*)\Disk Read Bytes/sec"),
            disk_write: add(r"\PhysicalDisk(*)\Disk Write Bytes/sec"),
            disk_response: add(r"\PhysicalDisk(*)\Avg. Disk sec/Transfer"),
            net_received: add(r"\Network Interface(*)\Bytes Received/sec"),
            net_sent: add(r"\Network Interface(*)\Bytes Sent/sec"),
            net_bandwidth: add(r"\Network Interface(*)\Current Bandwidth"),
            gpu_engine: add(r"\GPU Engine(*)\Utilization Percentage"),
            gpu_dedicated: add(r"\GPU Adapter Memory(*)\Dedicated Usage"),
            gpu_shared: add(r"\GPU Adapter Memory(*)\Shared Usage"),
            mem_modified: add(r"\Memory\Modified Page List Bytes"),
            mem_standby_core: add(r"\Memory\Standby Cache Core Bytes"),
            mem_standby_normal: add(r"\Memory\Standby Cache Normal Priority Bytes"),
            mem_standby_reserve: add(r"\Memory\Standby Cache Reserve Bytes"),
            mem_free_zero: add(r"\Memory\Free & Zero Page List Bytes"),
        })
    }

    fn collect(&self) -> bool {
        use windows_sys::Win32::System::Performance::PdhCollectQueryData;
        unsafe { PdhCollectQueryData(self.query) == 0 }
    }
}

fn read_single(counter: Option<&Counter>, format: u32) -> Option<f64> {
    use windows_sys::Win32::System::Performance::{
        PdhGetFormattedCounterValue, PDH_FMT_COUNTERVALUE,
    };
    let counter = counter?;
    let mut value: PDH_FMT_COUNTERVALUE = unsafe { std::mem::zeroed() };
    let mut kind = 0u32;
    let status = unsafe {
        PdhGetFormattedCounterValue(
            counter.handle,
            PDH_FMT_DOUBLE | format,
            &mut kind,
            &mut value,
        )
    };
    if status != 0 || !valid_status(value.CStatus) {
        return None;
    }
    let number = unsafe { value.Anonymous.doubleValue };
    number.is_finite().then_some(number)
}

fn valid_status(status: u32) -> bool {
    status == PDH_CSTATUS_VALID_DATA || status == PDH_CSTATUS_NEW_DATA
}

/// Reads every instance of a wildcard counter as (instance name, value).
fn read_array(counter: &Option<Counter>, format: u32) -> Vec<(String, f64)> {
    use windows_sys::Win32::System::Performance::{
        PdhGetFormattedCounterArrayW, PDH_FMT_COUNTERVALUE_ITEM_W,
    };
    let Some(counter) = counter.as_ref() else {
        return Vec::new();
    };
    let mut buffer_size = 0u32;
    let mut item_count = 0u32;
    let status = unsafe {
        PdhGetFormattedCounterArrayW(
            counter.handle,
            PDH_FMT_DOUBLE | format,
            &mut buffer_size,
            &mut item_count,
            std::ptr::null_mut(),
        )
    };
    if status != PDH_MORE_DATA || buffer_size == 0 {
        return Vec::new();
    }
    // The buffer holds the item array followed by the instance name strings,
    // so it is sized in bytes and aligned for the item struct.
    let item_size = std::mem::size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>();
    let slots = (buffer_size as usize).div_ceil(item_size) + 1;
    let mut storage: Vec<PDH_FMT_COUNTERVALUE_ITEM_W> = Vec::with_capacity(slots);
    let mut buffer_size = (slots * item_size) as u32;
    let status = unsafe {
        PdhGetFormattedCounterArrayW(
            counter.handle,
            PDH_FMT_DOUBLE | format,
            &mut buffer_size,
            &mut item_count,
            storage.as_mut_ptr(),
        )
    };
    if status != 0 {
        return Vec::new();
    }
    let items = unsafe { std::slice::from_raw_parts(storage.as_ptr(), item_count as usize) };
    items
        .iter()
        .filter(|item| valid_status(item.FmtValue.CStatus))
        .filter_map(|item| {
            let name = unsafe { wide_ptr_to_string(item.szName) }?;
            let value = unsafe { item.FmtValue.Anonymous.doubleValue };
            value.is_finite().then_some((name, value))
        })
        .collect()
}

unsafe fn wide_ptr_to_string(pointer: *mut u16) -> Option<String> {
    if pointer.is_null() {
        return None;
    }
    let mut length = 0usize;
    while *pointer.add(length) != 0 {
        length += 1;
        if length > 4096 {
            return None;
        }
    }
    Some(String::from_utf16_lossy(std::slice::from_raw_parts(
        pointer, length,
    )))
}

/* ------------------------------------------------------------ pure helpers */

/// Orders "group,index" processor instances ("0,0", "0,1", "1,0") numerically
/// and drops the `_Total` rows.
pub(crate) fn per_core_values(rows: &[(String, f64)]) -> Vec<f64> {
    let mut cores: Vec<((u32, u32), f64)> = rows
        .iter()
        .filter(|(name, _)| !name.contains("_Total"))
        .filter_map(|(name, value)| {
            let mut parts = name.split(',');
            let first = parts.next()?.trim().parse::<u32>().ok()?;
            let key = match parts.next() {
                Some(second) => (first, second.trim().parse::<u32>().ok()?),
                None => (0, first),
            };
            Some((key, value.clamp(0.0, 100.0)))
        })
        .collect();
    cores.sort_by_key(|(key, _)| *key);
    cores.into_iter().map(|(_, value)| value).collect()
}

/// Splits a PhysicalDisk instance like "0 C: D:" into (index, ["C:", "D:"]).
pub(crate) fn parse_disk_instance(name: &str) -> Option<(u32, Vec<String>)> {
    if name.contains("_Total") {
        return None;
    }
    let mut parts = name.split_whitespace();
    let index = parts.next()?.parse::<u32>().ok()?;
    let volumes = parts
        .filter(|part| part.len() == 2 && part.ends_with(':'))
        .map(|part| part.to_ascii_uppercase())
        .collect();
    Some((index, volumes))
}

/// Guesses the adapter type from its description.
pub(crate) fn network_kind(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if lower.contains("wi-fi")
        || lower.contains("wifi")
        || lower.contains("wireless")
        || lower.contains("802.11")
        || lower.contains("wlan")
    {
        "wifi"
    } else if lower.contains("ethernet")
        || lower.contains("gbe")
        || lower.contains("realtek pcie")
        || lower.contains("intel(r) i2")
        || lower.contains("intel[r] i2")
        || lower.contains("controller")
    {
        "ethernet"
    } else {
        "other"
    }
}

/// Virtual or tunnelling adapters Task Manager does not show as a network.
pub(crate) fn is_virtual_adapter(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [
        "isatap",
        "teredo",
        "loopback",
        "pseudo",
        "6to4",
        "wan miniport",
        "kernel debug",
        "hyper-v virtual",
        "vethernet",
        "virtualbox",
        "vmware",
        "tap-windows",
        "bluetooth device",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

/// Extracts (luid, engine type) from a GPU Engine instance such as
/// "pid_1234_luid_0x00000000_0x0000D1E5_phys_0_eng_0_engtype_3D".
pub(crate) fn parse_gpu_engine(name: &str) -> Option<(String, String)> {
    let luid_start = name.find("luid_")? + 5;
    let rest = &name[luid_start..];
    let luid_end = rest.find("_phys")?;
    let luid = rest[..luid_end].to_ascii_lowercase();
    let engine = name.split("engtype_").nth(1)?.trim();
    let engine = if engine.is_empty() { "Other" } else { engine };
    Some((luid, engine.to_string()))
}

/// Extracts the luid from a GPU Adapter Memory instance "luid_0x..._0x..._phys_0".
pub(crate) fn parse_gpu_adapter(name: &str) -> Option<String> {
    let start = name.find("luid_")? + 5;
    let rest = &name[start..];
    let end = rest.find("_phys").unwrap_or(rest.len());
    Some(rest[..end].to_ascii_lowercase())
}

/// Sums engine utilization per (adapter, engine type) across processes and
/// reports each adapter's busiest engine type as its utilization.
pub(crate) fn aggregate_gpu_engines(
    rows: &[(String, f64)],
) -> BTreeMap<String, BTreeMap<String, f64>> {
    let mut adapters: BTreeMap<String, BTreeMap<String, f64>> = BTreeMap::new();
    for (name, value) in rows {
        let Some((luid, engine)) = parse_gpu_engine(name) else {
            continue;
        };
        *adapters
            .entry(luid)
            .or_default()
            .entry(engine)
            .or_insert(0.0) += value.max(0.0);
    }
    for engines in adapters.values_mut() {
        for value in engines.values_mut() {
            *value = value.clamp(0.0, 100.0);
        }
    }
    adapters
}

fn friendly_engine(name: &str) -> String {
    match name {
        "3D" => "3D".into(),
        "Copy" => "Copy".into(),
        "VideoDecode" => "Video decode".into(),
        "VideoEncode" => "Video encode".into(),
        "VideoProcessing" => "Video processing".into(),
        "Compute_0" | "Compute_1" | "Compute" => "Compute".into(),
        other => other.replace('_', " "),
    }
}

/* ---------------------------------------------------------- static details */

#[allow(non_upper_case_globals)]
fn read_hardware() -> LiveStatic {
    use windows_sys::Win32::System::SystemInformation::{
        GetLogicalProcessorInformation, GetPhysicallyInstalledSystemMemory, RelationCache,
        RelationProcessorCore, RelationProcessorPackage, SYSTEM_LOGICAL_PROCESSOR_INFORMATION,
    };

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let mut cpu_name = "Processor".to_string();
    let mut base_speed_mhz = None;
    if let Ok(key) = hklm.open_subkey(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0") {
        if let Ok(name) = key.get_value::<String, _>("ProcessorNameString") {
            let collapsed = name.split_whitespace().collect::<Vec<_>>().join(" ");
            if !collapsed.is_empty() {
                cpu_name = collapsed;
            }
        }
        if let Ok(mhz) = key.get_value::<u32, _>("~MHz") {
            if mhz > 0 {
                base_speed_mhz = Some(mhz as f64);
            }
        }
    }

    let mut sockets = 0u32;
    let mut cores = 0u32;
    let mut logical = 0u32;
    let (mut l1, mut l2, mut l3) = (0u64, 0u64, 0u64);
    unsafe {
        let mut length = 0u32;
        GetLogicalProcessorInformation(std::ptr::null_mut(), &mut length);
        let item = std::mem::size_of::<SYSTEM_LOGICAL_PROCESSOR_INFORMATION>();
        if length > 0 {
            let count = (length as usize).div_ceil(item);
            let mut buffer: Vec<SYSTEM_LOGICAL_PROCESSOR_INFORMATION> =
                vec![std::mem::zeroed(); count];
            if GetLogicalProcessorInformation(buffer.as_mut_ptr(), &mut length) != 0 {
                let filled = (length as usize / item).min(buffer.len());
                for info in &buffer[..filled] {
                    match info.Relationship {
                        RelationProcessorPackage => sockets += 1,
                        RelationProcessorCore => {
                            cores += 1;
                            logical += info.ProcessorMask.count_ones();
                        }
                        RelationCache => {
                            let cache = info.Anonymous.Cache;
                            let size = cache.Size as u64;
                            match cache.Level {
                                1 => l1 += size,
                                2 => l2 += size,
                                3 => l3 += size,
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    if logical == 0 {
        logical = unsafe {
            use windows_sys::Win32::System::SystemInformation::{GetNativeSystemInfo, SYSTEM_INFO};
            let mut info: SYSTEM_INFO = std::mem::zeroed();
            GetNativeSystemInfo(&mut info);
            info.dwNumberOfProcessors.max(1)
        };
    }

    let mut installed_kb = 0u64;
    let installed_memory_bytes =
        (unsafe { GetPhysicallyInstalledSystemMemory(&mut installed_kb) } != 0 && installed_kb > 0)
            .then(|| installed_kb.saturating_mul(1024));

    LiveStatic {
        cpu_name,
        base_speed_mhz,
        sockets: sockets.max(1),
        cores: cores.max(1),
        logical_processors: logical.max(1),
        l1_cache_bytes: l1,
        l2_cache_bytes: l2,
        l3_cache_bytes: l3,
        installed_memory_bytes,
        gpu_names: gpu_names(&hklm),
    }
}

fn gpu_names(hklm: &RegKey) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let base = r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";
    if let Ok(class) = hklm.open_subkey(base) {
        for sub in class.enum_keys().flatten() {
            if !sub.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let Ok(adapter) = class.open_subkey(&sub) else {
                continue;
            };
            let Ok(description) = adapter.get_value::<String, _>("DriverDesc") else {
                continue;
            };
            let trimmed = description.trim().to_string();
            let lower = trimmed.to_ascii_lowercase();
            let is_virtual = [
                "remote",
                "miracast",
                "virtualbox",
                "vmware",
                "rdpdd",
                "basic display",
                "basic render",
                "parsec",
                "virtual display",
            ]
            .iter()
            .any(|needle| lower.contains(needle));
            if !trimmed.is_empty() && !is_virtual && !names.contains(&trimmed) {
                names.push(trimmed);
            }
        }
    }
    names
}

/* ---------------------------------------------------------------- sampler */

#[derive(Clone, Copy)]
struct ProcessSample {
    cpu_100ns: u64,
    io_bytes: u64,
}

struct Sampler {
    query: Option<LiveQuery>,
    last_collect: Option<Instant>,
    cpu_times: Option<CpuTimes>,
    processes: HashMap<u32, ProcessSample>,
    process_sampled_at: Option<Instant>,
}

static SAMPLER: Mutex<Option<Sampler>> = Mutex::new(None);
static HARDWARE: OnceLock<LiveStatic> = OnceLock::new();

fn windowed_pids() -> HashSet<u32> {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindow, GetWindowThreadProcessId, IsWindowVisible, GW_OWNER,
    };

    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> windows_sys::core::BOOL {
        let pids = &mut *(lparam as *mut HashSet<u32>);
        if IsWindowVisible(hwnd) != 0 && GetWindow(hwnd, GW_OWNER).is_null() {
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid != 0 {
                pids.insert(pid);
            }
        }
        1
    }

    let mut pids: HashSet<u32> = HashSet::new();
    unsafe {
        EnumWindows(Some(visit), &mut pids as *mut HashSet<u32> as LPARAM);
    }
    pids
}

fn sample_processes(sampler: &mut Sampler, logical: u32) -> Vec<LiveProcess> {
    let now = Instant::now();
    let elapsed = sampler
        .process_sampled_at
        .map(|previous| now.duration_since(previous).as_secs_f64())
        .unwrap_or(0.0);
    let handles = perf::open_processes();
    let windowed = windowed_pids();
    let denominator = elapsed * 10_000_000.0 * logical.max(1) as f64;

    let mut next: HashMap<u32, ProcessSample> = HashMap::with_capacity(handles.len());
    let mut rows: Vec<LiveProcess> = Vec::with_capacity(handles.len());
    for entry in handles.iter().take(MAX_LIVE_PROCESSES) {
        let counters = perf::read_process_counters(entry.handle);
        let io_bytes = counters.read_bytes.saturating_add(counters.write_bytes);
        let sample = ProcessSample {
            cpu_100ns: counters.cpu_100ns,
            io_bytes,
        };
        let (cpu_percent, io_bytes_per_sec) = match sampler.processes.get(&entry.pid) {
            Some(previous) if elapsed > 0.05 => (
                (counters.cpu_100ns.saturating_sub(previous.cpu_100ns) as f64 / denominator
                    * 100.0)
                    .clamp(0.0, 100.0),
                io_bytes.saturating_sub(previous.io_bytes) as f64 / elapsed,
            ),
            _ => (0.0, 0.0),
        };
        next.insert(entry.pid, sample);
        rows.push(LiveProcess {
            pid: entry.pid,
            name: entry.name.clone(),
            path: entry.path.clone(),
            cpu_percent,
            working_set_bytes: counters.working_set,
            private_bytes: counters.private_bytes,
            io_bytes_per_sec,
            has_window: windowed.contains(&entry.pid),
            is_critical: process::is_critical_process(
                entry.pid,
                &entry.name,
                entry.path.as_deref(),
            ),
            is_cyron: perf::is_cyron_process(entry.path.as_deref(), entry.pid),
        });
    }
    sampler.processes = next;
    sampler.process_sampled_at = Some(now);
    rows
}

fn read_memory(query: Option<&LiveQuery>) -> (LiveMemory, LiveSystem) {
    use windows_sys::Win32::System::ProcessStatus::{GetPerformanceInfo, PERFORMANCE_INFORMATION};
    use windows_sys::Win32::System::SystemInformation::{
        GetTickCount64, GlobalMemoryStatusEx, MEMORYSTATUSEX,
    };

    let mut status: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    let have_status = unsafe { GlobalMemoryStatusEx(&mut status) } != 0;
    let mut info: PERFORMANCE_INFORMATION = unsafe { std::mem::zeroed() };
    info.cb = std::mem::size_of::<PERFORMANCE_INFORMATION>() as u32;
    let have_info = unsafe { GetPerformanceInfo(&mut info, info.cb) } != 0;
    let page = if have_info {
        info.PageSize as u64
    } else {
        4096
    };

    let total = if have_status { status.ullTotalPhys } else { 0 };
    let available = if have_status { status.ullAvailPhys } else { 0 };
    let used = total.saturating_sub(available);

    let counter = |value: Option<&Counter>| -> Option<u64> {
        let value = value?;
        read_single(Some(value), 0).map(|bytes| bytes.max(0.0) as u64)
    };
    let modified = query.and_then(|q| counter(q.mem_modified.as_ref()));
    let standby = query.and_then(|q| {
        let core = counter(q.mem_standby_core.as_ref());
        let normal = counter(q.mem_standby_normal.as_ref());
        let reserve = counter(q.mem_standby_reserve.as_ref());
        if core.is_none() && normal.is_none() && reserve.is_none() {
            None
        } else {
            Some(core.unwrap_or(0) + normal.unwrap_or(0) + reserve.unwrap_or(0))
        }
    });
    let free = query.and_then(|q| counter(q.mem_free_zero.as_ref()));

    let memory = LiveMemory {
        total_bytes: total,
        available_bytes: available,
        used_bytes: used,
        used_percent: if total > 0 {
            (used as f64 / total as f64 * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        },
        commit_used_bytes: if have_info {
            (info.CommitTotal as u64).saturating_mul(page)
        } else {
            0
        },
        commit_limit_bytes: if have_info {
            (info.CommitLimit as u64).saturating_mul(page)
        } else {
            0
        },
        cached_bytes: if have_info {
            (info.SystemCache as u64).saturating_mul(page)
        } else {
            0
        },
        paged_pool_bytes: if have_info {
            (info.KernelPaged as u64).saturating_mul(page)
        } else {
            0
        },
        nonpaged_pool_bytes: if have_info {
            (info.KernelNonpaged as u64).saturating_mul(page)
        } else {
            0
        },
        modified_bytes: modified,
        standby_bytes: standby,
        free_bytes: free,
    };
    let system = LiveSystem {
        process_count: if have_info { info.ProcessCount } else { 0 },
        thread_count: if have_info { info.ThreadCount } else { 0 },
        handle_count: if have_info { info.HandleCount } else { 0 },
        uptime_seconds: unsafe { GetTickCount64() } / 1000,
    };
    (memory, system)
}

fn cpu_from_times(previous: Option<CpuTimes>, current: Option<CpuTimes>) -> (f64, f64, f64) {
    let (Some(start), Some(end)) = (previous, current) else {
        return (0.0, 0.0, 0.0);
    };
    let idle = end.idle.saturating_sub(start.idle) as f64;
    let kernel = end.kernel.saturating_sub(start.kernel) as f64;
    let user = end.user.saturating_sub(start.user) as f64;
    let total = kernel + user;
    if total <= 0.0 {
        return (0.0, 0.0, 0.0);
    }
    let busy = (total - idle).max(0.0);
    (
        (busy / total * 100.0).clamp(0.0, 100.0),
        (user / total * 100.0).clamp(0.0, 100.0),
        ((kernel - idle).max(0.0) / total * 100.0).clamp(0.0, 100.0),
    )
}

/// Takes one live sample. Call repeatedly (about once a second); the first
/// call primes the rate counters and reports `warm: false`.
pub fn sample(include_processes: bool) -> LivePerformance {
    let hardware = HARDWARE.get_or_init(read_hardware).clone();
    let mut guard = match SAMPLER.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let sampler = guard.get_or_insert_with(|| Sampler {
        query: LiveQuery::open(),
        last_collect: None,
        cpu_times: None,
        processes: HashMap::new(),
        process_sampled_at: None,
    });

    let mut notes = Vec::new();
    let now = Instant::now();
    let collected = sampler.query.as_ref().is_some_and(LiveQuery::collect);
    let interval_ms = sampler
        .last_collect
        .map(|previous| now.duration_since(previous).as_millis() as u64)
        .unwrap_or(0);
    let warm = collected && sampler.last_collect.is_some();
    sampler.last_collect = Some(now);
    if sampler.query.is_none() {
        notes.push(
            "Windows performance counters are unavailable; CPU and memory come from system APIs only."
                .into(),
        );
    }

    let times = perf::read_system_times();
    let (fallback_total, user_percent, kernel_percent) = cpu_from_times(sampler.cpu_times, times);
    sampler.cpu_times = times;

    let query = sampler.query.as_ref().filter(|_| warm);
    let utilization_percent = query
        .and_then(|q| read_single(q.cpu_total.as_ref(), PDH_FMT_NOCAP100))
        .map(|value| value.clamp(0.0, 100.0))
        .unwrap_or(fallback_total);
    let per_core_percent = query
        .map(|q| per_core_values(&read_array(&q.cpu_cores, PDH_FMT_NOCAP100)))
        .unwrap_or_default();
    let speed_mhz = query.and_then(|q| {
        let performance = read_single(q.cpu_performance.as_ref(), PDH_FMT_NOCAP100)?;
        let base = read_single(q.cpu_frequency.as_ref(), 0).or(hardware.base_speed_mhz)?;
        (base > 0.0).then_some(base * performance / 100.0)
    });

    let (memory, system) = read_memory(sampler.query.as_ref().filter(|_| collected));

    let mut disks: BTreeMap<u32, LiveDisk> = BTreeMap::new();
    if let Some(q) = query {
        for (name, idle) in read_array(&q.disk_idle, PDH_FMT_NOCAP100) {
            if let Some((index, volumes)) = parse_disk_instance(&name) {
                disks.insert(
                    index,
                    LiveDisk {
                        index,
                        volumes,
                        active_percent: (100.0 - idle).clamp(0.0, 100.0),
                        read_bytes_per_sec: 0.0,
                        write_bytes_per_sec: 0.0,
                        response_ms: 0.0,
                    },
                );
            }
        }
        for (name, value) in read_array(&q.disk_read, 0) {
            if let Some((index, _)) = parse_disk_instance(&name) {
                if let Some(disk) = disks.get_mut(&index) {
                    disk.read_bytes_per_sec = value.max(0.0);
                }
            }
        }
        for (name, value) in read_array(&q.disk_write, 0) {
            if let Some((index, _)) = parse_disk_instance(&name) {
                if let Some(disk) = disks.get_mut(&index) {
                    disk.write_bytes_per_sec = value.max(0.0);
                }
            }
        }
        for (name, value) in read_array(&q.disk_response, 0) {
            if let Some((index, _)) = parse_disk_instance(&name) {
                if let Some(disk) = disks.get_mut(&index) {
                    disk.response_ms = (value * 1000.0).max(0.0);
                }
            }
        }
    }

    let mut networks: Vec<LiveNetwork> = Vec::new();
    if let Some(q) = query {
        let received: HashMap<String, f64> = read_array(&q.net_received, 0).into_iter().collect();
        let sent: HashMap<String, f64> = read_array(&q.net_sent, 0).into_iter().collect();
        for (name, bandwidth) in read_array(&q.net_bandwidth, 0) {
            if bandwidth <= 0.0 || is_virtual_adapter(&name) {
                continue;
            }
            networks.push(LiveNetwork {
                kind: network_kind(&name).into(),
                received_bytes_per_sec: received.get(&name).copied().unwrap_or(0.0).max(0.0),
                sent_bytes_per_sec: sent.get(&name).copied().unwrap_or(0.0).max(0.0),
                link_bits_per_sec: bandwidth,
                name,
            });
        }
        networks.sort_by(|left, right| left.name.cmp(&right.name));
    }

    let mut gpus: Vec<LiveGpu> = Vec::new();
    if let Some(q) = query {
        let engines = aggregate_gpu_engines(&read_array(&q.gpu_engine, PDH_FMT_NOCAP100));
        let mut dedicated: HashMap<String, u64> = HashMap::new();
        for (name, value) in read_array(&q.gpu_dedicated, 0) {
            if let Some(luid) = parse_gpu_adapter(&name) {
                *dedicated.entry(luid).or_insert(0) += value.max(0.0) as u64;
            }
        }
        let mut shared: HashMap<String, u64> = HashMap::new();
        for (name, value) in read_array(&q.gpu_shared, 0) {
            if let Some(luid) = parse_gpu_adapter(&name) {
                *shared.entry(luid).or_insert(0) += value.max(0.0) as u64;
            }
        }
        for (luid, engine_map) in engines {
            let dedicated_used = dedicated.get(&luid).copied().unwrap_or(0);
            let shared_used = shared.get(&luid).copied().unwrap_or(0);
            // The Basic Render Driver and idle virtual adapters report engines
            // but never memory; Task Manager hides them too.
            if dedicated_used == 0 && shared_used == 0 {
                continue;
            }
            let mut engine_rows: Vec<LiveGpuEngine> = engine_map
                .iter()
                .map(|(name, percent)| LiveGpuEngine {
                    name: friendly_engine(name),
                    percent: *percent,
                })
                .collect();
            engine_rows.sort_by(|left, right| right.percent.total_cmp(&left.percent));
            let utilization = engine_rows
                .first()
                .map(|engine| engine.percent)
                .unwrap_or(0.0);
            gpus.push(LiveGpu {
                index: 0,
                name: None,
                luid,
                utilization_percent: utilization,
                engines: engine_rows,
                dedicated_used_bytes: dedicated_used,
                shared_used_bytes: shared_used,
            });
        }
        // Adapters with dedicated memory first, like Task Manager's GPU 0/1.
        gpus.sort_by(|left, right| right.dedicated_used_bytes.cmp(&left.dedicated_used_bytes));
        let names = &hardware.gpu_names;
        let discrete = names.iter().position(|name| {
            let lower = name.to_ascii_lowercase();
            lower.contains("nvidia")
                || lower.contains("geforce")
                || lower.contains("radeon rx")
                || lower.contains("rtx")
                || lower.contains(" arc")
        });
        let count = gpus.len();
        for (position, gpu) in gpus.iter_mut().enumerate() {
            gpu.index = position as u32;
            // Names come from the registry and cannot be tied to a LUID
            // without DXGI, so they are only attached when the match is unambiguous.
            gpu.name = if count == 1 && names.len() == 1 {
                names.first().cloned()
            } else if count == names.len() && count == 2 {
                let primary = discrete.unwrap_or(0);
                if position == 0 && gpu.dedicated_used_bytes > 0 {
                    names.get(primary).cloned()
                } else {
                    names.get(1 - primary.min(1)).cloned()
                }
            } else {
                None
            };
        }
    }

    let processes =
        include_processes.then(|| sample_processes(sampler, hardware.logical_processors));

    if !warm && sampler.query.is_some() {
        notes.push("Warming up: rate counters need a second sample.".into());
    }

    LivePerformance {
        captured_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_millis() as u64)
            .unwrap_or(0),
        interval_ms,
        warm,
        cpu: LiveCpu {
            utilization_percent,
            user_percent,
            kernel_percent,
            per_core_percent,
            speed_mhz,
        },
        memory,
        disks: disks.into_values().collect(),
        networks,
        gpus,
        system,
        processes,
        hardware,
        notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn per_core_values_sort_numerically_and_skip_totals() {
        let rows = vec![
            ("0,10".to_string(), 10.0),
            ("0,2".to_string(), 2.0),
            ("_Total".to_string(), 50.0),
            ("0,_Total".to_string(), 40.0),
            ("1,0".to_string(), 120.0),
        ];
        assert_eq!(per_core_values(&rows), vec![2.0, 10.0, 100.0]);
    }

    #[test]
    fn disk_instances_keep_index_and_letters() {
        assert_eq!(
            parse_disk_instance("1 D: e:"),
            Some((1, vec!["D:".to_string(), "E:".to_string()]))
        );
        assert_eq!(parse_disk_instance("0"), Some((0, Vec::new())));
        assert_eq!(parse_disk_instance("_Total"), None);
    }

    #[test]
    fn gpu_engines_aggregate_per_adapter_and_type() {
        let rows = vec![
            (
                "pid_1_luid_0x00000000_0x0000D1E5_phys_0_eng_0_engtype_3D".to_string(),
                30.0,
            ),
            (
                "pid_2_luid_0x00000000_0x0000D1E5_phys_0_eng_0_engtype_3D".to_string(),
                25.0,
            ),
            (
                "pid_2_luid_0x00000000_0x0000D1E5_phys_0_eng_5_engtype_Copy".to_string(),
                4.0,
            ),
            (
                "pid_9_luid_0x00000000_0x0000AAAA_phys_0_eng_1_engtype_VideoDecode".to_string(),
                7.0,
            ),
        ];
        let adapters = aggregate_gpu_engines(&rows);
        let first = &adapters["0x00000000_0x0000d1e5"];
        assert_eq!(first["3D"], 55.0);
        assert_eq!(first["Copy"], 4.0);
        assert_eq!(adapters["0x00000000_0x0000aaaa"]["VideoDecode"], 7.0);
        assert_eq!(
            parse_gpu_adapter("luid_0x00000000_0x0000D1E5_phys_0"),
            Some("0x00000000_0x0000d1e5".to_string())
        );
    }

    #[test]
    fn adapters_are_classified() {
        assert_eq!(network_kind("Intel(R) Wi-Fi 6E AX211 160MHz"), "wifi");
        assert_eq!(
            network_kind("Realtek PCIe 2.5GbE Family Controller"),
            "ethernet"
        );
        assert!(is_virtual_adapter("Microsoft ISATAP Adapter"));
        assert!(!is_virtual_adapter(
            "Intel[R] Ethernet Connection [7] I219-V"
        ));
    }
}
