//! Evidence-based Windows performance diagnostics.
//!
//! This module only observes. It never terminates processes, changes services,
//! edits the registry or deletes files. Recommendations cite the measurements
//! that produced them, and only the reversible startup toggle or an explicitly
//! confirmed Windows Performance Recorder trace can be started from this page.

use crate::{mft, process, volumes};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::c_void;
use std::fs;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const SAMPLE_WINDOW_MS: u64 = 700;
const MAX_PROCESSES: usize = 4096;
const TOP_PROCESSES: usize = 32;
const STARTUP_EVIDENCE_TTL_SECS: u64 = 120;
const MAX_STARTUP_EVENTS: usize = 160;
const MAX_SCHEDULED_TASKS: usize = 120;
const TRACE_CREATE_NO_WINDOW: u32 = 0x0800_0000;
const BASELINE_LIMIT: usize = 50;
const BASELINE_FILE: &str = "performance-baselines.json";

const CPU_WARNING_PERCENT: f64 = 80.0;
const MEMORY_AVAILABLE_WARNING: f64 = 0.10;
const COMMIT_WARNING_PERCENT: f64 = 85.0;
const FREE_SPACE_WARNING: f64 = 0.10;
const DISK_BUSY_WARNING: f64 = 70.0;
const DISK_QUEUE_WARNING: f64 = 2.0;
const STARTUP_IMPACT_MIN_MS: u64 = 1_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceSnapshot {
    pub captured_at: u64,
    pub sample_ms: u64,
    pub elevated: bool,
    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    pub disk_io: DiskIoMetrics,
    pub volumes: Vec<volumes::VolumeInfo>,
    pub processes: Vec<ProcessUsage>,
    pub process_count: u32,
    pub unmeasured_process_count: u32,
    pub startup: StartupAnalysis,
    pub recommendations: Vec<Recommendation>,
    pub findings: Vec<Finding>,
    pub limitations: Vec<String>,
    pub trace: TraceStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuMetrics {
    pub utilization_percent: f64,
    pub user_percent: f64,
    pub kernel_percent: f64,
    pub idle_percent: f64,
    pub logical_cores: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryMetrics {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_percent: f64,
    pub commit_used_bytes: u64,
    pub commit_limit_bytes: u64,
    pub commit_percent: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskIoMetrics {
    /// measured, unavailable.
    pub status: String,
    pub busy_percent: Option<f64>,
    pub queue_length: Option<f64>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessUsage {
    pub pid: u32,
    pub name: String,
    pub path: Option<String>,
    pub cpu_percent: f64,
    pub working_set_bytes: u64,
    pub private_bytes: u64,
    pub io_read_bytes_per_sec: f64,
    pub io_write_bytes_per_sec: f64,
    pub is_critical: bool,
    pub is_cyron: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupAnalysis {
    pub items: Vec<StartupEvidence>,
    pub scheduled_tasks: Vec<ScheduledStartupTask>,
    pub boot: Option<BootSummary>,
    pub offenders: Vec<BootOffender>,
    /// available, denied or unavailable.
    pub event_log_status: String,
    pub event_log_note: String,
    pub enabled_count: u32,
    pub total_count: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupEvidence {
    pub id: String,
    pub name: String,
    pub command: String,
    pub location: String,
    pub scope: String,
    pub enabled: bool,
    pub can_toggle: bool,
    pub is_protected: bool,
    pub launch_path: Option<String>,
    pub current_cpu_percent: f64,
    pub current_working_set_bytes: u64,
    pub boot_impact_ms: Option<u64>,
    pub boot_total_ms: Option<u64>,
    /// diagnostics-performance, live-process or inventory.
    pub evidence_source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledStartupTask {
    pub name: String,
    pub path: String,
    pub author: String,
    pub enabled: bool,
    pub triggers: Vec<String>,
    pub delay: Option<String>,
    pub command: String,
    pub arguments: String,
    pub is_microsoft: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootSummary {
    pub observed_at: String,
    pub boot_time_ms: u64,
    pub main_path_boot_ms: u64,
    pub post_boot_time_ms: u64,
    pub startup_app_count: u32,
    pub degradation_ms: i64,
    pub is_degradation: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootOffender {
    pub event_id: u32,
    pub name: String,
    pub friendly_name: String,
    pub path: String,
    pub total_time_ms: u64,
    pub degradation_time_ms: u64,
    /// application, driver, service, device or system.
    pub category: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationAction {
    /// startup, view, page, trace or none.
    pub kind: String,
    pub label: String,
    pub target_id: Option<String>,
    pub view: Option<String>,
    pub page: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    pub id: String,
    pub category: String,
    /// attention, review or info.
    pub severity: String,
    pub title: String,
    pub summary: String,
    pub evidence: Vec<Evidence>,
    pub expected_effect: String,
    pub risk: String,
    pub verification: String,
    pub action: RecommendationAction,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub id: String,
    /// ok, watch or attention.
    pub tone: String,
    pub title: String,
    pub detail: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceStatus {
    pub available: bool,
    pub active: bool,
    pub started_at: Option<u64>,
    pub output_path: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct TraceSession {
    pub started_at: u64,
    pub output_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceBaseline {
    pub id: String,
    pub label: String,
    pub recorded_at: u64,
    pub cpu_percent: f64,
    pub memory_used_percent: f64,
    pub available_memory_bytes: u64,
    pub commit_percent: f64,
    pub disk_busy_percent: Option<f64>,
    pub disk_queue_length: Option<f64>,
    pub top_cpu_process: Option<String>,
    pub top_memory_process: Option<String>,
    pub boot_time_ms: Option<u64>,
    pub enabled_startup_count: u32,
}

static STARTUP_CACHE: Mutex<Option<(Instant, StartupAnalysis)>> = Mutex::new(None);

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn filetime_value(low: u32, high: u32) -> u64 {
    ((high as u64) << 32) | low as u64
}

fn is_cyron_process(path: Option<&str>, pid: u32) -> bool {
    if pid == std::process::id() {
        return true;
    }
    path.map(|path| {
        Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("cyron.exe"))
    })
    .unwrap_or(false)
}

/* ------------------------------------------------------------------ sampling */

#[derive(Debug, Clone, Copy)]
struct CpuTimes {
    idle: u64,
    kernel: u64,
    user: u64,
}

fn read_system_times() -> Option<CpuTimes> {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::GetSystemTimes;

    let mut idle = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut kernel = idle;
    let mut user = idle;
    let ok = unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) };
    if ok == 0 {
        return None;
    }
    Some(CpuTimes {
        idle: filetime_value(idle.dwLowDateTime, idle.dwHighDateTime),
        kernel: filetime_value(kernel.dwLowDateTime, kernel.dwHighDateTime),
        user: filetime_value(user.dwLowDateTime, user.dwHighDateTime),
    })
}

fn cpu_delta(start: CpuTimes, end: CpuTimes) -> CpuMetrics {
    let idle = end.idle.saturating_sub(start.idle) as f64;
    let kernel = end.kernel.saturating_sub(start.kernel) as f64;
    let user = end.user.saturating_sub(start.user) as f64;
    // GetSystemTimes documents that kernel time already includes idle time.
    let total = kernel + user;
    let busy = (total - idle).max(0.0);
    let logical_cores = unsafe {
        use windows_sys::Win32::System::SystemInformation::{GetNativeSystemInfo, SYSTEM_INFO};
        let mut info: SYSTEM_INFO = std::mem::zeroed();
        GetNativeSystemInfo(&mut info);
        info.dwNumberOfProcessors.max(1)
    };
    if total <= 0.0 {
        return CpuMetrics {
            utilization_percent: 0.0,
            user_percent: 0.0,
            kernel_percent: 0.0,
            idle_percent: 0.0,
            logical_cores,
        };
    }
    CpuMetrics {
        utilization_percent: (busy / total * 100.0).clamp(0.0, 100.0),
        user_percent: (user / total * 100.0).clamp(0.0, 100.0),
        kernel_percent: ((kernel - idle).max(0.0) / total * 100.0).clamp(0.0, 100.0),
        idle_percent: (idle / total * 100.0).clamp(0.0, 100.0),
        logical_cores,
    }
}

fn read_memory() -> (MemoryMetrics, Option<String>) {
    use windows_sys::Win32::System::ProcessStatus::{GetPerformanceInfo, PERFORMANCE_INFORMATION};
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

    let mut total = 0u64;
    let mut available = 0u64;
    unsafe {
        let mut status = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            dwMemoryLoad: 0,
            ullTotalPhys: 0,
            ullAvailPhys: 0,
            ullTotalPageFile: 0,
            ullAvailPageFile: 0,
            ullTotalVirtual: 0,
            ullAvailVirtual: 0,
            ullAvailExtendedVirtual: 0,
        };
        if GlobalMemoryStatusEx(&mut status) != 0 {
            total = status.ullTotalPhys;
            available = status.ullAvailPhys;
        }
    }

    let mut commit_used = 0u64;
    let mut commit_limit = 0u64;
    let mut note = None;
    unsafe {
        let mut info: PERFORMANCE_INFORMATION = std::mem::zeroed();
        info.cb = std::mem::size_of::<PERFORMANCE_INFORMATION>() as u32;
        if GetPerformanceInfo(&mut info, info.cb) != 0 {
            let page = info.PageSize as u64;
            commit_used = (info.CommitTotal as u64).saturating_mul(page);
            commit_limit = (info.CommitLimit as u64).saturating_mul(page);
        } else {
            note = Some(
                "Windows did not return system commit information; only physical memory is shown."
                    .into(),
            );
        }
    }

    let used_percent = if total > 0 {
        ((total.saturating_sub(available)) as f64 / total as f64 * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    let commit_percent = if commit_limit > 0 {
        (commit_used as f64 / commit_limit as f64 * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };

    (
        MemoryMetrics {
            total_bytes: total,
            available_bytes: available,
            used_percent,
            commit_used_bytes: commit_used,
            commit_limit_bytes: commit_limit,
            commit_percent,
        },
        note,
    )
}

#[derive(Debug, Clone)]
struct OpenProcessHandle {
    pid: u32,
    name: String,
    path: Option<String>,
    handle: windows_sys::Win32::Foundation::HANDLE,
}

impl Drop for OpenProcessHandle {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

fn process_names() -> HashMap<u32, String> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let mut names = HashMap::new();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot.is_null() || snapshot == INVALID_HANDLE_VALUE {
            return names;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                let name = String::from_utf16_lossy(
                    &entry.szExeFile[..entry
                        .szExeFile
                        .iter()
                        .position(|c| *c == 0)
                        .unwrap_or(entry.szExeFile.len())],
                );
                names.insert(entry.th32ProcessID, name);
                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
    }
    names
}

fn enumerate_pids() -> Vec<u32> {
    use windows_sys::Win32::System::ProcessStatus::EnumProcesses;

    let mut capacity = 1024usize;
    loop {
        let mut pids = vec![0u32; capacity];
        let mut returned = 0u32;
        let ok = unsafe {
            EnumProcesses(
                pids.as_mut_ptr(),
                (pids.len() * std::mem::size_of::<u32>()) as u32,
                &mut returned,
            )
        };
        if ok == 0 {
            return Vec::new();
        }
        let count = returned as usize / std::mem::size_of::<u32>();
        // EnumProcesses does not report truncation, so a full buffer means retry.
        if count >= capacity && capacity < MAX_PROCESSES {
            capacity = (capacity * 2).min(MAX_PROCESSES);
            continue;
        }
        pids.truncate(count.min(MAX_PROCESSES));
        return pids;
    }
}

fn open_processes() -> Vec<OpenProcessHandle> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let names = process_names();
    let mut handles = Vec::new();
    for pid in enumerate_pids() {
        if pid == 0 {
            continue;
        }
        let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if raw.is_null() {
            continue;
        }
        let mut buffer = vec![0u16; 1024];
        let mut size = buffer.len() as u32;
        let ok = unsafe { QueryFullProcessImageNameW(raw, 0, buffer.as_mut_ptr(), &mut size) };
        let path = if ok != 0 && size > 0 {
            buffer.truncate(size as usize);
            Some(String::from_utf16_lossy(&buffer))
        } else {
            None
        };
        let name = path
            .as_deref()
            .and_then(|value| Path::new(value).file_name())
            .map(|value| value.to_string_lossy().to_string())
            .or_else(|| names.get(&pid).cloned())
            .unwrap_or_else(|| format!("PID {pid}"));
        if path.is_none() && !names.contains_key(&pid) {
            unsafe { CloseHandle(raw) };
            continue;
        }
        handles.push(OpenProcessHandle {
            pid,
            name,
            path,
            handle: raw,
        });
    }
    handles
}

#[derive(Debug, Clone, Copy, Default)]
struct ProcessCounters {
    cpu_100ns: u64,
    working_set: u64,
    private_bytes: u64,
    read_bytes: u64,
    write_bytes: u64,
}

fn read_process_counters(handle: windows_sys::Win32::Foundation::HANDLE) -> ProcessCounters {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::{
        GetProcessIoCounters, GetProcessTimes, IO_COUNTERS,
    };

    let mut counters = ProcessCounters::default();
    unsafe {
        let mut creation = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut exit = creation;
        let mut kernel = creation;
        let mut user = creation;
        if GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) != 0 {
            counters.cpu_100ns = filetime_value(kernel.dwLowDateTime, kernel.dwHighDateTime)
                + filetime_value(user.dwLowDateTime, user.dwHighDateTime);
        }

        let mut io: IO_COUNTERS = std::mem::zeroed();
        if GetProcessIoCounters(handle, &mut io) != 0 {
            counters.read_bytes = io.ReadTransferCount;
            counters.write_bytes = io.WriteTransferCount;
        }

        let mut memory: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        memory.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        if GetProcessMemoryInfo(handle, &mut memory, memory.cb) != 0 {
            counters.working_set = memory.WorkingSetSize as u64;
            counters.private_bytes = memory.PagefileUsage as u64;
        }
    }
    counters
}

fn snapshot_processes(
    handles: &[OpenProcessHandle],
) -> Vec<(u32, String, Option<String>, ProcessCounters)> {
    handles
        .iter()
        .map(|entry| {
            (
                entry.pid,
                entry.name.clone(),
                entry.path.clone(),
                read_process_counters(entry.handle),
            )
        })
        .collect()
}

fn process_usage(
    first: &[(u32, String, Option<String>, ProcessCounters)],
    second: &[(u32, String, Option<String>, ProcessCounters)],
    elapsed: Duration,
    logical_cores: u32,
) -> Vec<ProcessUsage> {
    let previous: HashMap<u32, &(u32, String, Option<String>, ProcessCounters)> =
        first.iter().map(|entry| (entry.0, entry)).collect();
    let seconds = elapsed.as_secs_f64().max(0.001);
    let denominator = seconds * 1000.0 * 10_000.0 * logical_cores.max(1) as f64;
    let mut usages: Vec<ProcessUsage> = second
        .iter()
        .filter_map(|(pid, name, path, counters)| {
            let prior = previous.get(pid)?;
            let cpu_delta = counters.cpu_100ns.saturating_sub(prior.3.cpu_100ns);
            let read_delta = counters.read_bytes.saturating_sub(prior.3.read_bytes);
            let write_delta = counters.write_bytes.saturating_sub(prior.3.write_bytes);
            Some(ProcessUsage {
                pid: *pid,
                name: name.clone(),
                path: path.clone(),
                cpu_percent: (cpu_delta as f64 / denominator * 100.0).clamp(0.0, 100.0),
                working_set_bytes: counters.working_set,
                private_bytes: counters.private_bytes,
                io_read_bytes_per_sec: read_delta as f64 / seconds,
                io_write_bytes_per_sec: write_delta as f64 / seconds,
                is_critical: process::is_critical_process(*pid, name, path.as_deref()),
                is_cyron: is_cyron_process(path.as_deref(), *pid),
            })
        })
        .collect();
    usages.sort_by(|left, right| {
        right
            .cpu_percent
            .total_cmp(&left.cpu_percent)
            .then_with(|| right.working_set_bytes.cmp(&left.working_set_bytes))
    });
    usages.truncate(TOP_PROCESSES);
    usages
}

/* ---------------------------------------------------------------- disk I/O */

struct DiskQuery {
    query: *mut c_void,
    busy: *mut c_void,
    queue: *mut c_void,
    valid: bool,
}

impl DiskQuery {
    fn start() -> Option<Self> {
        use windows_sys::Win32::System::Performance::{
            PdhAddEnglishCounterW, PdhCollectQueryData, PdhOpenQueryW,
        };

        unsafe {
            let mut query: *mut c_void = std::ptr::null_mut();
            if PdhOpenQueryW(std::ptr::null(), 0, &mut query) != 0 {
                return None;
            }
            let mut busy: *mut c_void = std::ptr::null_mut();
            let mut queue: *mut c_void = std::ptr::null_mut();
            let busy_path = wide(r"\PhysicalDisk(_Total)\% Idle Time");
            let queue_path = wide(r"\PhysicalDisk(_Total)\Avg. Disk Queue Length");
            let busy_ok = PdhAddEnglishCounterW(query, busy_path.as_ptr(), 0, &mut busy) == 0;
            let queue_ok = PdhAddEnglishCounterW(query, queue_path.as_ptr(), 0, &mut queue) == 0;
            let collect_ok = PdhCollectQueryData(query) == 0;
            Some(Self {
                query,
                busy,
                queue,
                valid: busy_ok && queue_ok && collect_ok,
            })
        }
    }

    fn finish(self) -> DiskIoMetrics {
        use windows_sys::Win32::System::Performance::{
            PdhCollectQueryData, PdhGetFormattedCounterValue, PDH_FMT_COUNTERVALUE,
        };

        if !self.valid {
            unsafe {
                windows_sys::Win32::System::Performance::PdhCloseQuery(self.query);
            }
            return DiskIoMetrics {
                status: "unavailable".into(),
                busy_percent: None,
                queue_length: None,
                note: Some(
                    "Windows performance counters did not return physical-disk data; disk I/O activity is still measured per process."
                        .into(),
                ),
            };
        }

        let mut busy_value: PDH_FMT_COUNTERVALUE = unsafe { std::mem::zeroed() };
        let mut queue_value: PDH_FMT_COUNTERVALUE = unsafe { std::mem::zeroed() };
        let mut value_type = 0u32;
        let collected = unsafe { PdhCollectQueryData(self.query) == 0 };
        let busy_ok = collected
            && unsafe {
                PdhGetFormattedCounterValue(
                    self.busy,
                    0x0000_0200, // PDH_FMT_DOUBLE
                    &mut value_type,
                    &mut busy_value,
                )
            } == 0;
        let queue_ok = collected
            && unsafe {
                PdhGetFormattedCounterValue(
                    self.queue,
                    0x0000_0200,
                    &mut value_type,
                    &mut queue_value,
                )
            } == 0;
        unsafe {
            windows_sys::Win32::System::Performance::PdhCloseQuery(self.query);
        }

        if !busy_ok || !queue_ok {
            return DiskIoMetrics {
                status: "unavailable".into(),
                busy_percent: None,
                queue_length: None,
                note: Some(
                    "The physical-disk counters could not be formatted; no disk-busy claim is made."
                        .into(),
                ),
            };
        }

        let idle = unsafe { busy_value.Anonymous.doubleValue };
        let queue = unsafe { queue_value.Anonymous.doubleValue };
        DiskIoMetrics {
            status: "measured".into(),
            busy_percent: Some((100.0 - idle).clamp(0.0, 100.0)),
            queue_length: Some(queue.max(0.0)),
            note: None,
        }
    }
}

/* ------------------------------------------------------------- boot evidence */

fn get_last_error() -> u32 {
    unsafe { windows_sys::Win32::Foundation::GetLastError() }
}

fn event_xml(event: windows_sys::Win32::System::EventLog::EVT_HANDLE) -> Option<String> {
    use windows_sys::Win32::System::EventLog::{EvtRender, EvtRenderEventXml};

    let mut used = 0u32;
    let mut properties = 0u32;
    let first = unsafe {
        EvtRender(
            0,
            event,
            EvtRenderEventXml,
            0,
            std::ptr::null_mut(),
            &mut used,
            &mut properties,
        )
    };
    if first != 0 || used == 0 {
        return None;
    }
    let mut buffer = vec![0u16; (used as usize).div_ceil(2) + 1];
    let ok = unsafe {
        EvtRender(
            0,
            event,
            EvtRenderEventXml,
            (buffer.len() * 2) as u32,
            buffer.as_mut_ptr() as *mut c_void,
            &mut used,
            &mut properties,
        )
    };
    if ok == 0 {
        return None;
    }
    let length = buffer
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(buffer.len());
    Some(String::from_utf16_lossy(&buffer[..length]))
}

fn read_boot_events() -> (Option<BootSummary>, Vec<BootOffender>, String, String) {
    use windows_sys::Win32::System::EventLog::{
        EvtClose, EvtNext, EvtQuery, EvtQueryChannelPath, EvtQueryReverseDirection,
    };

    let channel = wide("Microsoft-Windows-Diagnostics-Performance/Operational");
    let query = wide(
        "*[System[(EventID=100 or EventID=101 or EventID=102 or EventID=103 or EventID=109 or EventID=110)]]",
    );
    let result = unsafe {
        EvtQuery(
            0,
            channel.as_ptr(),
            query.as_ptr(),
            EvtQueryChannelPath | EvtQueryReverseDirection,
        )
    };
    if result == 0 {
        let code = get_last_error();
        let note = if code == 5 {
            "Windows only exposes its boot-performance log to administrators, so per-app boot impact is unavailable in this session. Reopen Cyron as administrator to include it."
                .to_string()
        } else {
            format!(
                "The Windows boot-performance log could not be opened (error {code}); no boot-impact claim is made."
            )
        };
        return (
            None,
            Vec::new(),
            if code == 5 { "denied" } else { "unavailable" }.into(),
            note,
        );
    }

    let mut boot: Option<BootSummary> = None;
    let mut offenders = Vec::new();
    let mut seen = 0usize;
    loop {
        let mut events = [0isize; 16];
        let mut returned = 0u32;
        let ok = unsafe {
            EvtNext(
                result,
                events.len() as u32,
                events.as_mut_ptr(),
                1_000,
                0,
                &mut returned,
            )
        };
        if ok == 0 {
            break;
        }
        for event in events.iter().take(returned as usize) {
            seen += 1;
            if let Some(xml) = event_xml(*event) {
                let event_id = tag_text(&xml, "EventID")
                    .and_then(|value| value.parse::<u32>().ok())
                    .unwrap_or(0);
                match event_id {
                    100 if boot.is_none() => boot = Some(parse_boot_summary(&xml)),
                    101..=110 => {
                        if let Some(offender) = parse_boot_offender(event_id, &xml) {
                            offenders.push(offender);
                        }
                    }
                    _ => {}
                }
            }
            unsafe {
                EvtClose(*event);
            }
            if seen >= MAX_STARTUP_EVENTS {
                break;
            }
        }
        if seen >= MAX_STARTUP_EVENTS || returned == 0 {
            break;
        }
    }
    unsafe {
        EvtClose(result);
    }

    offenders.sort_by(|left, right| {
        right
            .degradation_time_ms
            .cmp(&left.degradation_time_ms)
            .then_with(|| right.total_time_ms.cmp(&left.total_time_ms))
    });
    offenders.truncate(40);

    let note = if boot.is_some() {
        format!(
            "Windows boot-performance events were available ({} component records reviewed).",
            offenders.len()
        )
    } else {
        "The boot-performance channel is readable but contains no boot summary yet. Windows records one after a restart."
            .to_string()
    };
    (
        boot,
        offenders,
        if seen > 0 { "available" } else { "unavailable" }.into(),
        note,
    )
}

fn parse_boot_summary(xml: &str) -> BootSummary {
    BootSummary {
        observed_at: event_time(xml).unwrap_or_default(),
        boot_time_ms: data_u64(xml, "BootTime").unwrap_or(0),
        main_path_boot_ms: data_u64(xml, "MainPathBootTime").unwrap_or(0),
        post_boot_time_ms: data_u64(xml, "BootPostBootTime").unwrap_or(0),
        startup_app_count: data_u64(xml, "BootNumStartupApps").unwrap_or(0) as u32,
        degradation_ms: data_u64(xml, "BootDegradationDelta")
            .map(|value| value as i64)
            .unwrap_or(0),
        is_degradation: data_u64(xml, "BootIsDegradation").unwrap_or(0) != 0,
    }
}

fn parse_boot_offender(event_id: u32, xml: &str) -> Option<BootOffender> {
    let category = match event_id {
        101 => "application",
        102 => "driver",
        103 => "service",
        109 => "device",
        _ => "system",
    };
    let name = data_value(xml, "Name").unwrap_or_default();
    if name.is_empty() && data_value(xml, "Path").unwrap_or_default().is_empty() {
        return None;
    }
    Some(BootOffender {
        event_id,
        name,
        friendly_name: data_value(xml, "FriendlyName").unwrap_or_default(),
        path: data_value(xml, "Path").unwrap_or_default(),
        total_time_ms: data_u64(xml, "TotalTime").unwrap_or(0),
        degradation_time_ms: data_u64(xml, "DegradationTime").unwrap_or(0),
        category: category.into(),
    })
}

fn event_time(xml: &str) -> Option<String> {
    let marker = "<TimeCreated SystemTime=\"";
    let start = xml.find(marker)? + marker.len();
    let end = xml[start..].find('"')?;
    Some(xml[start..start + end].to_string())
}

fn tag_text(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)?;
    Some(xml_unescape(xml[start..start + end].trim()))
}

fn data_value(xml: &str, name: &str) -> Option<String> {
    let marker = format!("<Data Name=\"{name}\">");
    let start = xml.find(&marker)? + marker.len();
    let end = xml[start..].find("</Data>")?;
    Some(xml_unescape(xml[start..start + end].trim()))
}

fn data_u64(xml: &str, name: &str) -> Option<u64> {
    data_value(xml, name)?.parse::<u64>().ok()
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

/* ------------------------------------------------------------ startup items */

fn expand_environment(value: &str) -> String {
    let mut output = String::new();
    let mut rest = value;
    while let Some(start) = rest.find('%') {
        output.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('%') else {
            output.push_str(&rest[start..]);
            return output;
        };
        let name = &after[..end];
        match std::env::var(name) {
            Ok(replacement) => output.push_str(&replacement),
            Err(_) => {
                output.push('%');
                output.push_str(name);
                output.push('%');
            }
        }
        rest = &after[end + 1..];
    }
    output.push_str(rest);
    output
}

fn command_executable(command: &str) -> Option<String> {
    let expanded = expand_environment(command.trim());
    if expanded.is_empty() {
        return None;
    }
    let candidate = if let Some(stripped) = expanded.strip_prefix('"') {
        stripped.split('"').next().unwrap_or_default().to_string()
    } else {
        let lower = expanded.to_ascii_lowercase();
        if let Some(index) = lower.find(".exe") {
            expanded[..index + 4].to_string()
        } else {
            expanded
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_string()
        }
    };
    let trimmed = candidate.trim().trim_matches('"');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn path_key(value: &str) -> String {
    let expanded = expand_environment(value.trim().trim_matches('"'));
    let stripped = expanded.strip_prefix(r"\\?\").unwrap_or(&expanded);
    stripped
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

fn startup_scope(location: &str) -> &'static str {
    if location.contains("HKLM") || location.contains("All Users") {
        "machine"
    } else {
        "user"
    }
}

fn build_startup_evidence(
    items: Vec<crate::system::StartupItem>,
    boot: &Option<BootSummary>,
    offenders: &[BootOffender],
    event_log_status: &str,
    event_log_note: String,
) -> StartupAnalysis {
    let boot_impact: HashMap<String, (u64, u64)> = offenders
        .iter()
        .filter(|offender| offender.event_id == 101)
        .map(|offender| {
            (
                path_key(&offender.path),
                (offender.total_time_ms, offender.degradation_time_ms),
            )
        })
        .filter(|(key, _)| !key.is_empty())
        .collect();

    let mut evidence = Vec::new();
    for item in items {
        let launch = command_executable(&item.command);
        let launch_key = launch.as_deref().map(path_key);
        let impact = launch_key
            .as_deref()
            .and_then(|key| boot_impact.get(key))
            .copied();
        evidence.push(StartupEvidence {
            scope: startup_scope(&item.location).into(),
            id: item.id,
            name: item.name,
            command: item.command,
            location: item.location,
            enabled: item.enabled,
            can_toggle: item.can_toggle,
            is_protected: item.is_protected,
            launch_path: launch,
            current_cpu_percent: 0.0,
            current_working_set_bytes: 0,
            boot_impact_ms: impact.map(|(_, degradation)| degradation),
            boot_total_ms: impact.map(|(total, _)| total),
            evidence_source: if impact.is_some() {
                "diagnostics-performance".into()
            } else {
                "inventory".into()
            },
        });
    }

    let enabled_count = evidence.iter().filter(|item| item.enabled).count() as u32;
    StartupAnalysis {
        total_count: evidence.len() as u32,
        enabled_count,
        items: evidence,
        boot: boot.clone(),
        offenders: offenders.to_vec(),
        event_log_status: event_log_status.into(),
        event_log_note,
        scheduled_tasks: Vec::new(),
    }
}

fn parse_scheduled_tasks(xml: &str) -> Vec<ScheduledStartupTask> {
    let mut tasks = Vec::new();
    let mut cursor = 0usize;
    while let Some(found) = xml[cursor..].find("<Task version") {
        let start = cursor + found;
        let Some(close) = xml[start..].find("</Task>") else {
            break;
        };
        let end = start + close + "</Task>".len();
        let block = &xml[start..end];
        cursor = end;

        let mut triggers = Vec::new();
        let mut delay = None;
        for (tag, label) in [
            ("BootTrigger", "boot"),
            ("LogonTrigger", "logon"),
            ("RegistrationTrigger", "registration"),
        ] {
            let open = format!("<{tag}>");
            if let Some(trigger_start) = block.find(&open) {
                let content_start = trigger_start + open.len();
                if let Some(trigger_end) = block[content_start..].find(&format!("</{tag}>")) {
                    let trigger = &block[content_start..content_start + trigger_end];
                    if !trigger.contains("<Enabled>false</Enabled>") {
                        triggers.push(label.to_string());
                        if delay.is_none() {
                            delay = tag_text(trigger, "Delay");
                        }
                    }
                }
            }
        }
        if triggers.is_empty() {
            continue;
        }

        let previous_comment = xml[..start].rfind("<!--");
        let previous_close = xml[..start].rfind("-->");
        let path = match (previous_comment, previous_close) {
            (Some(comment_start), Some(comment_end))
                if comment_start < comment_end && comment_end < start =>
            {
                xml[comment_start + 4..comment_end]
                    .trim()
                    .trim_start_matches('\\')
                    .to_string()
            }
            _ => tag_text(block, "URI")
                .unwrap_or_default()
                .trim_start_matches('\\')
                .to_string(),
        };
        let settings = block
            .find("<Settings>")
            .and_then(|settings_start| {
                block[settings_start..]
                    .find("</Settings>")
                    .map(|settings_end| &block[settings_start..settings_start + settings_end])
            })
            .unwrap_or("");
        let enabled = !settings.contains("<Enabled>false</Enabled>");

        tasks.push(ScheduledStartupTask {
            name: path
                .rsplit('\\')
                .next()
                .filter(|name| !name.is_empty())
                .unwrap_or(&path)
                .to_string(),
            is_microsoft: path.to_ascii_lowercase().starts_with("microsoft\\windows"),
            path,
            author: tag_text(block, "Author").unwrap_or_default(),
            enabled,
            triggers,
            delay,
            command: tag_text(block, "Command").unwrap_or_default(),
            arguments: tag_text(block, "Arguments").unwrap_or_default(),
        });
        if tasks.len() >= MAX_SCHEDULED_TASKS {
            break;
        }
    }
    tasks
}

fn scheduled_startup_tasks() -> (Vec<ScheduledStartupTask>, Option<String>) {
    let output = match windows_command("System32\\schtasks.exe") {
        Ok(mut command) => command.args(["/query", "/xml"]).output(),
        Err(error) => return (Vec::new(), Some(error)),
    };
    match output {
        Ok(output) if output.status.success() => {
            let xml = String::from_utf8_lossy(&output.stdout);
            let mut tasks = parse_scheduled_tasks(&xml);
            tasks.sort_by(|left, right| {
                left.path
                    .to_ascii_lowercase()
                    .cmp(&right.path.to_ascii_lowercase())
            });
            let truncated = tasks.len() >= MAX_SCHEDULED_TASKS;
            (tasks, truncated.then(|| format!("Scheduled-task analysis stopped after {MAX_SCHEDULED_TASKS} boot/logon tasks.")))
        }
        Ok(output) => (
            Vec::new(),
            Some(format!(
                "Windows Task Scheduler XML could not be read (exit {}); scheduled startup tasks are not listed.",
                output.status.code().unwrap_or(-1)
            )),
        ),
        Err(error) => (
            Vec::new(),
            Some(format!(
                "Windows Task Scheduler could not be queried: {error}. Scheduled startup tasks are not listed."
            )),
        ),
    }
}

fn startup_analysis() -> StartupAnalysis {
    if let Ok(cache) = STARTUP_CACHE.lock() {
        if let Some((at, ref analysis)) = *cache {
            if at.elapsed() < Duration::from_secs(STARTUP_EVIDENCE_TTL_SECS) {
                return analysis.clone();
            }
        }
    }

    let (boot, offenders, event_status, event_note) = read_boot_events();
    let (tasks, task_note) = scheduled_startup_tasks();
    let mut analysis = build_startup_evidence(
        crate::system::list_startup_items(),
        &boot,
        &offenders,
        &event_status,
        event_note,
    );
    analysis.scheduled_tasks = tasks;
    if let Some(note) = task_note {
        analysis.event_log_note = format!("{} {note}", analysis.event_log_note);
    }
    if let Ok(mut cache) = STARTUP_CACHE.lock() {
        *cache = Some((Instant::now(), analysis.clone()));
    }
    analysis
}

fn annotate_live_startup_usage(analysis: &mut StartupAnalysis, processes: &[ProcessUsage]) {
    let mut by_path: HashMap<String, &ProcessUsage> = HashMap::new();
    for process in processes {
        if let Some(path) = process.path.as_deref() {
            by_path
                .entry(path_key(path))
                .and_modify(|current| {
                    if process.cpu_percent > current.cpu_percent {
                        *current = process;
                    }
                })
                .or_insert(process);
        }
    }
    for item in &mut analysis.items {
        if let Some(key) = item.launch_path.as_deref().map(path_key) {
            if let Some(process) = by_path.get(&key) {
                item.current_cpu_percent = process.cpu_percent;
                item.current_working_set_bytes = process.working_set_bytes;
                if item.evidence_source == "inventory" {
                    item.evidence_source = "live-process".into();
                }
            }
        }
    }
}

/* ------------------------------------------------------------ recommendations */

fn action(kind: &str, label: &str) -> RecommendationAction {
    RecommendationAction {
        kind: kind.into(),
        label: label.into(),
        target_id: None,
        view: None,
        page: None,
    }
}

fn recommendation(
    id: &str,
    category: &str,
    severity: &str,
    title: String,
    summary: String,
    evidence: Vec<Evidence>,
    expected_effect: &str,
    risk: &str,
    verification: &str,
    action: RecommendationAction,
) -> Recommendation {
    Recommendation {
        id: id.into(),
        category: category.into(),
        severity: severity.into(),
        title,
        summary,
        evidence,
        expected_effect: expected_effect.into(),
        risk: risk.into(),
        verification: verification.into(),
        action,
    }
}

fn evidence(label: &str, value: String) -> Evidence {
    Evidence {
        label: label.into(),
        value,
    }
}

fn percent(value: f64) -> String {
    format!("{value:.1}%")
}

fn bytes(value: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = value as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{value} B")
    } else if size >= 100.0 {
        format!("{size:.0} {}", UNITS[unit])
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

fn duration(ms: u64) -> String {
    if ms < 1_000 {
        format!("{ms} ms")
    } else if ms < 60_000 {
        format!("{:.1} s", ms as f64 / 1000.0)
    } else {
        format!("{} min {:.0} s", ms / 60_000, (ms % 60_000) as f64 / 1000.0)
    }
}

fn build_recommendations(
    cpu: &CpuMetrics,
    memory: &MemoryMetrics,
    disk: &DiskIoMetrics,
    volumes_list: &[volumes::VolumeInfo],
    processes: &[ProcessUsage],
    startup: &StartupAnalysis,
) -> Vec<Recommendation> {
    let mut recommendations = Vec::new();
    let user_processes: Vec<&ProcessUsage> = processes
        .iter()
        .filter(|process| !process.is_cyron && !process.is_critical)
        .collect();
    let top_cpu = user_processes
        .iter()
        .copied()
        .max_by(|left, right| left.cpu_percent.total_cmp(&right.cpu_percent));
    let top_memory = user_processes
        .iter()
        .copied()
        .filter(|process| process.working_set_bytes > 0)
        .max_by_key(|process| process.working_set_bytes);
    let top_io = processes
        .iter()
        .filter(|process| !process.is_cyron)
        .max_by(|left, right| {
            (left.io_read_bytes_per_sec + left.io_write_bytes_per_sec)
                .total_cmp(&(right.io_read_bytes_per_sec + right.io_write_bytes_per_sec))
        });
    let total_io: f64 = processes
        .iter()
        .map(|process| process.io_read_bytes_per_sec + process.io_write_bytes_per_sec)
        .sum();

    if cpu.utilization_percent >= CPU_WARNING_PERCENT {
        let mut evidence_items = vec![
            evidence(
                "Total CPU",
                format!(
                    "{} across {} logical processors ({} ms sample)",
                    percent(cpu.utilization_percent),
                    cpu.logical_cores,
                    SAMPLE_WINDOW_MS
                ),
            ),
            evidence(
                "User / kernel",
                format!(
                    "{} / {}",
                    percent(cpu.user_percent),
                    percent(cpu.kernel_percent)
                ),
            ),
        ];
        if let Some(process) = top_cpu.filter(|process| process.cpu_percent >= 1.0) {
            evidence_items.push(evidence(
                "Largest application process",
                format!(
                    "{} · {} · PID {}",
                    process.name,
                    percent(process.cpu_percent),
                    process.pid
                ),
            ));
            if let Some(path) = process.path.as_deref() {
                evidence_items.push(evidence("Executable", path.to_string()));
            }
            let mut recommendation_action = action("page", "Open Task Manager");
            recommendation_action.page = Some("task-manager".into());
            recommendations.push(recommendation(
                "cpu-top-process",
                "cpu",
                "attention",
                format!("{} is using most of the measured CPU", process.name),
                format!(
                    "Cyron sampled {}% total CPU and {} accounted for {}% of the machine.",
                    percent(cpu.utilization_percent),
                    process.name,
                    percent(process.cpu_percent)
                ),
                evidence_items,
                "Closing or updating this app can release the processor time it is using. Other processes may still keep the PC busy, so this is not a guarantee of faster performance.",
                "Closing an application can lose unsaved work. Cyron does not terminate it from this page.",
                "Return to Performance after the app is idle or closed and compare total CPU and this process.",
                recommendation_action,
            ));
        } else {
            let mut recommendation_action = action("trace", "Record a 60-second trace");
            recommendation_action.page = Some("trace".into());
            recommendations.push(recommendation(
                "cpu-many-processes",
                "cpu",
                "review",
                "CPU is busy across several processes".into(),
                format!(
                    "No single application process accounted for the {}% total CPU in this sample.",
                    percent(cpu.utilization_percent)
                ),
                evidence_items,
                "A short Windows Performance Recorder trace can show where the processor time is going. The trace adds some overhead while it runs.",
                "Tracing requires administrator rights, uses memory for buffers, and records process names and file paths locally.",
                "Compare the total CPU before, during and after the slowdown. A trace explains the sample it captured, not every future slowdown.",
                recommendation_action,
            ));
        }
    }

    let available_ratio = if memory.total_bytes > 0 {
        memory.available_bytes as f64 / memory.total_bytes as f64
    } else {
        1.0
    };
    if available_ratio < MEMORY_AVAILABLE_WARNING || memory.commit_percent >= COMMIT_WARNING_PERCENT
    {
        let mut evidence_items = vec![
            evidence(
                "Available physical memory",
                format!(
                    "{} of {} ({})",
                    bytes(memory.available_bytes),
                    bytes(memory.total_bytes),
                    percent(available_ratio * 100.0)
                ),
            ),
            evidence(
                "Committed memory",
                format!(
                    "{} of {} ({})",
                    bytes(memory.commit_used_bytes),
                    bytes(memory.commit_limit_bytes),
                    percent(memory.commit_percent)
                ),
            ),
        ];
        if let Some(process) = top_memory {
            evidence_items.push(evidence(
                "Largest application working set",
                format!(
                    "{} · {} · PID {}",
                    process.name,
                    bytes(process.working_set_bytes),
                    process.pid
                ),
            ));
        }
        let mut recommendation_action = action("page", "Open Task Manager");
        recommendation_action.page = Some("task-manager".into());
        recommendations.push(recommendation(
            "memory-pressure",
            "memory",
            if available_ratio < 0.05 || memory.commit_percent >= 95.0 { "attention" } else { "review" },
            "Memory is under pressure".into(),
            format!(
                "Windows reports {} available and {} of the commit limit in use.",
                bytes(memory.available_bytes),
                percent(memory.commit_percent)
            ),
            evidence_items,
            "Closing unused apps may raise available memory and reduce paging. Windows also uses free memory as cache, so some memory use is normal and lower use does not automatically mean a faster PC.",
            "Closing an app can lose unsaved work. Cyron does not change the page file, clear standby memory or run a memory optimizer.",
            "Open Performance again after closing an app and compare available memory, commit charge and responsiveness.",
            recommendation_action,
        ));
    }

    for volume in volumes_list
        .iter()
        .filter(|volume| volume.total > 0 && volume.kind == "fixed")
    {
        let free_ratio = volume.free as f64 / volume.total as f64;
        if free_ratio < FREE_SPACE_WARNING {
            let mut recommendation_action = action("view", "Review storage");
            recommendation_action.view = Some("cleaner".into());
            recommendations.push(recommendation(
                &format!("disk-space-{}", volume.mount.replace([':', '\\'], "")),
                "storage",
                if volume.free < 1_000_000_000 { "attention" } else { "review" },
                format!("{} is {:.0}% free", volume.mount, free_ratio * 100.0),
                format!(
                    "{} has {} free of {}.",
                    volume.mount,
                    bytes(volume.free),
                    bytes(volume.total)
                ),
                vec![
                    evidence("Free space", format!("{} ({})", bytes(volume.free), percent(free_ratio * 100.0))),
                    evidence("Capacity", bytes(volume.total)),
                ],
                "Freeing disk space gives Windows room for updates, temporary files and the page file. Disk space is a capacity issue; reclaiming it is not itself proof of a speed improvement.",
                "Deleting the wrong files can lose data. Cyron previews every target and quarantines what it moves.",
                "Check the volume's free space after a reviewed cleanup or after removing files yourself.",
                recommendation_action,
            ));
        }
    }

    // Only physical-disk counters may justify a disk recommendation. Per-process
    // I/O also includes network, pipe and device transfers, so a high process
    // total with an idle disk is not disk pressure.
    let io_pressure = disk
        .busy_percent
        .is_some_and(|busy| busy >= DISK_BUSY_WARNING)
        && disk
            .queue_length
            .is_some_and(|queue| queue >= DISK_QUEUE_WARNING);
    if io_pressure {
        let mut evidence_items = vec![];
        if let (Some(busy), Some(queue)) = (disk.busy_percent, disk.queue_length) {
            evidence_items.push(evidence("Physical disk busy", percent(busy)));
            evidence_items.push(evidence("Average disk queue", format!("{queue:.2}")));
        }
        evidence_items.push(evidence(
            "Process I/O rate",
            format!("{}/s", bytes(total_io as u64)),
        ));
        if let Some(process) = top_io
            .filter(|process| process.io_read_bytes_per_sec + process.io_write_bytes_per_sec > 1.0)
        {
            evidence_items.push(evidence(
                "Largest process I/O",
                format!(
                    "{} · {}/s read · {}/s write · PID {}",
                    process.name,
                    bytes(process.io_read_bytes_per_sec as u64),
                    bytes(process.io_write_bytes_per_sec as u64),
                    process.pid
                ),
            ));
        }
        let mut recommendation_action = action("page", "Open Task Manager");
        recommendation_action.page = Some("task-manager".into());
        recommendations.push(recommendation(
            "disk-io-pressure",
            "disk",
            "review",
            "The physical disk is under sustained pressure".into(),
            format!(
                "The physical disk was {} busy with a queue of {:.2} during the sample.",
                disk.busy_percent.map(percent).unwrap_or_default(),
                disk.queue_length.unwrap_or_default()
            ),
            evidence_items,
            "Waiting for the busy process to finish, updating it, or moving its working files to another drive may reduce contention. The measurement alone does not prove the app is the cause.",
            "Cyron does not stop the process or change disk layout. Updating or moving an application can change its behaviour or storage location.",
            "Repeat the measurement while the slowdown is happening. If the busy time and the lag rise together, use the trace option to identify the file and stack.",
            recommendation_action,
        ));
    }

    let mut startup_recommendations = 0;
    for item in &startup.items {
        if startup_recommendations >= 3 {
            break;
        }
        let Some(degradation) = item.boot_impact_ms else {
            continue;
        };
        if !item.enabled
            || !item.can_toggle
            || item.is_protected
            || degradation < STARTUP_IMPACT_MIN_MS
        {
            continue;
        }
        let mut action = action("startup", "Disable at startup");
        action.target_id = Some(item.id.clone());
        recommendations.push(recommendation(
            &format!("startup-{}", item.id),
            "startup",
            "review",
            format!("{} added {} to the last boot measurement", item.name, duration(degradation)),
            format!(
                "Windows Diagnostics-Performance recorded this startup item taking {} in total, {} longer than its baseline.",
                duration(item.boot_total_ms.unwrap_or(0)),
                duration(degradation)
            ),
            vec![
                evidence("Startup location", item.location.clone()),
                evidence("Command", item.command.clone()),
                evidence("Measured boot cost", duration(item.boot_total_ms.unwrap_or(0))),
                evidence("Degradation over baseline", duration(degradation)),
            ],
            "Disabling the startup entry removes it from the next sign-in. It can still be opened manually. The boot time may improve only by the measured amount, and other components may dominate.",
            "The application will not start automatically. Some background features may be unavailable until you open it. The change is reversible from this page or Windows Settings.",
            "Restart Windows, reopen Performance, and compare the newest boot summary and this item's next boot record.",
            action,
        ));
        startup_recommendations += 1;
    }

    recommendations
}

fn build_findings(
    cpu: &CpuMetrics,
    memory: &MemoryMetrics,
    disk: &DiskIoMetrics,
    startup: &StartupAnalysis,
    volumes_list: &[volumes::VolumeInfo],
) -> Vec<Finding> {
    let mut findings = vec![Finding {
        id: "cpu".into(),
        tone: if cpu.utilization_percent >= CPU_WARNING_PERCENT {
            "attention".into()
        } else {
            "ok".into()
        },
        title: "Processor".into(),
        detail: format!(
            "{} busy, {} user, {} kernel across {} logical processors.",
            percent(cpu.utilization_percent),
            percent(cpu.user_percent),
            percent(cpu.kernel_percent),
            cpu.logical_cores
        ),
    }];
    let available_ratio = if memory.total_bytes > 0 {
        memory.available_bytes as f64 / memory.total_bytes as f64
    } else {
        1.0
    };
    findings.push(Finding {
        id: "memory".into(),
        tone: if available_ratio < MEMORY_AVAILABLE_WARNING
            || memory.commit_percent >= COMMIT_WARNING_PERCENT
        {
            "attention".into()
        } else if available_ratio < 0.20 {
            "watch".into()
        } else {
            "ok".into()
        },
        title: "Memory".into(),
        detail: format!(
            "{} available of {}; commit {}.",
            bytes(memory.available_bytes),
            bytes(memory.total_bytes),
            percent(memory.commit_percent)
        ),
    });
    findings.push(Finding {
        id: "disk".into(),
        tone: if disk
            .busy_percent
            .is_some_and(|busy| busy >= DISK_BUSY_WARNING)
        {
            "attention".into()
        } else if disk.status == "unavailable" {
            "watch".into()
        } else {
            "ok".into()
        },
        title: "Disk I/O".into(),
        detail: match (disk.busy_percent, disk.queue_length) {
            (Some(busy), Some(queue)) => {
                format!("{} busy, average queue {:.2}.", percent(busy), queue)
            }
            _ => disk
                .note
                .clone()
                .unwrap_or_else(|| "Physical-disk counters were unavailable.".into()),
        },
    });
    if let Some(volume) = volumes_list
        .iter()
        .filter(|volume| volume.total > 0)
        .min_by(|left, right| {
            (left.free as f64 / left.total as f64)
                .total_cmp(&(right.free as f64 / right.total as f64))
        })
    {
        let ratio = volume.free as f64 / volume.total as f64;
        findings.push(Finding {
            id: "storage".into(),
            tone: if ratio < FREE_SPACE_WARNING {
                "attention".into()
            } else {
                "ok".into()
            },
            title: "Storage".into(),
            detail: format!(
                "Tightest volume is {} with {} free ({}).",
                volume.mount,
                bytes(volume.free),
                percent(ratio * 100.0)
            ),
        });
    }
    findings.push(Finding {
        id: "startup".into(),
        tone: if startup.event_log_status == "denied" { "watch".into() } else { "ok".into() },
        title: "Startup".into(),
        detail: if startup.event_log_status == "denied" {
            format!(
                "{} of {} startup entries enabled. Windows' per-app boot impact needs administrator access, so it is not shown.",
                startup.enabled_count, startup.total_count
            )
        } else {
            format!(
                "{} of {} startup entries enabled; {} boot/logon scheduled tasks found.",
                startup.enabled_count,
                startup.total_count,
                startup.scheduled_tasks.len()
            )
        },
    });
    findings
}

/* ------------------------------------------------------------------ snapshot */

pub fn capture_snapshot(force_startup_refresh: bool) -> PerformanceSnapshot {
    let elevated = mft::is_elevated();
    if force_startup_refresh {
        if let Ok(mut cache) = STARTUP_CACHE.lock() {
            *cache = None;
        }
    }
    let volumes_list = volumes::list_volumes();

    let disk_query = DiskQuery::start();
    let cpu_start = read_system_times();
    let handles = open_processes();
    let process_count = handles.len() as u32;
    let first = snapshot_processes(&handles);
    let sample_start = Instant::now();
    std::thread::sleep(Duration::from_millis(SAMPLE_WINDOW_MS));
    let second = snapshot_processes(&handles);
    let elapsed = sample_start.elapsed();
    let cpu_end = read_system_times();
    let disk_io = disk_query.map(DiskQuery::finish).unwrap_or(DiskIoMetrics {
        status: "unavailable".into(),
        busy_percent: None,
        queue_length: None,
        note: Some("Windows performance counters are not available in this process.".into()),
    });

    let (memory, memory_note) = read_memory();
    let cpu = match (cpu_start, cpu_end) {
        (Some(start), Some(end)) => cpu_delta(start, end),
        _ => CpuMetrics {
            utilization_percent: 0.0,
            user_percent: 0.0,
            kernel_percent: 0.0,
            idle_percent: 0.0,
            logical_cores: 1,
        },
    };
    let processes = process_usage(&first, &second, elapsed, cpu.logical_cores);
    let all_pids = handles.len();
    drop(handles);
    let unmeasured = enumerate_pids().len().saturating_sub(all_pids) as u32;

    let mut startup = startup_analysis();
    annotate_live_startup_usage(&mut startup, &processes);

    let recommendations =
        build_recommendations(&cpu, &memory, &disk_io, &volumes_list, &processes, &startup);
    let findings = build_findings(&cpu, &memory, &disk_io, &startup, &volumes_list);

    let mut limitations = vec![
        format!(
            "CPU, memory and per-process figures are a {} ms sample. Spikes between samples are not represented.",
            elapsed.as_millis()
        ),
        "Per-process I/O counters combine file, network, pipe and device transfers; Windows does not split them here.".into(),
        "Protected processes and processes owned by other users may not expose memory or I/O counters. Cyron shows only what Windows returned.".into(),
        "Cyron does not clear standby memory, disable services, delete Prefetch, change the page file, disable Defender or apply registry tweaks. Those are not supported performance fixes.".into(),
    ];
    if unmeasured > 0 {
        limitations.push(format!(
            "{unmeasured} processes could not be opened for measurement; they are absent from the process table."
        ));
    }
    if let Some(note) = memory_note {
        limitations.push(note);
    }
    if startup.event_log_status != "available" {
        limitations.push(startup.event_log_note.clone());
    }
    limitations.extend(disk_io.note.clone());

    PerformanceSnapshot {
        captured_at: now_secs(),
        sample_ms: elapsed.as_millis() as u64,
        elevated,
        cpu,
        memory,
        disk_io,
        volumes: volumes_list,
        processes,
        process_count,
        unmeasured_process_count: unmeasured,
        startup,
        recommendations,
        findings,
        limitations,
        trace: TraceStatus::default(),
    }
}

/* ------------------------------------------------------------------ baselines */

fn baseline_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    use tauri::Manager;
    app.path()
        .app_data_dir()
        .ok()
        .map(|dir| dir.join(BASELINE_FILE))
}

fn load_baselines_from(path: &Path) -> Vec<PerformanceBaseline> {
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(mut baselines) = serde_json::from_str::<Vec<PerformanceBaseline>>(&raw) else {
        return Vec::new();
    };
    baselines.sort_by(|left, right| right.recorded_at.cmp(&left.recorded_at));
    baselines.truncate(BASELINE_LIMIT);
    baselines
}

pub fn list_baselines(app: &tauri::AppHandle) -> Vec<PerformanceBaseline> {
    baseline_path(app)
        .map(|path| load_baselines_from(&path))
        .unwrap_or_default()
}

fn save_baselines(path: &Path, baselines: &[PerformanceBaseline]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)
            .map_err(|error| format!("Could not create the baseline folder: {error}"))?;
    }
    let raw = serde_json::to_string_pretty(baselines)
        .map_err(|error| format!("Could not encode performance baselines: {error}"))?;
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, &raw)
        .map_err(|error| format!("Could not write the baseline file: {error}"))?;
    if fs::rename(&temp, path).is_err() {
        // A reader may briefly hold the destination. Publish directly as a
        // last resort, but surface a real write failure instead of reporting a
        // measurement that was never saved.
        fs::write(path, &raw)
            .map_err(|error| format!("Could not update the baseline file: {error}"))?;
        let _ = fs::remove_file(&temp);
    }
    Ok(())
}

pub fn record_baseline(
    app: &tauri::AppHandle,
    label: &str,
    snapshot: &PerformanceSnapshot,
) -> Result<PerformanceBaseline, String> {
    let label = label.trim();
    if label.len() > 80 {
        return Err("Baseline labels are limited to 80 characters.".into());
    }
    let Some(path) = baseline_path(app) else {
        return Err("Cyron could not resolve its application data folder.".into());
    };
    let mut baselines = load_baselines_from(&path);
    let top_cpu = snapshot
        .processes
        .iter()
        .filter(|process| !process.is_cyron)
        .max_by(|left, right| left.cpu_percent.total_cmp(&right.cpu_percent));
    let top_memory = snapshot
        .processes
        .iter()
        .filter(|process| !process.is_cyron)
        .max_by_key(|process| process.working_set_bytes);
    let recorded_at = now_secs();
    let baseline = PerformanceBaseline {
        id: format!("perf-baseline-{recorded_at}-{}", std::process::id()),
        label: if label.is_empty() {
            "Performance measurement".into()
        } else {
            label.to_string()
        },
        recorded_at,
        cpu_percent: snapshot.cpu.utilization_percent,
        memory_used_percent: snapshot.memory.used_percent,
        available_memory_bytes: snapshot.memory.available_bytes,
        commit_percent: snapshot.memory.commit_percent,
        disk_busy_percent: snapshot.disk_io.busy_percent,
        disk_queue_length: snapshot.disk_io.queue_length,
        top_cpu_process: top_cpu
            .map(|process| format!("{} ({:.1}%)", process.name, process.cpu_percent)),
        top_memory_process: top_memory
            .map(|process| format!("{} ({})", process.name, bytes(process.working_set_bytes))),
        boot_time_ms: snapshot.startup.boot.as_ref().map(|boot| boot.boot_time_ms),
        enabled_startup_count: snapshot.startup.enabled_count,
    };
    baselines.insert(0, baseline.clone());
    baselines.truncate(BASELINE_LIMIT);
    save_baselines(&path, &baselines)?;
    Ok(baseline)
}

/* --------------------------------------------------------------------- traces */

/// Resolve a fixed Windows binary under `%SystemRoot%` without searching PATH.
/// Absolute paths and parent traversal are rejected so a caller cannot turn a
/// fixed diagnostic action into an arbitrary executable launch.
fn windows_binary(relative: &str) -> Option<PathBuf> {
    if relative.is_empty() || Path::new(relative).is_absolute() || relative.contains("..") {
        return None;
    }
    let root = std::env::var("SystemRoot").ok()?;
    let path = Path::new(&root).join(relative);
    path.is_file().then_some(path)
}

fn windows_command(relative: &str) -> Result<std::process::Command, String> {
    let path = windows_binary(relative)
        .ok_or_else(|| format!("Windows did not expose a valid {relative} under SystemRoot."))?;
    let mut command = std::process::Command::new(path);
    command.creation_flags(TRACE_CREATE_NO_WINDOW);
    Ok(command)
}

pub fn trace_status(active: &Option<TraceSession>) -> TraceStatus {
    let wpr = windows_binary("System32\\wpr.exe");
    TraceStatus {
        available: mft::is_elevated() && wpr.is_some(),
        active: active.is_some(),
        started_at: active.as_ref().map(|session| session.started_at),
        output_path: active
            .as_ref()
            .map(|session| session.output_path.to_string_lossy().to_string()),
        message: if !mft::is_elevated() {
            "Windows Performance Recorder needs administrator rights. Reopen Cyron as administrator to record a trace.".into()
        } else if wpr.is_none() {
            "wpr.exe was not found in System32, so advanced tracing is unavailable on this Windows installation.".into()
        } else if let Some(session) = active {
            format!(
                "A trace is recording. Stop it to write {}.",
                session.output_path.display()
            )
        } else {
            "A short trace can explain CPU, disk and wait behaviour that a one-second sample cannot. It records process names and file paths locally.".into()
        },
    }
}

pub fn start_trace(app: &tauri::AppHandle) -> Result<TraceSession, String> {
    if !mft::is_elevated() {
        return Err(
            "Windows Performance Recorder requires administrator rights. Close Cyron, start it with Run as administrator, and try again."
                .into(),
        );
    }
    let output_dir = {
        use tauri::Manager;
        app.path()
            .app_data_dir()
            .map_err(|error| format!("Could not resolve Cyron's data folder: {error}"))?
            .join("performance-traces")
    };
    fs::create_dir_all(&output_dir)
        .map_err(|error| format!("Could not create the trace folder: {error}"))?;
    let started_at = now_secs();
    let output_path = output_dir.join(format!("cyron-performance-{started_at}.etl"));

    let output = windows_command("System32\\wpr.exe")?
        .args(["-start", "GeneralProfile"])
        .output()
        .map_err(|error| format!("Windows Performance Recorder could not start: {error}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!(
            "Windows Performance Recorder refused to start{}. Another trace may already be running.",
            if detail.is_empty() {
                String::new()
            } else {
                format!(": {detail}")
            }
        ));
    }
    Ok(TraceSession {
        started_at,
        output_path,
    })
}

pub fn stop_trace(session: &TraceSession) -> Result<PathBuf, String> {
    let output = windows_command("System32\\wpr.exe")?
        .arg("-stop")
        .arg(&session.output_path)
        .arg("-skipPdbGen")
        .output()
        .map_err(|error| format!("Windows Performance Recorder could not stop: {error}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!(
            "Windows Performance Recorder did not finish the trace. It may already be stopped; retry after checking for an existing trace file. {}",
            detail
        ));
    }
    if !session.output_path.is_file() {
        return Err(format!(
            "The trace command finished but no file appeared at {}.",
            session.output_path.display()
        ));
    }
    Ok(session.output_path.clone())
}

/* ------------------------------------------------------------ supported tools */

pub fn open_supported_tool(page: &str) -> Result<(), String> {
    let (binary, argument) = match page {
        "startup-apps" => ("explorer.exe", Some("ms-settings:startupapps")),
        "storage" => ("explorer.exe", Some("ms-settings:storagesense")),
        "task-manager" => ("System32\\taskmgr.exe", None),
        "resource-monitor" => ("System32\\resmon.exe", None),
        _ => return Err("That Windows page is not supported by Cyron.".into()),
    };
    let mut command = windows_command(binary)?;
    if let Some(argument) = argument {
        command.arg(argument);
    }
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Windows could not open {page}: {error}"))
}

/* ---------------------------------------------------------------------- tests */

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_startup(
        id: &str,
        command: &str,
        enabled: bool,
        can_toggle: bool,
    ) -> crate::system::StartupItem {
        crate::system::StartupItem {
            id: id.into(),
            name: id.into(),
            command: command.into(),
            location: "HKCU\\Run".into(),
            enabled,
            publisher: None,
            is_protected: false,
            can_toggle,
        }
    }

    #[test]
    fn system_cpu_subtracts_idle_kernel_time() {
        let start = CpuTimes {
            idle: 0,
            kernel: 0,
            user: 0,
        };
        let end = CpuTimes {
            idle: 60,
            kernel: 80,
            user: 20,
        };
        let metrics = cpu_delta(start, end);
        assert!((metrics.utilization_percent - 40.0).abs() < 0.01);
        assert!((metrics.idle_percent - 60.0).abs() < 0.01);
        assert!((metrics.kernel_percent - 20.0).abs() < 0.01);
        assert!((metrics.user_percent - 20.0).abs() < 0.01);
    }

    #[test]
    fn command_paths_keep_spaces_and_quotes() {
        assert_eq!(
            command_executable(r#""C:\Program Files\App\app.exe" --background"#).as_deref(),
            Some(r"C:\Program Files\App\app.exe")
        );
        assert_eq!(
            command_executable(r"C:\Program Files\App\app.exe --background").as_deref(),
            Some(r"C:\Program Files\App\app.exe")
        );
    }

    #[test]
    fn boot_impact_matches_only_an_exact_executable_path() {
        let boot = Some(BootSummary {
            observed_at: "2026-11-17T00:00:00Z".into(),
            boot_time_ms: 12_000,
            main_path_boot_ms: 8_000,
            post_boot_time_ms: 4_000,
            startup_app_count: 1,
            degradation_ms: 1_500,
            is_degradation: true,
        });
        let offender = BootOffender {
            event_id: 101,
            name: "app.exe".into(),
            friendly_name: "App".into(),
            path: r"C:\Program Files\App\app.exe".into(),
            total_time_ms: 3_000,
            degradation_time_ms: 1_500,
            category: "application".into(),
        };
        let analysis = build_startup_evidence(
            vec![
                sample_startup(
                    "exact",
                    r#""C:\Program Files\App\app.exe" --min"#,
                    true,
                    true,
                ),
                sample_startup("same-name", r"C:\Other\app.exe", true, true),
            ],
            &boot,
            &[offender],
            "available",
            String::new(),
        );
        let exact = analysis
            .items
            .iter()
            .find(|item| item.id == "exact")
            .unwrap();
        let same_name = analysis
            .items
            .iter()
            .find(|item| item.id == "same-name")
            .unwrap();
        assert_eq!(exact.boot_impact_ms, Some(1_500));
        assert_eq!(exact.boot_total_ms, Some(3_000));
        assert_eq!(same_name.boot_impact_ms, None);
    }

    #[test]
    fn scheduled_tasks_keep_boot_and_logon_provenance() {
        let xml = r#"
<Tasks>
<!-- \Vendor\Updater -->
<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Author>Vendor</Author><URI>\Vendor\Updater</URI></RegistrationInfo>
  <Triggers><BootTrigger><Enabled>true</Enabled><Delay>PT1M</Delay></BootTrigger></Triggers>
  <Settings><Enabled>true</Enabled></Settings>
  <Actions><Exec><Command>C:\Vendor\update.exe</Command><Arguments>--quiet</Arguments></Exec></Actions>
</Task>
<!-- \Microsoft\Windows\Disabled -->
<Task version="1.2">
  <Triggers><LogonTrigger><Enabled>false</Enabled></LogonTrigger></Triggers>
  <Settings><Enabled>true</Enabled></Settings>
  <Actions><Exec><Command>C:\Windows\system32\noop.exe</Command></Exec></Actions>
</Task>
</Tasks>"#;
        let tasks = parse_scheduled_tasks(xml);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].path, r"Vendor\Updater");
        assert!(tasks[0].triggers.contains(&"boot".to_string()));
        assert_eq!(tasks[0].delay.as_deref(), Some("PT1M"));
        assert_eq!(tasks[0].author, "Vendor");
        assert_eq!(tasks[0].arguments, "--quiet");
    }

    #[test]
    fn recommendations_cite_evidence_and_never_claim_guaranteed_speed() {
        let cpu = CpuMetrics {
            utilization_percent: 91.0,
            user_percent: 70.0,
            kernel_percent: 21.0,
            idle_percent: 9.0,
            logical_cores: 8,
        };
        let memory = MemoryMetrics {
            total_bytes: 16 * 1024 * 1024 * 1024,
            available_bytes: 512 * 1024 * 1024,
            used_percent: 96.9,
            commit_used_bytes: 15 * 1024 * 1024 * 1024,
            commit_limit_bytes: 16 * 1024 * 1024 * 1024,
            commit_percent: 93.75,
        };
        let disk = DiskIoMetrics {
            status: "measured".into(),
            busy_percent: Some(90.0),
            queue_length: Some(4.0),
            note: None,
        };
        let startup = StartupAnalysis {
            items: Vec::new(),
            scheduled_tasks: Vec::new(),
            boot: None,
            offenders: Vec::new(),
            event_log_status: "denied".into(),
            event_log_note: String::new(),
            enabled_count: 0,
            total_count: 0,
        };
        let processes = vec![ProcessUsage {
            pid: 1234,
            name: "editor.exe".into(),
            path: Some(r"C:\Program Files\Editor\editor.exe".into()),
            cpu_percent: 75.0,
            working_set_bytes: 2 * 1024 * 1024 * 1024,
            private_bytes: 1024 * 1024 * 1024,
            io_read_bytes_per_sec: 30.0 * 1024.0 * 1024.0,
            io_write_bytes_per_sec: 2.0 * 1024.0 * 1024.0,
            is_critical: false,
            is_cyron: false,
        }];
        let recommendations =
            build_recommendations(&cpu, &memory, &disk, &[], &processes, &startup);
        assert!(recommendations.iter().any(|item| item.category == "cpu"));
        assert!(recommendations.iter().any(|item| item.category == "memory"));
        assert!(recommendations.iter().any(|item| item.category == "disk"));
        for item in &recommendations {
            assert!(!item.evidence.is_empty(), "{} has no evidence", item.id);
            assert!(!item.expected_effect.is_empty());
            assert!(!item.risk.is_empty());
            assert!(!item.verification.is_empty());
            assert!(!item.summary.to_ascii_lowercase().contains("guaranteed"));
        }
    }

    #[test]
    fn windows_binary_accepts_only_fixed_relative_names() {
        assert!(windows_binary("System32\\wpr.exe").is_some());
        assert!(windows_binary("..\\evil.exe").is_none());
        assert!(windows_binary(r"C:\Windows\System32\wpr.exe").is_none());
        assert!(windows_command("..\\evil.exe").is_err());
    }

    #[test]
    fn high_process_io_with_an_idle_physical_disk_is_not_disk_pressure() {
        let cpu = CpuMetrics {
            utilization_percent: 5.0,
            user_percent: 3.0,
            kernel_percent: 2.0,
            idle_percent: 95.0,
            logical_cores: 8,
        };
        let memory = MemoryMetrics {
            total_bytes: 16 * 1024 * 1024 * 1024,
            available_bytes: 8 * 1024 * 1024 * 1024,
            used_percent: 50.0,
            commit_used_bytes: 4 * 1024 * 1024 * 1024,
            commit_limit_bytes: 16 * 1024 * 1024 * 1024,
            commit_percent: 25.0,
        };
        let disk = DiskIoMetrics {
            status: "measured".into(),
            busy_percent: Some(0.1),
            queue_length: Some(0.0),
            note: None,
        };
        let startup = StartupAnalysis {
            items: Vec::new(),
            scheduled_tasks: Vec::new(),
            boot: None,
            offenders: Vec::new(),
            event_log_status: "available".into(),
            event_log_note: String::new(),
            enabled_count: 0,
            total_count: 0,
        };
        let processes = vec![ProcessUsage {
            pid: 99,
            name: "chat.exe".into(),
            path: Some(r"C:\Apps\chat.exe".into()),
            cpu_percent: 0.2,
            working_set_bytes: 200 * 1024 * 1024,
            private_bytes: 100 * 1024 * 1024,
            io_read_bytes_per_sec: 1.0 * 1024.0 * 1024.0,
            io_write_bytes_per_sec: 39.0 * 1024.0 * 1024.0,
            is_critical: false,
            is_cyron: false,
        }];
        let recommendations =
            build_recommendations(&cpu, &memory, &disk, &[], &processes, &startup);
        assert!(
            !recommendations.iter().any(|item| item.category == "disk"),
            "process I/O alone must not become a physical-disk recommendation: {:?}",
            recommendations
                .iter()
                .map(|item| &item.id)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn baseline_round_trip_is_bounded_and_persistent() {
        let dir = std::env::temp_dir().join(format!(
            "cyron-perf-baseline-test-{}-{}",
            std::process::id(),
            now_secs()
        ));
        fs::create_dir_all(&dir).expect("fixture directory");
        let path = dir.join(BASELINE_FILE);
        let baseline = PerformanceBaseline {
            id: "test".into(),
            label: "Before".into(),
            recorded_at: now_secs(),
            cpu_percent: 10.0,
            memory_used_percent: 20.0,
            available_memory_bytes: 1024,
            commit_percent: 30.0,
            disk_busy_percent: Some(5.0),
            disk_queue_length: Some(0.1),
            top_cpu_process: Some("test.exe (1.0%)".into()),
            top_memory_process: None,
            boot_time_ms: Some(10_000),
            enabled_startup_count: 3,
        };
        save_baselines(&path, std::slice::from_ref(&baseline)).expect("save baseline");
        let loaded = load_baselines_from(&path);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].label, "Before");
        let _ = fs::remove_dir_all(dir);
    }
}
