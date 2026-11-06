import type { CacheTarget } from "./types";

/** Cleanup review groups. These mirror the Rust catalog categories. */
export type CleanupGroupKey = "windows" | "applications" | "developer" | "ai";

export interface CleanupGroup {
  key: CleanupGroupKey;
  label: string;
  targets: CacheTarget[];
  bytes: number;
}

export const CLEANUP_GROUPS: { key: CleanupGroupKey; label: string }[] = [
  { key: "windows", label: "Windows" },
  { key: "applications", label: "Applications" },
  { key: "developer", label: "Developer tools" },
  { key: "ai", label: "AI tools" },
];

/** windows, ide, packages/containers and ai are the backend categories. */
export function groupForCategory(category: string): CleanupGroupKey {
  switch (category) {
    case "windows":
      return "windows";
    case "ide":
      return "applications";
    case "ai":
      return "ai";
    case "packages":
    case "containers":
      return "developer";
    default:
      return "developer";
  }
}

function bySize(a: CacheTarget, b: CacheTarget): number {
  return b.size - a.size || a.tool.localeCompare(b.tool) || a.label.localeCompare(b.label);
}

/** Only groups with targets, in fixed display order. The input is not mutated. */
export function groupTargets(targets: CacheTarget[]): CleanupGroup[] {
  const buckets = new Map<CleanupGroupKey, CacheTarget[]>();
  for (const target of targets) {
    const key = groupForCategory(target.category);
    const bucket = buckets.get(key);
    if (bucket) bucket.push(target);
    else buckets.set(key, [target]);
  }

  return CLEANUP_GROUPS.filter((group) => buckets.has(group.key)).map((group) => {
    const items = [...buckets.get(group.key)!].sort(bySize);
    return {
      key: group.key,
      label: group.label,
      targets: items,
      bytes: items.reduce((sum, target) => sum + target.size, 0),
    };
  });
}

/** Protected data and empty locations are never offered for cleaning. */
export function isSelectable(target: CacheTarget): boolean {
  if (target.risk === "protected") return false;
  if (!target.exists || !target.path) return false;
  return target.size > 0 || target.fileCount > 0;
}

export function toggleSelection(current: Set<string>, id: string): Set<string> {
  const next = new Set(current);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  return next;
}

/** Ignores ids that are no longer in the list and targets that may not be cleaned. */
export function selectedTotals(
  targets: CacheTarget[],
  selected: Set<string>,
): { count: number; bytes: number } {
  let count = 0;
  let bytes = 0;
  for (const target of targets) {
    if (!selected.has(target.id) || !isSelectable(target)) continue;
    count += 1;
    bytes += target.size;
  }
  return { count, bytes };
}

export function canConfirmCleanup(input: {
  readOnly: boolean;
  selectedCount: number;
  previewing: boolean;
  confirmed: boolean;
  running: boolean;
}): boolean {
  if (input.readOnly || input.running) return false;
  if (input.previewing) return false;
  if (input.selectedCount < 1) return false;
  return input.confirmed;
}
