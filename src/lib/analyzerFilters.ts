import { categoryForExtension, categoryForFile, type CategoryKey } from "./categories";
import type { FileStat, ScanNode } from "./types";

export type AnalyzerAge = "any" | "recent" | "mid" | "old";
export type AnalyzerSortField = "size" | "name" | "files" | "modified";
export type AnalyzerSortDir = "asc" | "desc";

export interface AnalyzerFilters {
  /** Minimum logical size in bytes; 0 means any size. */
  minSize: number;
  category: CategoryKey | "all";
  age: AnalyzerAge;
  /** Extension without the leading dot; empty means any extension. */
  extension: string;
  sortField: AnalyzerSortField;
  sortDir: AnalyzerSortDir;
}

export const defaultFilters: AnalyzerFilters = {
  minSize: 0,
  category: "all",
  age: "any",
  extension: "",
  sortField: "size",
  sortDir: "desc",
};

const DAY = 86_400;
const RECENT_DAYS = 30;
const OLD_DAYS = 365;

function nowSeconds(now: number): number {
  return now > 10_000_000_000 ? Math.floor(now / 1000) : Math.floor(now);
}

function matchesAge(modified: number | null, age: AnalyzerAge, now: number): boolean {
  if (age === "any") return true;
  if (!modified || !Number.isFinite(modified)) return false;
  const seconds = nowSeconds(now) - modified;
  if (seconds < 0) return false;
  if (age === "recent") return seconds < RECENT_DAYS * DAY;
  if (age === "mid") return seconds >= RECENT_DAYS * DAY && seconds < OLD_DAYS * DAY;
  return seconds >= OLD_DAYS * DAY;
}

function normalizeExtension(value: string): string {
  return value.trim().replace(/^\./, "").toLocaleLowerCase();
}

/** Files and synthetic group nodes can be tested against every filter. */
export function matchesFile(file: Pick<FileStat, "name" | "size" | "modified">, filters: AnalyzerFilters, now = Date.now()): boolean {
  if (filters.minSize > 0 && file.size < filters.minSize) return false;
  if (filters.extension) {
    const wanted = normalizeExtension(filters.extension);
    const actual = normalizeExtension(file.name.includes(".") ? file.name.split(".").pop() ?? "" : "");
    if (actual !== wanted) return false;
  }
  if (filters.category !== "all" && categoryForFile(file.name) !== filters.category) return false;
  return matchesAge(file.modified, filters.age, now);
}

/** A summarized tree node is a file, a group or a folder. */
export function matchesNode(node: ScanNode, filters: AnalyzerFilters, now = Date.now()): boolean {
  if (node.kind === "dir") return node.size >= filters.minSize;
  if (filters.category !== "all") {
    const category = categoryForExtension(extensionOfNode(node));
    if (category !== filters.category) return false;
  }
  return matchesFile({ name: node.name, size: node.size, modified: node.modified }, filters, now);
}

function extensionOfNode(node: ScanNode): string {
  if (node.kind === "file") {
    return node.name.includes(".") ? node.name.split(".").pop() ?? "" : "";
  }
  return "";
}

/** Keep a folder when it or any descendant matches. Returned sizes stay measured. */
export function filterTree(root: ScanNode, filters: AnalyzerFilters, now = Date.now()): ScanNode {
  const active =
    filters.minSize > 0 || filters.category !== "all" || filters.age !== "any" || filters.extension !== "";
  if (!active) return root;
  const visit = (node: ScanNode): ScanNode | null => {
    if (node.kind !== "dir") return matchesNode(node, filters, now) ? node : null;
    const children = node.children
      .map(visit)
      .filter((child): child is ScanNode => child !== null);
    if (children.length > 0) return { ...node, children };
    return node.size >= filters.minSize && filters.category === "all" && filters.extension === "" && filters.age === "any"
      ? { ...node, children: [] }
      : null;
  };
  const filtered = visit(root);
  return filtered ?? { ...root, children: [] };
}

function compare(a: ScanNode, b: ScanNode, field: AnalyzerSortField): number {
  if (field === "name") return a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
  if (field === "files") return a.fileCount - b.fileCount;
  if (field === "modified") return (a.modified ?? 0) - (b.modified ?? 0);
  return (a.size || a.allocated) - (b.size || b.allocated);
}

/** Stable sort of a folder's children; the input array is not mutated. */
export function sortNodes(nodes: ScanNode[], sort: Pick<AnalyzerFilters, "sortField" | "sortDir">): ScanNode[] {
  const direction = sort.sortDir === "asc" ? 1 : -1;
  return [...nodes].sort((a, b) => {
    if (a.kind === "dir" && b.kind !== "dir") return -1;
    if (b.kind === "dir" && a.kind !== "dir") return 1;
    const primary = compare(a, b, sort.sortField) * direction;
    if (primary !== 0) return primary;
    return a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
  });
}

/** Totals for what the filters actually show, not the full scan. */
export function filteredTotals(
  root: ScanNode,
  filters: AnalyzerFilters,
  now = Date.now(),
): { bytes: number; files: number } {
  let bytes = 0;
  let files = 0;
  const visit = (node: ScanNode) => {
    if (node.kind === "dir") {
      node.children.forEach(visit);
      return;
    }
    if (!matchesNode(node, filters, now)) return;
    bytes += node.size;
    files += node.fileCount || 1;
  };
  visit(filterTree(root, filters, now));
  return { bytes, files };
}

export function activeFilterCount(filters: AnalyzerFilters): number {
  let count = 0;
  if (filters.minSize > 0) count += 1;
  if (filters.category !== "all") count += 1;
  if (filters.age !== "any") count += 1;
  if (filters.extension.trim()) count += 1;
  return count;
}
