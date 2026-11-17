import { describe, expect, it } from "vitest";
import type { InstalledApp, UninstallSupport } from "../../lib/types";
import { batchEligibility } from "./batchEligibility";

function app(overrides: Partial<InstalledApp> = {}): InstalledApp {
  return {
    id: "machine-64|fixture",
    name: "Fixture App",
    version: "1.0",
    publisher: "Fixture",
    installLocation: "C:\\Apps\\Fixture",
    installDate: null,
    uninstallString: "C:\\Apps\\Fixture\\uninstall.exe /S",
    quietUninstallString: null,
    displayIcon: null,
    estimatedSize: null,
    hive: "machine-64",
    scope: "machine",
    architecture: "x64",
    isProtected: false,
    source: "win32",
    uninstallSupport: "standard",
    confidence: "confirmed",
    detectionReason: null,
    registryKeyName: "Fixture",
    ...overrides,
  };
}

function withSupport(support: UninstallSupport, overrides: Partial<InstalledApp> = {}) {
  return app({ uninstallSupport: support, ...overrides });
}

describe("batch uninstall eligibility", () => {
  it("accepts a registered Win32 command", () => {
    expect(batchEligibility(app(), true)).toEqual({ selectable: true, reason: null });
  });

  it("accepts a quiet-only command only while silent mode is on", () => {
    const silent = withSupport("silent_only", {
      uninstallString: null,
      quietUninstallString: "C:\\Apps\\Fixture\\uninstall.exe /quiet",
    });
    expect(batchEligibility(silent, true).selectable).toBe(true);
    expect(batchEligibility(silent, false).reason).toContain("quiet uninstaller");
  });

  it("rejects Steam games even though they carry a Steam URI", () => {
    const steam = app({
      id: "steam|123",
      source: "steam",
      hive: "steam",
      uninstallSupport: "store_managed",
      uninstallString: "steam://uninstall/123",
    });
    expect(batchEligibility(steam, true).selectable).toBe(false);
    expect(batchEligibility(steam, true).reason).toContain("Steam");
  });

  it("rejects portable and manually removed applications", () => {
    const portable = app({
      source: "portable",
      hive: "portable",
      uninstallSupport: "manual_only",
      uninstallString: null,
    });
    expect(batchEligibility(portable, true).selectable).toBe(false);
    expect(batchEligibility(portable, true).reason).toContain("Forced Removal");
  });

  it("rejects broken registrations with no command", () => {
    const broken = withSupport("missing", { uninstallString: null });
    expect(batchEligibility(broken, true).selectable).toBe(false);
    expect(batchEligibility(broken, true).reason).toContain("broken-registration");
  });

  it("rejects Store packages and protected apps", () => {
    expect(
      batchEligibility(
        app({ source: "store", hive: "store", uninstallSupport: "store_managed", uninstallString: null }),
        true,
      ).selectable,
    ).toBe(false);
    expect(batchEligibility(app({ isProtected: true }), true).selectable).toBe(false);
  });

  it("rejects a standard entry whose command is blank", () => {
    expect(batchEligibility(app({ uninstallString: "   " }), true).selectable).toBe(false);
  });
});
