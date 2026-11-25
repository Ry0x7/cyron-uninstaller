/** Immutable selection helpers shared by the Search table and its tests. */

export function toggleChecked(
  current: ReadonlySet<string>,
  path: string,
  checked?: boolean,
): Set<string> {
  const next = new Set(current);
  const shouldCheck = checked ?? !next.has(path);
  if (shouldCheck) {
    next.add(path);
  } else {
    next.delete(path);
  }
  return next;
}

/** Selects the inclusive range between the anchor and the target row. */
export function selectRange(
  paths: readonly string[],
  anchor: string,
  target: string,
): Set<string> {
  const anchorIndex = paths.indexOf(anchor);
  const targetIndex = paths.indexOf(target);
  if (anchorIndex < 0 || targetIndex < 0) {
    return new Set([target]);
  }
  const start = Math.min(anchorIndex, targetIndex);
  const end = Math.max(anchorIndex, targetIndex);
  return new Set(paths.slice(start, end + 1));
}

export function selectAll(paths: readonly string[]): Set<string> {
  return new Set(paths);
}

/** Visible order, used by copy and export requests. */
export function checkedPaths<T extends { fullPath: string }>(
  items: readonly T[],
  checked: ReadonlySet<string>,
): string[] {
  return items.filter((item) => checked.has(item.fullPath)).map((item) => item.fullPath);
}

export function checkedBytes<T extends { fullPath: string; size: number; isFolder: boolean }>(
  items: readonly T[],
  checked: ReadonlySet<string>,
): number {
  return items.reduce(
    (total, item) => (checked.has(item.fullPath) && !item.isFolder ? total + item.size : total),
    0,
  );
}

export function copyPaths(paths: readonly string[]): string {
  return paths.join("\n");
}
