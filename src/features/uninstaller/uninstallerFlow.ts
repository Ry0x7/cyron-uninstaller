export type UninstallOutcome = {
  tone: "safe" | "caution" | "danger";
  title: string;
  verifiedRemoved: boolean;
};

export function getUninstallOutcome(status: string): UninstallOutcome {
  switch (status) {
    case "completed":
      return { tone: "safe", title: "Removal confirmed", verifiedRemoved: true };
    case "restart_required":
      return { tone: "caution", title: "Restart required", verifiedRemoved: false };
    case "remaining":
      return { tone: "caution", title: "Application still registered", verifiedRemoved: false };
    case "cancelled":
      return { tone: "caution", title: "Uninstallation cancelled", verifiedRemoved: false };
    default:
      return { tone: "danger", title: "Uninstallation failed", verifiedRemoved: false };
  }
}
