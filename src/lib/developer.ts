import type {
  DeveloperArtifact,
  DeveloperProject,
  DeveloperStorageScan,
  Risk,
} from "./types";

export type ActivityFilter = "all" | "active" | "recent" | "old" | "unknown";

export const ACTIVITY_OPTIONS: { value: ActivityFilter; label: string }[] = [
  { value: "all", label: "All projects" },
  { value: "active", label: "Active" },
  { value: "recent", label: "Recent" },
  { value: "old", label: "Old" },
  { value: "unknown", label: "Unknown" },
];

export const ACTIVITY_LABEL: Record<string, string> = {
  active: "Active in the last 30 days",
  recent: "Touched in the last 6 months",
  old: "No recent writes",
  unknown: "Age unknown",
};

export const ARTIFACT_KIND_LABEL: Record<string, string> = {
  "node-modules": "Dependencies",
  dist: "Build output",
  build: "Build output",
  "cargo-target": "Rust build output",
  "maven-target": "Maven build output",
  "gradle-build": "Gradle build output",
  "gradle-cache": "Gradle project cache",
  "next-cache": "Next.js build cache",
  "turbo-cache": "Turborepo cache",
  pycache: "Python bytecode",
  "pytest-cache": "pytest cache",
  "mypy-cache": "mypy cache",
  "ruff-cache": "Ruff cache",
};

export function flattenArtifacts(scan: DeveloperStorageScan | null): DeveloperArtifact[] {
  if (!scan) return [];
  return scan.projects.flatMap((project) => project.artifacts);
}

export function filterProjects(
  projects: DeveloperProject[],
  query: string,
  activity: ActivityFilter,
): DeveloperProject[] {
  const needle = query.trim().toLowerCase();
  return projects.filter((project) => {
    if (activity !== "all" && project.activity !== activity) return false;
    if (!needle) return true;
    const haystack = [
      project.name,
      project.path,
      project.kind,
      ...project.frameworks,
      ...project.artifacts.map((artifact) => artifact.label),
    ]
      .join(" ")
      .toLowerCase();
    return haystack.includes(needle);
  });
}

export function selectedArtifactTotals(
  artifacts: DeveloperArtifact[],
  selected: ReadonlySet<string>,
): { count: number; bytes: number; allocated: number } {
  let count = 0;
  let bytes = 0;
  let allocated = 0;
  for (const artifact of artifacts) {
    if (!selected.has(artifact.id)) continue;
    count += 1;
    bytes += artifact.size;
    allocated += artifact.allocated;
  }
  return { count, bytes, allocated };
}

export function riskForArtifact(artifact: DeveloperArtifact): Risk {
  return artifact.risk;
}

export function verificationTone(status: string): "safe" | "caution" | "danger" {
  if (status === "removed") return "safe";
  if (status === "remaining") return "caution";
  return "danger";
}
