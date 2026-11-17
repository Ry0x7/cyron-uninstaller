import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import {
  Badge,
  Banner,
  Button,
  EmptyState,
  Panel,
  ProgressBar,
  SectionHeader,
  Segmented,
  Stat,
  Toggle,
  ViewHeader,
  cx,
} from "../../components/ui";
import {
  IconAlert,
  IconCheck,
  IconClock,
  IconExternal,
  IconGauge,
  IconInfo,
  IconRefresh,
  IconShield,
  IconStop,
} from "../../components/Icons";
import { api, errorMessage } from "../../lib/api";
import {
  displayPath,
  formatBytes,
  formatDate,
  formatDateTime,
  formatDuration,
  formatPercent,
  plural,
  relativeTime,
  truncateMiddle,
} from "../../lib/format";
import type {
  PerformanceBaseline,
  PerformanceRecommendation,
  PerformanceSnapshot,
  PerformanceTraceStatus,
  ProcessUsage,
  ScheduledStartupTask,
  StartupEvidence,
  SystemEvent,
  ViewId,
} from "../../lib/types";
import { SpikeCapture } from "./SpikeCapture";
import "./performance.css";

type SnapshotStatus = "loading" | "ready" | "error";
type ProcessSort = "cpu" | "memory" | "io";
type StartupTab = "apps" | "tasks";
type GuideId = "all" | "startup" | "app" | "kernel" | "disk" | "fan";

const GUIDES: { id: GuideId; label: string; title: string; detail: string }[] = [
  {
    id: "all",
    label: "Everything feels slow",
    title: "Start with the pressure that is actually present",
    detail:
      "Cyron looks at total CPU, available memory, commit charge, physical-disk pressure and free capacity. A short sample cannot explain every slowdown, so anything it cannot support is left out.",
  },
  {
    id: "startup",
    label: "Startup takes forever",
    title: "Separate what starts automatically from what Windows measured",
    detail:
      "Windows records per-app boot cost in its Diagnostics-Performance log, which is administrator-only. Without that access Cyron shows the real startup inventory and live usage, but does not invent an impact score.",
  },
  {
    id: "app",
    label: "One app stutters",
    title: "Find the process that is using the machine while you wait",
    detail:
      "The process table shows the live sample, not a lifetime average. Reproduce the lag, watch CPU and I/O, then compare the same app while idle.",
  },
  {
    id: "kernel",
    label: "CPU spikes / kernel time",
    title: "Separate kernel and driver work from applications",
    detail:
      "Kernel time has no process row. If it dominates while measured apps are quiet, the cause is kernel-mode work such as a driver, DPC/interrupt handling, storage or a security filter. Use the 20-second capture and the recent driver/event evidence below before changing anything.",
  },
  {
    id: "disk",
    label: "Disk is almost full",
    title: "Check capacity before blaming speed",
    detail:
      "Windows needs free space for updates, temporary files and the page file. Storage pressure is measured directly; reclaiming space is not automatically a speed improvement.",
  },
  {
    id: "fan",
    label: "The fan is loud",
    title: "Look for sustained CPU work, not a single spike",
    detail:
      "A 700 ms sample can show sustained processor work but cannot measure temperature or fan speed. Cyron does not change power plans, services or processor settings.",
  },
];

function guideRecommendations(
  recommendations: PerformanceRecommendation[],
  guide: GuideId,
): PerformanceRecommendation[] {
  switch (guide) {
    case "startup":
      return recommendations.filter((item) => item.category === "startup");
    case "app":
      return recommendations.filter((item) => item.category === "cpu" || item.category === "disk");
    case "kernel":
      return recommendations.filter((item) => item.category === "system" || item.category === "cpu");
    case "disk":
      return recommendations.filter((item) => item.category === "storage" || item.category === "disk");
    case "fan":
      return recommendations.filter((item) => item.category === "cpu");
    default:
      return recommendations;
  }
}

function ioTotal(process: ProcessUsage): number {
  return process.ioReadBytesPerSec + process.ioWriteBytesPerSec;
}

function toneForPercent(value: number, warning: number): "safe" | "caution" | "danger" {
  if (value >= warning + 10) return "danger";
  if (value >= warning) return "caution";
  return "safe";
}

function bootObservedAt(snapshot: PerformanceSnapshot): number | null {
  const raw = snapshot.startup.boot?.observedAt;
  if (!raw) return null;
  const parsed = Date.parse(raw);
  return Number.isFinite(parsed) ? parsed / 1000 : null;
}

function eventTimestamp(value: string): number | null {
  const parsed = Date.parse(value);
  return Number.isFinite(parsed) ? parsed / 1000 : null;
}

function driverAgeDays(date: string | null): number | null {
  if (!date) return null;
  const parsed = Date.parse(`${date}T00:00:00`);
  if (!Number.isFinite(parsed)) return null;
  return Math.max(0, Math.floor((Date.now() - parsed) / 86_400_000));
}

function eventTone(level: SystemEvent["level"]): "danger" | "caution" | "neutral" {
  if (level === "critical") return "danger";
  if (level === "error") return "caution";
  return "neutral";
}

function evidenceSourceLabel(source: StartupEvidence["evidenceSource"]): string {
  if (source === "diagnostics-performance") return "Windows boot event";
  if (source === "live-process") return "Live process";
  return "Inventory";
}

function scheduleTriggerLabel(task: ScheduledStartupTask): string {
  return task.triggers.map((trigger) => trigger[0].toUpperCase() + trigger.slice(1)).join(" + ");
}

function delayLabel(task: ScheduledStartupTask): string {
  if (!task.delay) return "No delay";
  const match = /^PT(?:(\d+)H)?(?:(\d+)M)?(?:(\d+)S)?$/.exec(task.delay);
  if (!match) return task.delay;
  const hours = Number(match[1] ?? 0);
  const minutes = Number(match[2] ?? 0);
  const seconds = Number(match[3] ?? 0);
  const parts = [hours ? `${hours}h` : "", minutes ? `${minutes}m` : "", seconds ? `${seconds}s` : ""]
    .filter(Boolean)
    .join(" ");
  return parts ? `Waits ${parts}` : "No delay";
}

function recommendationTone(severity: PerformanceRecommendation["severity"]): "caution" | "neutral" | "safe" {
  if (severity === "attention") return "caution";
  if (severity === "review") return "neutral";
  return "safe";
}

export function Performance({ onNavigate }: { onNavigate: (view: ViewId) => void }) {
  const [snapshot, setSnapshot] = useState<PerformanceSnapshot | null>(null);
  const snapshotRef = useRef<PerformanceSnapshot | null>(null);
  const inFlight = useRef(false);
  const rootRef = useRef<HTMLDivElement>(null);

  const [status, setStatus] = useState<SnapshotStatus>("loading");
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const [guide, setGuide] = useState<GuideId>("all");
  const [processSort, setProcessSort] = useState<ProcessSort>("cpu");
  const [showAllProcesses, setShowAllProcesses] = useState(false);
  const [startupTab, setStartupTab] = useState<StartupTab>("apps");
  const [showMicrosoftTasks, setShowMicrosoftTasks] = useState(false);
  const [togglingId, setTogglingId] = useState<string | null>(null);

  const [baselines, setBaselines] = useState<PerformanceBaseline[]>([]);
  const [baselineLabel, setBaselineLabel] = useState("");
  const [recordingBaseline, setRecordingBaseline] = useState(false);
  const [baselineMessage, setBaselineMessage] = useState<string | null>(null);
  const [baselineError, setBaselineError] = useState<string | null>(null);

  const [traceOverride, setTraceOverride] = useState<PerformanceTraceStatus | null>(null);
  const [traceBusy, setTraceBusy] = useState(false);

  const refresh = useCallback(async (silent = false, forceStartup = !silent) => {
    if (inFlight.current) return;
    inFlight.current = true;
    if (!silent) {
      setStatus((current) => (snapshotRef.current ? current : "loading"));
    }
    setRefreshing(true);
    try {
      const next = await api.capturePerformanceSnapshot(forceStartup);
      snapshotRef.current = next;
      setSnapshot(next);
      setTraceOverride((previous) =>
        previous && !previous.active && previous.outputPath && !next.trace.outputPath
          ? previous
          : next.trace,
      );
      setStatus("ready");
      setError(null);
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus((current) => (snapshotRef.current ? current : "error"));
    } finally {
      inFlight.current = false;
      setRefreshing(false);
    }
  }, []);

  const loadBaselines = useCallback(async () => {
    try {
      setBaselines(await api.listPerformanceBaselines());
    } catch {
      // A missing baseline file is not a diagnostics failure.
    }
  }, []);

  useEffect(() => {
    void refresh(false);
    void loadBaselines();
  }, [refresh, loadBaselines]);

  const [live, setLive] = useState(true);
  useEffect(() => {
    if (!live) return;
    const timer = window.setInterval(() => {
      if (document.visibilityState !== "visible") return;
      const slot = rootRef.current?.closest(".view-slot") as HTMLElement | null;
      if (slot?.hidden) return;
      if (snapshotRef.current?.trace.active) return;
      void refresh(true);
    }, 10_000);
    return () => window.clearInterval(timer);
  }, [live, refresh]);

  const trace = traceOverride ?? snapshot?.trace ?? null;
  const guides = useMemo(() => GUIDES, []);
  const selectGuide = (next: GuideId) => {
    setGuide(next);
    setNotice(null);
  };

  const visibleRecommendations = useMemo(
    () => guideRecommendations(snapshot?.recommendations ?? [], guide),
    [snapshot, guide],
  );

  const orderedProcesses = useMemo(() => {
    const list = [...(snapshot?.processes ?? [])];
    list.sort((left, right) => {
      if (processSort === "memory") return right.workingSetBytes - left.workingSetBytes;
      if (processSort === "io") return ioTotal(right) - ioTotal(left);
      return right.cpuPercent - left.cpuPercent;
    });
    return list;
  }, [snapshot, processSort]);

  const visibleProcesses = showAllProcesses ? orderedProcesses : orderedProcesses.slice(0, 12);

  const visibleTasks = useMemo(
    () =>
      (snapshot?.startup.scheduledTasks ?? []).filter(
        (task) => showMicrosoftTasks || !task.isMicrosoft,
      ),
    [snapshot, showMicrosoftTasks],
  );

  const lowestVolume = useMemo(() => {
    const volumes = (snapshot?.volumes ?? []).filter(
      (volume) => volume.total > 0 && volume.kind === "fixed",
    );
    return volumes.sort(
      (left, right) => left.free / left.total - right.free / right.total,
    )[0];
  }, [snapshot]);

  const toggleStartup = useCallback(
    async (item: StartupEvidence) => {
      if (!item.canToggle || item.isProtected) return;
      const target = !item.enabled;
      const verb = target ? "enable" : "disable";
      let confirmed = false;
      try {
        confirmed = await ask(
          `${verb[0].toUpperCase() + verb.slice(1)} "${item.name}" at startup?\n\n` +
            `${target ? "It will launch automatically again at your next sign-in." : "It will stop launching automatically. You can still open it yourself, and this change can be reversed here."}\n\n` +
            `Location: ${item.location}`,
          {
            title: target ? "Enable at startup?" : "Disable at startup?",
            kind: "warning",
            okLabel: target ? "Enable" : "Disable",
            cancelLabel: "Cancel",
          },
        );
      } catch (caught) {
        setError(errorMessage(caught));
        return;
      }
      if (!confirmed) return;
      setTogglingId(item.id);
      setNotice(null);
      try {
        const updated = await api.setStartupItemEnabled(item.id, target);
        setNotice(
          `${updated.name} is now ${updated.enabled ? "enabled" : "disabled"} at startup. Restart Windows before comparing boot time.`,
        );
        await refresh(true, true);
      } catch (caught) {
        setError(errorMessage(caught));
      } finally {
        setTogglingId(null);
      }
    },
    [refresh],
  );

  const runRecommendation = useCallback(
    async (recommendation: PerformanceRecommendation) => {
      const { action } = recommendation;
      if (action.kind === "startup" && action.targetId) {
        const item = snapshot?.startup.items.find((entry) => entry.id === action.targetId);
        if (item) await toggleStartup(item);
        return;
      }
      if (action.kind === "view" && action.view) {
        onNavigate(action.view);
        return;
      }
      if (action.kind === "page" && action.page && action.page !== "trace") {
        try {
          await api.openPerformanceSettings(action.page);
        } catch (caught) {
          setError(errorMessage(caught));
        }
        return;
      }
      if (action.kind === "trace") {
        document.getElementById("performance-trace")?.scrollIntoView({ block: "center" });
      }
    },
    [onNavigate, snapshot, toggleStartup],
  );

  const startTrace = async () => {
    if (!trace?.available) return;
    let confirmed = false;
    try {
      confirmed = await ask(
        "The trace adds a small amount of overhead while it runs, keeps buffers in memory, and records process names and file paths on this PC. Keep the reproduction short, then stop the trace.",
        {
          title: "Record a diagnostic trace?",
          kind: "warning",
          okLabel: "Start recording",
          cancelLabel: "Cancel",
        },
      );
    } catch (caught) {
      setError(errorMessage(caught));
      return;
    }
    if (!confirmed) return;
    setTraceBusy(true);
    setError(null);
    try {
      const next = await api.startPerformanceTrace();
      if (snapshotRef.current) {
        snapshotRef.current = { ...snapshotRef.current, trace: next };
      }
      setTraceOverride(next);
      setNotice("Trace recording started. Reproduce the slowdown, then stop it here.");
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setTraceBusy(false);
    }
  };

  const stopTrace = async () => {
    setTraceBusy(true);
    setError(null);
    try {
      const next = await api.stopPerformanceTrace();
      if (snapshotRef.current) {
        snapshotRef.current = { ...snapshotRef.current, trace: next };
      }
      setTraceOverride(next);
      setNotice(next.message);
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setTraceBusy(false);
    }
  };

  const recordBaseline = async () => {
    setRecordingBaseline(true);
    setBaselineError(null);
    setBaselineMessage(null);
    try {
      const recorded = await api.recordPerformanceBaseline(baselineLabel);
      setBaselineLabel("");
      setBaselineMessage(`${recorded.label} recorded at ${formatDateTime(recorded.recordedAt)}.`);
      await loadBaselines();
    } catch (caught) {
      setBaselineError(errorMessage(caught));
    } finally {
      setRecordingBaseline(false);
    }
  };

  const comparison = useMemo(() => {
    if (baselines.length < 2) return null;
    return { previous: baselines[1], latest: baselines[0] };
  }, [baselines]);

  const snapshotFresh = snapshot ? relativeTime(snapshot.capturedAt) : "Not measured";
  const recommendations = snapshot?.recommendations ?? [];
  const startup = snapshot?.startup;
  const kernelShare =
    snapshot && snapshot.cpu.utilizationPercent > 0
      ? snapshot.cpu.kernelPercent / snapshot.cpu.utilizationPercent
      : 0;
  const selectedGuide = guides.find((entry) => entry.id === guide) ?? guides[0];

  return (
    <div ref={rootRef}>
      <ViewHeader
        title="Performance"
        description="See what is actually using this PC, what Windows measured at startup, and which reversible change the evidence supports. Cyron does not use RAM boosters or claim guaranteed speed gains."
        meta={
          snapshot ? (
            <span className="perf-sample">
              {snapshot.sampleMs} ms sample · {snapshot.processCount} processes ·{" "}
              {snapshot.elevated ? "Administrator" : "Standard account"}
            </span>
          ) : undefined
        }
        actions={
          <>
            <button
              type="button"
              className="perf-live-toggle"
              aria-pressed={live}
              title="Measure every 10 seconds while this page is visible"
              onClick={() => setLive((value) => !value)}
            >
              <span className={cx("perf-live-dot", !live && "perf-live-dot--paused")} aria-hidden="true" />
              {live ? "Live updates" : "Paused"}
            </button>
            <Button
              variant="secondary"
              icon={<IconRefresh size={14} />}
              loading={refreshing}
              onClick={() => void refresh(false)}
              disabled={!snapshot && refreshing}
            >
              Refresh
            </Button>
          </>
        }
      />

      <div className="view-body perf">
        {error && (
          <Banner
            tone="danger"
            actions={
              <Button size="sm" variant="ghost" onClick={() => void refresh(false)}>
                Retry
              </Button>
            }
          >
            {error}
          </Banner>
        )}
        {notice && (
          <Banner
            tone="safe"
            actions={
              <Button size="sm" variant="ghost" onClick={() => setNotice(null)}>
                Dismiss
              </Button>
            }
          >
            {notice}
          </Banner>
        )}

        {status === "loading" && !snapshot && (
          <Panel title="Measuring this PC" subtitle="One read-only sample; nothing is changed.">
            <div className="perf-loading" aria-live="polite">
              <span className="spinner" aria-hidden="true" />
              <span>
                Reading CPU, memory, commit charge, disk pressure, processes and startup entries…
              </span>
            </div>
          </Panel>
        )}

        {snapshot && (
          <>
            <section className="perf-section">
              <SectionHeader
                title="Why is my PC slow?"
                description="Choose the symptom. Cyron shows only the evidence it actually measured."
              />
              <Panel bodyClassName="perf-guide-panel">
              <div className="perf-guide" role="group" aria-label="Slowdown symptoms">
                {guides.map((entry) => (
                  <button
                    key={entry.id}
                    type="button"
                    className="perf-guide__item"
                    aria-pressed={guide === entry.id}
                    onClick={() => selectGuide(entry.id)}
                  >
                    {entry.label}
                  </button>
                ))}
              </div>
              <div className="perf-guide__answer">
                <div>
                  <span className="perf-label">Guided check</span>
                  <h3>{selectedGuide.title}</h3>
                  <p>{selectedGuide.detail}</p>
                </div>
                <div className="perf-guide__facts">
                  <span>
                    <strong>{formatPercent(snapshot.cpu.utilizationPercent / 100)}</strong> CPU
                  </span>
                  <span>
                    <strong>{formatBytes(snapshot.memory.availableBytes)}</strong> available
                  </span>
                  <span>
                    <strong>
                      {snapshot.diskIo.busyPercent === null
                        ? "Not exposed"
                        : formatPercent(snapshot.diskIo.busyPercent / 100)}
                    </strong>{" "}
                    disk busy
                  </span>
                  <span>
                    <strong>{snapshot.startup.enabledCount}</strong> startup entries
                  </span>
                </div>
              </div>
              </Panel>
            </section>

            <section className="perf-section">
              <SectionHeader
                title="Live sample"
                description="One bounded reading. The figures move with what the PC is doing right now."
              />
              <Panel bodyClassName="perf-metrics">
              <div className="perf-metric">
                <div className="perf-metric__head">
                  <span className="perf-metric__label">Processor</span>
                  <span className="perf-metric__sub">{snapshot.cpu.logicalCores} logical processors</span>
                </div>
                <div className="perf-metric__value">
                  {formatPercent(snapshot.cpu.utilizationPercent / 100)}
                  <span>busy</span>
                </div>
                <ProgressBar
                  value={snapshot.cpu.utilizationPercent / 100}
                  tone={toneForPercent(snapshot.cpu.utilizationPercent, 80)}
                  label="Total CPU utilization"
                />
                <div className="perf-metric__meta">
                  <span>User {formatPercent(snapshot.cpu.userPercent / 100)}</span>
                  <span>Kernel {formatPercent(snapshot.cpu.kernelPercent / 100)}</span>
                  <span>Idle {formatPercent(snapshot.cpu.idlePercent / 100)}</span>
                </div>
              </div>

              <div className="perf-metric">
                <div className="perf-metric__head">
                  <span className="perf-metric__label">Memory</span>
                  <span className="perf-metric__sub">{formatBytes(snapshot.memory.totalBytes)} physical</span>
                </div>
                <div className="perf-metric__value">
                  {formatPercent(snapshot.memory.usedPercent / 100)}
                  <span>in use</span>
                </div>
                <ProgressBar
                  value={snapshot.memory.usedPercent / 100}
                  tone={snapshot.memory.usedPercent >= 90 ? "danger" : snapshot.memory.usedPercent >= 80 ? "caution" : "accent"}
                  label="Physical memory in use"
                />
                <div className="perf-metric__meta">
                  <span>{formatBytes(snapshot.memory.availableBytes)} available</span>
                  <span>Commit {formatPercent(snapshot.memory.commitPercent / 100)}</span>
                </div>
              </div>

              <div className="perf-metric">
                <div className="perf-metric__head">
                  <span className="perf-metric__label">Disk I/O</span>
                  <span className="perf-metric__sub">Physical disk pressure</span>
                </div>
                {snapshot.diskIo.status === "measured" ? (
                  <>
                    <div className="perf-metric__value">
                      {formatPercent((snapshot.diskIo.busyPercent ?? 0) / 100)}
                      <span>busy</span>
                    </div>
                    <ProgressBar
                      value={(snapshot.diskIo.busyPercent ?? 0) / 100}
                      tone={toneForPercent(snapshot.diskIo.busyPercent ?? 0, 70)}
                      label="Physical disk busy time"
                    />
                    <div className="perf-metric__meta">
                      <span>Queue {(snapshot.diskIo.queueLength ?? 0).toFixed(2)}</span>
                      <span>Windows PDH counter</span>
                    </div>
                  </>
                ) : (
                  <div className="perf-unavailable">
                    <IconInfo size={16} />
                    <span>{snapshot.diskIo.note ?? "Physical-disk counters were unavailable."}</span>
                  </div>
                )}
              </div>

              <div className="perf-metric">
                <div className="perf-metric__head">
                  <span className="perf-metric__label">Storage</span>
                  <span className="perf-metric__sub">Tightest fixed volume</span>
                </div>
                {lowestVolume ? (
                  <>
                    <div className="perf-metric__value">
                      {formatPercent(lowestVolume.free / lowestVolume.total)}
                      <span>free</span>
                    </div>
                    <ProgressBar
                      value={lowestVolume.free / lowestVolume.total}
                      tone={lowestVolume.free / lowestVolume.total < 0.1 ? "danger" : lowestVolume.free / lowestVolume.total < 0.2 ? "caution" : "accent"}
                      label={`Free space on ${lowestVolume.mount}`}
                    />
                    <div className="perf-metric__meta">
                      <span>{lowestVolume.mount}</span>
                      <span>{formatBytes(lowestVolume.free)} free</span>
                    </div>
                  </>
                ) : (
                  <p className="perf-unavailable">No fixed volume with capacity was reported.</p>
                )}
              </div>
              </Panel>
            </section>

            <section className="perf-section">
              <SectionHeader
                title="Catch a short spike"
                description="A burst capture keeps the peak instead of only the latest reading."
              />
              <Panel>
                <SpikeCapture />
              </Panel>
            </section>

            <section className="perf-section">
              <SectionHeader
                title="Kernel, drivers & recent changes"
                description="Kernel time has no process row. These read-only sources help correlate a spike with a driver or device event without guessing."
              />
              <Panel bodyClassName="perf-diagnostics-panel">
                <div className="perf-kernel-overview">
                  <div className="perf-kernel-overview__copy">
                    <span className="perf-label">Kernel share of busy CPU</span>
                    <strong>{formatPercent(kernelShare)}</strong>
                    <p>
                      {kernelShare >= 0.6 && snapshot.cpu.kernelPercent >= 20
                        ? "Kernel-mode work is the main source in this sample. Compare the driver dates and System events below, then repeat the spike capture before and after a change."
                        : "Kernel time is not dominant in this sample. A short spike can still be kernel-heavy; use the 20-second capture while the lag is visible."}{" "}
                      Windows does not attribute DPC/ISR time to one driver in these counters.
                    </p>
                  </div>
                  <div className="perf-kernel-overview__stats">
                    <Stat label="Kernel" value={formatPercent(snapshot.cpu.kernelPercent / 100)} />
                    <Stat label="User" value={formatPercent(snapshot.cpu.userPercent / 100)} />
                    <Stat
                      label="Drivers listed"
                      value={String(snapshot.drivers.items.length)}
                      meta="Display, storage, network"
                    />
                  </div>
                </div>

                <div className="perf-diagnostics-grid">
                  <div className="perf-evidence-block">
                    <div className="perf-evidence-block__head">
                      <div>
                        <span className="perf-label">Driver inventory</span>
                        <h3>Installed display, storage and network drivers</h3>
                      </div>
                      <Badge tone="neutral">{snapshot.drivers.items.length}</Badge>
                    </div>
                    {snapshot.drivers.items.length === 0 ? (
                      <p className="perf-note">
                        {snapshot.drivers.note ?? "Windows did not return any named drivers."}
                      </p>
                    ) : (
                      <ul className="perf-driver-list">
                        {snapshot.drivers.items.slice(0, 8).map((driver) => {
                          const age = driverAgeDays(driver.date);
                          return (
                            <li key={`${driver.class}-${driver.name}-${driver.version ?? "none"}`}>
                              <div className="perf-driver__main">
                                <span className="perf-driver__name">{driver.name}</span>
                                <span className="perf-source__evidence">
                                  {driver.class}
                                  {driver.version ? ` · v${driver.version}` : ""}
                                </span>
                              </div>
                              <div className="perf-driver__date">
                                <span className="mono small faint">
                                  {driver.date ? formatDate(driver.date) : "No date"}
                                </span>
                                {age !== null && age <= 30 && <Badge tone="caution">Recent</Badge>}
                              </div>
                            </li>
                          );
                        })}
                      </ul>
                    )}
                    <p className="perf-note">
                      Dates come from the driver package, not the install time. A recent date beside
                      a new slowdown is a lead, not proof that the driver caused it.
                    </p>
                  </div>

                  <div className="perf-evidence-block">
                    <div className="perf-evidence-block__head">
                      <div>
                        <span className="perf-label">System event log</span>
                        <h3>Driver and device errors from the last 7 days</h3>
                      </div>
                      <Badge tone={snapshot.systemEvents.items.length > 0 ? "caution" : "neutral"}>
                        {snapshot.systemEvents.items.length}
                      </Badge>
                    </div>
                    {snapshot.systemEvents.items.length === 0 ? (
                      <p className="perf-note">{snapshot.systemEvents.note}</p>
                    ) : (
                      <ul className="perf-event-list">
                        {snapshot.systemEvents.items.slice(0, 8).map((event) => (
                          <li key={`${event.source}-${event.eventId}-${event.observedAt}`}>
                            <Badge tone={eventTone(event.level)}>{event.level}</Badge>
                            <div className="perf-event__main">
                              <span className="perf-event__summary">{event.summary}</span>
                              <span className="perf-source__evidence">
                                {event.source} · Event {event.eventId} · {event.category}
                              </span>
                            </div>
                            <span className="mono small faint">
                              {formatDateTime(eventTimestamp(event.observedAt))}
                            </span>
                          </li>
                        ))}
                      </ul>
                    )}
                    {snapshot.systemEvents.items.length > 8 && (
                      <p className="perf-note">
                        {snapshot.systemEvents.items.length - 8} more matching events are kept in
                        the snapshot but not listed here.
                      </p>
                    )}
                    <p className="perf-note">
                      Only providers Windows recorded are shown. A missing event is not proof that a
                      driver is healthy. Per-process DPC/ISR attribution requires the advanced trace.
                    </p>
                  </div>
                </div>
              </Panel>
            </section>

            <section className="perf-section">
              <SectionHeader
                title="What the evidence supports"
                description={
                  visibleRecommendations.length === 0
                    ? "No supported recommendation for this symptom in the current sample."
                    : `${plural(visibleRecommendations.length, "recommendation")} with measured evidence`
                }
                actions={
                  <Button
                    size="sm"
                    variant="ghost"
                    icon={<IconRefresh size={13} />}
                    onClick={() => void refresh(false)}
                    loading={refreshing}
                  >
                    Measure again
                  </Button>
                }
              />
              <div className="perf-columns">
              <Panel bodyClassName="perf-recommendations-panel">
                {visibleRecommendations.length === 0 ? (
                  <EmptyState
                    compact
                    icon={IconCheck}
                    title="No pressure matches this symptom"
                    description={
                      guide === "startup"
                        ? "Windows did not expose boot-impact evidence in this session, and no reversible startup action is justified by the measurements. Review the startup inventory below instead."
                        : "Cyron found no measured CPU, memory, I/O or capacity pressure that supports a change. A short sample can miss lag; reproduce it and use the advanced trace if it continues."
                    }
                  />
                ) : (
                  <div className="perf-recommendations">
                    {visibleRecommendations.map((recommendation) => (
                      <article
                        key={recommendation.id}
                        className={cx("perf-rec", `perf-rec--${recommendation.severity}`)}
                      >
                        <header className="perf-rec__head">
                          <Badge tone={recommendationTone(recommendation.severity)}>
                            {recommendation.severity === "attention"
                              ? "Needs attention"
                              : recommendation.severity === "review"
                                ? "Review"
                                : "Informational"}
                          </Badge>
                          <h3>{recommendation.title}</h3>
                        </header>
                        <p className="perf-rec__summary">{recommendation.summary}</p>
                        <dl className="perf-evidence">
                          {recommendation.evidence.map((item) => (
                            <div key={`${item.label}-${item.value}`}>
                              <dt>{item.label}</dt>
                              <dd>{item.value}</dd>
                            </div>
                          ))}
                        </dl>
                        <div className="perf-rec__contract">
                          <div>
                            <span className="perf-label">What may change</span>
                            <p>{recommendation.expectedEffect}</p>
                          </div>
                          <div>
                            <span className="perf-label">Risk</span>
                            <p>{recommendation.risk}</p>
                          </div>
                          <div>
                            <span className="perf-label">How to check</span>
                            <p>{recommendation.verification}</p>
                          </div>
                        </div>
                        {recommendation.action.kind !== "none" && (
                          <div className="perf-rec__actions">
                            <Button
                              size="sm"
                              variant={recommendation.action.kind === "startup" ? "primary" : "secondary"}
                              loading={
                                recommendation.action.kind === "startup" &&
                                togglingId === recommendation.action.targetId
                              }
                              onClick={() => void runRecommendation(recommendation)}
                            >
                              {recommendation.action.label}
                            </Button>
                          </div>
                        )}
                      </article>
                    ))}
                  </div>
                )}
              </Panel>

              <Panel title="Current findings" subtitle="Raw observations, before any recommendation">
                {snapshot.findings.length === 0 ? (
                  <p className="perf-note">No findings were produced.</p>
                ) : (
                  <ul className="perf-findings">
                    {snapshot.findings.map((finding) => (
                      <li key={finding.id} className={`perf-finding perf-finding--${finding.tone}`}>
                        <span className="perf-finding__dot" aria-hidden="true" />
                        <div>
                          <span className="perf-finding__title">{finding.title}</span>
                          <p>{finding.detail}</p>
                        </div>
                      </li>
                    ))}
                  </ul>
                )}
                <div className="perf-sample-note">
                  <IconClock size={13} />
                  <span>Captured {snapshotFresh.toLowerCase()}.</span>
                </div>
              </Panel>
              </div>
            </section>

            <section className="perf-section">
              <SectionHeader
                title="Processes"
                description="Live sample. Cyron never ends a process from this page."
                actions={
                  <div className="perf-actions">
                    <Segmented<ProcessSort>
                      size="sm"
                      ariaLabel="Sort processes"
                      value={processSort}
                      onChange={setProcessSort}
                      options={[
                        { value: "cpu", label: "CPU" },
                        { value: "memory", label: "Memory" },
                        { value: "io", label: "I/O" },
                      ]}
                    />
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() => setShowAllProcesses((value) => !value)}
                    >
                      {showAllProcesses ? "Show top 12" : "Show all"}
                    </Button>
                  </div>
                }
              />
              <Panel bodyClassName="perf-table-panel">
              <div className="table-wrap">
                <table className="table perf-table">
                  <caption className="sr-only">
                    Processes ordered by {processSort}. Measurements cover the last sample only.
                  </caption>
                  <thead>
                    <tr>
                      <th scope="col">Process</th>
                      <th scope="col" className="numeric">CPU</th>
                      <th scope="col" className="numeric">Working set</th>
                      <th scope="col" className="numeric">Read / write</th>
                      <th scope="col" className="numeric perf-col-pid">PID</th>
                    </tr>
                  </thead>
                  <tbody>
                    {visibleProcesses.map((process) => (
                      <tr key={process.pid}>
                        <td>
                          <div className="perf-process">
                            <span className="perf-process__name">
                              <span className="truncate">{process.name}</span>
                              {process.isCritical && (
                                <Badge tone="neutral">
                                  <IconShield size={10} /> Protected
                                </Badge>
                              )}
                              {process.isCyron && <Badge tone="accent">Cyron</Badge>}
                            </span>
                            {process.path && (
                              <span className="perf-process__path" title={displayPath(process.path)}>
                                {truncateMiddle(displayPath(process.path), 72)}
                              </span>
                            )}
                          </div>
                        </td>
                        <td className="numeric mono">
                          {process.cpuPercent >= 0.1 ? `${process.cpuPercent.toFixed(1)}%` : "<0.1%"}
                        </td>
                        <td className="numeric mono">{formatBytes(process.workingSetBytes)}</td>
                        <td className="numeric mono">
                          {formatBytes(process.ioReadBytesPerSec)}/s · {formatBytes(process.ioWriteBytesPerSec)}/s
                        </td>
                        <td className="numeric mono perf-col-pid">{process.pid}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
              {orderedProcesses.length === 0 && (
                <p className="perf-note perf-note--pad">
                  Windows did not expose process counters for this sample.
                </p>
              )}
              {snapshot.unmeasuredProcessCount > 0 && (
                <p className="perf-note perf-note--pad">
                  {snapshot.unmeasuredProcessCount} protected or other-user processes could not be opened
                  and are not shown.
                </p>
              )}
              </Panel>
            </section>

            <section className="perf-section">
              <SectionHeader
                title="Startup and scheduled tasks"
                description="Provenance first. Impact appears only when Windows recorded it."
                actions={
                  <Segmented<StartupTab>
                    size="sm"
                    ariaLabel="Startup evidence type"
                    value={startupTab}
                    onChange={setStartupTab}
                    options={[
                      { value: "apps", label: "Startup apps", count: startup?.items.length },
                      { value: "tasks", label: "Scheduled tasks", count: startup?.scheduledTasks.length },
                    ]}
                  />
                }
              />
              <Panel bodyClassName="perf-table-panel">
              {startup?.eventLogStatus === "denied" && (
                <div className="perf-startup-banner">
                  <Banner
                    tone="caution"
                    actions={
                      <Button
                        size="sm"
                        variant="ghost"
                        icon={<IconExternal size={12} />}
                        onClick={() => void api.openPerformanceSettings("startup-apps").catch((caught) => setError(errorMessage(caught)))}
                      >
                        Windows startup settings
                      </Button>
                    }
                  >
                    <strong>Boot impact is administrator-only.</strong> {startup.eventLogNote}
                  </Banner>
                </div>
              )}

              {startup?.eventLogStatus === "available" && startup.boot && (
                <div className="perf-boot">
                  <div className="perf-boot__head">
                    <Badge tone={startup.boot.isDegradation ? "caution" : "safe"}>
                      {startup.boot.isDegradation ? "Boot degradation recorded" : "No boot degradation flagged"}
                    </Badge>
                    <span className="mono faint">
                      {bootObservedAt(snapshot) ? formatDateTime(bootObservedAt(snapshot) as number) : startup.boot.observedAt}
                    </span>
                  </div>
                  <div className="perf-boot__stats">
                    <Stat label="Total boot" value={formatDuration(startup.boot.bootTimeMs)} />
                    <Stat label="Main path" value={formatDuration(startup.boot.mainPathBootMs)} />
                    <Stat label="After desktop" value={formatDuration(startup.boot.postBootTimeMs)} />
                    <Stat label="Startup apps" value={String(startup.boot.startupAppCount)} />
                  </div>
                  <p className="perf-note">
                    Windows measured these phases during the last recorded boot. The log contains only
                    components that crossed its own degradation thresholds; absence is not proof of speed.
                  </p>
                </div>
              )}

              {startup?.eventLogStatus === "unavailable" && (
                <div className="perf-startup-banner">
                  <Banner tone="info">{startup.eventLogNote}</Banner>
                </div>
              )}

              {startupTab === "apps" ? (
                <>
                  <div className="table-wrap">
                    <table className="table perf-table">
                      <caption className="sr-only">
                        Startup applications with source, live usage and boot impact when Windows exposed it.
                      </caption>
                      <thead>
                        <tr>
                          <th scope="col">Entry</th>
                          <th scope="col" className="perf-col-source">Source</th>
                          <th scope="col" className="numeric">Boot impact</th>
                          <th scope="col" className="numeric perf-col-live">Live now</th>
                          <th scope="col">State</th>
                          <th scope="col">Action</th>
                        </tr>
                      </thead>
                      <tbody>
                        {startup?.items.map((item) => (
                          <tr key={item.id}>
                            <td>
                              <div className="perf-process">
                                <span className="perf-process__name">
                                  <span className="truncate">{item.name}</span>
                                  {item.isProtected && <Badge tone="caution">Protected</Badge>}
                                  <Badge tone="neutral">{item.scope === "machine" ? "All users" : "This user"}</Badge>
                                </span>
                                <span className="perf-process__path" title={displayPath(item.command)}>
                                  {truncateMiddle(displayPath(item.command), 64)}
                                </span>
                              </div>
                            </td>
                            <td className="perf-col-source">
                              <span className="perf-source">{item.location}</span>
                              <span className="perf-source__evidence">{evidenceSourceLabel(item.evidenceSource)}</span>
                            </td>
                            <td className="numeric mono">
                              {item.bootImpactMs === null ? (
                                <span className="faint">Unavailable</span>
                              ) : (
                                <span title={`Total ${formatDuration(item.bootTotalMs ?? 0)}`}>
                                  +{formatDuration(item.bootImpactMs)}
                                </span>
                              )}
                            </td>
                            <td className="numeric mono perf-col-live">
                              {item.currentCpuPercent > 0.05
                                ? `${item.currentCpuPercent.toFixed(1)}% CPU`
                                : item.currentWorkingSetBytes > 0
                                  ? formatBytes(item.currentWorkingSetBytes)
                                  : <span className="faint">Not matched</span>}
                            </td>
                            <td>
                              <Badge tone={item.enabled ? "safe" : "neutral"}>
                                {item.enabled ? "Enabled" : "Disabled"}
                              </Badge>
                            </td>
                            <td>
                              {item.canToggle ? (
                                <Button
                                  size="sm"
                                  variant={item.enabled ? "secondary" : "primary"}
                                  loading={togglingId === item.id}
                                  onClick={() => void toggleStartup(item)}
                                >
                                  {item.enabled ? "Disable" : "Enable"}
                                </Button>
                              ) : (
                                <span className="faint small">
                                  {item.isProtected ? "Windows protected" : "Read-only"}
                                </span>
                              )}
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                  {startup?.items.length === 0 && (
                    <p className="perf-note perf-note--pad">No startup applications were detected.</p>
                  )}
                  <p className="perf-note perf-note--pad">
                    {startup?.enabledCount ?? 0} of {startup?.totalCount ?? 0} entries are enabled.
                    Disabling changes the next sign-in only; it is reversible here. Cyron does not
                    delete Run entries or disable services.
                  </p>
                </>
              ) : (
                <>
                  <div className="perf-task-toolbar">
                    <Toggle
                      checked={showMicrosoftTasks}
                      onChange={setShowMicrosoftTasks}
                      label="Include Microsoft tasks"
                      description="Windows maintenance tasks are read-only here and usually are not the cause of lag."
                    />
                    <span className="faint small">
                      {visibleTasks.length} of {startup?.scheduledTasks.length ?? 0} boot/logon tasks
                    </span>
                  </div>
                  <div className="table-wrap">
                    <table className="table perf-table">
                      <caption className="sr-only">
                        Scheduled tasks that run at boot, logon or registration.
                      </caption>
                      <thead>
                        <tr>
                          <th scope="col">Task</th>
                          <th scope="col">Trigger</th>
                          <th scope="col" className="perf-col-author">Author</th>
                          <th scope="col">Action</th>
                          <th scope="col">State</th>
                        </tr>
                      </thead>
                      <tbody>
                        {visibleTasks.map((task) => (
                          <tr key={task.path || task.name}>
                            <td>
                              <div className="perf-process">
                                <span className="perf-process__name">
                                  <span className="truncate">{task.name}</span>
                                  {task.isMicrosoft && <Badge tone="neutral">Microsoft</Badge>}
                                </span>
                                <span className="perf-process__path" title={displayPath(task.path)}>
                                  {displayPath(task.path)}
                                </span>
                              </div>
                            </td>
                            <td>
                              <span>{scheduleTriggerLabel(task)}</span>
                              <span className="perf-source__evidence">{delayLabel(task)}</span>
                            </td>
                            <td className="perf-source perf-col-author">{task.author || "Not declared"}</td>
                            <td>
                              <span className="perf-process__path" title={`${task.command} ${task.arguments}`}>
                                {truncateMiddle(displayPath(task.command), 46)}
                                {task.arguments ? ` ${truncateMiddle(task.arguments, 22)}` : ""}
                              </span>
                            </td>
                            <td>
                              <Badge tone={task.enabled ? "safe" : "neutral"}>
                                {task.enabled ? "Enabled" : "Disabled"}
                              </Badge>
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                  {visibleTasks.length === 0 && (
                    <p className="perf-note perf-note--pad">
                      No boot, logon or registration tasks match this filter.
                    </p>
                  )}
                  <p className="perf-note perf-note--pad">
                    Cyron lists scheduled tasks read-only. It does not disable tasks, change triggers
                    or run them. Use Task Scheduler or Windows Settings for changes.
                  </p>
                </>
              )}
              </Panel>
            </section>

            <div className="perf-columns perf-columns--balanced">
              <Panel
                title="Before and after"
                subtitle="Local measurements you record yourself, with explicit limits."
              >
                <div className="perf-baseline-form">
                  <label htmlFor="perf-baseline-label">Label</label>
                  <input
                    id="perf-baseline-label"
                    className="input"
                    value={baselineLabel}
                    maxLength={80}
                    placeholder="e.g. Before closing background apps"
                    onChange={(event) => setBaselineLabel(event.target.value)}
                  />
                  <Button
                    size="sm"
                    variant="primary"
                    loading={recordingBaseline}
                    onClick={() => void recordBaseline()}
                  >
                    Record measurement
                  </Button>
                </div>
                {baselineMessage && <p className="perf-inline-success">{baselineMessage}</p>}
                {baselineError && <Banner tone="danger">{baselineError}</Banner>}

                {comparison ? (
                  <div className="perf-compare">
                    <div className="perf-compare__head">
                      <span className="mono faint">Previous</span>
                      <span className="mono faint">Latest</span>
                    </div>
                    <CompareRow
                      label="CPU busy"
                      previous={`${comparison.previous.cpuPercent.toFixed(1)}%`}
                      latest={`${comparison.latest.cpuPercent.toFixed(1)}%`}
                    />
                    <CompareRow
                      label="Memory in use"
                      previous={`${comparison.previous.memoryUsedPercent.toFixed(1)}%`}
                      latest={`${comparison.latest.memoryUsedPercent.toFixed(1)}%`}
                    />
                    <CompareRow
                      label="Available memory"
                      previous={formatBytes(comparison.previous.availableMemoryBytes)}
                      latest={formatBytes(comparison.latest.availableMemoryBytes)}
                    />
                    <CompareRow
                      label="Commit"
                      previous={`${comparison.previous.commitPercent.toFixed(1)}%`}
                      latest={`${comparison.latest.commitPercent.toFixed(1)}%`}
                    />
                    {comparison.latest.diskBusyPercent !== null && (
                      <CompareRow
                        label="Disk busy"
                        previous={
                          comparison.previous.diskBusyPercent === null
                            ? "Unavailable"
                            : `${comparison.previous.diskBusyPercent.toFixed(1)}%`
                        }
                        latest={`${comparison.latest.diskBusyPercent.toFixed(1)}%`}
                      />
                    )}
                    {comparison.latest.bootTimeMs !== null && (
                      <CompareRow
                        label="Boot time"
                        previous={
                          comparison.previous.bootTimeMs === null
                            ? "Unavailable"
                            : formatDuration(comparison.previous.bootTimeMs)
                        }
                        latest={formatDuration(comparison.latest.bootTimeMs)}
                      />
                    )}
                    <p className="perf-note">
                      Two samples show association, not causation. CPU and disk change with what you
                      are doing, and boot time changes only after a restart.
                    </p>
                  </div>
                ) : (
                  <p className="perf-note perf-note--spaced">
                    Record one measurement, make the change you want to evaluate, then record another.
                    Cyron compares the two numbers without claiming a guaranteed speed gain.
                  </p>
                )}

                {baselines.length > 0 && (
                  <ul className="perf-baseline-list">
                    {baselines.slice(0, 5).map((baseline) => (
                      <li key={baseline.id}>
                        <div>
                          <span className="truncate">{baseline.label}</span>
                          <span className="faint small">{formatDateTime(baseline.recordedAt)}</span>
                        </div>
                        <span className="mono small">
                          CPU {baseline.cpuPercent.toFixed(1)}% · {formatBytes(baseline.availableMemoryBytes)} free
                        </span>
                      </li>
                    ))}
                  </ul>
                )}
              </Panel>

              <Panel
                title="Advanced diagnostic trace"
                subtitle="Optional. Only when a short sample cannot explain the lag."
              >
                <div id="performance-trace" className="perf-trace">
                  {trace?.active ? (
                    <Badge tone="caution">
                      <span className="perf-live-dot" aria-hidden="true" /> Recording
                    </Badge>
                  ) : (
                    <Badge tone={trace?.available ? "safe" : "neutral"}>
                      {trace?.available ? "Ready" : "Needs administrator"}
                    </Badge>
                  )}
                  <p>{trace?.message}</p>
                  {trace?.active ? (
                    <>
                      <Button
                        variant="danger"
                        icon={<IconStop size={14} />}
                        loading={traceBusy}
                        onClick={() => void stopTrace()}
                      >
                        Stop and save trace
                      </Button>
                      {trace.startedAt && (
                        <span className="faint small">
                          Started {relativeTime(trace.startedAt).toLowerCase()}. Keep it short.
                        </span>
                      )}
                    </>
                  ) : (
                    <Button
                      variant="secondary"
                      icon={<IconGauge size={14} />}
                      disabled={!trace?.available}
                      loading={traceBusy}
                      onClick={() => void startTrace()}
                    >
                      Record trace
                    </Button>
                  )}
                  {!trace?.available && (
                    <p className="perf-note">
                      Close Cyron, choose <strong>Run as administrator</strong>, and reopen this page.
                      Cyron never elevates itself.
                    </p>
                  )}
                  {trace?.outputPath && !trace.active && (
                    <div className="perf-trace__path">
                      <span className="mono" title={trace.outputPath}>
                        {truncateMiddle(trace.outputPath, 58)}
                      </span>
                      <Button
                        size="sm"
                        variant="ghost"
                        icon={<IconExternal size={12} />}
                        onClick={() => void api.revealPath(trace.outputPath as string).catch((caught) => setError(errorMessage(caught)))}
                      >
                        Show file
                      </Button>
                    </div>
                  )}
                  <p className="perf-note">
                    The trace is stored locally, never uploaded. It can be opened in Windows
                    Performance Analyzer. WPR adds overhead and reserves memory while recording.
                  </p>
                </div>
              </Panel>
            </div>

            <Panel title="What this measurement cannot tell you" subtitle="Limits are part of the result">
              <ul className="perf-limitations">
                {snapshot.limitations.map((limitation) => (
                  <li key={limitation}>{limitation}</li>
                ))}
              </ul>
              {recommendations.length === 0 && (
                <p className="perf-note perf-note--spaced">
                  No recommendation met its evidence threshold. That is a result, not a failure.
                </p>
              )}
            </Panel>
          </>
        )}

        {status === "error" && !snapshot && (
          <Panel title="Performance measurement unavailable">
            <EmptyState
              icon={IconAlert}
              title="Cyron could not read performance counters"
              description={error ?? "The Windows APIs did not return a sample. Try again from the desktop app."}
              action={
                <Button onClick={() => void refresh(false)} icon={<IconRefresh size={13} />}>
                  Try again
                </Button>
              }
            />
          </Panel>
        )}
      </div>
    </div>
  );
}

function CompareRow({
  label,
  previous,
  latest,
}: {
  label: string;
  previous: string;
  latest: string;
}) {
  return (
    <div className="perf-compare__row">
      <span>{label}</span>
      <span className="mono">{previous}</span>
      <span className="mono strong">{latest}</span>
    </div>
  );
}
