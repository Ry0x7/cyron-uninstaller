import { getBundleType, getVersion } from "@tauri-apps/api/app";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";
import { useEffect, useState } from "react";
import { formatBytes } from "./format";

/** Display metadata for an update announced by the configured release feed. */
export interface AvailableUpdateInfo {
  currentVersion: string;
  version: string;
  date?: string;
  body?: string;
}

export interface UpdateProgress {
  downloaded: number;
  total: number | null;
  /** 0..100, or null while the server has not reported a content length. */
  percent: number | null;
}

export interface UpdateDownloadHandlers {
  onProgress?: (progress: UpdateProgress) => void;
  onDownloaded?: () => void;
}

/**
 * A checked update. The Rust side keeps a resource until `downloadAndInstall`
 * runs or `close` releases it, so the UI must keep this handle for the session.
 */
export interface AvailableUpdate {
  info: AvailableUpdateInfo;
  downloadAndInstall: (handlers?: UpdateDownloadHandlers) => Promise<void>;
  close: () => Promise<void>;
}

function progressState(downloaded: number, total: number | null): UpdateProgress {
  const percent =
    total !== null && total > 0
      ? Math.min(100, Math.max(0, Math.round((downloaded / total) * 100)))
      : null;
  return { downloaded, total, percent };
}

/** Human-readable download progress for the updater panel. */
export function formatDownloadProgress(progress: UpdateProgress): string {
  if (progress.total === null) return `${formatBytes(progress.downloaded)} downloaded`;
  return `${formatBytes(progress.downloaded)} of ${formatBytes(progress.total)}`;
}

function wrapUpdate(update: Update): AvailableUpdate {
  return {
    info: {
      currentVersion: update.currentVersion,
      version: update.version,
      date: update.date,
      body: update.body,
    },
    downloadAndInstall: async ({ onProgress, onDownloaded } = {}) => {
      let downloaded = 0;
      let total: number | null = null;

      await update.downloadAndInstall((event: DownloadEvent) => {
        switch (event.event) {
          case "Started": {
            const length = event.data.contentLength;
            total = typeof length === "number" && length > 0 ? length : null;
            onProgress?.(progressState(0, total));
            break;
          }
          case "Progress":
            downloaded += event.data.chunkLength;
            onProgress?.(progressState(downloaded, total));
            break;
          case "Finished":
            downloaded = total ?? downloaded;
            onProgress?.(progressState(downloaded, total));
            onDownloaded?.();
            break;
        }
      });
    },
    close: () => update.close(),
  };
}

/**
 * Checks the configured GitHub Releases manifest. This is only called after an
 * explicit user action; Cyron never polls for updates in the background.
 */
export async function checkForUpdate(): Promise<AvailableUpdate | null> {
  const update = await check({ timeout: 20_000 });
  return update ? wrapUpdate(update) : null;
}

/** Restarts Cyron after the Windows installer has finished. */
export async function relaunchApp(): Promise<void> {
  await relaunch();
}

/** Reads the version of the installed executable, not the version of this source tree. */
export async function getInstalledVersion(): Promise<string> {
  return getVersion();
}

/** The installed version once the native app answers; null while loading or unavailable. */
export function useInstalledVersion(): string | null {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    void getInstalledVersion()
      .then((value) => {
        if (active) setVersion(value);
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, []);

  return version;
}

/** `nsis`, `msi`, or null when the native bundle type is unavailable. */
export async function getInstalledBundleType(): Promise<string | null> {
  return getBundleType() ?? null;
}

/** In-app updates use the per-user NSIS package; never offer it to an MSI install. */
export function supportsInAppUpdates(bundleType: string | null | undefined): boolean {
  return bundleType === "nsis";
}

/** The installed package type; undefined while the native app is still answering. */
export function useInstalledBundleType(): string | null | undefined {
  const [bundleType, setBundleType] = useState<string | null | undefined>(undefined);

  useEffect(() => {
    let active = true;
    void getInstalledBundleType()
      .then((value) => {
        if (active) setBundleType(value);
      })
      .catch(() => {
        if (active) setBundleType(null);
      });
    return () => {
      active = false;
    };
  }, []);

  return bundleType;
}
