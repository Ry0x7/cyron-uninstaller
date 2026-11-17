#!/usr/bin/env node
// Runs the real-desktop E2E suite with disposable registry/startup/project/cache
// fixtures and restores the existing Cyron app data and WebView settings after.
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { mergeDesktopResults } from "./desktop-results.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const results = path.join(root, "test-results", "desktop");
const evidence = process.env.CYRON_E2E_EVIDENCE_DIR
  ? path.resolve(root, process.env.CYRON_E2E_EVIDENCE_DIR)
  : path.join(root, "docs", "evidence", "phase7");
const fixture = path.join(root, "src-tauri", "target", "debug", "examples", "desktop_e2e_fixture.exe");
const statePath = path.join(results, "fixture.json");
const workspace = path.join(os.tmpdir(), `cyron-desktop-e2e-${process.pid}-${Date.now()}`);
const wdio = path.join(root, "node_modules", "@wdio", "cli", "bin", "wdio.js");

function fail(message) {
  console.error(`DESKTOP_E2E_FAIL: ${message}`);
  process.exit(1);
}

function run(command, args, options = {}) {
  return spawnSync(command, args, {
    cwd: root,
    encoding: "utf8",
    windowsHide: true,
    ...options,
  });
}

function cyronPids() {
  const result = run("powershell", [
    "-NoProfile",
    "-NonInteractive",
    "-Command",
    "Get-Process cyron -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id",
  ]);
  return `${result.stdout ?? ""}`
    .split(/\s+/)
    .map((value) => value.trim())
    .filter(Boolean);
}

function git(args) {
  const result = run("git", args);
  return `${result.stdout ?? ""}`.trim();
}

function writeRevision(status) {
  const revision = {
    testedAt: new Date().toISOString(),
    head: git(["rev-parse", "HEAD"]),
    branch: git(["rev-parse", "--abbrev-ref", "HEAD"]),
    dirty: git(["status", "--porcelain"]).split("\n").filter(Boolean).length > 0,
    status,
    os: `${os.version()} (${os.release()})`,
    node: process.version,
    webView2: run("powershell", [
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      "(Get-Item 'C:\\Program Files (x86)\\Microsoft\\EdgeWebView\\Application\\*\\msedgewebview2.exe' -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 1).VersionInfo.ProductVersion",
    ]).stdout?.trim(),
    fixtures: fs.existsSync(statePath) ? JSON.parse(fs.readFileSync(statePath, "utf8")) : null,
  };
  fs.writeFileSync(path.join(results, "revision.json"), JSON.stringify(revision, null, 2));
  return revision;
}

fs.mkdirSync(results, { recursive: true });
fs.mkdirSync(evidence, { recursive: true });

const existing = cyronPids();
if (existing.length > 0) {
  fail(`Cyron is already running (PID ${existing.join(", ")}). Close it before the desktop suite.`);
}

// Build the disposable fixture binary.
const build = run(process.execPath, [
  path.join(root, "scripts", "msvc-env.mjs"),
  "cargo",
  "build",
  "--quiet",
  "--manifest-path",
  "src-tauri/Cargo.toml",
  "--example",
  "desktop_e2e_fixture",
]);
if (build.status !== 0) {
  process.stderr.write(build.stderr ?? "");
  fail("could not build desktop_e2e_fixture");
}
if (!fs.existsSync(fixture)) fail(`fixture binary missing at ${fixture}`);

let exitCode = 1;
let child;
let appData = path.join(process.env.APPDATA ?? "", "dev.cyron.uninstaller");
let appDataBackup = "";
let appDataExisted = false;

try {
  fs.rmSync(statePath, { force: true });
  for (const name of fs.readdirSync(results)) {
    if (/^wdio-\d+-\d+-json-reporter\.json$/.test(name)) {
      fs.rmSync(path.join(results, name), { force: true });
    }
  }
  fs.rmSync(path.join(results, "results.json"), { force: true });
  // WebView2 does not start reliably under a redirected user profile, so the
  // suite runs in the real profile and isolates itself with disposable HKCU
  // entries, temp workspaces and a backed-up Cyron app-data directory.
  const suiteEnv = { ...process.env };
  appDataBackup = path.join(results, `appdata-backup-${process.pid}`);
  appDataExisted = fs.existsSync(appData);
  if (appDataExisted) fs.cpSync(appData, appDataBackup, { recursive: true });
  fs.mkdirSync(appData, { recursive: true });
  const setup = run(fixture, ["setup", workspace, statePath], { env: suiteEnv });
  if (setup.status !== 0 || !fs.existsSync(statePath)) {
    process.stderr.write(setup.stderr ?? "");
    throw new Error("fixture setup failed");
  }

  const log = fs.createWriteStream(path.join(results, "run.log"));
  await new Promise((resolve, reject) => {
    child = spawn(process.execPath, [wdio, "run", "wdio.conf.mjs", ...process.argv.slice(2)], {
      cwd: root,
      env: {
        ...suiteEnv,
        CYRON_E2E_STATE: statePath,
        CYRON_E2E_RESULTS: results,
        CYRON_E2E_EVIDENCE: evidence,
      },
      windowsHide: true,
      stdio: ["ignore", "pipe", "pipe"],
    });
    child.stdout.on("data", (chunk) => {
      process.stdout.write(chunk);
      log.write(chunk);
    });
    child.stderr.on("data", (chunk) => {
      process.stderr.write(chunk);
      log.write(chunk);
    });
    child.on("error", reject);
    child.on("exit", (code) => {
      exitCode = code ?? 1;
      resolve();
    });
  });
  await new Promise((resolve) => log.end(resolve));
} catch (error) {
  exitCode = 1;
  console.error(`DESKTOP_E2E_FAIL: ${error instanceof Error ? error.message : error}`);
} finally {
  const pids = cyronPids();
  for (const pid of pids) {
    run("taskkill", ["/PID", pid, "/T", "/F"]);
  }
  if (cyronPids().length > 0) {
    console.error("DESKTOP_E2E_FAIL: Cyron stayed running after the suite; app data was not restored.");
  } else {
    if (fs.existsSync(statePath)) {
      const teardown = run(fixture, ["teardown", statePath]);
      if (teardown.status !== 0) process.stderr.write(teardown.stderr ?? "");
    }
    fs.rmSync(appData, { recursive: true, force: true });
    if (appDataExisted) {
      fs.cpSync(appDataBackup, appData, { recursive: true });
    }
    if (appDataBackup) fs.rmSync(appDataBackup, { recursive: true, force: true });
  }
  const revision = writeRevision(exitCode === 0 ? "passed" : "failed");
  try {
    const merged = mergeDesktopResults(results);
    if (merged.summary.tests === 0 || merged.summary.failed > 0) {
      console.error(
        `DESKTOP_E2E_FAIL: ${merged.summary.failed} failed, ${merged.summary.tests} total in results.json`,
      );
      exitCode = 1;
    }
  } catch (error) {
    console.error(
      `DESKTOP_E2E_FAIL: could not merge JSON results: ${error instanceof Error ? error.message : error}`,
    );
    exitCode = 1;
  }
  for (const name of ["results.json", "revision.json"]) {
    const source = path.join(results, name);
    if (fs.existsSync(source)) fs.copyFileSync(source, path.join(evidence, name));
  }
  console.log(`DESKTOP_E2E_REVISION ${revision.head}${revision.dirty ? "-dirty" : ""}`);
}

if (exitCode !== 0) fail(`WDIO exited with ${exitCode}; see test-results/desktop/run.log`);
console.log("DESKTOP_E2E_OK");
