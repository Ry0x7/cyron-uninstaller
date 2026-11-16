#!/usr/bin/env node
/**
 * Phase 2 verification aggregator.
 *
 * Every runnable flag executes a real command and prints its success marker only
 * after the command exits 0 and the expected assertions are present. Manual or
 * unavailable live checks fail closed with UNVERIFIED instead of pretending to
 * pass.
 */
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";
import process from "node:process";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const CARGO_TIMEOUT_MS = 20 * 60 * 1000;
const NPM_TIMEOUT_MS = 10 * 60 * 1000;
const MAX_BUFFER = 64 * 1024 * 1024;

const USAGE = `Cyron Phase 2 verification

Usage: node scripts/verify-phase2.mjs <flag> [flag...]

Runnable:
  --discovery    discovery + apps/store inventory tests
  --store        Store removal plan/refusal tests
  --history      persistent app history tests
  --remnants     historical remnant ownership tests
  --quarantine   cross-volume leftover quarantine tests
  --completion   vendor completion/reconciliation tests
  --safety       security boundary suite
  --frontend     typecheck, Vitest, production build
  --regression   full Rust suite and docs:check
  --fixture      disposable Win32 install/uninstall example

Unavailable on this machine (fails closed):
  --live-store   requires a disposable registered MSIX package
  --live-msi     requires a disposable MSI product
  --manual-ui    record desktop evidence in the gate ledger instead
`;

function npmCommand() {
  return process.platform === "win32" ? "npm.cmd" : "npm";
}

function run(command, args, timeout) {
  const result = spawnSync(command, args, {
    cwd: ROOT,
    env: process.env,
    encoding: "utf8",
    timeout,
    maxBuffer: MAX_BUFFER,
    windowsHide: true,
    shell: false,
  });
  const stdout = result.stdout ?? "";
  const stderr = result.stderr ?? "";
  const output = `${stdout}${stderr ? `\n${stderr}` : ""}`;
  return {
    status: result.status,
    signal: result.signal,
    error: result.error,
    output,
    timedOut: result.error?.code === "ETIMEDOUT" || result.signal === "SIGTERM",
  };
}

function tail(output, maxChars = 5000) {
  if (output.length <= maxChars) return output;
  return `... (truncated)\n${output.slice(-maxChars)}`;
}

function reportFailure(label, result) {
  console.error(`\n[FAIL] ${label}`);
  if (result.error) console.error(`spawn error: ${result.error.message}`);
  if (result.timedOut) console.error("command timed out");
  else if (result.status !== 0) console.error(`exit status: ${result.status}`);
  if (result.output.trim()) console.error(tail(result.output));
}

/** Sum passing tests from every "test result: ok. N passed" line. */
function passedTests(output) {
  let total = 0;
  for (const match of output.matchAll(/test result: ok\.\s+(\d+) passed/g)) {
    total += Number(match[1]);
  }
  return total;
}

function cargoTest(args, timeout = CARGO_TIMEOUT_MS) {
  return run(
    process.execPath,
    ["scripts/msvc-env.mjs", "cargo", "test", "--manifest-path", "src-tauri/Cargo.toml", ...args],
    timeout,
  );
}

function runFilteredTests(label, filter, { requireMatch = true } = {}) {
  const result = cargoTest(["--lib", filter]);
  if (!result.timedOut && result.status === 0) {
    const passed = passedTests(result.output);
    if (!requireMatch || passed > 0) return { ...result, passed };
  }
  reportFailure(label, result);
  return null;
}

function runSecuritySuite() {
  const result = cargoTest(["--test", "security"]);
  if (!result.timedOut && result.status === 0 && passedTests(result.output) > 0) {
    return result;
  }
  reportFailure("security boundary suite", result);
  return null;
}

function runFullRustSuite() {
  const result = cargoTest([]);
  if (!result.timedOut && result.status === 0 && passedTests(result.output) > 0) {
    return result;
  }
  reportFailure("full Rust suite", result);
  return null;
}

function runNpm(args, timeout = NPM_TIMEOUT_MS) {
  // npm is a .cmd shim on Windows; invoke it through cmd.exe with the shim
  // quoted so an installation path containing spaces is not split.
  if (process.platform === "win32") {
    return run(
      process.env.ComSpec || "cmd.exe",
      ["/d", "/c", `${npmCommand()} ${args.join(" ")}`],
      timeout,
    );
  }
  return run(npmCommand(), args, timeout);
}

function runNpmScript(label, script, { requireTests = false } = {}) {
  const result = runNpm(["run", script]);
  if (!result.timedOut && result.status === 0) {
    if (!requireTests) return result;
    // Vitest colorizes the summary; strip ANSI before reading counts.
    const plain = result.output.replace(/\u001b\[[0-9;]*m/g, "");
    const match = plain.match(/Tests\s+(\d+)\s+passed/);
    if (match && Number(match[1]) > 0) return result;
    reportFailure(`${label} (no passing tests found)`, result);
    return null;
  }
  reportFailure(label, result);
  return null;
}

function runDocsCheck() {
  const result = runNpm(["run", "docs:check"]);
  if (!result.timedOut && result.status === 0 && result.output.includes("DOCS_CHECK: PASSED")) {
    return result;
  }
  reportFailure("documentation integrity", result);
  return null;
}

function mark(name) {
  console.log(name);
  return true;
}

const handlers = {
  "--discovery": () => {
    const phase2 = runFilteredTests("phase 2 discovery tests", "phase2_discovery");
    if (!phase2) return false;
    const apps = runFilteredTests("application inventory tests", "apps::tests", {
      requireMatch: false,
    });
    if (!apps) return false;
    const store = runFilteredTests("Store inventory tests", "store::tests", {
      requireMatch: false,
    });
    if (!store) return false;
    return mark("PHASE2_DISCOVERY_OK");
  },

  "--store": () => {
    if (!runFilteredTests("Store removal plan tests", "phase2_store")) return false;
    return mark("PHASE2_STORE_PLAN_OK");
  },

  "--history": () => {
    if (!runFilteredTests("persistent app history tests", "phase2_history")) return false;
    return mark("PHASE2_HISTORY_OK");
  },

  "--remnants": () => {
    if (!runFilteredTests("historical remnant tests", "phase2_remnant")) return false;
    return mark("PHASE2_REMNANTS_OK");
  },

  "--quarantine": () => {
    if (!runFilteredTests("cross-volume quarantine tests", "phase2_quarantine")) return false;
    return mark("PHASE2_QUARANTINE_OK");
  },

  "--completion": () => {
    if (!runFilteredTests("vendor completion tests", "phase2_completion")) return false;
    return mark("PHASE2_COMPLETION_OK");
  },

  "--safety": () => {
    if (!runSecuritySuite()) return false;
    return mark("PHASE2_SAFETY_OK");
  },

  "--frontend": () => {
    if (!runNpmScript("TypeScript typecheck", "typecheck")) return false;
    if (!runNpmScript("frontend tests", "test", { requireTests: true })) return false;
    if (!runNpmScript("production frontend build", "build")) return false;
    return mark("PHASE2_FRONTEND_OK");
  },

  "--regression": () => {
    if (!runFullRustSuite()) return false;
    if (!runDocsCheck()) return false;
    return mark("PHASE2_REGRESSION_OK");
  },

  "--fixture": () => {
    const result = run(
      process.execPath,
      [
        "scripts/msvc-env.mjs",
        "cargo",
        "run",
        "--manifest-path",
        "src-tauri/Cargo.toml",
        "--example",
        "phase2_win32_fixture",
      ],
      CARGO_TIMEOUT_MS,
    );
    if (result.timedOut || result.status !== 0) {
      reportFailure("disposable Win32 uninstall fixture", result);
      return false;
    }
    return mark("PHASE2_FIXTURE_OK");
  },

  "--live-store": () => {
    console.error(
      "UNVERIFIED: --live-store needs a disposable registered MSIX package. " +
        "No disposable package can be registered on this machine without enabling developer mode " +
        "or changing the certificate trust store, both of which are out of scope. " +
        "Plan/refusal tests run with --store; live removal must be recorded manually on a VM.",
    );
    return false;
  },

  "--live-msi": () => {
    console.error(
      "UNVERIFIED: --live-msi needs a disposable MSI product. " +
        "WiX/NSIS are not installed on this machine, so no disposable MSI was built or removed. " +
        "Command parsing and exit-code handling run with --completion; live MSI removal must be " +
        "recorded manually on a VM.",
    );
    return false;
  },

  "--manual-ui": () => {
    console.error(
      "UNVERIFIED: --manual-ui requires an observed desktop session. " +
        "Run the app, exercise Store fallback, persistent history, historical remnant search, and " +
        "reviewed leftover cleanup, then record the exact evidence in GATES.md. " +
        "This script does not fabricate UI evidence.",
    );
    return false;
  },

  "--help": () => {
    console.log(USAGE);
    return true;
  },
};

const requested = process.argv.slice(2);
if (requested.length === 0) {
  console.error(USAGE);
  process.exit(2);
}

const unknown = requested.filter((flag) => !Object.hasOwn(handlers, flag));
if (unknown.length > 0) {
  console.error(`Unknown flag(s): ${unknown.join(", ")}\n`);
  console.error(USAGE);
  process.exit(2);
}

let failed = false;
for (const flag of requested) {
  const ok = handlers[flag]();
  if (!ok) failed = true;
}

process.exit(failed ? 1 : 0);
