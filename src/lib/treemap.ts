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

/**
 * Squarified treemap layout (Bruls, Huizing, van Wijk). Produces rectangles that
 * fill the given box in proportion to their values, keeping tiles close to
 * square. Coordinates are in the same unit as width/height.
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

  const scale = (width * height) / total;
  const queues = entries.map((item) => ({ key: item.key, area: item.value * scale }));
  const output: TreemapRect[] = [];

  let x = 0;
  let y = 0;
  let w = width;
  let h = height;
  let row: { key: string; area: number }[] = [];

  const worstRatio = (candidate: { area: number }[], side: number): number => {
    if (side <= 0 || candidate.length === 0) return Number.POSITIVE_INFINITY;
    const sum = candidate.reduce((acc, item) => acc + item.area, 0);
    if (sum <= 0) return Number.POSITIVE_INFINITY;
    let max = 0;
    let min = Number.POSITIVE_INFINITY;
    for (const item of candidate) {
      if (item.area > max) max = item.area;
      if (item.area < min) min = item.area;
    }
    if (min <= 0) return Number.POSITIVE_INFINITY;
    return Math.max((side * side * max) / (sum * sum), (sum * sum) / (side * side * min));
  };

  const layoutRow = () => {
    if (row.length === 0) return;
    const sum = row.reduce((acc, item) => acc + item.area, 0);
    if (sum <= 0 || w <= 0 || h <= 0) {
      row = [];
      return;
    }
    if (w >= h) {
      const columnWidth = Math.min(sum / h, w);
      let cursor = y;
      for (const item of row) {
        const itemHeight = Math.min(item.area / columnWidth, h - (cursor - y));
        output.push({ key: item.key, x, y: cursor, w: columnWidth, h: Math.max(itemHeight, 0) });
        cursor += itemHeight;
      }
      x += columnWidth;
      w = Math.max(w - columnWidth, 0);
    } else {
      const rowHeight = Math.min(sum / w, h);
      let cursor = x;
      for (const item of row) {
        const itemWidth = Math.min(item.area / rowHeight, w - (cursor - x));
        output.push({ key: item.key, x: cursor, y, w: Math.max(itemWidth, 0), h: rowHeight });
        cursor += itemWidth;
      }
      y += rowHeight;
      h = Math.max(h - rowHeight, 0);
    }
    row = [];
  };

  for (const item of queues) {
    if (w <= 0 || h <= 0) break;
    const side = Math.min(w, h);
    if (row.length === 0) {
      row.push(item);
      continue;
    }
    const current = worstRatio(row, side);
    const withItem = worstRatio([...row, item], side);
    if (withItem <= current) {
      row.push(item);
    } else {
      layoutRow();
      row.push(item);
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
