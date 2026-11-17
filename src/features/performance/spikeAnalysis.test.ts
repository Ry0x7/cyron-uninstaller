import { describe, expect, it } from "vitest";
import type { ProcessUsage } from "../../lib/types";
import { analyzeSpike, type SpikePoint } from "./spikeAnalysis";

function point(overrides: Partial<SpikePoint> = {}): SpikePoint {
  return {
    at: 1_700_000_000,
    cpuPercent: 20,
    userPercent: 10,
    kernelPercent: 10,
    diskBusyPercent: 5,
    ...overrides,
  };
}

function processUsage(overrides: Partial<ProcessUsage> = {}): ProcessUsage {
  return {
    pid: 42,
    name: "game.exe",
    path: "C:\\Games\\game.exe",
    cpuPercent: 5,
    workingSetBytes: 100 * 1024 * 1024,
    privateBytes: 50 * 1024 * 1024,
    ioReadBytesPerSec: 0,
    ioWriteBytesPerSec: 0,
    isCritical: false,
    isCyron: false,
    ...overrides,
  };
}

describe("short-spike analysis", () => {
  it("identifies kernel-dominated time without naming a driver", () => {
    const summary = analyzeSpike(
      [
        point({ at: 100, cpuPercent: 15, kernelPercent: 8 }),
        point({ at: 105, cpuPercent: 70, userPercent: 8, kernelPercent: 62 }),
      ],
      processUsage({ cpuPercent: 6 }),
    );
    expect(summary?.verdict).toBe("kernel");
    expect(summary?.peakKernelPercent).toBe(62);
    expect(summary?.peakOffsetMs).toBe(5_000);
    expect(summary?.title).toContain("Kernel/driver");
    expect(summary?.detail).toContain("does not attribute");
  });

  it("identifies a process-dominated spike", () => {
    const summary = analyzeSpike(
      [
        point({ at: 100, cpuPercent: 10 }),
        point({ at: 104, cpuPercent: 55, userPercent: 50, kernelPercent: 5 }),
      ],
      processUsage({ cpuPercent: 48 }),
    );
    expect(summary?.verdict).toBe("process");
    expect(summary?.title).toContain("game.exe");
  });

  it("identifies disk pressure when CPU is not the dominant signal", () => {
    const summary = analyzeSpike(
      [
        point({ at: 100, cpuPercent: 5, diskBusyPercent: 10 }),
        point({ at: 109, cpuPercent: 12, kernelPercent: 4, diskBusyPercent: 92 }),
      ],
      processUsage({ cpuPercent: 2 }),
    );
    expect(summary?.verdict).toBe("disk");
    expect(summary?.peakDiskPercent).toBe(92);
  });

  it("returns an idle result when the capture missed the slowdown", () => {
    const summary = analyzeSpike([point({ cpuPercent: 4, kernelPercent: 1, diskBusyPercent: 3 })], null);
    expect(summary?.verdict).toBe("idle");
    expect(summary?.peakOffsetMs).toBe(0);
  });

  it("handles an empty capture", () => {
    expect(analyzeSpike([], null)).toBeNull();
  });
});
