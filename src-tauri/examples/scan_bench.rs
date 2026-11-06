//! Read-only scanner benchmark. Prints one JSON line per run so
//! `scripts/bench-scan.mjs` can aggregate results without guessing.
//!
//! Usage:
//!   cargo run --release --example scan_bench -- --path C:\Windows --method auto --runs 3
//!
//! Nothing is written to the scanned tree. The only output is stdout/stderr.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;

use cyron_lib::mft;
use cyron_lib::scan::{self, ScanOptions, ScanProgress};
use serde_json::json;
use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows_sys::Win32::System::Threading::GetCurrentProcess;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Method {
    Directory,
    Mft,
    Auto,
}

impl Method {
    fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "directory" | "walk" => Ok(Self::Directory),
            "mft" => Ok(Self::Mft),
            "auto" => Ok(Self::Auto),
            other => Err(format!(
                "unknown --method {other:?}; use directory, mft or auto"
            )),
        }
    }

    fn requested_name(self) -> &'static str {
        match self {
            Self::Directory => "directory",
            Self::Mft => "mft",
            Self::Auto => "auto",
        }
    }
}

struct Args {
    path: PathBuf,
    method: Method,
    runs: u32,
    json_only: bool,
}

fn usage() -> &'static str {
    "scan_bench --path <absolute path> [--method directory|mft|auto] [--runs N] [--json]\n\
     \n\
     Measures the read-only directory walk and, where supported (NTFS + administrator),\n\
     the MFT scanner. Emits one JSON object per run on stdout; --json hides the human\n\
     summary on stderr."
}

fn parse_args() -> Result<Args, String> {
    let mut args = std::env::args().skip(1);
    let mut path: Option<PathBuf> = None;
    let mut method = Method::Auto;
    let mut runs: u32 = 3;
    let mut json_only = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--path" | "-p" => {
                let value = args.next().ok_or("--path needs a value")?;
                path = Some(PathBuf::from(value));
            }
            "--method" | "-m" => {
                let value = args.next().ok_or("--method needs a value")?;
                method = Method::parse(&value)?;
            }
            "--runs" | "-r" => {
                let value = args.next().ok_or("--runs needs a value")?;
                runs = value
                    .parse::<u32>()
                    .map_err(|_| format!("--runs must be a number, got {value:?}"))?;
            }
            "--json" => json_only = true,
            "--help" | "-h" => {
                println!("{}", usage());
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument {other:?}\n{}", usage())),
        }
    }

    let path = path.ok_or("--path is required")?;
    if !path.exists() {
        return Err(format!("path does not exist: {}", path.display()));
    }
    if !path.is_dir() {
        return Err(format!("path is not a directory: {}", path.display()));
    }
    Ok(Args {
        path,
        method,
        runs: runs.clamp(1, 50),
        json_only,
    })
}

/// Process peak working set in bytes. This is an upper bound for the whole
/// process, not the incremental cost of one scan.
fn peak_working_set() -> u64 {
    unsafe {
        let mut counters: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        if GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) != 0 {
            counters.PeakWorkingSetSize as u64
        } else {
            0
        }
    }
}

struct Measurement {
    method: &'static str,
    files: u64,
    dirs: u64,
    bytes: u64,
    elapsed_ms: u64,
    cancelled: bool,
    method_note: Option<String>,
}

fn measure_directory(path: &Path, job_id: &str) -> Result<Measurement, String> {
    let options = ScanOptions {
        include_hidden: true,
        collect_tree: true,
        ..ScanOptions::default()
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let no_progress = |_progress: ScanProgress| {};
    let started = Instant::now();
    let result = scan::scan_directory(job_id, path, &options, cancel, &no_progress);
    Ok(Measurement {
        method: "directory",
        files: result.file_count,
        dirs: result.dir_count,
        bytes: result.total_size,
        elapsed_ms: started.elapsed().as_millis() as u64,
        cancelled: result.cancelled,
        method_note: None,
    })
}

fn measure_mft(path: &Path, job_id: &str) -> Result<Measurement, String> {
    let options = ScanOptions {
        include_hidden: true,
        collect_tree: true,
        ..ScanOptions::default()
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let no_progress = |_progress: ScanProgress| {};
    let started = Instant::now();
    let outcome = mft::scan(job_id, path, &options, cancel, &no_progress)?;
    Ok(Measurement {
        method: "mft",
        files: outcome.file_count,
        dirs: outcome.dir_count,
        bytes: outcome.total_size,
        elapsed_ms: started.elapsed().as_millis() as u64,
        cancelled: outcome.cancelled,
        method_note: None,
    })
}

fn measure(path: &Path, method: Method, job_id: &str) -> Result<Measurement, String> {
    match method {
        Method::Directory => measure_directory(path, job_id),
        Method::Mft => measure_mft(path, job_id),
        Method::Auto => match mft::support(path) {
            Ok(()) => match measure_mft(path, job_id) {
                Ok(measurement) => Ok(measurement),
                Err(error) => {
                    let mut fallback = measure_directory(path, job_id)?;
                    fallback.method_note =
                        Some(format!("MFT scan failed ({error}); measured directory walk"));
                    Ok(fallback)
                }
            },
            Err(note) => {
                let mut fallback = measure_directory(path, job_id)?;
                fallback.method_note = Some(format!("MFT unavailable ({note}); used directory walk"));
                Ok(fallback)
            }
        },
    }
}

fn main() {
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            eprintln!("scan_bench: {message}");
            std::process::exit(2);
        }
    };

    if !args.path.is_absolute() {
        eprintln!(
            "scan_bench: --path must be absolute; got {}",
            args.path.display()
        );
        std::process::exit(2);
    }

    for run in 1..=args.runs {
        let job_id = format!("bench-{}-{run}", std::process::id());
        let measured = match measure(&args.path, args.method, &job_id) {
            Ok(measurement) => measurement,
            Err(error) => {
                eprintln!("scan_bench: run {run} failed: {error}");
                std::process::exit(2);
            }
        };
        let peak = peak_working_set();
        println!(
            "{}",
            json!({
                "target": args.path.display().to_string(),
                "run": run,
                "methodRequested": args.method.requested_name(),
                "method": measured.method,
                "files": measured.files,
                "dirs": measured.dirs,
                "bytes": measured.bytes,
                "elapsedMs": measured.elapsed_ms,
                "peakWorkingSetBytes": peak,
                "cancelled": measured.cancelled,
                "methodNote": measured.method_note,
            })
        );
        if !args.json_only {
            eprintln!(
                "run {run}/{}: {} — {} files, {} dirs, {:.1} MiB in {:.2}s (peak {:.1} MiB)",
                args.runs,
                measured.method,
                measured.files,
                measured.dirs,
                measured.bytes as f64 / (1024.0 * 1024.0),
                measured.elapsed_ms as f64 / 1000.0,
                peak as f64 / (1024.0 * 1024.0),
            );
        }
    }
}
