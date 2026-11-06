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
import { IconActivity, IconRefresh, IconTrash } from "../../components/Icons";
import "./activity.css";

const KIND_LABEL: Record<string, string> = {
  scan: "Disk scan",
  "cache-scan": "Cache scan",
  "app-scan": "Application scan",
};

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
  // regains focus so scans recorded in another view are visible on return.
  useEffect(() => {
    const refreshOnFocus = () => {
      if (document.visibilityState === "visible") history.reload();
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
        description="Every disk scan this app has recorded. Scans are read-only, and the history stays on this machine."
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
              History cleared. New scans will appear here as you run them.
            </Banner>
          </div>
        )}

        <Panel bodyClassName="act-summary">
          <Stat
            label="Scans recorded"
            value={loading ? <Skeleton width={44} height={24} /> : formatCount(entries.length)}
            meta={entries.length === 0 ? "No scans yet" : "Newest first"}
          />
          <Stat
            label="Data scanned"
            value={loading ? <Skeleton width={72} height={24} /> : formatBytes(totals.bytes)}
            meta="Logical file size"
          />
          <Stat
            label="Items indexed"
            value={loading ? <Skeleton width={60} height={24} /> : formatCount(totals.items)}
            meta="Files counted"
          />
          <Stat
            label="Last scan"
            value={loading ? <Skeleton width={70} height={24} /> : relativeTime(lastScan?.at)}
            meta={lastScan ? formatDateTime(lastScan.at) : "Nothing recorded"}
          />
        </Panel>

        <Panel
          title="Operation history"
          subtitle="Recorded only when a scan finishes or is cancelled. Nothing else is logged."
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
              title="No scans recorded yet"
              description="Run a disk scan from Disk Analyzer. Cyron records the path, how much data it read, how long it took and whether it finished or was cancelled."
            />
          ) : (
            <div className="table-wrap">
              <table className="table">
                <thead>
                  <tr>
                    <th scope="col">Result</th>
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
                      <td className="act-op">
                        <div className="cell-name">
                          <span className="cell-name__title" title={entry.title}>
                            {entry.title}
                          </span>
                          <span
                            className="cell-name__meta"
                            title={entry.detail || KIND_LABEL[entry.kind] || entry.kind}
                          >
                            {entry.detail || KIND_LABEL[entry.kind] || entry.kind}
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
            history removes these records only; it does not touch any file on disk.
          </p>
        )}
      </div>
    </>
  );
}
