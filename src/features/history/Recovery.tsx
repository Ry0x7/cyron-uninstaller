import { useCallback, useEffect, useState } from "react";
import {
  Badge,
  Banner,
  Button,
  EmptyState,
  Panel,
  Skeleton,
} from "../../components/ui";
import { IconCheck, IconRefresh, IconUndo } from "../../components/Icons";
import { api, errorMessage } from "../../lib/api";
import {
  formatBytes,
  formatCount,
  formatDateTime,
  operationTitle,
  plural,
} from "../../lib/format";
import type { CleanupLogEntry, QuarantineBatch } from "../../lib/types";

type Pending = { kind: "restore" | "purge"; batch: QuarantineBatch };
type Status = "loading" | "ready" | "error";

function batchTotal(batches: QuarantineBatch[]): number {
  return batches.reduce((total, batch) => total + batch.bytes, 0);
}

function statusTone(status: CleanupLogEntry["status"]): "safe" | "caution" | "danger" | "neutral" {
  if (status === "completed") return "safe";
  if (status === "cancelled") return "caution";
  if (status === "failed") return "danger";
  return "neutral";
}

export function Recovery() {
  const [batches, setBatches] = useState<QuarantineBatch[]>([]);
  const [log, setLog] = useState<CleanupLogEntry[]>([]);
  const [status, setStatus] = useState<Status>("loading");
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [pending, setPending] = useState<Pending | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(async () => {
    setStatus((current) => (current === "ready" ? current : "loading"));
    try {
      const [nextBatches, nextLog] = await Promise.all([
        api.listQuarantine(),
        api.getCleanupLog(),
      ]);
      setBatches(nextBatches);
      setLog(nextLog);
      setError(null);
      setStatus("ready");
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus((current) => (batches.length > 0 ? current : "error"));
    }
  }, [batches.length]);

  useEffect(() => {
    void load();
    // Initial read only; later reads follow an explicit action.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const confirm = async () => {
    if (!pending || busy) return;
    const { kind, batch } = pending;
    setBusy(batch.runId);
    setNote(null);
    setError(null);
    try {
      if (kind === "restore") {
        const result = await api.restoreQuarantine(batch.runId);
        const detail = result.failed > 0 && result.errors.length > 0 ? ` ${result.errors[0]}` : "";
        setNote(
          `Restored ${plural(result.restored, "file")}${
            result.skipped ? `, ${plural(result.skipped, "skipped")}` : ""
          }${result.failed ? `, ${plural(result.failed, "failed")}` : ""}.${detail}`,
        );
      } else {
        const result = await api.purgeQuarantine(batch.runId);
        setNote(
          `Permanently removed ${plural(result.removedFiles, "quarantined file")} and freed ${formatBytes(
            result.freedBytes,
          )}.`,
        );
      }
      setPending(null);
      await load();
      window.dispatchEvent(new Event("cyron:history-changed"));
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setBusy(null);
    }
  };

  const loading = status === "loading" && batches.length === 0 && log.length === 0;

  return (
    <div className="recovery">
      <Panel
        title="Quarantine"
        subtitle="Files moved out of caches stay here until you restore or purge them. Restoring never overwrites an existing file."
        actions={
          <Button size="sm" icon={<IconRefresh size={14} />} onClick={() => void load()}>
            Refresh
          </Button>
        }
      >
        {note && <Banner tone="safe">{note}</Banner>}
        {error && (
          <Banner tone="danger">
            <strong>Recovery could not complete.</strong> {error}
          </Banner>
        )}

        {pending && (
          <Banner
            tone={pending.kind === "purge" ? "danger" : "caution"}
            actions={
              <>
                <Button size="sm" variant="ghost" onClick={() => setPending(null)} disabled={busy !== null}>
                  Cancel
                </Button>
                <Button
                  size="sm"
                  variant={pending.kind === "purge" ? "danger" : "primary"}
                  loading={busy === pending.batch.runId}
                  onClick={() => void confirm()}
                >
                  {pending.kind === "purge" ? "Delete permanently" : "Restore files"}
                </Button>
              </>
            }
          >
            {pending.kind === "purge" ? (
              <>
                <strong>Permanently delete this batch?</strong>{" "}
                {plural(pending.batch.fileCount, "file")} ({formatBytes(pending.batch.bytes)}) will be
                unrecoverable.
              </>
            ) : (
              <>
                <strong>Restore this batch?</strong> {plural(pending.batch.fileCount, "file")} will go
                back to the original path. Existing files are never overwritten.
              </>
            )}
          </Banner>
        )}

        {loading ? (
          <div className="recovery-skeleton" aria-busy="true" aria-label="Loading quarantine">
            {[0, 1, 2].map((index) => (
              <div className="recovery-skeleton__row" key={index}>
                <Skeleton width={180} height={12} />
                <Skeleton width="34%" height={10} />
                <Skeleton width={70} height={10} />
              </div>
            ))}
          </div>
        ) : batches.length === 0 ? (
          <EmptyState
            compact
            icon={IconCheck}
            title="Nothing is waiting in quarantine"
            description="Confirmed cache cleanups appear here with their recoverable files, size and expiry."
          />
        ) : (
          <>
            <div className="recovery-summary">
              <span>
                <strong>{plural(batches.length, "batch", "batches")}</strong> recoverable
              </span>
              <span>{formatBytes(batchTotal(batches))} in quarantine</span>
            </div>
            <ul className="recovery-list">
              {batches.map((batch) => (
                <li className="recovery-row" key={batch.runId}>
                  <span className="recovery-row__icon" aria-hidden="true">
                    <IconUndo size={16} />
                  </span>
                  <div className="recovery-row__main">
                    <span className="recovery-row__title">
                      {formatCount(batch.fileCount)} files · {formatBytes(batch.bytes)}
                    </span>
                    <span className="recovery-row__path" title={batch.dir}>
                      {batch.dir}
                    </span>
                    <span className="recovery-row__meta">
                      Created {formatDateTime(batch.createdAt)} · expires {formatDateTime(batch.expiresAt)}
                    </span>
                  </div>
                  <div className="recovery-row__actions">
                    <Button
                      size="sm"
                      onClick={() => {
                        setNote(null);
                        setPending({ kind: "restore", batch });
                      }}
                      disabled={busy !== null}
                    >
                      Restore
                    </Button>
                    <Button
                      size="sm"
                      variant="danger"
                      onClick={() => {
                        setNote(null);
                        setPending({ kind: "purge", batch });
                      }}
                      disabled={busy !== null}
                    >
                      Purge
                    </Button>
                  </div>
                </li>
              ))}
            </ul>
          </>
        )}
      </Panel>

      <Panel
        title="Cleanup log"
        subtitle="One line per run, kept on this PC. Per-file detail stays in the quarantine manifest."
        actions={
          <Button size="sm" icon={<IconRefresh size={14} />} onClick={() => void load()}>
            Refresh
          </Button>
        }
      >
        {loading ? (
          <div className="recovery-skeleton" aria-busy="true" aria-label="Loading cleanup log">
            {[0, 1, 2].map((index) => (
              <div className="recovery-skeleton__row" key={index}>
                <Skeleton width={130} height={11} />
                <Skeleton width="40%" height={10} />
                <Skeleton width={64} height={10} />
              </div>
            ))}
          </div>
        ) : log.length === 0 ? (
          <EmptyState
            compact
            title="No cleanup runs yet"
            description="Confirmed cleanups appear here with the bytes moved and the per-item outcome."
          />
        ) : (
          <ul className="recovery-log">
            {log.map((entry) => (
              <li className="recovery-log__row" key={entry.id}>
                <span className="recovery-log__when">{formatDateTime(entry.at)}</span>
                <span className="recovery-log__title">
                  {operationTitle(entry.title, "Cleanup")}
                </span>
                <span className="recovery-log__detail">{entry.detail}</span>
                <span className="recovery-log__bytes">{formatBytes(entry.bytes)}</span>
                <Badge tone={statusTone(entry.status)}>{entry.status}</Badge>
              </li>
            ))}
          </ul>
        )}
      </Panel>
    </div>
  );
}
