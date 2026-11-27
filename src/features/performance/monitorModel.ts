import type { LivePerformance } from "../../lib/types";
import { formatBytes } from "../../lib/format";

/** Seconds of history kept per series; the monitor polls about once a second. */
export const HISTORY_LENGTH = 60;

export type HistoryStore = Map<string, number[]>;

export type ResourceKind = "cpu" | "memory" | "disk" | "network" | "gpu";

export interface ResourceEntry {
  id: string;
  kind: ResourceKind;
  title: string;
  /** Hardware or volume detail shown under the title. */
  detail: string;
  /** Short live reading for the sidebar, e.g. "12%" or "↓ 1.2 MB/s". */
  reading: string;
  /** Series drawn as the sidebar sparkline. */
  seriesKey: string;
  /** Fixed 0–100 scale, or scaled to the visible maximum. */
  scale: "percent" | "auto";
}

export const seriesKeys = {
  cpu: "cpu",
  cpuKernel: "cpu.kernel",
  core: (index: number) => `cpu.core.${index}`,
  memory: "memory",
  commit: "memory.commit",
  diskActive: (index: number) => `disk.${index}.active`,
  diskRead: (index: number) => `disk.${index}.read`,
  diskWrite: (index: number) => `disk.${index}.write`,
  netReceive: (name: string) => `net.${name}.rx`,
  netSend: (name: string) => `net.${name}.tx`,
  netTotal: (name: string) => `net.${name}.total`,
  gpu: (luid: string) => `gpu.${luid}`,
  gpuEngine: (luid: string, engine: string) => `gpu.${luid}.engine.${engine}`,
  gpuDedicated: (luid: string) => `gpu.${luid}.dedicated`,
};

function push(store: HistoryStore, key: string, value: number) {
  const series = store.get(key) ?? [];
  series.push(Number.isFinite(value) ? value : 0);
  if (series.length > HISTORY_LENGTH) series.splice(0, series.length - HISTORY_LENGTH);
  store.set(key, series);
}

/** Appends one live sample to every series it carries. Cold samples are skipped for rates. */
export function recordSample(store: HistoryStore, live: LivePerformance): void {
  push(store, seriesKeys.cpu, live.cpu.utilizationPercent);
  push(store, seriesKeys.cpuKernel, live.cpu.kernelPercent);
  live.cpu.perCorePercent.forEach((value, index) => push(store, seriesKeys.core(index), value));
  push(store, seriesKeys.memory, live.memory.usedPercent);
  push(
    store,
    seriesKeys.commit,
    live.memory.commitLimitBytes > 0
      ? (live.memory.commitUsedBytes / live.memory.commitLimitBytes) * 100
      : 0,
  );
  for (const disk of live.disks) {
    push(store, seriesKeys.diskActive(disk.index), disk.activePercent);
    push(store, seriesKeys.diskRead(disk.index), disk.readBytesPerSec);
    push(store, seriesKeys.diskWrite(disk.index), disk.writeBytesPerSec);
  }
  for (const adapter of live.networks) {
    push(store, seriesKeys.netReceive(adapter.name), adapter.receivedBytesPerSec * 8);
    push(store, seriesKeys.netSend(adapter.name), adapter.sentBytesPerSec * 8);
    push(
      store,
      seriesKeys.netTotal(adapter.name),
      (adapter.receivedBytesPerSec + adapter.sentBytesPerSec) * 8,
    );
  }
  for (const gpu of live.gpus) {
    push(store, seriesKeys.gpu(gpu.luid), gpu.utilizationPercent);
    push(store, seriesKeys.gpuDedicated(gpu.luid), gpu.dedicatedUsedBytes);
    for (const engine of gpu.engines) {
      push(store, seriesKeys.gpuEngine(gpu.luid, engine.name), engine.percent);
    }
  }
}

export function formatRate(bytesPerSec: number): string {
  if (!Number.isFinite(bytesPerSec) || bytesPerSec < 1) return "0 KB/s";
  if (bytesPerSec < 1024) return `${Math.round(bytesPerSec)} B/s`;
  return `${formatBytes(bytesPerSec)}/s`;
}

const BIT_UNITS = ["bps", "Kbps", "Mbps", "Gbps", "Tbps"];

/** Decimal network units, the way adapters and Task Manager report them. */
export function formatBits(bitsPerSec: number): string {
  if (!Number.isFinite(bitsPerSec) || bitsPerSec <= 0) return "0 Kbps";
  let value = bitsPerSec;
  let unit = 0;
  while (value >= 1000 && unit < BIT_UNITS.length - 1) {
    value /= 1000;
    unit += 1;
  }
  const digits = unit === 0 ? 0 : value >= 100 ? 0 : value >= 10 ? 1 : 2;
  return `${Number(value.toFixed(digits))} ${BIT_UNITS[unit]}`;
}

export function formatSpeed(mhz: number | null | undefined): string {
  if (!mhz || !Number.isFinite(mhz) || mhz <= 0) return "—";
  return mhz >= 1000 ? `${(mhz / 1000).toFixed(2)} GHz` : `${Math.round(mhz)} MHz`;
}

export function formatUptime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "—";
  const days = Math.floor(seconds / 86_400);
  const hours = Math.floor((seconds % 86_400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = Math.floor(seconds % 60);
  const clock = [hours, minutes, secs].map((part) => String(part).padStart(2, "0")).join(":");
  return days > 0 ? `${days}:${clock}` : clock;
}

export function formatPercentValue(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "0%";
  return value < 1 ? `${value.toFixed(1)}%` : `${Math.round(value)}%`;
}

/** Rounds a chart maximum up to 1, 2 or 5 × 10ⁿ so the axis reads cleanly. */
export function niceCeiling(value: number, floor = 1): number {
  const target = Math.max(value, floor);
  const magnitude = 10 ** Math.floor(Math.log10(target));
  for (const step of [1, 2, 5, 10]) {
    if (step * magnitude >= target) return step * magnitude;
  }
  return 10 * magnitude;
}

export function diskTitle(index: number, volumes: string[]): string {
  return volumes.length > 0 ? `Disk ${index} (${volumes.join(" ")})` : `Disk ${index}`;
}

export function networkTitle(kind: string): string {
  if (kind === "wifi") return "Wi-Fi";
  if (kind === "ethernet") return "Ethernet";
  return "Network";
}

/** The sidebar entries, in Task Manager's order: CPU, Memory, disks, networks, GPUs. */
export function resourceEntries(live: LivePerformance): ResourceEntry[] {
  const entries: ResourceEntry[] = [
    {
      id: "cpu",
      kind: "cpu",
      title: "CPU",
      detail: live.cpu.speedMhz
        ? `${formatPercentValue(live.cpu.utilizationPercent)}  ${formatSpeed(live.cpu.speedMhz)}`
        : formatPercentValue(live.cpu.utilizationPercent),
      reading: formatPercentValue(live.cpu.utilizationPercent),
      seriesKey: seriesKeys.cpu,
      scale: "percent",
    },
    {
      id: "memory",
      kind: "memory",
      title: "Memory",
      detail: `${formatBytes(live.memory.usedBytes)} of ${formatBytes(live.memory.totalBytes)} (${formatPercentValue(live.memory.usedPercent)})`,
      reading: formatPercentValue(live.memory.usedPercent),
      seriesKey: seriesKeys.memory,
      scale: "percent",
    },
  ];
  for (const disk of live.disks) {
    entries.push({
      id: `disk:${disk.index}`,
      kind: "disk",
      title: diskTitle(disk.index, disk.volumes),
      detail: `${formatPercentValue(disk.activePercent)} active`,
      reading: formatPercentValue(disk.activePercent),
      seriesKey: seriesKeys.diskActive(disk.index),
      scale: "percent",
    });
  }
  const kindCounts = new Map<string, number>();
  for (const adapter of live.networks) {
    const title = networkTitle(adapter.kind);
    const seen = kindCounts.get(title) ?? 0;
    kindCounts.set(title, seen + 1);
    entries.push({
      id: `net:${adapter.name}`,
      kind: "network",
      title: seen > 0 ? `${title} ${seen + 1}` : title,
      detail: `↑ ${formatBits(adapter.sentBytesPerSec * 8)}  ↓ ${formatBits(adapter.receivedBytesPerSec * 8)}`,
      reading: formatBits((adapter.sentBytesPerSec + adapter.receivedBytesPerSec) * 8),
      seriesKey: seriesKeys.netTotal(adapter.name),
      scale: "auto",
    });
  }
  for (const gpu of live.gpus) {
    entries.push({
      id: `gpu:${gpu.luid}`,
      kind: "gpu",
      title: `GPU ${gpu.index}`,
      detail: gpu.name ?? `${formatPercentValue(gpu.utilizationPercent)} busiest engine`,
      reading: formatPercentValue(gpu.utilizationPercent),
      seriesKey: seriesKeys.gpu(gpu.luid),
      scale: "percent",
    });
  }
  return entries;
}
