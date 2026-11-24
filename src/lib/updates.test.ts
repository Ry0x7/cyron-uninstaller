import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  check: vi.fn(),
  close: vi.fn(),
  downloadAndInstall: vi.fn(),
  getBundleType: vi.fn(),
  getVersion: vi.fn(),
  relaunch: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-updater", () => ({ check: mocks.check }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: mocks.relaunch }));
vi.mock("@tauri-apps/api/app", () => ({
  getBundleType: mocks.getBundleType,
  getVersion: mocks.getVersion,
}));

import {
  checkForUpdate,
  formatDownloadProgress,
  getInstalledBundleType,
  getInstalledVersion,
  relaunchApp,
  supportsInAppUpdates,
  type UpdateProgress,
} from "./updates";

beforeEach(() => {
  mocks.check.mockReset();
  mocks.close.mockReset();
  mocks.downloadAndInstall.mockReset();
  mocks.getBundleType.mockReset();
  mocks.getVersion.mockReset();
  mocks.relaunch.mockReset();
});

function updateFixture() {
  mocks.downloadAndInstall.mockImplementation(async (onEvent: (event: unknown) => void) => {
    onEvent({ event: "Started", data: { contentLength: 1000 } });
    onEvent({ event: "Progress", data: { chunkLength: 250 } });
    onEvent({ event: "Progress", data: { chunkLength: 250 } });
    onEvent({ event: "Finished" });
  });
  return {
    currentVersion: "0.2.2",
    version: "0.2.3",
    date: "2026-11-25T00:00:00Z",
    body: "Bug fixes.",
    downloadAndInstall: mocks.downloadAndInstall,
    close: mocks.close,
  };
}

describe("updater wrapper", () => {
  it("reports no update when the feed has nothing newer", async () => {
    mocks.check.mockResolvedValue(null);
    await expect(checkForUpdate()).resolves.toBeNull();
    expect(mocks.check).toHaveBeenCalledWith({ timeout: 20_000 });
  });

  it("maps the announced version and release notes", async () => {
    mocks.check.mockResolvedValue(updateFixture());
    const available = await checkForUpdate();

    expect(available?.info).toEqual({
      currentVersion: "0.2.2",
      version: "0.2.3",
      date: "2026-11-25T00:00:00Z",
      body: "Bug fixes.",
    });
  });

  it("reports bounded download progress and a completion callback", async () => {
    mocks.check.mockResolvedValue(updateFixture());
    const available = await checkForUpdate();
    const seen: UpdateProgress[] = [];
    const downloaded = vi.fn();

    await available?.downloadAndInstall({ onProgress: (progress) => seen.push(progress), onDownloaded: downloaded });

    expect(seen).toEqual([
      { downloaded: 0, total: 1000, percent: 0 },
      { downloaded: 250, total: 1000, percent: 25 },
      { downloaded: 500, total: 1000, percent: 50 },
      { downloaded: 1000, total: 1000, percent: 100 },
    ]);
    expect(downloaded).toHaveBeenCalledOnce();
  });

  it("handles a feed without a content length", async () => {
    mocks.check.mockResolvedValue(updateFixture());
    mocks.downloadAndInstall.mockImplementation(async (onEvent: (event: unknown) => void) => {
      onEvent({ event: "Progress", data: { chunkLength: 512 } });
    });
    const available = await checkForUpdate();
    const seen: UpdateProgress[] = [];

    await available?.downloadAndInstall({ onProgress: (progress) => seen.push(progress) });

    expect(seen).toEqual([{ downloaded: 512, total: null, percent: null }]);
    expect(formatDownloadProgress(seen[0])).toContain("downloaded");
  });

  it("releases the update resource and relaunches the app", async () => {
    mocks.check.mockResolvedValue(updateFixture());
    const available = await checkForUpdate();
    await available?.close();
    await relaunchApp();

    expect(mocks.close).toHaveBeenCalledOnce();
    expect(mocks.relaunch).toHaveBeenCalledOnce();
  });

  it("reads the installed executable version and bundle type", async () => {
    mocks.getVersion.mockResolvedValue("0.2.3");
    mocks.getBundleType.mockResolvedValue("nsis");

    await expect(getInstalledVersion()).resolves.toBe("0.2.3");
    await expect(getInstalledBundleType()).resolves.toBe("nsis");
  });

  it("offers in-app updates only to an NSIS installation", () => {
    expect(supportsInAppUpdates("nsis")).toBe(true);
    expect(supportsInAppUpdates("msi")).toBe(false);
    expect(supportsInAppUpdates(null)).toBe(false);
    expect(supportsInAppUpdates(undefined)).toBe(false);
  });
});
