import { beforeEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { api } from "./api";

beforeEach(() => {
  invoke.mockReset().mockResolvedValue(undefined);
  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: { __TAURI_INTERNALS__: {} },
  });
});

describe("capability IPC contract", () => {
  it("prepares Analyzer actions with exact paths and operation", async () => {
    const paths = ["C:\\Users\\me\\large.bin", "D:\\Archive\\old"];
    await api.prepareAnalyzerAction(paths, "permanent");
    expect(invoke).toHaveBeenCalledExactlyOnceWith("prepare_analyzer_action", {
      paths,
      operation: "permanent",
    });
  });

  it("executes and cancels Analyzer actions by plan id only", async () => {
    await api.executeAnalyzerAction("analyzer-plan");
    expect(invoke).toHaveBeenCalledExactlyOnceWith("execute_analyzer_action", {
      planId: "analyzer-plan",
    });
    invoke.mockClear();
    await api.cancelAnalyzerAction("analyzer-plan");
    expect(invoke).toHaveBeenCalledExactlyOnceWith("cancel_analyzer_action", {
      planId: "analyzer-plan",
    });
  });

  it("lists caches with optional selected drive letters and lists removed apps", async () => {
    await api.listCaches(["C", "D"]);
    expect(invoke).toHaveBeenCalledExactlyOnceWith("list_caches", { driveLetters: ["C", "D"] });
    invoke.mockClear();
    await api.listCaches();
    expect(invoke).toHaveBeenCalledExactlyOnceWith("list_caches", undefined);
    invoke.mockClear();
    await api.listRemovedApps();
    expect(invoke).toHaveBeenCalledExactlyOnceWith("list_removed_apps", undefined);
  });

  it("cancels batch uninstall using its plan id", async () => {
    await api.cancelBatchUninstall("batch-plan");
    expect(invoke).toHaveBeenCalledExactlyOnceWith("cancel_batch_uninstall", {
      planId: "batch-plan",
    });
  });

  it("previews remnant IDs through the capped Cleaner engine with exclusions and drive scope", async () => {
    await api.previewRemnantCleanup("app-1", "leftover-plan", ["cand-0"], ["D:\\keep"], ["D"]);
    expect(invoke).toHaveBeenCalledExactlyOnceWith("preview_remnant_cleanup", {
      appId: "app-1", planId: "leftover-plan", candidateIds: ["cand-0"],
      exclusions: ["D:\\keep"], driveLetters: ["D"],
    });
  });

  it("propagates backend failures unchanged", async () => {
    const failure = new Error("operation refused");
    invoke.mockRejectedValueOnce(failure);
    await expect(api.executeAnalyzerAction("analyzer-plan")).rejects.toBe(failure);
  });
});
