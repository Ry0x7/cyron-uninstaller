import { useEffect, useRef, useState } from "react";
import { Badge, Banner, Button, ProgressBar, Stat } from "../../components/ui";
import { IconGauge, IconStop } from "../../components/Icons";
import { api, errorMessage } from "../../lib/api";
import { formatDuration, formatPercent } from "../../lib/format";
import type { PerformanceSnapshot, ProcessUsage } from "../../lib/types";
import { analyzeSpike, type SpikePoint, type SpikeSummary } from "./spikeAnalysis";

const TARGET_SAMPLES = 20;
const SAMPLE_GAP_MS = 300;

function sleep(ms: number) {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

function topProcess(snapshot: PerformanceSnapshot): ProcessUsage | null {
  return (
    snapshot.processes
      .filter((process) => !process.isCyron)
      .reduce<ProcessUsage | null>(
        (current, process) =>
          current === null || process.cpuPercent > current.cpuPercent ? process : current,
        null,
      ) ?? null
  );
}

export function SpikeCapture() {
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState(0);
  const [points, setPoints] = useState<SpikePoint[]>([]);
  const [summary, setSummary] = useState<SpikeSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  const runToken = useRef(0);
  const peakProcess = useRef<ProcessUsage | null>(null);
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      runToken.current += 1;
    };
  }, []);

  const capture = async () => {
    if (running) return;
    const token = ++runToken.current;
    setRunning(true);
    setProgress(0);
    setPoints([]);
    setSummary(null);
    setError(null);
    peakProcess.current = null;
    let captured: SpikePoint[] = [];
    let peakSnapshot: PerformanceSnapshot | null = null;

    try {
      for (let index = 0; index < TARGET_SAMPLES; index += 1) {
        const next = await api.capturePerformanceSnapshot(false);
        if (!mounted.current || runToken.current !== token) return;
        const point: SpikePoint = {
          at: next.capturedAt,
          cpuPercent: next.cpu.utilizationPercent,
          userPercent: next.cpu.userPercent,
          kernelPercent: next.cpu.kernelPercent,
          diskBusyPercent: next.diskIo.busyPercent,
        };
        captured = [...captured, point];
        setPoints(captured);
        setProgress(index + 1);
        if (
          peakSnapshot === null ||
          next.cpu.utilizationPercent > peakSnapshot.cpu.utilizationPercent
        ) {
          peakSnapshot = next;
          peakProcess.current = topProcess(next);
        }
        if (index < TARGET_SAMPLES - 1) await sleep(SAMPLE_GAP_MS);
      }
      if (!mounted.current || runToken.current !== token) return;
      setSummary(analyzeSpike(captured, peakProcess.current));
    } catch (caught) {
      if (mounted.current && runToken.current === token) setError(errorMessage(caught));
    } finally {
      if (mounted.current && runToken.current === token) setRunning(false);
    }
  };

  const stop = () => {
    runToken.current += 1;
    setRunning(false);
    setSummary(analyzeSpike(points, peakProcess.current));
  };

  const maxCpu = Math.max(20, ...points.map((point) => point.cpuPercent));
  const chartLabel = summary
    ? `${summary.title}. Peak CPU ${formatPercent(summary.peakCpuPercent / 100)}, peak kernel ${formatPercent(summary.peakKernelPercent / 100)}.`
    : "No spike capture yet.";

  return (
    <div className="spike">
      <div className="spike__head">
        <div>
          <span className="perf-label">20-second local capture</span>
          <h3>Catch an intermittent spike</h3>
          <p>
            A single 700 ms sample can miss a short lag. This capture takes about 20 one-second
            readings and keeps the peak, not just the latest value. Nothing is written to disk.
          </p>
        </div>
        {running ? (
          <Button variant="danger" icon={<IconStop size={14} />} onClick={stop}>
            Stop capture
          </Button>
        ) : (
          <Button variant="secondary" icon={<IconGauge size={14} />} onClick={() => void capture()}>
            {points.length > 0 ? "Capture again" : "Record 20 seconds"}
          </Button>
        )}
      </div>

      {error && <Banner tone="danger">{error}</Banner>}

      {running && (
        <div className="spike__progress">
          <ProgressBar
            value={progress / TARGET_SAMPLES}
            tone="accent"
            label="Spike capture progress"
          />
          <span className="small faint">
            {progress} of {TARGET_SAMPLES} readings · {formatDuration(progress * 1_000)}
          </span>
        </div>
      )}

      {points.length > 0 && (
        <div className="spike__chart" role="img" aria-label={chartLabel}>
          {points.map((point, index) => (
            <span
              key={`${point.at}-${index}`}
              className="spike__bar"
              title={`${index + 1}: ${formatPercent(point.cpuPercent / 100)} CPU · ${formatPercent(point.kernelPercent / 100)} kernel`}
              style={{ height: `${Math.max(3, (point.cpuPercent / maxCpu) * 100)}%` }}
            >
              <span
                className="spike__bar-kernel"
                style={{
                  height: `${Math.min(100, (point.kernelPercent / Math.max(point.cpuPercent, 1)) * 100)}%`,
                }}
              />
            </span>
          ))}
        </div>
      )}

      {summary && (
        <>
          <div className="spike__stats">
            <Stat
              label="Peak CPU"
              value={formatPercent(summary.peakCpuPercent / 100)}
              meta={`at +${formatDuration(summary.peakOffsetMs)}`}
            />
            <Stat
              label="Peak kernel"
              value={formatPercent(summary.peakKernelPercent / 100)}
              meta={`${formatPercent(summary.kernelShare)} of the peak`}
            />
            <Stat
              label="Peak disk"
              value={
                summary.peakDiskPercent === null
                  ? "Unavailable"
                  : formatPercent(summary.peakDiskPercent / 100)
              }
              meta="Physical disk busy"
            />
            <Stat
              label="Largest process"
              value={summary.peakProcess?.name ?? "None measured"}
              meta={
                summary.peakProcess
                  ? `${formatPercent(summary.peakProcess.cpuPercent / 100)} at the peak`
                  : "No usable process counter"
              }
            />
          </div>
          <Banner tone={summary.verdict === "kernel" || summary.verdict === "disk" ? "caution" : "info"}>
            <strong>{summary.title}.</strong> {summary.detail}
          </Banner>
        </>
      )}

      {summary && (
        <div className="spike__legend">
          <Badge tone="neutral">Total CPU</Badge>
          <span className="spike__legend-kernel" aria-hidden="true" />
          <span className="small faint">Kernel portion</span>
        </div>
      )}
    </div>
  );
}
