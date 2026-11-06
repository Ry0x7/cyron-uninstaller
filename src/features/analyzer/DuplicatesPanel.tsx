import { useEffect, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { Banner, Button, EmptyState, Select, Spinner } from "../../components/ui";
import { IconAlert, IconExternal, IconSearch } from "../../components/Icons";
import { api, errorMessage } from "../../lib/api";
import {
  formatBytes,
  formatCount,
  formatDateTime,
  formatDuration,
  formatPercent,
  plural,
  truncateMiddle,
} from "../../lib/format";
import {
  DUPLICATE_EVENT,
  type DuplicateGroup,
  type DuplicateProgress,
  type DuplicateReport,
} from "../../lib/types";
import "./duplicates.css";

const MIN_SIZE_OPTIONS = [
  { value: String(1_048_576), label: "1 MB and larger" },
  { value: String(10_485_760), label: "10 MB and larger" },
  { value: String(104_857_600), label: "100 MB and larger" },
];

const PHASE_LABEL: Record<string, string> = {
  index: "Indexing files by size",
  partial: "Hashing the first and last block",
  full: "Verifying full file contents",
};

const GROUP_PAGE = 50;

function createJobId(): string {
  if (typeof crypto.randomUUID === "function") return crypto.randomUUID();
  const bytes = new Uint8Array(16);
  if (typeof crypto.getRandomValues === "function") crypto.getRandomValues(bytes);
  else for (let index = 0; index < bytes.length; index += 1) bytes[index] = Math.floor(Math.random() * 256);
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

type Phase = "idle" | "running" | "done";

export function DuplicatesPanel({
  rootPath,
  includeHidden,
  exclusions,
}: {
  rootPath: string;
  includeHidden: boolean;
  exclusions: string[];
}) {
  const [phase, setPhase] = useState<Phase>("idle");
  const [minSize, setMinSize] = useState(String(10_485_760));
  const [progress, setProgress] = useState<DuplicateProgress | null>(null);
  const [report, setReport] = useState<DuplicateReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [cancelling, setCancelling] = useState(false);
  const [elapsed, setElapsed] = useState(0);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [visibleGroups, setVisibleGroups] = useState(GROUP_PAGE);
  const [revealError, setRevealError] = useState<string | null>(null);
  const [revealing, setRevealing] = useState<string | null>(null);

  const jobRef = useRef<string | null>(null);
  const runningRef = useRef(false);
  const unlistenRef = useRef<UnlistenFn | null>(null);
  const mountedRef = useRef(true);
  const startedAtRef = useRef(0);
  const revealTokenRef = useRef(0);
  const scopeKey = JSON.stringify([rootPath, includeHidden, exclusions]);
  const scopeKeyRef = useRef(scopeKey);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      unlistenRef.current?.();
      unlistenRef.current = null;
      if (runningRef.current && jobRef.current) {
        runningRef.current = false;
        void api.cancelDuplicateScan(jobRef.current).catch(() => undefined);
      }
    };
  }, []);

  // Results belong to the exact root/options that produced them. If the parent
  // changes scan scope while this panel is mounted, cancel the old job and do
  // not leave its results presented as if they described the new folder.
  useEffect(() => {
    if (scopeKeyRef.current === scopeKey) return;
    scopeKeyRef.current = scopeKey;
    if (jobRef.current) void api.cancelDuplicateScan(jobRef.current).catch(() => undefined);
    runningRef.current = false;
    jobRef.current = null;
    unlistenRef.current?.();
    unlistenRef.current = null;
    resetRun();
    setPhase("idle");
  }, [scopeKey]);

  useEffect(() => {
    if (phase !== "running") return;
    startedAtRef.current = Date.now();
    setElapsed(0);
    const timer = window.setInterval(() => setElapsed(Date.now() - startedAtRef.current), 250);
    return () => window.clearInterval(timer);
  }, [phase]);

  const resetRun = () => {
    setProgress(null);
    setReport(null);
    setError(null);
    setExpanded(new Set());
    setVisibleGroups(GROUP_PAGE);
    setRevealError(null);
    setCancelling(false);
  };

  const run = async () => {
    if (phase === "running" || !rootPath) return;
    resetRun();
    setPhase("running");
    const jobId = createJobId();
    const runScopeKey = scopeKey;
    let runUnlisten: UnlistenFn | null = null;
    jobRef.current = jobId;
    runningRef.current = true;

    try {
      runUnlisten = await listen<DuplicateProgress>(DUPLICATE_EVENT, (event) => {
        if (
          event.payload.jobId !== jobId ||
          !runningRef.current ||
          scopeKeyRef.current !== runScopeKey
        ) return;
        setProgress(event.payload);
      });
      if (!mountedRef.current || scopeKeyRef.current !== runScopeKey) {
        runUnlisten();
        return;
      }
      unlistenRef.current = runUnlisten;

      const result = await api.startDuplicateScan(jobId, rootPath, {
        minSize: Number(minSize),
        includeHidden,
        exclusions,
      });
      if (!mountedRef.current || scopeKeyRef.current !== runScopeKey) return;
      runningRef.current = false;
      setReport(result);
      setPhase("done");
    } catch (scanError) {
      if (!mountedRef.current || scopeKeyRef.current !== runScopeKey) return;
      runningRef.current = false;
      setError(errorMessage(scanError));
      setPhase("done");
    } finally {
      runUnlisten?.();
      if (unlistenRef.current === runUnlisten) unlistenRef.current = null;
      if (jobRef.current === jobId) {
        jobRef.current = null;
        runningRef.current = false;
      }
    }
  };

  const cancel = () => {
    const jobId = jobRef.current;
    if (!jobId || cancelling || !runningRef.current) return;
    setCancelling(true);
    void api.cancelDuplicateScan(jobId).catch((cancelError: unknown) => {
      if (!mountedRef.current) return;
      setError(errorMessage(cancelError));
      setCancelling(false);
    });
  };

  const reveal = async (path: string) => {
    if (!mountedRef.current) return;
    const token = revealTokenRef.current + 1;
    revealTokenRef.current = token;
    setRevealing(path);
    setRevealError(null);
    try {
      await api.revealPath(path);
    } catch (revealFailure) {
      if (mountedRef.current && revealTokenRef.current === token) setRevealError(errorMessage(revealFailure));
    } finally {
      if (mountedRef.current && revealTokenRef.current === token) setRevealing(null);
    }
  };

  const toggleGroup = (group: DuplicateGroup) => {
    setExpanded((previous) => {
      const next = new Set(previous);
      if (next.has(group.id)) next.delete(group.id);
      else next.add(group.id);
      return next;
    });
  };

  const groups = report?.groups.slice(0, visibleGroups) ?? [];
  const groupsHidden = report ? report.groups.length - groups.length : 0;
  const wasted = report?.totalWasted ?? 0;
  const shareLabel = (group: DuplicateGroup) =>
    wasted > 0 ? `${formatPercent(group.wasted / wasted)} of recoverable` : "—";

  return (
    <div className="dup">
      <div className="dup-head">
        <div className="dup-head__copy">
          <span className="dup-eyebrow">Duplicate files · read-only</span>
          <p className="dup-lede">
            Cyron groups files by size, hashes the first and last block, then verifies full contents
            with SHA-256. It only reports duplicates — nothing is deleted or moved.
          </p>
        </div>
        <div className="dup-controls">
          <Select
            value={minSize}
            onChange={setMinSize}
            options={MIN_SIZE_OPTIONS}
            ariaLabel="Minimum file size for duplicate detection"
          />
          <Button
            variant="primary"
            onClick={run}
            loading={phase === "running" && !cancelling}
            disabled={phase === "running" || !rootPath}
            icon={<IconSearch size={14} />}
          >
            {report ? "Check again" : "Find duplicates"}
          </Button>
        </div>
      </div>

      <p className="dup-scope" title={rootPath}>
        Scope: <span className="dup-scope__path">{rootPath ? truncateMiddle(rootPath, 72) : "no folder scanned yet"}</span>
      </p>

      {!rootPath && (
        <Banner tone="info">Scan a folder in Map view first, then run duplicate detection on that same folder.</Banner>
      )}

      {phase === "running" && (
        <div className="dup-progress" role="status" aria-live="polite">
          <div className="dup-progress__head">
            <span className="dup-progress__phase">
              <Spinner size={14} />
              {PHASE_LABEL[progress?.phase ?? "index"] ?? "Working"}
            </span>
            <span className="dup-progress__elapsed">{formatDuration(elapsed)}</span>
          </div>
          <div className="dup-progress__metrics">
            <span>{formatCount(progress?.files ?? 0)} files</span>
            <span>{formatBytes(progress?.bytesHashed ?? 0)} hashed</span>
          </div>
          {progress?.currentPath && (
            <p className="dup-progress__path" title={progress.currentPath}>
              {truncateMiddle(progress.currentPath, 88)}
            </p>
          )}
          <div className="dup-progress__actions">
            <Button size="sm" variant="secondary" onClick={cancel} disabled={cancelling}>
              {cancelling ? "Cancelling…" : "Cancel"}
            </Button>
            <span className="dup-progress__note">Hashing can take a while on large folders.</span>
          </div>
        </div>
      )}

      {error && (
        <Banner
          tone="danger"
          actions={
            <Button size="sm" variant="secondary" onClick={run}>
              Retry
            </Button>
          }
        >
          Duplicate detection stopped: {error}
        </Banner>
      )}

      {report && (
        <>
          {report.cancelled && (
            <Banner tone="caution">
              The scan was cancelled. Results below cover only the files that were hashed before
              cancellation, so recoverable space is a lower bound.
            </Banner>
          )}
          {report.truncated && (
            <Banner tone="caution">
              This folder produced more candidates than the safety cap allows. Only the largest
              groups are listed and the scan marker is incomplete.
            </Banner>
          )}

          <div className="dup-summary">
            <div className="dup-metric">
              <span className="dup-metric__label">Recoverable</span>
              <span className="dup-metric__value">{formatBytes(report.totalWasted)}</span>
              <span className="dup-metric__meta">if only one copy of each group is kept</span>
            </div>
            <div className="dup-metric">
              <span className="dup-metric__label">Groups</span>
              <span className="dup-metric__value">{formatCount(report.totalGroups)}</span>
              <span className="dup-metric__meta">{formatCount(report.totalFiles)} duplicate files</span>
            </div>
            <div className="dup-metric">
              <span className="dup-metric__label">Indexed</span>
              <span className="dup-metric__value">{formatCount(report.scannedFiles)}</span>
              <span className="dup-metric__meta">{formatCount(report.candidateFiles)} size candidates</span>
            </div>
            <div className="dup-metric">
              <span className="dup-metric__label">Verified</span>
              <span className="dup-metric__value">{formatCount(report.hashedFiles)}</span>
              <span className="dup-metric__meta">full-content hashes · {formatDuration(report.elapsedMs)}</span>
            </div>
          </div>

          {report.errors.length > 0 && (
            <div className="dup-warnings">
              <p className="dup-warnings__title">
                <IconAlert size={13} /> {formatCount(report.errors.length)} locations could not be read
              </p>
              <ul>
                {report.errors.slice(0, 20).map((message) => (
                  <li key={message} title={message}>
                    {message}
                  </li>
                ))}
              </ul>
            </div>
          )}

          {revealError && <Banner tone="danger">{revealError}</Banner>}

          {report.groups.length === 0 ? (
            <EmptyState
              icon={IconSearch}
              title="No duplicate files found"
              description={
                report.scannedFiles === 0
                  ? "No readable files matched the filters in this folder."
                  : "Every size candidate had different content. That is a good result."
              }
            />
          ) : (
            <div className="dup-groups">
              <div className="dup-groups__head">
                <span>Duplicate group</span>
                <span className="dup-groups__count">
                  {formatCount(report.groups.length)} {plural(report.groups.length, "group")}
                </span>
              </div>
              {groups.map((group) => {
                const open = expanded.has(group.id);
                return (
                  <section className={`dup-group${open ? " is-open" : ""}`} key={group.id}>
                    <button
                      type="button"
                      className="dup-group__head"
                      aria-expanded={open}
                      aria-controls={`dup-files-${group.id}`}
                      onClick={() => toggleGroup(group)}
                    >
                      <span className="dup-group__chevron" aria-hidden="true">
                        ›
                      </span>
                      <span className="dup-group__size">{formatBytes(group.size)}</span>
                      <span className="dup-group__title" title={group.files[0]?.path}>
                        {group.files.length} identical copies
                      </span>
                      <span className="dup-group__wasted">{formatBytes(group.wasted)} wasted</span>
                      <span className="dup-group__share">{shareLabel(group)}</span>
                    </button>
                    {open && (
                      <ul className="dup-files" id={`dup-files-${group.id}`}>
                        {group.files.map((file) => (
                          <li className="dup-file" key={file.path}>
                            <div className="dup-file__info">
                              <span className="dup-file__path" title={file.path}>
                                {truncateMiddle(file.path, 96)}
                              </span>
                              <span className="dup-file__meta">
                                {file.hardLink ? "Hard link · shares the same data" : "Modified "}
                                {file.hardLink ? "" : formatDateTime(file.modified)}
                              </span>
                            </div>
                            <Button
                              size="sm"
                              variant="ghost"
                              icon={<IconExternal size={13} />}
                              onClick={() => void reveal(file.path)}
                              loading={revealing === file.path}
                              aria-label={`Show ${file.path} in Explorer`}
                            >
                              Show
                            </Button>
                          </li>
                        ))}
                      </ul>
                    )}
                  </section>
                );
              })}
              {groupsHidden > 0 && (
                <div className="dup-groups__more">
                  <Button
                    size="sm"
                    variant="secondary"
                    onClick={() => setVisibleGroups((count) => count + GROUP_PAGE)}
                  >
                    Load {Math.min(groupsHidden, GROUP_PAGE)} more
                  </Button>
                  <span>{formatCount(groupsHidden)} groups not shown yet</span>
                </div>
              )}
            </div>
          )}
        </>
      )}

      {phase === "idle" && !report && (
        <p className="dup-footnote">
          Hard links are counted once. Files that change size while hashing are skipped rather than
          reported as duplicates.
        </p>
      )}
    </div>
  );
}
