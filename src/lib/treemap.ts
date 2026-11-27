import { Float64Arena } from "./arena";

export interface TreemapInput {
  key: string;
  value: number;
}

export interface TreemapRect {
  key: string;
  x: number;
  y: number;
  w: number;
  h: number;
}

// Global reusable coordinate & area arena for zero-allocation treemap computations.
const globalArena = new Float64Arena(4096);

/**
 * Squarified treemap layout (Bruls, Huizing, van Wijk) with memory arena allocation.
 *
 * Produces rectangles that fill the given box in proportion to their values,
 * keeping tiles close to square. Uses contiguous typed memory arena buffers
 * to eliminate GC pauses and quadratic array allocation churn.
 */
export function squarify(
  items: TreemapInput[],
  width: number,
  height: number,
): TreemapRect[] {
  if (!(width > 0) || !(height > 0)) return [];
  const entries = items
    .filter((item) => Number.isFinite(item.value) && item.value > 0)
    .sort((a, b) => b.value - a.value);
  const total = entries.reduce((sum, item) => sum + item.value, 0);
  if (total <= 0) return [];

  const n = entries.length;
  const scale = (width * height) / total;

  // Reset the memory arena for this layout pass
  globalArena.reset();
  // Allocate contiguous float64 area buffer from the memory arena
  const areas = globalArena.allocate(n);
  const keys: string[] = new Array(n);
  for (let i = 0; i < n; i += 1) {
    areas[i] = entries[i].value * scale;
    keys[i] = entries[i].key;
  }

  const output: TreemapRect[] = [];

  let x = 0;
  let y = 0;
  let w = width;
  let h = height;

  let rowStart = 0;
  let rowCount = 0;
  let rowSum = 0;
  let rowMax = 0;
  let rowMin = Number.POSITIVE_INFINITY;

  const worstRatioCandidate = (
    cSum: number,
    cMax: number,
    cMin: number,
    side: number,
  ): number => {
    if (side <= 0 || cSum <= 0 || cMin <= 0) return Number.POSITIVE_INFINITY;
    const sideSq = side * side;
    const sumSq = cSum * cSum;
    return Math.max((sideSq * cMax) / sumSq, sumSq / (sideSq * cMin));
  };

  const layoutRow = () => {
    if (rowCount === 0) return;
    if (rowSum <= 0 || w <= 0 || h <= 0) {
      rowStart += rowCount;
      rowCount = 0;
      rowSum = 0;
      rowMax = 0;
      rowMin = Number.POSITIVE_INFINITY;
      return;
    }

    if (w >= h) {
      const columnWidth = Math.min(rowSum / h, w);
      let cursor = y;
      for (let i = 0; i < rowCount; i += 1) {
        const itemArea = areas[rowStart + i];
        const itemKey = keys[rowStart + i];
        const itemHeight = Math.min(itemArea / columnWidth, h - (cursor - y));
        output.push({
          key: itemKey,
          x,
          y: cursor,
          w: columnWidth,
          h: Math.max(itemHeight, 0),
        });
        cursor += itemHeight;
      }
      x += columnWidth;
      w = Math.max(w - columnWidth, 0);
    } else {
      const rowHeight = Math.min(rowSum / w, h);
      let cursor = x;
      for (let i = 0; i < rowCount; i += 1) {
        const itemArea = areas[rowStart + i];
        const itemKey = keys[rowStart + i];
        const itemWidth = Math.min(itemArea / rowHeight, w - (cursor - x));
        output.push({
          key: itemKey,
          x: cursor,
          y,
          w: Math.max(itemWidth, 0),
          h: rowHeight,
        });
        cursor += itemWidth;
      }
      y += rowHeight;
      h = Math.max(h - rowHeight, 0);
    }

    rowStart += rowCount;
    rowCount = 0;
    rowSum = 0;
    rowMax = 0;
    rowMin = Number.POSITIVE_INFINITY;
  };

  for (let i = 0; i < n; i += 1) {
    if (w <= 0 || h <= 0) break;
    const side = Math.min(w, h);
    const itemArea = areas[i];

    if (rowCount === 0) {
      rowCount = 1;
      rowSum = itemArea;
      rowMax = itemArea;
      rowMin = itemArea;
      continue;
    }

    const currentWorst = worstRatioCandidate(rowSum, rowMax, rowMin, side);
    const candidateSum = rowSum + itemArea;
    const candidateMax = Math.max(rowMax, itemArea);
    const candidateMin = Math.min(rowMin, itemArea);
    const withItemWorst = worstRatioCandidate(candidateSum, candidateMax, candidateMin, side);

    if (withItemWorst <= currentWorst) {
      rowCount += 1;
      rowSum = candidateSum;
      rowMax = candidateMax;
      rowMin = candidateMin;
    } else {
      layoutRow();
      rowCount = 1;
      rowSum = itemArea;
      rowMax = itemArea;
      rowMin = itemArea;
    }
  }

  layoutRow();

  // Guard against accumulated floating point drift making the last tile
  // overshoot the canvas by a hair.
  return output.map((rect) => ({
    key: rect.key,
    x: clamp(rect.x, 0, width),
    y: clamp(rect.y, 0, height),
    w: Math.max(0, Math.min(rect.w, width - clamp(rect.x, 0, width))),
    h: Math.max(0, Math.min(rect.h, height - clamp(rect.y, 0, height))),
  }));
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

export interface NestedNode {
  size: number;
  allocated: number;
  kind: string;
  children: NestedNode[];
}

export interface NestedTile<T extends NestedNode> {
  node: T;
  x: number;
  y: number;
  w: number;
  h: number;
  depth: number;
  /** Index of the enclosing folder tile, or -1 for the top level. */
  parent: number;
  /** True when this folder's own contents are drawn inside it. */
  nested: boolean;
  /** Height of the folder's name strip when nested; 0 otherwise. */
  header: number;
}

export interface NestedLayoutOptions {
  /** Levels of folders drawn inside each other, counting the top level. */
  maxDepth?: number;
  /** Name strip height for nested folders. */
  header?: number;
  /** Frame between a folder's edge and its contents. */
  padding?: number;
  /** Smallest folder (in px) whose contents are still drawn. */
  minNestedWidth?: number;
  minNestedHeight?: number;
  /** Hard ceiling on tiles, so a wide tree cannot stall a frame. */
  maxTiles?: number;
}

const tileValue = (node: NestedNode) => node.size || node.allocated;

/**
 * Lays out a folder and, breadth first, the contents of every folder large
 * enough to show them, the way WizTree and WinDirStat nest folders. Each
 * nested folder keeps a frame and a name strip, so siblings stay visibly
 * separate instead of reading as one block of same-coloured tiles.
 */
export function nestedSquarify<T extends NestedNode>(
  root: T,
  width: number,
  height: number,
  options: NestedLayoutOptions = {},
): NestedTile<T>[] {
  const {
    maxDepth = 4,
    header = 17,
    padding = 3,
    minNestedWidth = 54,
    minNestedHeight = 40,
    maxTiles = 5000,
  } = options;
  const tiles: NestedTile<T>[] = [];
  if (!(width > 0) || !(height > 0)) return tiles;

  const queue: Array<{ node: T; x: number; y: number; w: number; h: number; depth: number; parent: number }> = [
    { node: root, x: 0, y: 0, w: width, h: height, depth: 0, parent: -1 },
  ];

  while (queue.length > 0 && tiles.length < maxTiles) {
    const job = queue.shift()!;
    const children = job.node.children.filter((child) => tileValue(child) > 0) as T[];
    if (children.length === 0) continue;
    const rects = squarify(
      children.map((child, index) => ({ key: String(index), value: tileValue(child) })),
      job.w,
      job.h,
    );
    for (const rect of rects) {
      if (tiles.length >= maxTiles) break;
      if (rect.w < 0.75 || rect.h < 0.75) continue;
      const child = children[Number(rect.key)];
      const index = tiles.length;
      const tile: NestedTile<T> = {
        node: child,
        x: job.x + rect.x,
        y: job.y + rect.y,
        w: rect.w,
        h: rect.h,
        depth: job.depth,
        parent: job.parent,
        nested: false,
        header: 0,
      };
      tiles.push(tile);
      const canNest =
        child.kind === "dir" &&
        child.children.length > 0 &&
        job.depth + 1 < maxDepth &&
        rect.w >= minNestedWidth &&
        rect.h >= minNestedHeight;
      if (!canNest) continue;
      const strip = rect.h >= header * 2.6 && rect.w >= 64 ? header : 0;
      const innerW = rect.w - padding * 2;
      const innerH = rect.h - strip - padding * (strip ? 1 : 2);
      if (innerW < 8 || innerH < 8) continue;
      tile.nested = true;
      tile.header = strip;
      queue.push({
        node: child,
        x: tile.x + padding,
        y: tile.y + (strip || padding),
        w: innerW,
        h: innerH,
        depth: job.depth + 1,
        parent: index,
      });
    }
  }
  return tiles;
}

/** The deepest tile under a point. Tiles are ordered parents before children. */
export function hitTest<T extends NestedNode>(tiles: NestedTile<T>[], x: number, y: number): number {
  for (let index = tiles.length - 1; index >= 0; index -= 1) {
    const tile = tiles[index];
    if (x >= tile.x && x < tile.x + tile.w && y >= tile.y && y < tile.y + tile.h) return index;
  }
  return -1;
}
