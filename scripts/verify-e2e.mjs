#!/usr/bin/env node
// End-to-end RC1 uninstaller, process termination, leftover cleaning,
// and registry multi-type backup/restore verification against isolated %TEMP% and HKCU fixtures.
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const msvc = path.join(root, "scripts", "msvc-env.mjs");
const marker = "RC1_E2E_FIXTURE_OK";

// First build both test fixtures so disposable_gui_target.exe is available alongside rc1_e2e_fixture
console.log("verify-e2e: building disposable_gui_target and rc1_e2e_fixture...");
const buildRes = spawnSync(process.execPath, [
  msvc,
  "cargo",
  "build",
  "--quiet",
  "--manifest-path",
  "src-tauri/Cargo.toml",
  "--example",
  "disposable_gui_target",
  "--example",
  "rc1_e2e_fixture",
], {
  cwd: root,
  encoding: "utf8",
  timeout: 300_000,
  windowsHide: true,
});
if (buildRes.status !== 0) {
  if (buildRes.stderr) process.stderr.write(buildRes.stderr);
  console.error(`verify-e2e: could not build test fixtures`);
  process.exit(1);
}

const args = [
  msvc,
  "cargo",
  "run",
  "--quiet",
  "--manifest-path",
  "src-tauri/Cargo.toml",
  "--example",
  "rc1_e2e_fixture",
];

console.log(`verify-e2e: ${process.execPath} ${args.join(" ")}`);
const result = spawnSync(process.execPath, args, {
  cwd: root,
  encoding: "utf8",
  timeout: 600_000,
  maxBuffer: 32 * 1024 * 1024,
  windowsHide: true,
});

if (result.stdout) process.stdout.write(result.stdout);
if (result.stderr) process.stderr.write(result.stderr);

if (result.error) {
  console.error(`verify-e2e: could not start the fixture binary: ${result.error.message}`);
  process.exit(1);
}
if (result.status !== 0) {
  console.error(`verify-e2e: fixture binary exited with ${result.status}`);
  process.exit(1);
}
if (!`${result.stdout}`.includes(marker)) {
  console.error(`verify-e2e: ${marker} was not printed`);
  process.exit(1);
}

console.log(`verify-e2e: ${marker}`);
