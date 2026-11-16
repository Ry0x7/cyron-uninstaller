import { describe, expect, it } from "vitest";
import {
  filterProjects,
  flattenArtifacts,
  selectedArtifactTotals,
  verificationTone,
} from "./developer";
import type { DeveloperArtifact, DeveloperProject, DeveloperStorageScan } from "./types";

function artifact(overrides: Partial<DeveloperArtifact> = {}): DeveloperArtifact {
  return {
    id: "artifact-1",
    projectId: "project-1",
    projectName: "app",
    path: "C:\\code\\app\\node_modules",
    kind: "node-modules",
    label: "Installed dependencies",
    risk: "caution",
    confidence: "high",
    evidence: "Direct child of C:\\code\\app; package.json.",
    rebuild: "npm ci",
    loss: "Reinstall time.",
    size: 1024,
    allocated: 2048,
    fileCount: 10,
    dirCount: 4,
    lastModified: 100,
    activity: "recent",
    stale: false,
    skippedLinks: 0,
    truncated: false,
    ...overrides,
  };
}

function project(overrides: Partial<DeveloperProject> = {}): DeveloperProject {
  return {
    id: "project-1",
    name: "app",
    path: "C:\\code\\app",
    kind: "node",
    markers: ["package.json"],
    frameworks: ["vite"],
    modified: 100,
    activity: "recent",
    artifacts: [artifact()],
    protected: [],
    sourceProtected: true,
    ...overrides,
  };
}

describe("developer storage helpers", () => {
  it("flattens artifacts across projects", () => {
    const second = project({
      id: "project-2",
      name: "service",
      artifacts: [artifact({ id: "artifact-2", projectId: "project-2" })],
    });
    const scan: DeveloperStorageScan = {
      scanId: "devscan-1",
      scannedAt: 1,
      roots: [],
      projects: [project(), second],
      totals: {
        projectCount: 2,
        artifactCount: 2,
        artifactBytes: 2048,
        artifactAllocated: 4096,
        protectedBytes: 0,
        activeProjects: 0,
        oldProjects: 1,
      },
      warnings: [],
    };
    expect(flattenArtifacts(scan)).toHaveLength(2);
    expect(flattenArtifacts(null)).toEqual([]);
  });

  it("filters by activity and searches project metadata and artifact labels", () => {
    const active = project();
    const old = project({
      id: "project-2",
      name: "legacy",
      activity: "old",
      frameworks: ["react"],
      artifacts: [artifact({ id: "artifact-2", label: "Build output", kind: "dist" })],
    });
    expect(filterProjects([active, old], "", "old").map((item) => item.id)).toEqual(["project-2"]);
    expect(filterProjects([active, old], "vite", "all").map((item) => item.id)).toEqual(["project-1"]);
    expect(filterProjects([active, old], "build output", "all").map((item) => item.id)).toEqual([
      "project-2",
    ]);
    expect(filterProjects([active, old], "missing", "all")).toEqual([]);
  });

  it("sums only selected artifacts", () => {
    const first = artifact();
    const second = artifact({ id: "artifact-2", size: 500, allocated: 700 });
    expect(selectedArtifactTotals([first, second], new Set(["artifact-1"]))).toEqual({
      count: 1,
      bytes: 1024,
      allocated: 2048,
    });
    expect(selectedArtifactTotals([first, second], new Set())).toEqual({
      count: 0,
      bytes: 0,
      allocated: 0,
    });
  });

  it("maps verification status to honest badge tones", () => {
    expect(verificationTone("removed")).toBe("safe");
    expect(verificationTone("remaining")).toBe("caution");
    expect(verificationTone("changed")).toBe("danger");
  });
});
