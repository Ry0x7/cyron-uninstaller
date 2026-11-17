#!/usr/bin/env node
// Reproducible Cyron scanner benchmark for the release checklist.
//
// This script measures only Cyron. It does not install or run competitor
// products, so it must not write comparative performance numbers. The
// documentation-level comparison lives in docs/COMPETITOR_COMPARISON.md and
// labels every vendor claim with its source.
//
// Output: docs/BENCHMARKS.md, regenerated from the measurements in this run.
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const runs = process.env.CYRON_BENCH_RUNS ?? "2";
const targets = [
  path.join(root, "src"),
  path.join(root, "node_modules"),
  "C:\\Windows\\System32",
];

const args = [
  path.join(root, "scripts", "bench-scan.mjs"),
  ...targets.flatMap((target) => ["--path", target]),
  "--runs",
  runs,
  "--write",
  path.join(root, "docs", "BENCHMARKS.md"),
];

console.log("Cyron scanner benchmark (Cyron only; no competitor figures are produced).");
const result = spawnSync(process.execPath, args, {
  cwd: root,
  stdio: "inherit",
  windowsHide: true,
});
if (result.error) {
  console.error(`BENCHMARK_FAIL: ${result.error.message}`);
  process.exit(1);
}
if (result.status !== 0) {
  console.error(`BENCHMARK_FAIL: bench-scan exited with ${result.status}`);
  process.exit(result.status ?? 1);
}
console.log("BENCHMARK_OK");
