#!/usr/bin/env node
// Reproducible Hunter Mode Targeting and Rust processing benchmark verification.
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const msvc = path.join(root, "scripts", "msvc-env.mjs");
const marker = "HUNTER_BENCHMARK_OK";

const args = [
  msvc,
  "cargo",
  "run",
  "--quiet",
  "--manifest-path",
  "src-tauri/Cargo.toml",
  "--example",
  "hunter_benchmark",
];

console.log(`verify-hunter-perf: ${process.execPath} ${args.join(" ")}`);
const result = spawnSync(process.execPath, args, {
  cwd: root,
  encoding: "utf8",
  timeout: 300_000,
  maxBuffer: 16 * 1024 * 1024,
  windowsHide: true,
});

if (result.stdout) process.stdout.write(result.stdout);
if (result.stderr) process.stderr.write(result.stderr);

if (result.error) {
  console.error(`verify-hunter-perf: could not start benchmark: ${result.error.message}`);
  process.exit(1);
}
if (result.status !== 0) {
  console.error(`verify-hunter-perf: benchmark exited with status ${result.status}`);
  process.exit(1);
}
if (!`${result.stdout}`.includes(marker)) {
  console.error(`verify-hunter-perf: ${marker} was not printed`);
  process.exit(1);
}

console.log(`verify-hunter-perf: ${marker}`);
