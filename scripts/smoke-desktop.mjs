#!/usr/bin/env node
// Launches the compiled Cyron desktop app, waits for its real window to appear,
// then terminates exactly that process tree. No image-name killing.
import { spawn, spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const TIMEOUT_MS = 25_000;
const POLL_MS = 750;

function fail(message) {
  console.error(`DESKTOP_SMOKE_FAIL: ${message}`);
  process.exit(1);
}

if (process.platform !== "win32") {
  fail("this smoke test drives a Windows desktop binary and only runs on Windows");
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
if (!binary) fail(`no desktop binary found. Looked in:\n  ${candidates.join("\n  ")}`);

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

function mainWindowTitle(pid) {
  const script = `
$p = Get-Process -Id ${pid} -ErrorAction SilentlyContinue
if ($p -and $p.MainWindowTitle) {
  Write-Output $p.MainWindowTitle
  exit
}
try {
  $def = @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public class WinSmokeHelper {
    public delegate bool EnumProc(IntPtr hWnd, IntPtr lParam);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc lpEnumFunc, IntPtr lParam);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint lpdwProcessId);
    [DllImport("user32.dll", CharSet = CharSet.Auto)] public static extern int GetWindowText(IntPtr hWnd, StringBuilder lpString, int nMaxCount);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
    public static string FindTitle(uint targetPid) {
        string found = "";
        EnumWindows((hWnd, lParam) => {
            uint pid;
            GetWindowThreadProcessId(hWnd, out pid);
            if (pid == targetPid && IsWindowVisible(hWnd)) {
                StringBuilder sb = new StringBuilder(256);
                GetWindowText(hWnd, sb, 256);
                string t = sb.ToString();
                if (!string.IsNullOrEmpty(t)) {
                    found = t;
                    if (t.IndexOf("cyron", StringComparison.OrdinalIgnoreCase) >= 0) {
                        return false;
                    }
                }
            }
            return true;
        }, IntPtr.Zero);
        return found;
    }
}
'@
  Add-Type -TypeDefinition $def -ErrorAction SilentlyContinue
  [WinSmokeHelper]::FindTitle(${pid})
} catch {}
`;
  const result = spawnSync(
    "powershell",
    ["-NoProfile", "-NonInteractive", "-Command", script],
    { encoding: "utf8", timeout: 10_000, windowsHide: true },
  );
  if (result.error) return "";
  return (result.stdout ?? "").trim();
}

function processGone(pid) {
  const result = spawnSync(
    "powershell",
    [
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      `if (Get-Process -Id ${pid} -ErrorAction SilentlyContinue) { "alive" } else { "gone" }`,
    ],
    { encoding: "utf8", timeout: 10_000, windowsHide: true },
  );
  return (result.stdout ?? "").trim() === "gone";
}

let child;
let title = "";
let launched = false;

try {
  child = spawn(binary, [], {
    detached: true,
    stdio: "ignore",
    windowsHide: false,
  });
  child.on("error", (error) => fail(`could not launch ${binary}: ${error.message}`));
  launched = true;

  const pid = child.pid;
  if (!pid) fail("the launched process has no pid");
  console.log(`launched ${path.basename(binary)} (pid ${pid})`);

  const deadline = Date.now() + TIMEOUT_MS;
  while (Date.now() < deadline) {
    await sleep(POLL_MS);
    if (child.exitCode !== null) {
      fail(`the app exited during startup with code ${child.exitCode}`);
    }
    title = mainWindowTitle(pid);
    if (/cyron/i.test(title)) break;
  }

  if (!/cyron/i.test(title)) {
    fail(
      `no Cyron window appeared within ${TIMEOUT_MS / 1000}s (last title: ${JSON.stringify(title)})`,
    );
  }
  console.log(`window ready: ${JSON.stringify(title)}`);
} finally {
  if (launched && child?.pid) {
    const pid = child.pid;
    spawnSync("taskkill", ["/PID", String(pid), "/T", "/F"], {
      stdio: "ignore",
      windowsHide: true,
    });
    try {
      child.kill();
    } catch {
      // Already gone; taskkill is the authoritative cleanup.
    }
    for (let attempt = 0; attempt < 12; attempt += 1) {
      if (processGone(pid)) break;
      await sleep(250);
    }
    if (!processGone(pid)) fail(`process ${pid} is still running after taskkill`);
  }
}

console.log("DESKTOP_SMOKE_OK");
