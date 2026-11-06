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
