import { beforeEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { api, BACKEND_OFFLINE, errorMessage, invalidateCaches, isDesktop, loadCaches } from "./api";
import { SCAN_EVENT, type CacheInventory } from "./types";

function setDesktop(enabled: boolean) {
  if (enabled) {
    Object.defineProperty(globalThis, "window", {
      configurable: true,
      value: { __TAURI_INTERNALS__: {} },
    });
  } else {
    Reflect.deleteProperty(globalThis, "window");
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const inventory = (scannedAt: number): CacheInventory => ({
  targets: [],
  totalSize: 0,
  totalSafe: 0,
  totalCaution: 0,
  totalProtected: 0,
  scannedAt,
  warnings: [],
});

beforeEach(() => {
  invoke.mockReset();
  setDesktop(true);
  invalidateCaches();
});

describe("Tauri IPC contract", () => {
  it("uses the Rust command names and camelCase payloads", async () => {
    const options = { includeHidden: true, exclusions: [], collectTree: true, maxDepth: 8 };
    const calls = [
      { run: () => api.listInstalledApps(), command: "list_installed_apps", args: undefined },
      { run: () => api.listVolumes(), command: "list_volumes", args: undefined },
      { run: () => api.listCaches(), command: "list_caches", args: undefined },
      { run: () => api.startScan("job-1", "C:\\scan", options), command: "start_scan", args: { jobId: "job-1", path: "C:\\scan", options } },
      { run: () => api.cancelScan("job-1"), command: "cancel_scan", args: { jobId: "job-1" } },
      { run: () => api.measurePath("C:\\scan"), command: "measure_path", args: { path: "C:\\scan" } },
      { run: () => api.revealPath("C:\\scan"), command: "reveal_path", args: { path: "C:\\scan" } },
      { run: () => api.getHistory(), command: "get_history", args: undefined },
      { run: () => api.clearHistory(), command: "clear_history", args: undefined },
    ];

    for (const { run, command, args } of calls) {
      invoke.mockClear();
      invoke.mockResolvedValue(undefined);
      await run();
      expect(invoke).toHaveBeenCalledExactlyOnceWith(command, args);
    }
    expect(SCAN_EVENT).toBe("cyron-scan-progress");
  });

  it("uses the cleanup command names and payloads", async () => {
    const calls = [
      { run: () => api.previewCleanup(["user-temp", "npm-cache"]), command: "preview_cleanup", args: { targetIds: ["user-temp", "npm-cache"], exclusions: [] } },
      { run: () => api.startCleanup("plan-1", ["user-temp"]), command: "start_cleanup", args: { planId: "plan-1", targetIds: ["user-temp"] } },
      { run: () => api.getCleanupMode(), command: "get_cleanup_mode", args: undefined },
      { run: () => api.setCleanupMode(true), command: "set_cleanup_mode", args: { readOnly: true } },
      { run: () => api.listQuarantine(), command: "list_quarantine", args: undefined },
      { run: () => api.restoreQuarantine("run-1"), command: "restore_quarantine", args: { runId: "run-1" } },
      { run: () => api.purgeQuarantine("run-1"), command: "purge_quarantine", args: { runId: "run-1" } },
      { run: () => api.getCleanupLog(), command: "get_cleanup_log", args: undefined },
    ];

    for (const { run, command, args } of calls) {
      invoke.mockClear();
      invoke.mockResolvedValue(undefined);
      await run();
      expect(invoke).toHaveBeenCalledExactlyOnceWith(command, args);
    }
  });

  it("uses the Phase 4 uninstall and leftover commands and payloads", async () => {
    const calls = [
      { run: () => api.discoverAppProcesses("app-1"), command: "discover_app_processes", args: { appId: "app-1" } },
      { run: () => api.terminateAppProcesses("app-1", [1234, 5678], false), command: "terminate_app_processes", args: { appId: "app-1", pids: [1234, 5678], force: false } },
      { run: () => api.parseUninstallCommand("C:\\Program Files\\App\\uninstall.exe /S"), command: "parse_uninstall_command", args: { rawCommand: "C:\\Program Files\\App\\uninstall.exe /S" } },
      { run: () => api.prepareUninstall("app-1", false), command: "prepare_uninstall", args: { appId: "app-1", quiet: false } },
      { run: () => api.executeUninstall("app-1", "plan-1", 60), command: "execute_uninstall", args: { appId: "app-1", planId: "plan-1", timeoutSecs: 60 } },
      { run: () => api.scanAppLeftovers("app-1"), command: "scan_app_leftovers", args: { appId: "app-1" } },
      { run: () => api.cleanAppLeftovers("app-1", "plan-1", ["c1"]), command: "clean_app_leftovers", args: { appId: "app-1", planId: "plan-1", candidateIds: ["c1"] } },
      { run: () => api.executeBatchUninstall(["app-1", "app-2"]), command: "execute_batch_uninstall", args: { appIds: ["app-1", "app-2"], quiet: false } },
    ];

    for (const { run, command, args } of calls) {
      invoke.mockClear();
      invoke.mockResolvedValue(undefined);
      await run();
      expect(invoke).toHaveBeenCalledExactlyOnceWith(command, args);
    }
  });

  it("sends Settings exclusions with a preview so the backend can enforce them", async () => {
    invoke.mockResolvedValue(undefined);
    await api.previewCleanup(["user-temp"], ["C:\\keep"]);
    expect(invoke).toHaveBeenCalledExactlyOnceWith("preview_cleanup", {
      targetIds: ["user-temp"],
      exclusions: ["C:\\keep"],
    });
  });

  it("surfaces the backend refusal message for a cleanup execution", async () => {
    invoke.mockRejectedValue("Read-only mode is on. Enable cleaning in Settings first.");
    await expect(api.startCleanup("plan-1", ["user-temp"])).rejects.toMatch(/read-only/i);
  });

  it("rejects calls outside Tauri with the actionable offline message", async () => {
    setDesktop(false);
    expect(isDesktop()).toBe(false);
    await expect(api.listVolumes()).rejects.toThrow(BACKEND_OFFLINE);
    expect(invoke).not.toHaveBeenCalled();
  });
});

describe("cache request sharing", () => {
  it("shares concurrent requests and reuses the settled result", async () => {
    const request = deferred<CacheInventory>();
    invoke.mockReturnValue(request.promise);
    const first = loadCaches();
    const second = loadCaches();
    expect(invoke).toHaveBeenCalledTimes(1);
    request.resolve(inventory(10));
    await expect(first).resolves.toEqual(inventory(10));
    await expect(second).resolves.toEqual(inventory(10));
    await expect(loadCaches()).resolves.toEqual(inventory(10));
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it("forces a fresh request without allowing an older rejection to evict it", async () => {
    const older = deferred<CacheInventory>();
    const fresh = deferred<CacheInventory>();
    invoke.mockReturnValueOnce(older.promise).mockReturnValueOnce(fresh.promise);
    const oldCall = loadCaches();
    const freshCall = loadCaches(true);
    expect(invoke).toHaveBeenCalledTimes(2);
    older.reject(new Error("old failure"));
    await expect(oldCall).rejects.toThrow("old failure");
    fresh.resolve(inventory(20));
    await expect(freshCall).resolves.toEqual(inventory(20));
    await expect(loadCaches()).resolves.toEqual(inventory(20));
    expect(invoke).toHaveBeenCalledTimes(2);
  });

  it("retries after failure and invalidation leaves existing callers intact", async () => {
    const failed = deferred<CacheInventory>();
    const pending = deferred<CacheInventory>();
    const fresh = deferred<CacheInventory>();
    invoke.mockReturnValueOnce(failed.promise).mockReturnValueOnce(pending.promise).mockReturnValueOnce(fresh.promise);

    const first = loadCaches();
    failed.reject({ message: "measurement unavailable" });
    await expect(first).rejects.toEqual({ message: "measurement unavailable" });
    const waiting = loadCaches();
    invalidateCaches();
    const afterInvalidation = loadCaches();
    expect(invoke).toHaveBeenCalledTimes(3);
    pending.resolve(inventory(30));
    await expect(waiting).resolves.toEqual(inventory(30));
    fresh.resolve(inventory(40));
    await expect(afterInvalidation).resolves.toEqual(inventory(40));
  });
});

describe("errorMessage", () => {
  it("normalizes strings, Errors, and structured rejection objects", () => {
    expect(errorMessage("  denied  ")).toBe("denied");
    expect(errorMessage(new Error("disk unavailable"))).toBe("disk unavailable");
    expect(errorMessage({ message: "permission denied" })).toBe("permission denied");
    expect(errorMessage({ code: 17, reason: "busy" })).toBe('{"code":17,"reason":"busy"}');
    expect(errorMessage({ message: { message: "nested" } })).toBe("nested");
    expect(errorMessage(null)).toBe("An unexpected error occurred.");
    expect(errorMessage({})).toBe("An unexpected error occurred.");
  });
});
