import { describe, expect, it } from "vitest";
import {
  activeFilterCount,
  defaultFilters,
  filterTree,
  filteredTotals,
  matchesFile,
  sortNodes,
  type AnalyzerFilters,
} from "./analyzerFilters";
import type { ScanNode } from "./types";

const NOW = Date.UTC(2026, 10, 6) / 1000;
const DAY = 86_400;

function file(name: string, size: number, modified = NOW): ScanNode {
  return {
    name,
    path: `C:\\scan\\${name}`,
    kind: "file",
    size,
    allocated: size,
    fileCount: 1,
    dirCount: 0,
    modified,
    children: [],
  };
}

function dir(name: string, children: ScanNode[], modified = NOW): ScanNode {
  const size = children.reduce((sum, child) => sum + child.size, 0);
  return {
    name,
    path: `C:\\scan\\${name}`,
    kind: "dir",
    size,
    allocated: size,
    fileCount: children.reduce((sum, child) => sum + child.fileCount, 0),
    dirCount: children.filter((child) => child.kind === "dir").length,
    modified,
    children,
  };
}

const filters = (overrides: Partial<AnalyzerFilters> = {}): AnalyzerFilters => ({
  ...defaultFilters,
  ...overrides,
});

describe("matchesFile", () => {
  it("applies size, category, extension and age", () => {
    expect(matchesFile({ name: "big.mp4", size: 5000, modified: NOW }, filters({ minSize: 1000 }), NOW)).toBe(true);
    expect(matchesFile({ name: "small.mp4", size: 10, modified: NOW }, filters({ minSize: 1000 }), NOW)).toBe(false);
    expect(matchesFile({ name: "big.mp4", size: 5000, modified: NOW }, filters({ category: "video" }), NOW)).toBe(true);
    expect(matchesFile({ name: "big.mp4", size: 5000, modified: NOW }, filters({ category: "code" }), NOW)).toBe(false);
    expect(matchesFile({ name: "notes.TXT", size: 30, modified: NOW }, filters({ extension: ".txt" }), NOW)).toBe(true);
    expect(matchesFile({ name: "notes.txt", size: 30, modified: NOW }, filters({ extension: "md" }), NOW)).toBe(false);
  });

  it("keeps unknown ages out of age-filtered results", () => {
    expect(matchesFile({ name: "a.bin", size: 10, modified: null }, filters({ age: "old" }), NOW)).toBe(false);
    expect(matchesFile({ name: "a.bin", size: 10, modified: NOW - 2 * DAY }, filters({ age: "recent" }), NOW)).toBe(true);
    expect(matchesFile({ name: "a.bin", size: 10, modified: NOW - 60 * DAY }, filters({ age: "mid" }), NOW)).toBe(true);
    expect(matchesFile({ name: "a.bin", size: 10, modified: NOW - 400 * DAY }, filters({ age: "old" }), NOW)).toBe(true);
  });

  it("treats a future timestamp as not old", () => {
    expect(matchesFile({ name: "a.bin", size: 10, modified: NOW + DAY }, filters({ age: "old" }), NOW)).toBe(false);
  });
});

describe("filterTree", () => {
  const root = dir("scan", [
    dir("media", [file("clip.mp4", 900), file("photo.jpg", 400)]),
    dir("code", [file("main.rs", 100)]),
    file("root.bin", 50),
  ]);

  it("returns the original tree when no filter is active", () => {
    expect(filterTree(root, defaultFilters, NOW)).toBe(root);
  });

  it("keeps folders with matching descendants and drops empty folders", () => {
    const result = filterTree(root, filters({ category: "video" }), NOW);
    expect(result.children.map((node) => node.name)).toEqual(["media"]);
    expect(result.children[0].children.map((node) => node.name)).toEqual(["clip.mp4"]);
  });

  it("applies the size threshold to files without altering measured sizes", () => {
    const result = filterTree(root, filters({ minSize: 500 }), NOW);
    expect(result.children[0].children.map((node) => node.name)).toEqual(["clip.mp4"]);
    expect(result.size).toBe(root.size);
    expect(result.children[0].size).toBe(1300);
  });

  it("returns an empty root when nothing matches", () => {
    const result = filterTree(root, filters({ extension: "does-not-exist" }), NOW);
    expect(result.children).toEqual([]);
    expect(result.size).toBe(root.size);
  });
});

describe("sortNodes", () => {
  const nodes = [file("a.bin", 10), dir("zzz", [file("b.bin", 500), file("c.bin", 20)]), file("m.bin", 100)];

  it("sorts by size with folders first", () => {
    expect(sortNodes(nodes, { sortField: "size", sortDir: "desc" }).map((node) => node.name)).toEqual([
      "zzz",
      "m.bin",
      "a.bin",
    ]);
  });

  it("sorts ascending and by name", () => {
    expect(sortNodes(nodes, { sortField: "size", sortDir: "asc" }).map((node) => node.name)).toEqual([
      "zzz",
      "a.bin",
      "m.bin",
    ]);
    expect(sortNodes(nodes, { sortField: "name", sortDir: "asc" }).map((node) => node.name)).toEqual([
      "zzz",
      "a.bin",
      "m.bin",
    ]);
  });

  it("does not mutate the input", () => {
    const before = nodes.map((node) => node.name);
    sortNodes(nodes, { sortField: "size", sortDir: "asc" });
    expect(nodes.map((node) => node.name)).toEqual(before);
  });
});

describe("filteredTotals", () => {
  const root = dir("scan", [dir("media", [file("clip.mp4", 900), file("photo.jpg", 400)]), file("root.bin", 50)]);

  it("sums only matching files", () => {
    const totals = filteredTotals(root, filters({ category: "video" }), NOW);
    expect(totals).toEqual({ bytes: 900, files: 1 });
  });

  it("sums the whole tree with no filters", () => {
    const totals = filteredTotals(root, defaultFilters, NOW);
    expect(totals).toEqual({ bytes: 1350, files: 3 });
  });
});

describe("activeFilterCount", () => {
  it("counts only non-default filters", () => {
    expect(activeFilterCount(defaultFilters)).toBe(0);
    expect(
      activeFilterCount(filters({ minSize: 1024, category: "image", age: "old", extension: "png" })),
    ).toBe(4);
  });
});
