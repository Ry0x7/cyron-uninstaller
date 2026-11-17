import type { InstalledApp } from "../../lib/types";

export interface BatchEligibility {
  selectable: boolean;
  reason: string | null;
}

/**
 * Batch uninstall runs only registered vendor commands. Store packages use the
 * package plan, Steam games use Steam, and portable/broken entries have no
 * command at all. Keeping this rule in one place prevents the checkbox and the
 * backend from disagreeing about what can be queued.
 */
export function batchEligibility(app: InstalledApp, quiet: boolean): BatchEligibility {
  if (app.isProtected) {
    return { selectable: false, reason: "Windows-protected applications cannot be batch uninstalled." };
  }
  if (app.source === "steam" || app.uninstallSupport === "store_managed") {
    if (app.source === "steam") {
      return { selectable: false, reason: "Steam games are removed through Steam, not the batch queue." };
    }
    return { selectable: false, reason: "Store packages must be removed individually." };
  }
  if (app.source === "portable" || app.uninstallSupport === "manual_only") {
    return {
      selectable: false,
      reason: "No uninstaller is registered. Use Forced Removal from the application row.",
    };
  }
  if (app.uninstallSupport === "missing") {
    return {
      selectable: false,
      reason: "No uninstaller is registered. Use the broken-registration cleanup.",
    };
  }
  if (app.uninstallSupport === "silent_only" && !quiet) {
    return {
      selectable: false,
      reason: "Only a quiet uninstaller is registered. Enable the silent switch to batch this app.",
    };
  }
  const command = quiet
    ? app.quietUninstallString ?? app.uninstallString
    : app.uninstallString;
  if (!command || command.trim() === "") {
    return { selectable: false, reason: "No uninstall command was recorded." };
  }
  return { selectable: true, reason: null };
}

export function batchSelectable(app: InstalledApp, quiet: boolean): boolean {
  return batchEligibility(app, quiet).selectable;
}
