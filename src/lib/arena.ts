/**
 * Memory Arena Allocator
 *
 * Provides region-based / arena memory management for high-frequency,
 * repetitive calculations (treemap layout, tree view row virtualization,
 * and coordinate calculations).
 *
 * Instead of generating thousands of short-lived heap objects per frame or per
 * scan node, allocations are carved out of reusable contiguous typed buffers
 * or object blocks and reset all at once in O(1).
 */

export interface ArenaAllocation<T> {
  data: T[];
  count: number;
}

/**
 * Contiguous typed numeric arena for 2D layout coordinates and bounds calculations
 * (x, y, w, h, area, min, max, sum).
 */
export class Float64Arena {
  private buffer: Float64Array;
  private offset: number;

  constructor(initialCapacity = 16384) {
    this.buffer = new Float64Array(initialCapacity);
    this.offset = 0;
  }

  /**
   * Resets the arena pointer to the beginning. Does not reallocate or free buffer memory.
   */
  reset(): void {
    this.offset = 0;
  }

  /**
   * Current allocated size in 64-bit floats.
   */
  get size(): number {
    return this.offset;
  }

  /**
   * Total capacity in 64-bit floats.
   */
  get capacity(): number {
    return this.buffer.length;
  }

  /**
   * Allocates a contiguous sub-array of the requested length.
   * Doubles capacity if required.
   */
  allocate(length: number): Float64Array {
    if (this.offset + length > this.buffer.length) {
      const nextCapacity = Math.max(this.buffer.length * 2, this.offset + length + 1024);
      const nextBuffer = new Float64Array(nextCapacity);
      nextBuffer.set(this.buffer);
      this.buffer = nextBuffer;
    }
    const start = this.offset;
    this.offset += length;
    return this.buffer.subarray(start, this.offset);
  }

  /**
   * Pushes a single float into the arena.
   */
  push(value: number): number {
    if (this.offset >= this.buffer.length) {
      const nextCapacity = this.buffer.length * 2;
      const nextBuffer = new Float64Array(nextCapacity);
      nextBuffer.set(this.buffer);
      this.buffer = nextBuffer;
    }
    const idx = this.offset;
    this.buffer[idx] = value;
    this.offset += 1;
    return idx;
  }

  /**
   * Reads a float at the given arena index.
   */
  get(index: number): number {
    return this.buffer[index];
  }

  /**
   * Writes a float at the given arena index.
   */
  set(index: number, value: number): void {
    this.buffer[index] = value;
  }
}

/**
 * Generic reusable object arena pool. Carves out pre-allocated objects without
 * invoking GC garbage collection churn on rapid re-renders or calculations.
 */
export class ObjectArena<T> {
  private pool: T[];
  private factory: () => T;
  private count: number;

  constructor(factory: () => T, initialCapacity = 512) {
    this.factory = factory;
    this.count = 0;
    this.pool = new Array(initialCapacity);
    for (let i = 0; i < initialCapacity; i += 1) {
      this.pool[i] = factory();
    }
  }

  /**
   * Resets the allocation count in O(1).
   */
  reset(): void {
    this.count = 0;
  }

  /**
   * Current number of acquired items from the arena.
   */
  get size(): number {
    return this.count;
  }

  /**
   * Total allocated capacity in the pool.
   */
  get capacity(): number {
    return this.pool.length;
  }

  /**
   * Borrows an object from the arena, expanding capacity on demand.
   */
  borrow(): T {
    if (this.count >= this.pool.length) {
      const additional = Math.max(this.pool.length, 256);
      for (let i = 0; i < additional; i += 1) {
        this.pool.push(this.factory());
      }
    }
    const item = this.pool[this.count];
    this.count += 1;
    return item;
  }

  /**
   * Returns a slice of the actively borrowed items.
   */
  active(): T[] {
    return this.pool.slice(0, this.count);
  }
}
