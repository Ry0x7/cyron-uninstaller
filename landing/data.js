// Benchmark rows are generated from docs/BENCHMARKS.md by scripts/sync-landing.mjs.
// The comparison matrix is capability-only and mirrors docs/COMPETITOR_COMPARISON.md.
// No competitor was installed, timed or otherwise measured for this page.
export { BENCHMARKS, BENCHMARK_SOURCE } from "./benchmarks.js";

export const TOOLS = ["Cyron", "Revo", "BCUninstaller", "WizTree", "BleachBit"];

export const COMPARISON = [
  {
    label: "Uninstall + leftover removal",
    values: ["yes", "yes", "yes", "no", "partial"],
  },
  {
    label: "Recoverable file quarantine",
    values: ["yes", "unknown", "partial", "no", "no"],
  },
  {
    label: "Disk treemap",
    values: ["yes", "no", "reported", "yes", "no"],
  },
  {
    label: "Developer / AI cache review",
    values: ["yes", "no", "partial", "no", "yes"],
  },
  {
    label: "Read-only performance evidence",
    values: ["yes", "no", "no", "no", "no"],
  },
  {
    label: "Timed on this machine",
    values: ["measured", "notrun", "notrun", "notrun", "notrun"],
  },
];

export const LEGEND = {
  yes: "Yes",
  partial: "Partial",
  no: "No",
  unknown: "Not verified",
  reported: "Reported",
  measured: "Measured",
  notrun: "Not run",
};

// Dev-only stress fixture. It is never shown on a public host.
export const STRESS_TOOLS = [
  "Cyron Uninstaller",
  "Revo Uninstaller Pro",
  "Bulk Crap Uninstaller (BCUninstaller) Community Edition",
  "WizTree Disk Space Analyzer",
  "BleachBit",
];

export const STRESS_BENCHMARKS = [
  {
    label:
      "C:\\Users\\Aleksandra.Wiśniewska-Kowalczyk\\AppData\\Local\\Programs\\Microsoft VS Code\\resources\\app\\extensions\\ms-vscode.vscode-typescript-next\\node_modules\\typescript\\lib",
    full: "A realistic long Windows path with a very long unbroken segment",
    median: 1284.75,
    files: 1284000000,
    size: "9,999.9 GiB",
    peak: "1,024.5 MiB",
  },
  {
    label: "A directory name that is exactly this long to test the row",
    full: "Second stress row",
    median: 42.125,
    files: 1,
    size: "1.0 KiB",
    peak: "999.9 MiB",
  },
  {
    label: "empty",
    full: "Empty directory",
    median: 0,
    files: 0,
    size: "0 B",
    peak: "1.0 MiB",
  },
];

export const STRESS_COMPARISON = [
  {
    label:
      "Uninstall a registered application whose uninstaller is missing and whose install folder contains a junction, a hard link and a locked file",
    values: ["measured", "unknown", "reported", "partial", "notrun"],
  },
  {
    label: "A capability name that is deliberately longer than a normal table cell",
    values: ["yes", "partial", "yes", "unknown", "no"],
  },
  {
    label: "Read-only performance evidence",
    values: ["yes", "no", "partial", "no", "notrun"],
  },
  {
    label: "Timed on this machine",
    values: ["measured", "notrun", "notrun", "notrun", "notrun"],
  },
];
