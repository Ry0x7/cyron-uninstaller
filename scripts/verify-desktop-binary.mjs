#!/usr/bin/env node
// Confirms the compiled Tauri application binary exists and is a real build.
import { existsSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function fail(message) {
  console.error(`BINARY_FAIL: ${message}`);
  process.exit(1);
}

const candidates = [
  "src-tauri/target/debug/cyron.exe",
  "src-tauri/target/debug/Cyron.exe",
  "src-tauri/target/release/cyron.exe",
  "src-tauri/target/release/Cyron.exe",
];

const binary = candidates
  .map((relative) => path.join(root, relative))
  .find((candidate) => existsSync(candidate));

if (!binary) fail(`no Cyron.exe found. Looked in:\n  ${candidates.join("\n  ")}`);

const size = statSync(binary).size;
if (size < 1024 * 1024) fail(`${binary} is only ${size} bytes; that is not a linked application`);

console.log(`desktop binary: ${binary}`);
console.log(`size: ${(size / 1024 / 1024).toFixed(2)} MiB`);
console.log("DESKTOP_BINARY_OK");
