import { describe, it, expect } from "vitest";
import { squarify, type TreemapRect } from "./treemap";

const W = 1280;
const H = 720;
const AREA = W * H;

function area(rect: TreemapRect): number {
  return rect.w * rect.h;
}

function overlapArea(a: TreemapRect, b: TreemapRect): number {
  const w = Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x);
  const h = Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y);
  return Math.max(w, 0) * Math.max(h, 0);
}

function expectInsideCanvas(rects: TreemapRect[], width = W, height = H) {
  for (const rect of rects) {
    expect(rect.x).toBeGreaterThanOrEqual(0);
    expect(rect.y).toBeGreaterThanOrEqual(0);
    expect(rect.w).toBeGreaterThanOrEqual(0);
    expect(rect.h).toBeGreaterThanOrEqual(0);
    expect(rect.x + rect.w).toBeLessThanOrEqual(width + 1e-6);
    expect(rect.y + rect.h).toBeLessThanOrEqual(height + 1e-6);
  }
}

function expectNoOverlap(rects: TreemapRect[]) {
  for (let i = 0; i < rects.length; i += 1) {
    for (let j = i + 1; j < rects.length; j += 1) {
      expect(overlapArea(rects[i], rects[j])).toBeLessThan(1e-6);
    }
  }
}

describe("squarify", () => {
  it("returns nothing without a canvas", () => {
    expect(squarify([{ key: "a", value: 1 }], 0, 100)).toEqual([]);
    expect(squarify([{ key: "a", value: 1 }], 100, 0)).toEqual([]);
    expect(squarify([{ key: "a", value: 1 }], -10, -10)).toEqual([]);
  });

  it("returns nothing when no value is positive", () => {
    expect(squarify([], W, H)).toEqual([]);
    expect(
      squarify(
        [
          { key: "zero", value: 0 },
          { key: "negative", value: -12 },
        ],
        W,
        H,
      ),
    ).toEqual([]);
  });

  it("drops zero, negative and non-finite values", () => {
    const rects = squarify(
      [
        { key: "good", value: 8 },
        { key: "zero", value: 0 },
        { key: "negative", value: -2 },
        { key: "nan", value: Number.NaN },
        { key: "infinite", value: Number.POSITIVE_INFINITY },
        { key: "also-good", value: 4 },
      ],
      W,
      H,
    );
    expect(rects.map((rect) => rect.key)).toEqual(["good", "also-good"]);
  });

  it("fills the box exactly and proportionally", () => {
    const items = [
      { key: "huge", value: 500 },
      { key: "big", value: 210 },
      { key: "medium", value: 120 },
      { key: "small", value: 48 },
      { key: "tiny", value: 12 },
      { key: "grain", value: 4 },
    ];
    const total = items.reduce((sum, item) => sum + item.value, 0);
    const rects = squarify(items, W, H);

    expect(rects).toHaveLength(items.length);
    expect(rects.reduce((sum, rect) => sum + area(rect), 0)).toBeCloseTo(AREA, 6);

    for (const item of items) {
      const rect = rects.find((candidate) => candidate.key === item.key);
      expect(rect).toBeDefined();
      const expectedShare = item.value / total;
      expect(area(rect as TreemapRect) / AREA).toBeCloseTo(expectedShare, 6);
    }

    expectInsideCanvas(rects);
    expectNoOverlap(rects);
  });

  it("fills the whole canvas with a single item", () => {
    const [rect] = squarify([{ key: "only", value: 42 }], W, H);
    expect(rect).toMatchObject({ key: "only", x: 0, y: 0 });
    expect(rect.w).toBeCloseTo(W, 6);
    expect(rect.h).toBeCloseTo(H, 6);
  });

  it("keeps equal tiles close to square", () => {
    const items = Array.from({ length: 9 }, (_, index) => ({ key: `tile-${index}`, value: 1 }));
    const rects = squarify(items, 900, 600);
    expect(rects).toHaveLength(9);
    for (const rect of rects) {
      const longer = Math.max(rect.w, rect.h);
      const shorter = Math.min(rect.w, rect.h);
      expect(shorter).toBeGreaterThan(0);
      expect(longer / shorter).toBeLessThan(8);
    }
  });

  it("keeps a 200 tile layout inside the canvas without overlapping", () => {
    let seed = 0x2f6e2b1;
    const next = () => {
      seed = (seed * 1103515245 + 12345) % 2147483648;
      return seed / 2147483648;
    };
    const items = Array.from({ length: 200 }, (_, index) => ({
      key: `item-${index}`,
      value: 1 + Math.floor(next() * 1_000_000),
    }));
    const rects = squarify(items, 1440, 900);

    expect(rects).toHaveLength(200);
    expect(new Set(rects.map((rect) => rect.key)).size).toBe(200);
    expectInsideCanvas(rects, 1440, 900);
    expectNoOverlap(rects);

    const total = items.reduce((sum, item) => sum + item.value, 0);
    expect(rects.reduce((sum, rect) => sum + area(rect), 0)).toBeCloseTo(1440 * 900, 5);
    // Every tile must keep a proportional share, even in a crowded layout.
    for (const item of items) {
      const rect = rects.find((candidate) => candidate.key === item.key) as TreemapRect;
      expect(area(rect) / (1440 * 900)).toBeCloseTo(item.value / total, 5);
    }
  });
});
