#!/usr/bin/env node
// End-to-end cleanup verification against an isolated %TEMP% fixture.
//
// Runs the `cleanup_fixture` example through the MSVC shim and requires its
// success marker. It never points the engine at a real cache or user folder.
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const msvc = path.join(root, "scripts", "msvc-env.mjs");
const marker = "CLEANUP_FIXTURE_OK";

const args = [
  msvc,
  "cargo",
  "run",
  "--quiet",
  "--manifest-path",
  "src-tauri/Cargo.toml",
  "--example",
  "cleanup_fixture",
];

console.log(`verify-cleanup: ${process.execPath} ${args.join(" ")}`);
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
  console.error(`verify-cleanup: could not start the fixture binary: ${result.error.message}`);
  process.exit(1);
}
if (result.status !== 0) {
  console.error(`verify-cleanup: fixture binary exited with ${result.status}`);
  process.exit(1);
}
if (!`${result.stdout}`.includes(marker)) {
  console.error(`verify-cleanup: ${marker} was not printed`);
  process.exit(1);
}

console.log(`verify-cleanup: ${marker}`);
