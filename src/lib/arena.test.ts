import { describe, expect, it } from "vitest";
import { Float64Arena, ObjectArena } from "./arena";

describe("Float64Arena", () => {
  it("allocates contiguous slices without memory leaks", () => {
    const arena = new Float64Arena(16);
    expect(arena.size).toBe(0);
    expect(arena.capacity).toBe(16);

    const slice1 = arena.allocate(4);
    slice1[0] = 1.5;
    slice1[1] = 2.5;
    slice1[2] = 3.5;
    slice1[3] = 4.5;
    expect(arena.size).toBe(4);

    const slice2 = arena.allocate(4);
    slice2[0] = 10.0;
    expect(arena.size).toBe(8);
    expect(slice1[0]).toBe(1.5);
    expect(slice2[0]).toBe(10.0);

    arena.reset();
    expect(arena.size).toBe(0);
    expect(arena.capacity).toBe(16);
  });

  it("grows buffer dynamically when requested capacity exceeds initial capacity", () => {
    const arena = new Float64Arena(8);
    const large = arena.allocate(32);
    expect(large.length).toBe(32);
    expect(arena.capacity).toBeGreaterThanOrEqual(32);
    expect(arena.size).toBe(32);
  });

  it("pushes and reads floats accurately", () => {
    const arena = new Float64Arena(4);
    const i1 = arena.push(42.2);
    const i2 = arena.push(99.9);
    expect(i1).toBe(0);
    expect(i2).toBe(1);
    expect(arena.get(0)).toBeCloseTo(42.2, 5);
    expect(arena.get(1)).toBeCloseTo(99.9, 5);

    arena.set(0, 77.7);
    expect(arena.get(0)).toBeCloseTo(77.7, 5);
  });
});

describe("ObjectArena", () => {
  interface NodeHolder {
    id: number;
    name: string;
    value: number;
  }

  it("pre-allocates object instances and borrows without reallocation", () => {
    let creations = 0;
    const arena = new ObjectArena<NodeHolder>(() => {
      creations += 1;
      return { id: 0, name: "", value: 0 };
    }, 10);

    expect(creations).toBe(10);
    expect(arena.size).toBe(0);

    const item1 = arena.borrow();
    item1.id = 1;
    item1.name = "First";
    item1.value = 100;

    const item2 = arena.borrow();
    item2.id = 2;
    item2.name = "Second";
    item2.value = 200;

    expect(arena.size).toBe(2);
    expect(creations).toBe(10); // Zero additional allocations

    arena.reset();
    expect(arena.size).toBe(0);

    // Reuse existing instances
    const reused = arena.borrow();
    expect(reused.id).toBe(1); // Old contents remain, ready to be overwritten
    expect(creations).toBe(10);
  });

  it("expands capacity cleanly when borrowing beyond initial size", () => {
    const arena = new ObjectArena(() => ({ x: 0 }), 5);
    for (let i = 0; i < 20; i += 1) {
      const obj = arena.borrow();
      obj.x = i;
    }
    expect(arena.size).toBe(20);
    expect(arena.capacity).toBeGreaterThanOrEqual(20);
    expect(arena.active()).toHaveLength(20);
  });
});
