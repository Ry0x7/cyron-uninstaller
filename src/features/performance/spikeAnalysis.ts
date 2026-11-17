import type { ProcessUsage } from "../../lib/types";

export interface SpikePoint {
  /** Unix seconds when the snapshot was captured. */
  at: number;
  cpuPercent: number;
  userPercent: number;
  kernelPercent: number;
  diskBusyPercent: number | null;
}

export type SpikeVerdict = "kernel" | "process" | "disk" | "mixed" | "idle";

export interface SpikeSummary {
  peakCpuPercent: number;
  peakKernelPercent: number;
  peakDiskPercent: number | null;
  peakOffsetMs: number;
  peakProcess: ProcessUsage | null;
  kernelShare: number;
  verdict: SpikeVerdict;
  title: string;
  detail: string;
}

function formatPercentValue(value: number): string {
  return `${value >= 10 ? Math.round(value) : value.toFixed(1)}%`;
}

/**
 * Summarizes a burst capture without inventing a cause. A high kernel share is
 * reported as kernel-mode activity, not as a named driver: the sampled
 * counters cannot attribute DPC/ISR time to one component.
 */
export function analyzeSpike(
  points: SpikePoint[],
  peakProcess: ProcessUsage | null,
): SpikeSummary | null {
  if (points.length === 0) return null;
  const peak = points.reduce((current, point) =>
    point.cpuPercent > current.cpuPercent ? point : current,
  );
  const kernelPoint = points.reduce((current, point) =>
    point.kernelPercent > current.kernelPercent ? point : current,
  );
  const peakKernel = kernelPoint.kernelPercent;
  const peakDisk = points.reduce<number | null>((current, point) => {
    if (point.diskBusyPercent === null) return current;
    return current === null ? point.diskBusyPercent : Math.max(current, point.diskBusyPercent);
  }, null);
  const kernelShare = kernelPoint.cpuPercent > 0 ? kernelPoint.kernelPercent / kernelPoint.cpuPercent : 0;
  const peakOffsetMs = Math.max(0, (peak.at - points[0].at) * 1000);
  const processCpu = peakProcess?.cpuPercent ?? 0;

  let verdict: SpikeVerdict = "mixed";
  let title = "The spike had no single dominant source";
  let detail =
    "The capture reached its peak without one process, kernel share or physical disk crossing a clear threshold. Reproduce the lag again or use the trace if it continues.";

  if (peak.cpuPercent < 20 && (peakDisk ?? 0) < 30) {
    verdict = "idle";
    title = "No material spike was captured";
    detail =
      "CPU and disk stayed low for the whole capture. The lag may happen between samples, or it may not be a CPU/disk event. Try capturing while the slowdown is visible.";
  } else if (peakKernel >= 20 && kernelShare >= 0.6) {
    verdict = "kernel";
    title = "Kernel/driver activity dominated the spike";
    detail = `${formatPercentValue(peakKernel)} of the machine was kernel time at the peak; measured application processes accounted for ${formatPercentValue(processCpu)}. Drivers, interrupt/DPC handling, storage or security filters are plausible kernel-mode sources. Windows does not attribute that time to one driver in this sample.`;
  } else if (peakProcess && processCpu >= 15) {
    verdict = "process";
    title = `${peakProcess.name} accounted for most of the spike`;
    detail = `${peakProcess.name} used ${formatPercentValue(processCpu)} while total CPU reached ${formatPercentValue(peak.cpuPercent)}. Check whether the slowdown tracks this process before changing system settings.`;
  } else if ((peakDisk ?? 0) >= 70) {
    verdict = "disk";
    title = "The physical disk was the clearest pressure point";
    detail = `The physical disk reached ${formatPercentValue(peakDisk ?? 0)} busy during the capture. A process may still be waiting on storage without high CPU; use the trace to inspect file and stack activity.`;
  }

  return {
    peakCpuPercent: peak.cpuPercent,
    peakKernelPercent: peakKernel,
    peakDiskPercent: peakDisk,
    peakOffsetMs,
    peakProcess,
    kernelShare,
    verdict,
    title,
    detail,
  };
}
