import { invoke } from "@tauri-apps/api/core";
import type {
  AppInventory,
  AppProcess,
  BatchUninstallPlan,
  BatchUninstallReport,
  CacheInventory,
  CleanupLogEntry,
  CleanupPreview,
  CleanupReport,
  DuplicateOptions,
  DuplicateReport,
  HistoryEntry,
  LeftoverCleanReport,
  LeftoverScanReport,
  Measured,
  ParsedUninstallCommand,
  ProcessTerminateResult,
  PurgeReport,
  QuarantineBatch,
  RestoreReport,
  ScanOptions,
  ScanResult,
  UninstallExecutionResult,
  UninstallPlan,
  VolumeInfo,
  StartupRoute,
  ExplorerIntegrationStatus,
  CleanableInspection,
  CleanupProviderDef,
} from "./types";

export function isDesktop(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export const BACKEND_OFFLINE =
  "Cyron's desktop backend is not connected. Start it with `npm run dev:desktop`.";

function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isDesktop()) return Promise.reject(new Error(BACKEND_OFFLINE));
  return invoke<T>(command, args);
}

export function errorMessage(error: unknown): string {
  const seen = new WeakSet<object>();
  const message = (value: unknown): string | null => {
    if (typeof value === "string" && value.trim()) return value.trim();
    if (value instanceof Error && value.message.trim()) return value.message.trim();
    if (value && typeof value === "object" && "message" in value && !seen.has(value)) {
      seen.add(value);
      return message((value as { message: unknown }).message);
    }
    return null;
  };

  const extracted = message(error);
  if (extracted) return extracted;
  if (error && typeof error === "object") {
    try {
      const serialized = JSON.stringify(error);
      if (serialized && serialized !== "{}") return serialized;
    } catch {
      // Fall through for circular or otherwise non-serializable rejection data.
    }
    return "An unexpected error occurred.";
  }
  if (error === null || error === undefined) return "An unexpected error occurred.";
  return String(error);
}

export const api = {
  listInstalledApps: () => call<AppInventory>("list_installed_apps"),
  listVolumes: () => call<VolumeInfo[]>("list_volumes"),
  listCaches: () => call<CacheInventory>("list_caches"),
  startScan: (jobId: string, path: string, options: ScanOptions) =>
    call<ScanResult>("start_scan", { jobId, path, options }),
  cancelScan: (jobId: string) => call<void>("cancel_scan", { jobId }),
  measurePath: (path: string) => call<Measured>("measure_path", { path }),
  revealPath: (path: string) => call<void>("reveal_path", { path }),
  getHistory: () => call<HistoryEntry[]>("get_history"),
  clearHistory: () => call<void>("clear_history"),
  startDuplicateScan: (jobId: string, path: string, options: DuplicateOptions) =>
    call<DuplicateReport>("start_duplicate_scan", { jobId, path, options }),
  cancelDuplicateScan: (jobId: string) => call<void>("cancel_duplicate_scan", { jobId }),
  /** Exclusions are absolute folder paths; the backend refuses anything else. */
  previewCleanup: (targetIds: string[], exclusions: string[] = []) =>
    call<CleanupPreview>("preview_cleanup", { targetIds, exclusions }),
  startCleanup: (planId: string, targetIds: string[]) =>
    call<CleanupReport>("start_cleanup", { planId, targetIds }),
  /** true means the backend is in read-only mode and refuses to execute. */
  getCleanupMode: () => call<boolean>("get_cleanup_mode"),
  setCleanupMode: (readOnly: boolean) => call<boolean>("set_cleanup_mode", { readOnly }),
  listQuarantine: () => call<QuarantineBatch[]>("list_quarantine"),
  restoreQuarantine: (runId: string) =>
    call<RestoreReport>("restore_quarantine", { runId }),
  purgeQuarantine: (runId: string) => call<PurgeReport>("purge_quarantine", { runId }),
  getCleanupLog: () => call<CleanupLogEntry[]>("get_cleanup_log"),
  discoverAppProcesses: (appId: string) =>
    call<AppProcess[]>("discover_app_processes", { appId }),
  terminateAppProcesses: (appId: string, pids: number[], force: boolean) =>
    call<ProcessTerminateResult[]>("terminate_app_processes", { appId, pids, force }),
  parseUninstallCommand: (rawCommand: string) =>
    call<ParsedUninstallCommand>("parse_uninstall_command", { rawCommand }),
  prepareUninstall: (appId: string, quiet = false) =>
    call<UninstallPlan>("prepare_uninstall", { appId, quiet }),
  executeUninstall: (appId: string, planId: string, timeoutSecs?: number) =>
    call<UninstallExecutionResult>("execute_uninstall", { appId, planId, timeoutSecs }),
  scanAppLeftovers: (appId: string) =>
    call<LeftoverScanReport>("scan_app_leftovers", { appId }),
  cleanAppLeftovers: (appId: string, planId: string, candidateIds: string[]) =>
    call<LeftoverCleanReport>("clean_app_leftovers", { appId, planId, candidateIds }),
  prepareBatchUninstall: (appIds: string[], quiet = false) =>
    call<BatchUninstallPlan>("prepare_batch_uninstall", { appIds, quiet }),
  executeBatchUninstall: (planId: string) =>
    call<BatchUninstallReport>("execute_batch_uninstall", { planId }),
  getStartupRoute: () => call<StartupRoute | null>("get_startup_route"),
  getExplorerIntegrationStatus: () =>
    call<ExplorerIntegrationStatus>("get_explorer_integration_status"),
  setExplorerIntegration: (enabled: boolean) =>
    call<void>("set_explorer_integration", { enabled }),
  repairExplorerIntegration: () => call<void>("repair_explorer_integration"),
  listCleanupProviders: () => call<CleanupProviderDef[]>("list_cleanup_providers"),
  inspectCleanableDirectory: (path: string) =>
    call<CleanableInspection>("inspect_cleanable_directory", { path }),
};

let cacheRequest: Promise<CacheInventory> | null = null;

/**
 * Cache discovery walks every known tool location, so concurrent pages share a
 * single request. Pass force to measure again after files changed.
 */
export function loadCaches(force = false): Promise<CacheInventory> {
  if (force) cacheRequest = null;
  if (!cacheRequest) {
    let request: Promise<CacheInventory>;
    request = api.listCaches().catch((error: unknown) => {
      // A failed older request must not clear a newer forced refresh.
      if (cacheRequest === request) cacheRequest = null;
      throw error;
    });
    cacheRequest = request;
  }
  return cacheRequest;
}

export function invalidateCaches(): void {
  cacheRequest = null;
}

/** A cleanup run changed cache folders on disk, so measured sizes are stale. */
export function cachesChanged(): void {
  cacheRequest = null;
}
