import { invoke } from "@tauri-apps/api/core";
import type {
  AppInventory,
  CacheInventory,
  HistoryEntry,
  Measured,
  ScanOptions,
  ScanResult,
  VolumeInfo,
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
