import { describe, expect, it } from "vitest";
import {
  HISTORY_LENGTH,
  formatBits,
  formatUptime,
  niceCeiling,
  recordSample,
  resourceEntries,
  seriesKeys,
  type HistoryStore,
} from "./monitorModel";
import type { LivePerformance } from "../../lib/types";

function sample(overrides: Partial<LivePerformance> = {}): LivePerformance {
  return {
    capturedAt: 0,
    intervalMs: 1000,
    warm: true,
    cpu: { utilizationPercent: 40, userPercent: 30, kernelPercent: 10, perCorePercent: [20, 60], speedMhz: 3600 },
    memory: {
      totalBytes: 100,
      availableBytes: 40,
      usedBytes: 60,
      usedPercent: 60,
      commitUsedBytes: 50,
      commitLimitBytes: 200,
      cachedBytes: 0,
      pagedPoolBytes: 0,
      nonpagedPoolBytes: 0,
      modifiedBytes: null,
      standbyBytes: null,
      freeBytes: null,
    },
    disks: [{ index: 0, volumes: ["C:"], activePercent: 12, readBytesPerSec: 10, writeBytesPerSec: 5, responseMs: 1 }],
    networks: [
      { name: "Intel Wi-Fi", kind: "wifi", receivedBytesPerSec: 1000, sentBytesPerSec: 250, linkBitsPerSec: 1e9 },
      { name: "Second Wi-Fi", kind: "wifi", receivedBytesPerSec: 0, sentBytesPerSec: 0, linkBitsPerSec: 1e8 },
    ],
    gpus: [],
    system: { processCount: 1, threadCount: 1, handleCount: 1, uptimeSeconds: 0 },
    processes: null,
    hardware: {
      cpuName: "CPU",
      baseSpeedMhz: 3000,
      sockets: 1,
      cores: 2,
      logicalProcessors: 2,
      l1CacheBytes: 0,
      l2CacheBytes: 0,
      l3CacheBytes: 0,
      installedMemoryBytes: null,
      gpuNames: [],
    },
    notes: [],
    ...overrides,
  };
}

describe("monitor model", () => {
  it("keeps a bounded history per series", () => {
    const store: HistoryStore = new Map();
    for (let index = 0; index < HISTORY_LENGTH + 15; index += 1) recordSample(store, sample());
    expect(store.get(seriesKeys.cpu)).toHaveLength(HISTORY_LENGTH);
    expect(store.get(seriesKeys.core(1))?.at(-1)).toBe(60);
    expect(store.get(seriesKeys.commit)?.at(-1)).toBe(25);
    // Network history is stored in bits per second.
    expect(store.get(seriesKeys.netTotal("Intel Wi-Fi"))?.at(-1)).toBe(10_000);
  });

  it("lists resources in Task Manager order and numbers repeated adapters", () => {
    const entries = resourceEntries(sample());
    expect(entries.map((entry) => entry.title)).toEqual(["CPU", "Memory", "Disk 0 (C:)", "Wi-Fi", "Wi-Fi 2"]);
    expect(entries[0].reading).toBe("40%");
  });

  it("formats rates, uptime and chart ceilings", () => {
    expect(formatBits(1_250_000)).toBe("1.25 Mbps");
    expect(formatBits(0)).toBe("0 Kbps");
    expect(formatUptime(90_061)).toBe("1:01:01:01");
    expect(niceCeiling(0.3, 1)).toBe(1);
    expect(niceCeiling(130)).toBe(200);
    expect(niceCeiling(4200)).toBe(5000);
  });
});
