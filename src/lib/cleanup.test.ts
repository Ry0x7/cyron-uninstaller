import { describe, expect, it } from "vitest";
import {
  CLEANUP_GROUPS,
  canConfirmCleanup,
  groupForCategory,
  groupTargets,
  isSelectable,
  selectedTotals,
  toggleSelection,
} from "./cleanup";
import type { CacheTarget, Risk } from "./types";

let sequence = 0;

function target(overrides: Partial<CacheTarget> = {}): CacheTarget {
  sequence += 1;
  return {
    id: `target-${sequence}`,
    tool: "Tool",
    label: "Cache",
    path: `C:\\cache\\${sequence}`,
    exists: true,
    size: 1024,
    allocated: 4096,
    fileCount: 3,
    dirCount: 1,
    risk: "safe" as Risk,
    category: "packages",
    note: "Regenerated on demand.",
    lastModified: null,
    ...overrides,
  };
}

describe("groupForCategory", () => {
  it("maps every catalog category to its review group", () => {
    expect(groupForCategory("windows")).toBe("windows");
    expect(groupForCategory("ide")).toBe("applications");
    expect(groupForCategory("packages")).toBe("developer");
    expect(groupForCategory("containers")).toBe("developer");
    expect(groupForCategory("ai")).toBe("ai");
  });

  it("falls back to developer for unknown categories", () => {
    expect(groupForCategory("")).toBe("developer");
    expect(groupForCategory("something-new")).toBe("developer");
  });
});

describe("CLEANUP_GROUPS", () => {
  it("is ordered windows, applications, developer, ai", () => {
    expect(CLEANUP_GROUPS.map((group) => group.key)).toEqual([
      "windows",
      "applications",
      "developer",
      "ai",
    ]);
    expect(CLEANUP_GROUPS.map((group) => group.label)).toEqual([
      "Windows",
      "Applications",
      "Developer tools",
      "AI tools",
    ]);
  });
});

describe("isSelectable", () => {
  it("accepts safe and review targets with measured content", () => {
    expect(isSelectable(target({ risk: "safe" }))).toBe(true);
    expect(isSelectable(target({ risk: "caution" }))).toBe(true);
    expect(isSelectable(target({ fileCount: 0, size: 10 }))).toBe(true);
    expect(isSelectable(target({ fileCount: 2, size: 0 }))).toBe(true);
  });

  it("never accepts protected data, even when it is large", () => {
    expect(
      isSelectable(target({ risk: "protected", size: 40 * 1024 ** 3, fileCount: 9000 })),
    ).toBe(false);
  });

  it("rejects missing, empty and path-less locations", () => {
    expect(isSelectable(target({ exists: false }))).toBe(false);
    expect(isSelectable(target({ path: "" }))).toBe(false);
    expect(isSelectable(target({ size: 0, fileCount: 0 }))).toBe(false);
  });
});

describe("groupTargets", () => {
  it("omits empty groups and keeps the fixed order", () => {
    const groups = groupTargets([
      target({ category: "ai", tool: "Ollama" }),
      target({ category: "windows", tool: "Chrome" }),
    ]);
    expect(groups.map((group) => group.key)).toEqual(["windows", "ai"]);
  });

  it("sorts by size descending, then tool and label", () => {
    const groups = groupTargets([
      target({ category: "ide", tool: "Zed", label: "Cache", size: 10 }),
      target({ category: "ide", tool: "Cursor", label: "Logs", size: 30 }),
      target({ category: "ide", tool: "Cursor", label: "Code cache", size: 30 }),
      target({ category: "ide", tool: "VS Code", label: "Logs", size: 20 }),
    ]);
    expect(groups).toHaveLength(1);
    expect(groups[0].targets.map((item) => `${item.tool}/${item.label}`)).toEqual([
      "Cursor/Code cache",
      "Cursor/Logs",
      "VS Code/Logs",
      "Zed/Cache",
    ]);
    expect(groups[0].bytes).toBe(90);
  });

  it("does not mutate the input array or its order", () => {
    const first = target({ category: "windows", size: 1 });
    const second = target({ category: "windows", size: 99 });
    const input = [first, second];
    groupTargets(input);
    expect(input).toEqual([first, second]);
  });

  it("returns nothing for an empty list", () => {
    expect(groupTargets([])).toEqual([]);
  });
});

describe("toggleSelection", () => {
  it("adds then removes with fresh sets", () => {
    const original = new Set<string>(["a"]);
    const added = toggleSelection(original, "b");
    expect(added).toEqual(new Set(["a", "b"]));
    expect(original).toEqual(new Set(["a"]));
    expect(added).not.toBe(original);

    const removed = toggleSelection(added, "a");
    expect(removed).toEqual(new Set(["b"]));
    expect(added).toEqual(new Set(["a", "b"]));
  });
});

describe("selectedTotals", () => {
  it("sums chosen selectable targets only", () => {
    const safe = target({ size: 100 });
    const review = target({ risk: "caution", size: 250 });
    const protectedTarget = target({ risk: "protected", size: 5000 });
    const missing = target({ exists: false, size: 900 });
    const totals = selectedTotals(
      [safe, review, protectedTarget, missing],
      new Set([safe.id, review.id, protectedTarget.id, missing.id, "stale-id"]),
    );
    expect(totals).toEqual({ count: 2, bytes: 350 });
  });

  it("ignores stale ids and returns zero for an empty selection", () => {
    expect(selectedTotals([target()], new Set(["gone"]))).toEqual({ count: 0, bytes: 0 });
    expect(selectedTotals([], new Set(["a"]))).toEqual({ count: 0, bytes: 0 });
  });
});

describe("canConfirmCleanup", () => {
  const ready = {
    readOnly: false,
    selectedCount: 2,
    previewing: false,
    confirmed: true,
    running: false,
  };

  it("allows a confirmed, previewed, writable selection", () => {
    expect(canConfirmCleanup(ready)).toBe(true);
  });

  it("blocks read-only mode, an empty selection, a missing preview, missing confirmation and a running job", () => {
    expect(canConfirmCleanup({ ...ready, readOnly: true })).toBe(false);
    expect(canConfirmCleanup({ ...ready, selectedCount: 0 })).toBe(false);
    expect(canConfirmCleanup({ ...ready, previewing: true })).toBe(false);
    expect(canConfirmCleanup({ ...ready, confirmed: false })).toBe(false);
    expect(canConfirmCleanup({ ...ready, running: true })).toBe(false);
  });

  it("prevents a double submit while a run is active", () => {
    expect(canConfirmCleanup({ ...ready, running: true })).toBe(false);
    expect(canConfirmCleanup({ ...ready, running: false })).toBe(true);
  });
});
