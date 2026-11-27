import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { cx, Segmented, Skeleton } from "../../components/ui";
import { formatBytes, formatCount } from "../../lib/format";
import type { LiveDisk, LiveGpu, LiveNetwork, LivePerformance, LiveProcess } from "../../lib/types";
import { LiveChart } from "./LiveChart";
import {
  diskTitle,
  formatBits,
  formatPercentValue,
  formatRate,
  formatSpeed,
  formatUptime,
  networkTitle,
  niceCeiling,
  resourceEntries,
  seriesKeys,
  type HistoryStore,
  type ResourceEntry,
} from "./monitorModel";

interface ResourceMonitorProps {
  live: LivePerformance | null;
  history: HistoryStore;
  error: string | null;
}

function series(history: HistoryStore, key: string): number[] {
  return (history.get(key) ?? []).slice();
}

function seriesMax(values: number[][], floor: number): number {
  let peak = 0;
  for (const list of values) for (const value of list) if (value > peak) peak = value;
  return niceCeiling(peak * 1.1, floor);
}

/**
 * Task Manager's Performance view, rebuilt: a resource rail with sparklines on
 * the left, one large 60-second chart and its figures on the right, and the
 * processes behind the load right next to the numbers.
 */
export function ResourceMonitor({ live, history, error }: ResourceMonitorProps) {
  const [selectedId, setSelectedId] = useState("cpu");
  const entries = useMemo(() => (live ? resourceEntries(live) : []), [live]);
  const railRef = useRef<HTMLDivElement>(null);

  // A disk or adapter can disappear (unplugged); fall back to CPU.
  useEffect(() => {
    if (entries.length > 0 && !entries.some((entry) => entry.id === selectedId)) {
      setSelectedId("cpu");
    }
  }, [entries, selectedId]);

  const selected = entries.find((entry) => entry.id === selectedId) ?? entries[0] ?? null;

  const onRailKey = (event: KeyboardEvent<HTMLDivElement>) => {
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const index = entries.findIndex((entry) => entry.id === selected?.id);
    let next = index;
    if (event.key === "ArrowDown") next = Math.min(entries.length - 1, index + 1);
    if (event.key === "ArrowUp") next = Math.max(0, index - 1);
    if (event.key === "Home") next = 0;
    if (event.key === "End") next = entries.length - 1;
    const target = entries[next];
    if (!target) return;
    setSelectedId(target.id);
    railRef.current
      ?.querySelector<HTMLButtonElement>(`[data-resource="${CSS.escape(target.id)}"]`)
      ?.focus();
  };

  if (!live) {
    return (
      <div className="monitor" aria-busy="true">
        <div className="monitor__rail">
          {[0, 1, 2, 3].map((row) => (
            <div className="monitor-res monitor-res--skeleton" key={row}>
              <Skeleton width={64} height={34} />
              <div className="stack">
                <Skeleton width={90} height={11} />
                <Skeleton width={130} height={9} />
              </div>
            </div>
          ))}
        </div>
        <div className="monitor__detail">
          {error ? (
            <p className="perf-note">{error}</p>
          ) : (
            <>
              <Skeleton width={180} height={22} />
              <Skeleton width="100%" height={260} />
            </>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="monitor">
      <div
        className="monitor__rail"
        role="tablist"
        aria-orientation="vertical"
        aria-label="Resources"
        ref={railRef}
        onKeyDown={onRailKey}
      >
        {entries.map((entry) => (
          <ResourceButton
            key={entry.id}
            entry={entry}
            values={series(history, entry.seriesKey)}
            selected={entry.id === selected?.id}
            onSelect={() => setSelectedId(entry.id)}
          />
        ))}
      </div>
      <div className="monitor__detail" role="tabpanel" aria-label={selected?.title}>
        {selected?.kind === "cpu" && <CpuDetail live={live} history={history} />}
        {selected?.kind === "memory" && <MemoryDetail live={live} history={history} />}
        {selected?.kind === "disk" && (
          <DiskDetail
            disk={live.disks.find((disk) => `disk:${disk.index}` === selected.id) ?? null}
            live={live}
            history={history}
          />
        )}
        {selected?.kind === "network" && (
          <NetworkDetail
            adapter={live.networks.find((adapter) => `net:${adapter.name}` === selected.id) ?? null}
            title={selected.title}
            history={history}
          />
        )}
        {selected?.kind === "gpu" && (
          <GpuDetail
            gpu={live.gpus.find((gpu) => `gpu:${gpu.luid}` === selected.id) ?? null}
            live={live}
            history={history}
          />
        )}
        {!live.warm && <p className="monitor__warming">Measuring the first interval…</p>}
      </div>
    </div>
  );
}

function ResourceButton({
  entry,
  values,
  selected,
  onSelect,
}: {
  entry: ResourceEntry;
  values: number[];
  selected: boolean;
  onSelect: () => void;
}) {
  const max = entry.scale === "percent" ? 100 : seriesMax([values], 1000);
  return (
    <button
      type="button"
      role="tab"
      data-resource={entry.id}
      aria-selected={selected}
      tabIndex={selected ? 0 : -1}
      className={cx("monitor-res", selected && "is-selected")}
      onClick={onSelect}
    >
      <span className="monitor-res__spark">
        <LiveChart
          compact
          series={[{ label: entry.title, values }]}
          max={max}
          height={34}
          formatValue={(value) => String(value)}
          ariaLabel={`${entry.title} history`}
        />
      </span>
      <span className="monitor-res__text">
        <span className="monitor-res__title">{entry.title}</span>
        <span className="monitor-res__detail numeric">{entry.detail}</span>
      </span>
    </button>
  );
}

/* ------------------------------------------------------------- layout bits */

function DetailHeader({ title, hardware }: { title: string; hardware?: string | null }) {
  return (
    <header className="monitor-head">
      <h3 className="monitor-head__title">{title}</h3>
      {hardware && (
        <span className="monitor-head__hardware" title={hardware}>
          {hardware}
        </span>
      )}
    </header>
  );
}

function Figure({ label, value, hint }: { label: string; value: ReactNode; hint?: string }) {
  return (
    <div className="monitor-fig" title={hint}>
      <span className="monitor-fig__label">{label}</span>
      <span className="monitor-fig__value numeric">{value}</span>
    </div>
  );
}

function SpecList({ items }: { items: { label: string; value: ReactNode }[] }) {
  return (
    <dl className="monitor-specs">
      {items.map((item) => (
        <div className="monitor-specs__row" key={item.label}>
          <dt>{item.label}</dt>
          <dd className="numeric">{item.value}</dd>
        </div>
      ))}
    </dl>
  );
}

function TopProcesses({
  title,
  processes,
  metric,
  format,
}: {
  title: string;
  processes: LiveProcess[] | null;
  metric: (process: LiveProcess) => number;
  format: (value: number) => string;
}) {
  const top = useMemo(
    () =>
      (processes ?? [])
        .filter((process) => metric(process) > 0)
        .sort((left, right) => metric(right) - metric(left))
        .slice(0, 5),
    [processes, metric],
  );
  const peak = top.length > 0 ? metric(top[0]) : 0;
  return (
    <section className="monitor-top" aria-label={title}>
      <h4 className="monitor-top__title">{title}</h4>
      {top.length === 0 ? (
        <p className="monitor-top__empty">Nothing measurable in this interval.</p>
      ) : (
        <ol className="monitor-top__list">
          {top.map((process) => (
            <li className="monitor-top__row" key={process.pid} title={process.path ?? process.name}>
              <span className="monitor-top__name">{process.name}</span>
              <span className="monitor-top__value numeric">{format(metric(process))}</span>
              <span className="monitor-top__bar" aria-hidden="true">
                <span style={{ width: `${peak > 0 ? (metric(process) / peak) * 100 : 0}%` }} />
              </span>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}

const cpuMetric = (process: LiveProcess) => process.cpuPercent;
const memoryMetric = (process: LiveProcess) => process.workingSetBytes;
const ioMetric = (process: LiveProcess) => process.ioBytesPerSec;

/* ---------------------------------------------------------------- details */

function CpuDetail({ live, history }: { live: LivePerformance; history: HistoryStore }) {
  const [view, setView] = useState<"overall" | "cores">("overall");
  const hardware = live.hardware;
  const cores = live.cpu.perCorePercent;
  return (
    <>
      <DetailHeader title="CPU" hardware={hardware.cpuName} />
      <div className="monitor-toolbar">
        <Segmented
          size="sm"
          ariaLabel="CPU graph"
          value={view}
          onChange={setView}
          options={[
            { value: "overall", label: "Overall" },
            { value: "cores", label: `Logical processors${cores.length ? ` (${cores.length})` : ""}` },
          ]}
        />
        <span className="monitor-toolbar__legend">
          <span className="live-chart__swatch" /> Utilization
          {view === "overall" && (
            <>
              <span className="live-chart__swatch live-chart__swatch--secondary" /> Kernel time
            </>
          )}
        </span>
      </div>
      {view === "overall" || cores.length === 0 ? (
        <LiveChart
          caption="% Utilization"
          maxLabel="100%"
          max={100}
          series={[
            { label: "Utilization", values: series(history, seriesKeys.cpu) },
            { label: "Kernel", values: series(history, seriesKeys.cpuKernel), tone: "secondary" },
          ]}
          formatValue={formatPercentValue}
          ariaLabel="CPU utilization over the last 60 seconds"
        />
      ) : (
        <div
          className="monitor-cores"
          style={{ "--core-columns": Math.min(8, Math.ceil(Math.sqrt(cores.length * 1.6))) } as React.CSSProperties}
        >
          {cores.map((value, index) => (
            <div className="monitor-cores__cell" key={index}>
              <LiveChart
                compact
                max={100}
                height={cores.length > 16 ? 46 : 64}
                series={[{ label: `CPU ${index}`, values: series(history, seriesKeys.core(index)) }]}
                formatValue={formatPercentValue}
                ariaLabel={`Logical processor ${index}`}
              />
              <span className="monitor-cores__label numeric">
                {index} · {formatPercentValue(value)}
              </span>
            </div>
          ))}
        </div>
      )}
      <div className="monitor-body">
        <div className="monitor-figures">
          <Figure label="Utilization" value={formatPercentValue(live.cpu.utilizationPercent)} />
          <Figure label="Speed" value={formatSpeed(live.cpu.speedMhz)} />
          <Figure label="Processes" value={formatCount(live.system.processCount)} />
          <Figure label="Threads" value={formatCount(live.system.threadCount)} />
          <Figure label="Handles" value={formatCount(live.system.handleCount)} />
          <Figure label="Up time" value={formatUptime(live.system.uptimeSeconds)} />
        </div>
        <SpecList
          items={[
            { label: "Base speed", value: formatSpeed(hardware.baseSpeedMhz) },
            { label: "Sockets", value: hardware.sockets },
            { label: "Cores", value: hardware.cores },
            { label: "Logical processors", value: hardware.logicalProcessors },
            { label: "User / kernel", value: `${formatPercentValue(live.cpu.userPercent)} / ${formatPercentValue(live.cpu.kernelPercent)}` },
            { label: "L1 cache", value: hardware.l1CacheBytes ? formatBytes(hardware.l1CacheBytes) : "—" },
            { label: "L2 cache", value: hardware.l2CacheBytes ? formatBytes(hardware.l2CacheBytes) : "—" },
            { label: "L3 cache", value: hardware.l3CacheBytes ? formatBytes(hardware.l3CacheBytes) : "—" },
          ]}
        />
        <TopProcesses
          title="Using the most CPU"
          processes={live.processes}
          metric={cpuMetric}
          format={(value) => (value < 0.1 ? "<0.1%" : `${value.toFixed(1)}%`)}
        />
      </div>
    </>
  );
}

function MemoryDetail({ live, history }: { live: LivePerformance; history: HistoryStore }) {
  const memory = live.memory;
  const installed = live.hardware.installedMemoryBytes;
  const reserved = installed && installed > memory.totalBytes ? installed - memory.totalBytes : 0;
  const modified = memory.modifiedBytes ?? 0;
  const standby = memory.standbyBytes ?? 0;
  const free = memory.freeBytes ?? Math.max(memory.availableBytes - standby, 0);
  const inUse = Math.max(memory.usedBytes - modified, 0);
  const composition = [
    { key: "use", label: "In use", bytes: inUse },
    { key: "modified", label: "Modified", bytes: modified },
    { key: "standby", label: "Standby", bytes: standby },
    { key: "free", label: "Free", bytes: free },
  ];
  const compositionTotal = composition.reduce((sum, part) => sum + part.bytes, 0) || 1;
  return (
    <>
      <DetailHeader
        title="Memory"
        hardware={installed ? `${formatBytes(installed)} installed` : `${formatBytes(memory.totalBytes)} usable`}
      />
      <LiveChart
        caption="Memory usage"
        maxLabel={formatBytes(memory.totalBytes)}
        max={100}
        series={[
          { label: "In use", values: series(history, seriesKeys.memory) },
          { label: "Committed", values: series(history, seriesKeys.commit), tone: "secondary" },
        ]}
        formatValue={formatPercentValue}
        ariaLabel="Memory in use over the last 60 seconds"
      />
      <div className="monitor-composition" aria-label="Memory composition">
        <span className="monitor-composition__caption">Memory composition</span>
        <div className="monitor-composition__bar">
          {composition.map((part) => (
            <span
              key={part.key}
              className={`monitor-composition__part monitor-composition__part--${part.key}`}
              style={{ flexGrow: part.bytes / compositionTotal }}
              title={`${part.label}: ${formatBytes(part.bytes)}`}
            />
          ))}
        </div>
        <div className="monitor-composition__legend">
          {composition.map((part) => (
            <span key={part.key}>
              <span className={`monitor-composition__dot monitor-composition__part--${part.key}`} />
              {part.label} <span className="numeric">{formatBytes(part.bytes)}</span>
            </span>
          ))}
        </div>
      </div>
      <div className="monitor-body">
        <div className="monitor-figures">
          <Figure label="In use" value={formatBytes(memory.usedBytes)} />
          <Figure label="Available" value={formatBytes(memory.availableBytes)} />
          <Figure
            label="Committed"
            value={`${formatBytes(memory.commitUsedBytes)} / ${formatBytes(memory.commitLimitBytes)}`}
            hint="Memory promised to programs, against RAM plus the page file."
          />
          <Figure label="Cached" value={formatBytes(memory.cachedBytes)} />
          <Figure label="Paged pool" value={formatBytes(memory.pagedPoolBytes)} />
          <Figure label="Non-paged pool" value={formatBytes(memory.nonpagedPoolBytes)} />
        </div>
        <SpecList
          items={[
            { label: "Usable", value: formatBytes(memory.totalBytes) },
            { label: "Installed", value: installed ? formatBytes(installed) : "—" },
            { label: "Hardware reserved", value: reserved ? formatBytes(reserved) : "—" },
            { label: "Standby", value: memory.standbyBytes !== null ? formatBytes(standby) : "—" },
            { label: "Free", value: memory.freeBytes !== null ? formatBytes(free) : "—" },
          ]}
        />
        <TopProcesses
          title="Using the most memory"
          processes={live.processes}
          metric={memoryMetric}
          format={formatBytes}
        />
      </div>
    </>
  );
}

function DiskDetail({
  disk,
  live,
  history,
}: {
  disk: LiveDisk | null;
  live: LivePerformance;
  history: HistoryStore;
}) {
  if (!disk) return null;
  const read = series(history, seriesKeys.diskRead(disk.index));
  const write = series(history, seriesKeys.diskWrite(disk.index));
  const rateMax = seriesMax([read, write], 1024 * 1024);
  return (
    <>
      <DetailHeader title={diskTitle(disk.index, disk.volumes)} hardware="Physical disk" />
      <LiveChart
        caption="Active time"
        maxLabel="100%"
        max={100}
        height={150}
        series={[{ label: "Active", values: series(history, seriesKeys.diskActive(disk.index)) }]}
        formatValue={formatPercentValue}
        ariaLabel="Disk active time over the last 60 seconds"
      />
      <LiveChart
        caption="Disk transfer rate"
        maxLabel={formatRate(rateMax)}
        max={rateMax}
        height={110}
        series={[
          { label: "Read", values: read },
          { label: "Write", values: write, tone: "secondary" },
        ]}
        formatValue={formatRate}
        ariaLabel="Disk transfer rate over the last 60 seconds"
      />
      <div className="monitor-body">
        <div className="monitor-figures">
          <Figure label="Active time" value={formatPercentValue(disk.activePercent)} />
          <Figure label="Response time" value={`${disk.responseMs.toFixed(1)} ms`} />
          <Figure label="Read speed" value={formatRate(disk.readBytesPerSec)} />
          <Figure label="Write speed" value={formatRate(disk.writeBytesPerSec)} />
        </div>
        <SpecList
          items={[
            { label: "Volumes", value: disk.volumes.join(" ") || "—" },
            { label: "Disk number", value: disk.index },
          ]}
        />
        <TopProcesses
          title="Most I/O right now"
          processes={live.processes}
          metric={ioMetric}
          format={formatRate}
        />
      </div>
    </>
  );
}

function NetworkDetail({
  adapter,
  title,
  history,
}: {
  adapter: LiveNetwork | null;
  title: string;
  history: HistoryStore;
}) {
  if (!adapter) return null;
  const receive = series(history, seriesKeys.netReceive(adapter.name));
  const send = series(history, seriesKeys.netSend(adapter.name));
  const max = seriesMax([receive, send], 100_000);
  return (
    <>
      <DetailHeader title={title || networkTitle(adapter.kind)} hardware={adapter.name} />
      <LiveChart
        caption="Throughput"
        maxLabel={formatBits(max)}
        max={max}
        series={[
          { label: "Receive", values: receive },
          { label: "Send", values: send, tone: "secondary" },
        ]}
        formatValue={formatBits}
        ariaLabel="Network throughput over the last 60 seconds"
      />
      <div className="monitor-body">
        <div className="monitor-figures">
          <Figure label="Receive" value={formatBits(adapter.receivedBytesPerSec * 8)} />
          <Figure label="Send" value={formatBits(adapter.sentBytesPerSec * 8)} />
          <Figure label="Link speed" value={formatBits(adapter.linkBitsPerSec)} />
          <Figure
            label="Link used"
            value={formatPercentValue(
              adapter.linkBitsPerSec > 0
                ? (((adapter.receivedBytesPerSec + adapter.sentBytesPerSec) * 8) / adapter.linkBitsPerSec) * 100
                : 0,
            )}
          />
        </div>
        <SpecList
          items={[
            { label: "Adapter", value: adapter.name },
            { label: "Connection type", value: networkTitle(adapter.kind) },
          ]}
        />
      </div>
    </>
  );
}

function GpuDetail({
  gpu,
  live,
  history,
}: {
  gpu: LiveGpu | null;
  live: LivePerformance;
  history: HistoryStore;
}) {
  if (!gpu) return null;
  const engines = gpu.engines.slice(0, 4);
  const dedicated = series(history, seriesKeys.gpuDedicated(gpu.luid));
  const dedicatedMax = seriesMax([dedicated], 256 * 1024 * 1024);
  return (
    <>
      <DetailHeader title={`GPU ${gpu.index}`} hardware={gpu.name ?? live.hardware.gpuNames.join(" · ")} />
      <div className="monitor-engines">
        {engines.map((engine) => (
          <div className="monitor-engines__cell" key={engine.name}>
            <LiveChart
              caption={engine.name}
              maxLabel={formatPercentValue(engine.percent)}
              max={100}
              height={96}
              series={[{ label: engine.name, values: series(history, seriesKeys.gpuEngine(gpu.luid, engine.name)) }]}
              formatValue={formatPercentValue}
              ariaLabel={`${engine.name} engine utilization`}
            />
          </div>
        ))}
      </div>
      <LiveChart
        caption="Dedicated GPU memory usage"
        maxLabel={formatBytes(dedicatedMax)}
        max={dedicatedMax}
        height={80}
        series={[{ label: "Dedicated", values: dedicated }]}
        formatValue={formatBytes}
        ariaLabel="Dedicated GPU memory over the last 60 seconds"
      />
      <div className="monitor-body">
        <div className="monitor-figures">
          <Figure label="Utilization" value={formatPercentValue(gpu.utilizationPercent)} hint="The busiest engine, as Task Manager reports it." />
          <Figure label="Dedicated memory" value={formatBytes(gpu.dedicatedUsedBytes)} />
          <Figure label="Shared memory" value={formatBytes(gpu.sharedUsedBytes)} />
        </div>
        <SpecList items={gpu.engines.map((engine) => ({ label: engine.name, value: formatPercentValue(engine.percent) }))} />
      </div>
    </>
  );
}
