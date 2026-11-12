import { describe, expect, it } from "vitest";
import { getUninstallOutcome } from "./uninstallerFlow";

describe("uninstaller result flow", () => {
  it("only reports confirmed removal for a live-verified completion", () => {
    expect(getUninstallOutcome("completed")).toEqual({
      tone: "safe",
      title: "Removal confirmed",
      verifiedRemoved: true,
    });
    for (const status of ["cancelled", "failed", "remaining", "restart_required"]) {
      expect(getUninstallOutcome(status).verifiedRemoved).toBe(false);
    }
  });

  it("keeps cancellation and a remaining registration visible as caution states", () => {
    expect(getUninstallOutcome("cancelled")).toMatchObject({
      tone: "caution",
      title: "Uninstallation cancelled",
      verifiedRemoved: false,
    });
    expect(getUninstallOutcome("remaining")).toMatchObject({
      tone: "caution",
      title: "Application still registered",
      verifiedRemoved: false,
    });
  });

  it("treats unknown backend statuses as failures", () => {
    expect(getUninstallOutcome("unexpected")).toMatchObject({
      tone: "danger",
      title: "Uninstallation failed",
      verifiedRemoved: false,
    });
  });
});
