import { useEffect, useMemo, useState } from "react";
import { api, errorMessage } from "../../lib/api";
import {
  formatBytes,
  formatCount,
  formatDateTime,
  plural,
  relativeTime,
} from "../../lib/format";
import { useAsync } from "../../lib/hooks";
import type { HistoryEntry } from "../../lib/types";
import {
  Badge,
  Banner,
  Button,
  EmptyState,
  Panel,
  Skeleton,
  Stat,
  ViewHeader,
  type BadgeTone,
} from "../../components/ui";
import {
  IconActivity,
  IconCleaner,
  IconRefresh,
  IconTrash,
} from "../../components/Icons";
import "./activity.css";

const KIND_LABEL: Record<string, string> = {
  scan: "Disk scan",
  cleanup: "Cleanup",
  "cache-scan": "Cache scan",
  "app-scan": "Application scan",
};

const KIND_TONE: Record<string, BadgeTone> = {
  scan: "neutral",
  cleanup: "accent",
};

const KIND_ICON: Record<string, typeof IconActivity> = {
  scan: IconActivity,
  cleanup: IconCleaner,
};

function kindLabel(kind: string): string {
  return KIND_LABEL[kind] ?? "Operation";
}

/**
 * Older entries may carry an empty detail. Cleanup runs still explain
 * themselves, and a failed or cancelled run never claims files were moved.
 */
function describeEntry(entry: HistoryEntry): string {
  if (entry.detail) return entry.detail;
  if (entry.kind === "cleanup") {
    if (entry.status === "completed") {
      return `${formatCount(entry.items)} files moved to quarantine · ${formatBytes(entry.bytes)} · restorable for 7 days`;
    }
    if (entry.status === "cancelled") {
      return `Stopped early. Files moved before the stop stay in quarantine for 7 days`;
    }
    return "No files were moved. Cyron reported the failure instead of retrying blindly";
  }
  return kindLabel(entry.kind);
}

const STATUS_TONE: Record<HistoryEntry["status"], BadgeTone> = {
  completed: "safe",
  cancelled: "caution",
  failed: "danger",
};

const STATUS_LABEL: Record<HistoryEntry["status"], string> = {
  completed: "Completed",
  cancelled: "Cancelled",
  failed: "Failed",
};

export function Activity() {
  const history = useAsync<HistoryEntry[]>(() => api.getHistory(), []);
  const [lastEntries, setLastEntries] = useState<HistoryEntry[] | null>(null);
  const [confirmingClear, setConfirmingClear] = useState(false);
  const [clearing, setClearing] = useState(false);
  const [clearError, setClearError] = useState<string | null>(null);
  const [cleared, setCleared] = useState(false);

  useEffect(() => {
    if (history.data) setLastEntries(history.data);
  }, [history.data]);

  // The shell keeps views mounted while hidden. Refresh when the desktop window
  // regains focus so scans recorded in another view are visible on return. A
  // single focus change fires both events; the timestamp guard keeps it to one read.
  useEffect(() => {
    if (!confirmingClear || clearing) return;
    const cancelOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setConfirmingClear(false);
    };
    window.addEventListener("keydown", cancelOnEscape);
    return () => window.removeEventListener("keydown", cancelOnEscape);
  }, [confirmingClear, clearing]);

  useEffect(() => {
    let lastRefresh = 0;
    const refreshOnFocus = () => {
      if (document.visibilityState !== "visible") return;
      const now = Date.now();
      if (now - lastRefresh < 500) return;
      lastRefresh = now;
      history.reload();
    };
    window.addEventListener("focus", refreshOnFocus);
    document.addEventListener("visibilitychange", refreshOnFocus);
    return () => {
      window.removeEventListener("focus", refreshOnFocus);
      document.removeEventListener("visibilitychange", refreshOnFocus);
    };
  }, [history.reload]);

  const entries = useMemo(
    () =>
      [...(history.data ?? lastEntries ?? [])].sort(
        (a, b) => b.at - a.at || a.id.localeCompare(b.id),
      ),
    [history.data, lastEntries],
  );

  const totals = useMemo(
    () =>
      entries.reduce(
        (acc, entry) => {
          acc.bytes += entry.bytes;
          acc.items += entry.items;
          return acc;
        },
        { bytes: 0, items: 0 },
      ),
    [entries],
  );

  async function handleClear() {
    if (clearing) return;
    setClearing(true);
    setClearError(null);
    try {
      await api.clearHistory();
      setConfirmingClear(false);
      setCleared(true);
      history.reload();
      window.dispatchEvent(new Event("cyron:history-changed"));
    } catch (error) {
      setClearError(errorMessage(error));
      setConfirmingClear(false);
    } finally {
      setClearing(false);
    }
  }

  function refresh() {
    setClearError(null);
    setCleared(false);
    setConfirmingClear(false);
    history.reload();
  }

  function beginClear() {
    setClearError(null);
    setCleared(false);
    setConfirmingClear(true);
  }

  const loading = history.status === "loading" && history.data === null;
  const stale = Boolean(history.error && entries.length > 0);
  const lastScan = entries[0];

  return (
    <>
      <ViewHeader
        title="Activity"
        description="Every scan and cleanup this app has recorded. Scans only read metadata; cleanup moves files into a restorable quarantine. The history stays on this machine."
        actions={
          <Button
            variant="secondary"
            size="sm"
            icon={<IconRefresh size={14} />}
            loading={history.status === "loading" && history.data !== null}
            onClick={refresh}
          >
            Refresh
          </Button>
        }
      />

      <div className="view-body">
        {history.error && (stale ? (
          <Banner
            tone="caution"
            actions={
              <Button variant="ghost" size="sm" onClick={refresh}>
                Retry
              </Button>
            }
          >
            <strong>Showing the last successful history.</strong> Refresh failed. {history.error}
          </Banner>
        ) : (
          <Banner
            tone="danger"
            actions={
              <Button variant="ghost" size="sm" onClick={refresh}>
                Retry
              </Button>
            }
          >
            <strong>History could not be loaded.</strong> {history.error}
          </Banner>
        ))}

        {clearError && (
          <div role="alert">
            <Banner tone="danger">
              <strong>History was not cleared.</strong> {clearError}
            </Banner>
          </div>
        )}

        {cleared && (
          <div role="status">
            <Banner tone="safe">
              History cleared. New scans and cleanups will appear here as you run them.
            </Banner>
          </div>
        )}

        <Panel bodyClassName="act-summary">
          <Stat
            label="Operations recorded"
            value={loading ? <Skeleton width={44} height={24} /> : formatCount(entries.length)}
            meta={entries.length === 0 ? "Nothing yet" : "Newest first"}
          />
          <Stat
            label="Data processed"
            value={loading ? <Skeleton width={72} height={24} /> : formatBytes(totals.bytes)}
            meta="Scanned plus quarantined bytes"
          />
          <Stat
            label="Items counted"
            value={loading ? <Skeleton width={60} height={24} /> : formatCount(totals.items)}
            meta="Files either way"
          />
          <Stat
            label="Last operation"
            value={loading ? <Skeleton width={70} height={24} /> : relativeTime(lastScan?.at)}
            meta={lastScan ? formatDateTime(lastScan.at) : "Nothing recorded"}
          />
        </Panel>

        <Panel
          title="Operation history"
          subtitle="Recorded when a scan or cleanup finishes, is cancelled or fails. Cyron logs outcomes, never file contents."
          flush
          actions={
            entries.length > 0 &&
            (confirmingClear ? (
              <div className="act-confirm">
                <span className="act-confirm__text">Delete all entries?</span>
                <Button
                  variant="danger"
                  size="sm"
                  loading={clearing}
                  disabled={clearing}
                  onClick={handleClear}
                >
                  Clear all
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={clearing}
                  onClick={() => setConfirmingClear(false)}
                >
                  Cancel
                </Button>
              </div>
            ) : (
              <Button
                variant="ghost"
                size="sm"
                icon={<IconTrash size={14} />}
                onClick={beginClear}
              >
                Clear history
              </Button>
            ))
          }
        >
          {loading ? (
            <div className="act-loading" aria-busy="true" aria-label="Loading history">
              {[0, 1, 2].map((index) => (
                <div className="act-loading__row" key={index}>
                  <Skeleton width={64} height={18} />
                  <Skeleton width={`${44 - index * 6}%`} height={13} />
                  <Skeleton width={66} height={12} />
                </div>
              ))}
            </div>
          ) : history.error && !stale ? (
            <EmptyState
              compact
              icon={IconActivity}
              title="History unavailable"
              description="Cyron could not read its local operation log. Retry above; scanning itself is unaffected."
            />
          ) : entries.length === 0 ? (
            <EmptyState
              icon={IconActivity}
              title="No operations recorded yet"
              description="Run a disk scan from Disk Analyzer, or move a cache into quarantine from Cleaner. Cyron records the target, how much data was involved, the outcome and whether files can still be restored."
            />
          ) : (
            <div className="table-wrap">
              <table className="table">
                <thead>
                  <tr>
                    <th scope="col">Result</th>
                    <th scope="col">Type</th>
                    <th scope="col">Operation</th>
                    <th scope="col" className="numeric">
                      Data
                    </th>
                    <th scope="col" className="numeric">
                      Items
                    </th>
                    <th scope="col" className="act-when">
                      When
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {entries.map((entry) => (
                    <tr key={entry.id}>
                      <td>
                        <Badge tone={STATUS_TONE[entry.status]}>{STATUS_LABEL[entry.status]}</Badge>
                      </td>
                      <td>
                        <KindBadge kind={entry.kind} />
                      </td>
                      <td className="act-op">
                        <div className="cell-name">
                          <span className="cell-name__title" title={entry.title}>
                            {entry.title}
                          </span>
                          <span className="cell-name__meta" title={describeEntry(entry)}>
                            {describeEntry(entry)}
                          </span>
                        </div>
                      </td>
                      <td className="numeric">{formatBytes(entry.bytes)}</td>
                      <td className="numeric">{formatCount(entry.items)}</td>
                      <td>
                        <div className="act-when__stack" title={formatDateTime(entry.at)}>
                          <span>{relativeTime(entry.at)}</span>
                          <span className="act-when__absolute">{formatDateTime(entry.at)}</span>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </Panel>

        {entries.length > 0 && (
          <p className="act-footnote">
            {plural(entries.length, "entry", "entries")} kept in this profile. Clearing the
            history removes these records only; it does not touch any file on disk and does not
            empty the quarantine.
          </p>
        )}
      </div>
    </>
  );
}

/** Unknown kinds keep rendering instead of leaving an empty cell. */
function KindBadge({ kind }: { kind: string }) {
  const Icon = KIND_ICON[kind] ?? IconActivity;
  return (
    <Badge tone={KIND_TONE[kind] ?? "neutral"}>
      <Icon size={11} />
      {kindLabel(kind)}
    </Badge>
  );
}
