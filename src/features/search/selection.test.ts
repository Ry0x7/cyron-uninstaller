import { describe, expect, it } from "vitest";
import {
  checkedBytes,
  checkedPaths,
  copyPaths,
  selectAll,
  selectRange,
  toggleChecked,
} from "./selection";

const items = [
  { fullPath: "C:\\one.txt", size: 10, isFolder: false },
  { fullPath: "C:\\two.txt", size: 20, isFolder: false },
  { fullPath: "C:\\three", size: 0, isFolder: true },
];

describe("search selection helpers", () => {
  it("toggles without mutating the previous set", () => {
    const first = toggleChecked(new Set(), items[0].fullPath);
    const second = toggleChecked(first, items[1].fullPath);
    expect([...first]).toEqual([items[0].fullPath]);
    expect([...second]).toEqual([items[0].fullPath, items[1].fullPath]);
    expect([...toggleChecked(second, items[0].fullPath)]).toEqual([items[1].fullPath]);
    expect([...toggleChecked(second, items[0].fullPath, true)]).toEqual([
      items[0].fullPath,
      items[1].fullPath,
    ]);
  });

  it("selects an inclusive range in either direction", () => {
    const paths = items.map((item) => item.fullPath);
    expect([...selectRange(paths, paths[0], paths[2])]).toEqual(paths);
    expect([...selectRange(paths, paths[2], paths[0])]).toEqual(paths);
    expect([...selectRange(paths, paths[0], paths[1])]).toEqual(paths.slice(0, 2));
    expect([...selectRange(paths, "missing", paths[1])]).toEqual([paths[1]]);
  });

  it("selects all loaded paths without duplicates", () => {
    const selected = selectAll(["a", "b", "a"]);
    expect(selected.size).toBe(2);
    expect([...selected]).toEqual(["a", "b"]);
  });

  it("returns checked paths in visible order and sums only files", () => {
    const checked = new Set([items[2].fullPath, items[0].fullPath]);
    expect(checkedPaths(items, checked)).toEqual([items[0].fullPath, items[2].fullPath]);
    expect(checkedBytes(items, checked)).toBe(10);
    expect(copyPaths(["a", "b"])).toBe("a\nb");
  });
});
