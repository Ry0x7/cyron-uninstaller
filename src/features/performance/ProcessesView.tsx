import { useMemo, useState } from "react";
import { IconChevronDown, IconChevronUp, IconExternal, IconShield } from "../../components/Icons";
import { Badge, EmptyState, IconButton, SearchField, Segmented, cx } from "../../components/ui";
import { api } from "../../lib/api";
import { displayPath, formatBytes, formatCount } from "../../lib/format";
import type { LivePerformance, LiveProcess } from "../../lib/types";
import { formatRate } from "./monitorModel";

type SortKey = "name" | "cpu" | "memory" | "io" | "pid";
type Grouping = "grouped" | "flat";

const COLUMNS: { key: SortKey; label: string; numeric: boolean }[] = [
  { key: "name", label: "Name", numeric: false },
  { key: "pid", label: "PID", numeric: true },
  { key: "cpu", label: "CPU", numeric: true },
  { key: "memory", label: "Memory", numeric: true },
  { key: "io", label: "Disk I/O", numeric: true },
];

function compare(left: LiveProcess, right: LiveProcess, key: SortKey): number {
  switch (key) {
    case "name":
      return left.name.localeCompare(right.name, undefined, { sensitivity: "base" });
    case "pid":
      return left.pid - right.pid;
    case "cpu":
      return left.cpuPercent - right.cpuPercent;
    case "memory":
      return left.workingSetBytes - right.workingSetBytes;
    case "io":
      return left.ioBytesPerSec - right.ioBytesPerSec;
  }
}

/** Heat from 0 to 1, used to tint a cell like Task Manager's heat map. */
function heat(value: number, scale: number): number {
  if (!(value > 0) || !(scale > 0)) return 0;
  return Math.min(1, value / scale);
}

interface ProcessesViewProps {
  live: LivePerformance | null;
  error: string | null;
}

/**
 * A live process table: grouped into apps, background processes and Windows
 * processes, sortable on every column, with heat-tinted figures. Read-only:
 * ending a process stays in Applications and Hunter Mode, which verify the
 * target before anything is terminated.
 */
export function ProcessesView({ live, error }: ProcessesViewProps) {
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<{ key: SortKey; descending: boolean }>({
    key: "cpu",
    descending: true,
  });
  const [grouping, setGrouping] = useState<Grouping>("grouped");
  const [selectedPid, setSelectedPid] = useState<number | null>(null);

  const processes = live?.processes ?? null;
  const totalMemory = live?.memory.totalBytes ?? 0;

  const filtered = useMemo(() => {
    const needle = query.trim().toLocaleLowerCase();
    const rows = (processes ?? []).filter(
      (process) =>
        !needle ||
        process.name.toLocaleLowerCase().includes(needle) ||
        String(process.pid) === needle ||
        (process.path ?? "").toLocaleLowerCase().includes(needle),
    );
    rows.sort((left, right) => {
      const order = compare(left, right, sort.key);
      const directed = sort.descending ? -order : order;
      return directed !== 0 ? directed : left.pid - right.pid;
    });
    return rows;
  }, [processes, query, sort]);

  const groups = useMemo(() => {
    if (grouping === "flat") return [{ id: "all", title: "All processes", rows: filtered }];
    const apps = filtered.filter((process) => process.hasWindow && !process.isCritical);
    const windows = filtered.filter((process) => process.isCritical);
    const background = filtered.filter((process) => !process.hasWindow && !process.isCritical);
    return [
      { id: "apps", title: "Apps", rows: apps },
      { id: "background", title: "Background processes", rows: background },
      { id: "windows", title: "Windows processes", rows: windows },
    ].filter((group) => group.rows.length > 0);
  }, [filtered, grouping]);

  const totals = useMemo(() => {
    const list = processes ?? [];
    return {
      cpu: live?.cpu.utilizationPercent ?? 0,
      memory: live?.memory.usedPercent ?? 0,
      io: list.reduce((sum, process) => sum + process.ioBytesPerSec, 0),
    };
  }, [processes, live]);

  const toggleSort = (key: SortKey) =>
    setSort((current) =>
      current.key === key
        ? { key, descending: !current.descending }
        : { key, descending: key !== "name" && key !== "pid" },
    );

  if (!processes) {
    return error ? (
      <EmptyState title="Processes are unavailable" description={error} />
    ) : (
      <div className="procs procs--loading" aria-busy="true">
        <p className="perf-note">Reading processes…</p>
      </div>
    );
  }

  const headerFigure = (key: SortKey) => {
    if (key === "cpu") return `${Math.round(totals.cpu)}%`;
    if (key === "memory") return `${Math.round(totals.memory)}%`;
    if (key === "io") return formatRate(totals.io);
    return null;
  };

  return (
    <div className="procs">
      <div className="procs__toolbar">
        <SearchField
          value={query}
          onChange={setQuery}
          placeholder="Search by name, PID or path"
          ariaLabel="Search processes"
          className="procs__search"
        />
        <Segmented<Grouping>
          size="sm"
          ariaLabel="Process grouping"
          value={grouping}
          onChange={setGrouping}
          options={[
            { value: "grouped", label: "Grouped" },
            { value: "flat", label: "Flat list" },
          ]}
        />
        <span className="procs__count numeric">
          {formatCount(filtered.length)} of {formatCount(processes.length)} processes
        </span>
      </div>

      <div className="procs__table-wrap">
        <table className="procs__table">
          <caption className="sr-only">
            Live processes. CPU is a share of all logical processors over the last second; memory is
            the working set.
          </caption>
          <colgroup>
            <col />
            <col className="procs__col-pid" />
            <col className="procs__col-num" />
            <col className="procs__col-num" />
            <col className="procs__col-num" />
            <col className="procs__col-act" />
          </colgroup>
          <thead>
            <tr>
              {COLUMNS.map((column) => {
                const active = sort.key === column.key;
                const figure = headerFigure(column.key);
                return (
                  <th
                    key={column.key}
                    scope="col"
                    className={cx(column.numeric && "is-numeric", active && "is-sorted")}
                    aria-sort={active ? (sort.descending ? "descending" : "ascending") : "none"}
                  >
                    <button type="button" className="procs__sort" onClick={() => toggleSort(column.key)}>
                      {figure && <span className="procs__head-figure numeric">{figure}</span>}
                      <span className="procs__head-label">
                        {column.label}
                        {active &&
                          (sort.descending ? <IconChevronDown size={11} /> : <IconChevronUp size={11} />)}
                      </span>
                    </button>
                  </th>
                );
              })}
              <th scope="col" className="procs__col-act">
                <span className="sr-only">Actions</span>
              </th>
            </tr>
          </thead>
          {groups.map((group) => (
            <tbody key={group.id}>
              {grouping === "grouped" && (
                <tr className="procs__group">
                  <th scope="rowgroup" colSpan={6}>
                    {group.title} <span className="procs__group-count numeric">({group.rows.length})</span>
                  </th>
                </tr>
              )}
              {group.rows.map((process) => {
                const cpuHeat = heat(process.cpuPercent, 25);
                const memoryHeat = heat(process.workingSetBytes, totalMemory * 0.15);
                const ioHeat = heat(process.ioBytesPerSec, 20 * 1024 * 1024);
                return (
                  <tr
                    key={process.pid}
                    className={cx(selectedPid === process.pid && "is-selected")}
                    onClick={() => setSelectedPid(process.pid)}
                  >
                    <td>
                      <span className="procs__name">
                        <span className="procs__name-text" title={process.path ? displayPath(process.path) : process.name}>
                          {process.name}
                        </span>
                        {process.isCritical && (
                          <Badge tone="neutral">
                            <IconShield size={10} /> Protected
                          </Badge>
                        )}
                        {process.isCyron && <Badge tone="accent">Cyron</Badge>}
                      </span>
                    </td>
                    <td className="is-numeric numeric procs__pid">{process.pid}</td>
                    <td className="is-numeric numeric procs__heat" style={{ "--heat": cpuHeat } as React.CSSProperties}>
                      {process.cpuPercent >= 0.1 ? `${process.cpuPercent.toFixed(1)}%` : "0%"}
                    </td>
                    <td className="is-numeric numeric procs__heat" style={{ "--heat": memoryHeat } as React.CSSProperties}>
                      {formatBytes(process.workingSetBytes)}
                    </td>
                    <td className="is-numeric numeric procs__heat" style={{ "--heat": ioHeat } as React.CSSProperties}>
                      {process.ioBytesPerSec >= 1024 ? formatRate(process.ioBytesPerSec) : "0 KB/s"}
                    </td>
                    <td className="procs__col-act">
                      {process.path && (
                        <IconButton
                          label={`Open file location of ${process.name}`}
                          className="procs__reveal"
                          onClick={(event) => {
                            event.stopPropagation();
                            void api.revealPath(process.path as string).catch(() => undefined);
                          }}
                        >
                          <IconExternal size={13} />
                        </IconButton>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          ))}
        </table>
        {filtered.length === 0 && (
          <EmptyState compact title="No matching processes" description="Try a different name, PID or folder." />
        )}
      </div>
      <p className="perf-note">
        Updated every second while this tab is open. Protected and other-user processes Windows will not
        open are left out. To end a program, use Applications or Hunter Mode, which verify the target first.
      </p>
    </div>
  );
}
