#!/usr/bin/env node
// Reproducible read-only scanner benchmark.
//
// Runs the `scan_bench` example for each target and method, parses its JSON
// lines, and prints a Markdown table. No filesystem writes except an optional
// --write <path> report.
import { spawnSync } from "node:child_process";
import { writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const msvc = path.join(root, "scripts", "msvc-env.mjs");
const DEFAULT_RUNS = 3;

function usage() {
  return `Usage: node scripts/bench-scan.mjs [options]

Read-only benchmark of Cyron's directory walk and (when available) the NTFS MFT
scanner. Runs are measured inside the Rust process; cold OS cache is not
controlled.

Options:
  --path <dir>        Target to benchmark. Repeatable. Defaults: src/, %TEMP%,
                      C:\\Windows\\System32
  --runs <n>          Runs per target/method (default ${DEFAULT_RUNS}, max 20)
  --method <name>     directory | mft | auto | both (default: both when
                      elevated, directory otherwise)
  --no-mft            Skip MFT even when running elevated
  --write [file]      Write a Markdown report (default docs/BENCHMARKS.md)
  --help              Show this message

The MFT scanner needs an NTFS volume and administrator rights; without them the
script only measures the directory walk and says so.`;
}

function fail(message) {
  console.error(`bench-scan: ${message}`);
  process.exit(1);
}

function isElevated() {
  if (process.platform !== "win32") return false;
  const result = spawnSync("net", ["session"], { stdio: "ignore", windowsHide: true });
  return result.status === 0;
}

function parseArgs(argv) {
  const options = {
    targets: [],
    runs: DEFAULT_RUNS,
    method: null,
    mft: true,
    write: null,
    help: false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    switch (arg) {
      case "--path": {
        const value = argv[++index];
        if (!value) fail("--path needs a directory");
        options.targets.push(path.resolve(value));
        break;
      }
      case "--runs": {
        const value = Number(argv[++index]);
        if (!Number.isInteger(value) || value < 1 || value > 20) {
          fail("--runs must be an integer between 1 and 20");
        }
        options.runs = value;
        break;
      }
      case "--method": {
        const value = argv[++index];
        if (!["directory", "mft", "auto", "both"].includes(value)) {
          fail("--method must be directory, mft, auto or both");
        }
        options.method = value;
        break;
      }
      case "--no-mft":
        options.mft = false;
        break;
      case "--write":
        options.write = argv[index + 1] && !argv[index + 1].startsWith("--")
          ? path.resolve(argv[++index])
          : path.join(root, "docs", "BENCHMARKS.md");
        break;
      case "--help":
      case "-h":
        options.help = true;
        break;
      default:
        fail(`unknown argument ${arg}\n\n${usage()}`);
    }
  }
  return options;
}

function runExample(target, method, runs) {
  const args = [
    msvc,
    "cargo",
    "run",
    "--quiet",
    "--release",
    "--manifest-path",
    "src-tauri/Cargo.toml",
    "--example",
    "scan_bench",
    "--",
    "--path",
    target,
    "--method",
    method,
    "--runs",
    String(runs),
    "--json",
  ];
  const result = spawnSync(process.execPath, args, {
    cwd: root,
    encoding: "utf8",
    timeout: 1_800_000,
    maxBuffer: 64 * 1024 * 1024,
    windowsHide: true,
  });
  if (result.error) {
    return { measurements: [], error: result.error.message };
  }
  if (result.status !== 0) {
    const detail = `${result.stderr ?? ""}`.trim().split("\n").slice(-8).join("\n");
    return {
      measurements: [],
      error: `scan_bench exited with ${result.status}${detail ? `\n${detail}` : ""}`,
    };
  }
  const measurements = [];
  for (const line of `${result.stdout ?? ""}`.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed.startsWith("{")) continue;
    try {
      const parsed = JSON.parse(trimmed);
      if (parsed && typeof parsed.elapsedMs === "number") measurements.push(parsed);
    } catch {
      // Cargo or Windows occasionally interleaves non-JSON output; ignore it.
    }
  }
  if (measurements.length !== runs) {
    return {
      measurements: [],
      error: `scan_bench emitted ${measurements.length} valid measurement(s); expected ${runs}`,
    };
  }
  return { measurements, error: null };
}

function median(values) {
  const sorted = [...values].sort((a, b) => a - b);
  if (sorted.length === 0) return 0;
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0
    ? (sorted[middle - 1] + sorted[middle]) / 2
    : sorted[middle];
}

function seconds(ms) {
  return `${(ms / 1000).toFixed(2)}s`;
}

function bytes(value) {
  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  let size = value;
  let unit = 0;
  while (size >= 1024 && unit < units.length - 1) {
    size /= 1024;
    unit += 1;
  }
  return `${size.toFixed(unit === 0 ? 0 : 1)} ${units[unit]}`;
}

function summarize(entries) {
  const groups = new Map();
  for (const entry of entries) {
    const key = `${entry.target}\u0000${entry.method}`;
    if (!groups.has(key)) {
      groups.set(key, {
        target: entry.target,
        method: entry.method,
        runs: [],
        notes: new Set(),
      });
    }
    const group = groups.get(key);
    group.runs.push(entry);
    if (entry.methodNote) group.notes.add(entry.methodNote);
  }
  return [...groups.values()]
    .map((group) => {
      const elapsed = group.runs.map((run) => run.elapsedMs);
      const last = group.runs[group.runs.length - 1];
      return {
        ...group,
        count: group.runs.length,
        median: median(elapsed),
        min: Math.min(...elapsed),
        peak: Math.max(...group.runs.map((run) => run.peakWorkingSetBytes || 0)),
        files: last.files,
        dirs: last.dirs,
        size: last.bytes,
        cancelled: group.runs.some((run) => run.cancelled),
      };
    })
    .sort((a, b) => a.target.localeCompare(b.target) || a.method.localeCompare(b.method));
}

function markdownTable(groups) {
  const lines = [
    "| Target | Method | Runs | Median | Min | Peak working set | Files | Listed size |",
    "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |",
  ];
  for (const group of groups) {
    lines.push(
      `| \`${group.target}\` | ${group.method}${group.cancelled ? " (cancelled)" : ""} | ${group.count} | ${seconds(group.median)} | ${seconds(group.min)} | ${bytes(group.peak)} | ${group.files.toLocaleString("en-US")} | ${bytes(group.size)} |`,
    );
  }
  return lines.join("\n");
}

function reportDocument(groups, failures, meta) {
  const notes = groups
    .flatMap((group) => [...group.notes])
    .filter((note, index, all) => all.indexOf(note) === index);
  return `# Cyron scanner benchmarks

Generated by \`node scripts/bench-scan.mjs --write\` on ${meta.date}.
These are measurements from one machine, not a product claim.

## Environment

- Platform: ${meta.platform}
- Node: ${meta.node}
- Elevated: ${meta.elevated ? "yes" : "no"}
- Runs per target/method: ${meta.runs}
- Command: \`node scripts/bench-scan.mjs${meta.commandArgs ? ` ${meta.commandArgs}` : ""}\`

## Results

${markdownTable(groups)}

${notes.length > 0 ? `Notes reported by the scanner:\n\n${notes.map((note) => `- ${note}`).join("\n")}\n` : ""}
${failures.length > 0 ? `## Measurements not produced\n\n${failures.map(({ target, method, error }) => `- \`${method}\` on \`${target}\`: ${error.replaceAll("\n", " ")}`).join("\n")}\n\n` : ""}## What is measured

- Wall-clock duration of the scan call only, as reported by the benchmark binary.
- Files, directories and logical bytes counted by the scanner.
- Process peak working set after each run. This is an upper bound for the whole
  process, not the incremental memory of one scan.

## What is not measured

- Cold-cache behaviour. The OS file cache is not flushed between runs, so the
  first run on a target may be cold and later runs warm. Report the median;
  do not present a warm run as cold performance.
- Disk throughput, CPU time, or per-directory latency.
- MFT results when the process is not elevated or the volume is not NTFS.

Do not compare these numbers with WizTree or any other product unless both were
measured on the same machine, same target and same method.
`;
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  if (options.help) {
    console.log(usage());
    return;
  }
  if (process.platform !== "win32") {
    fail("this benchmark targets the Windows scanners and only runs on Windows");
  }

  const elevated = isElevated();
  const targets =
    options.targets.length > 0
      ? options.targets
      : [path.join(root, "src"), os.tmpdir(), "C:\\Windows\\System32"];

  let methods;
  if (options.method === "both") methods = elevated ? ["directory", "mft"] : ["directory"];
  else if (options.method) methods = [options.method];
  else methods = elevated ? ["directory", "mft"] : ["directory"];
  if (options.method === "mft" && !elevated) {
    fail("the MFT scanner needs an elevated terminal; run this from an administrator shell or use --method directory");
  }
  if (!options.mft && options.method === "mft") {
    fail("--method mft conflicts with --no-mft; use --method directory or remove --no-mft");
  }
  if (!options.mft || !elevated) methods = methods.filter((method) => method !== "mft");
  if (!options.mft && methods.includes("auto")) methods = ["directory"];
  if (methods.length === 0) methods = ["directory"];

  console.log(`bench-scan: ${targets.length} target(s), methods: ${methods.join(", ")}, ${options.runs} run(s)`);
  if (!elevated) {
    console.log("bench-scan: not elevated — MFT scanning is unavailable, measuring the directory walk only.");
  }

  const measurements = [];
  const failures = [];
  for (const target of targets) {
    for (const method of methods) {
      console.log(`bench-scan: ${method} on ${target}`);
      const { measurements: runs, error } = runExample(target, method, options.runs);
      if (error) {
        console.error(`bench-scan: ${method} failed on ${target}\n${error}`);
        failures.push({ target, method, error });
        if (method === "mft") {
          console.error("bench-scan: continuing without MFT for this target.");
          continue;
        }
        process.exit(1);
      }
      measurements.push(...runs);
    }
  }

  if (measurements.length === 0) {
    fail("no measurements were produced; the example did not emit JSON lines");
  }

  const groups = summarize(measurements);
  console.log("");
  console.log(markdownTable(groups));

  if (options.write) {
    const document = reportDocument(groups, failures, {
      date: new Date().toISOString().slice(0, 10),
      platform: `${os.platform()} ${os.release()} (${os.arch()})`,
      node: process.version,
      elevated,
      runs: options.runs,
      commandArgs: process.argv.slice(2).join(" "),
    });
    writeFileSync(options.write, document, "utf8");
    console.log(`\nbench-scan: wrote ${options.write}`);
  }
}

main();
