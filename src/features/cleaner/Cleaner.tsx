import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  Badge,
  Banner,
  Button,
  Drawer,
  EmptyState,
  Eyebrow,
  KeyValue,
  Panel,
  ProgressBar,
  RiskBadge,
  SearchField,
  Segmented,
  Select,
  Skeleton,
  Stat,
  ViewHeader,
} from "../../components/ui";
import {
  IconCheck,
  IconChevronRight,
  IconCleaner,
  IconInfo,
  IconLock,
  IconRefresh,
} from "../../components/Icons";
import { api, errorMessage, invalidateCaches, loadCaches } from "../../lib/api";
import {
  CLEANUP_GROUPS,
  canConfirmCleanup,
  groupForCategory,
  groupTargets,
  isSelectable,
  selectedTotals,
  toggleSelection,
} from "../../lib/cleanup";
import {
  formatBytes,
  formatCount,
  formatDateTime,
  formatPercent,
  plural,
  relativeTime,
  truncateMiddle,
} from "../../lib/format";
import { RISK_DESCRIPTION, RISK_LABEL, RISK_ORDER } from "../../lib/risk";
import { useSettings } from "../../lib/settings";
import {
  CLEANUP_EVENT,
  type CacheInventory,
  type CacheTarget,
  type CleanupLogEntry,
  type CleanupPreview,
  type CleanupProgress,
  type CleanupReport,
  type QuarantineBatch,
  type Risk,
} from "../../lib/types";
import "./cleaner.css";

type RiskFilter = "all" | Risk;
type Status = "loading" | "ready" | "error";
type Tab = "cleanup" | "quarantine" | "history";
type Pending = { kind: "restore" | "purge"; batch: QuarantineBatch };

const CATEGORY_LABEL: Record<string, string> = {
  ai: "AI tools",
  ide: "Editors & IDEs",
  packages: "Package managers",
  containers: "Containers & WSL",
  windows: "Windows & browsers",
};

const RESULT_PAGE = 200;

function categoryOf(target: CacheTarget): string {
  return target.category || "other";
}

function categoryLabel(category: string): string {
  return CATEGORY_LABEL[category] ?? `Other · ${category}`;
}

function bySize(a: CacheTarget, b: CacheTarget): number {
  return b.size - a.size || a.tool.localeCompare(b.tool) || a.label.localeCompare(b.label);
}

function detailId(target: CacheTarget): string {
  return `cl-detail-${target.id.replace(/[^a-zA-Z0-9_-]/g, "-")}`;
}

function groupLabel(key: string): string {
  return CLEANUP_GROUPS.find((group) => group.key === key)?.label ?? key;
}

function batchTotal(batches: QuarantineBatch[] | null): number {
  return (batches ?? []).reduce((total, batch) => total + batch.bytes, 0);
}

export function Cleaner() {
  const { settings } = useSettings();
  const exclusions = settings.exclusions;
  const [inventory, setInventory] = useState<CacheInventory | null>(null);
  const [status, setStatus] = useState<Status>("loading");
  const [error, setError] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);

  const [search, setSearch] = useState("");
  const [riskFilter, setRiskFilter] = useState<RiskFilter>("all");
  const [categoryFilter, setCategoryFilter] = useState<string>("all");
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [tab, setTab] = useState<Tab>("cleanup");

  const [readOnly, setReadOnly] = useState<boolean | null>(null);
  const [modeBusy, setModeBusy] = useState(false);
  const [modeError, setModeError] = useState<string | null>(null);

  const [preview, setPreview] = useState<CleanupPreview | null>(null);
  const [previewOpen, setPreviewOpen] = useState(false);
  const [previewLoading, setPreviewLoading] = useState(false);
  const [previewError, setPreviewError] = useState<string | null>(null);

  const [confirmed, setConfirmed] = useState(false);
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<CleanupProgress | null>(null);
  const [report, setReport] = useState<CleanupReport | null>(null);
  const [runError, setRunError] = useState<string | null>(null);

  const [quarantine, setQuarantine] = useState<QuarantineBatch[] | null>(null);
  const [quarantineError, setQuarantineError] = useState<string | null>(null);
  const [pending, setPending] = useState<Pending | null>(null);
  const [busyBatch, setBusyBatch] = useState<string | null>(null);
  const [sideNote, setSideNote] = useState<string | null>(null);

  const [log, setLog] = useState<CleanupLogEntry[] | null>(null);
  const [logError, setLogError] = useState<string | null>(null);

  const mounted = useRef(true);
  const request = useRef(0);
  const previewRequest = useRef(0);
  const runningRef = useRef(false);
  const activeRun = useRef<string | null>(null);
  const unlistenRef = useRef<UnlistenFn | null>(null);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      runningRef.current = false;
      unlistenRef.current?.();
      unlistenRef.current = null;
    };
  }, []);

  const load = useCallback(async (force: boolean) => {
    const id = ++request.current;
    if (force) setRefreshing(true);
    else setStatus("loading");
    try {
      const data = await loadCaches(force);
      if (!mounted.current || request.current !== id) return;
      setInventory(data);
      setError(null);
      setStatus("ready");
    } catch (caught) {
      if (!mounted.current || request.current !== id) return;
      setError(errorMessage(caught));
      setStatus("error");
    } finally {
      if (mounted.current && request.current === id) setRefreshing(false);
    }
  }, []);

  const loadMode = useCallback(async () => {
    try {
      const mode = await api.getCleanupMode();
      if (mounted.current) {
        setReadOnly(mode);
        setModeError(null);
      }
    } catch (caught) {
      if (mounted.current) setModeError(errorMessage(caught));
    }
  }, []);

  const loadQuarantine = useCallback(async () => {
    try {
      const batches = await api.listQuarantine();
      if (!mounted.current) return;
      setQuarantine(batches);
      setQuarantineError(null);
    } catch (caught) {
      if (!mounted.current) return;
      setQuarantineError(errorMessage(caught));
    }
  }, []);

  const loadLog = useCallback(async () => {
    try {
      const entries = await api.getCleanupLog();
      if (!mounted.current) return;
      setLog(entries);
      setLogError(null);
    } catch (caught) {
      if (!mounted.current) return;
      setLogError(errorMessage(caught));
    }
  }, []);

  useEffect(() => {
    void load(false);
    void loadMode();
    void loadQuarantine();
    void loadLog();
  }, [load, loadMode, loadQuarantine, loadLog]);

  const existing = useMemo(
    () => (inventory?.targets ?? []).filter((target) => target.exists && target.path),
    [inventory],
  );

  const totals = useMemo(() => {
    const sum = (risk: Risk) =>
      existing.reduce((acc, target) => (target.risk === risk ? acc + target.size : acc), 0);
    return {
      size: existing.reduce((acc, target) => acc + target.size, 0),
      safe: sum("safe"),
      caution: sum("caution"),
      protected: sum("protected"),
      max: Math.max(1, ...existing.map((target) => target.size)),
    };
  }, [existing]);

  const counts = useMemo(() => {
    const result: Record<RiskFilter, number> = {
      all: existing.length,
      safe: 0,
      caution: 0,
      protected: 0,
    };
    for (const target of existing) result[target.risk] += 1;
    return result;
  }, [existing]);

  const categoriesPresent = useMemo(() => {
    const present = new Set(existing.map(categoryOf));
    return Array.from(present).sort((a, b) => a.localeCompare(b));
  }, [existing]);

  const visible = useMemo(() => {
    const query = search.trim().toLowerCase();
    return existing.filter((target) => {
      if (riskFilter !== "all" && target.risk !== riskFilter) return false;
      if (categoryFilter !== "all" && categoryOf(target) !== categoryFilter) return false;
      if (!query) return true;
      return `${target.tool} ${target.label} ${target.path}`.toLowerCase().includes(query);
    });
  }, [existing, search, riskFilter, categoryFilter]);

  const groups = useMemo(
    () => groupTargets(visible.filter((target) => target.risk !== "protected")),
    [visible],
  );

  const protectedItems = useMemo(
    () => visible.filter((target) => target.risk === "protected").sort(bySize),
    [visible],
  );

  const selectableVisible = useMemo(() => visible.filter(isSelectable), [visible]);
  const selection = useMemo(() => selectedTotals(existing, selected), [existing, selected]);

  const appliedFilters = search.trim() !== "" || riskFilter !== "all" || categoryFilter !== "all";

  const clearFilters = () => {
    setSearch("");
    setRiskFilter("all");
    setCategoryFilter("all");
  };

  const toggleExpanded = (id: string) => {
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const toggleTarget = (target: CacheTarget) => {
    if (!isSelectable(target)) return;
    setSelected((current) => toggleSelection(current, target.id));
  };

  const toggleGroup = (items: CacheTarget[], on: boolean) => {
    setSelected((current) => {
      const next = new Set(current);
      for (const target of items) {
        if (!isSelectable(target)) continue;
        if (on) next.add(target.id);
        else next.delete(target.id);
      }
      return next;
    });
  };

  const enableCleaning = async () => {
    setModeBusy(true);
    setModeError(null);
    try {
      const mode = await api.setCleanupMode(false);
      if (!mounted.current) return;
      setReadOnly(mode);
    } catch (caught) {
      if (mounted.current) setModeError(errorMessage(caught));
    } finally {
      if (mounted.current) setModeBusy(false);
    }
  };

  const openPreview = async () => {
    const ids = Array.from(selected);
    if (ids.length === 0) return;
    const ticket = ++previewRequest.current;
    setPreviewLoading(true);
    setPreviewError(null);
    setReport(null);
    setRunError(null);
    setProgress(null);
    setConfirmed(false);
    setPreviewOpen(true);
    try {
      // Settings exclusions are enforced by the backend, which revalidates
      // every one of them; the preview simply reflects them.
      const data = await api.previewCleanup(ids, exclusions);
      if (!mounted.current || previewRequest.current !== ticket) return;
      setPreview(data);
    } catch (caught) {
      if (!mounted.current || previewRequest.current !== ticket) return;
      setPreview(null);
      setPreviewError(errorMessage(caught));
    } finally {
      if (mounted.current && previewRequest.current === ticket) setPreviewLoading(false);
    }
  };

  const closePreview = () => {
    if (running) return;
    previewRequest.current += 1;
    setPreviewOpen(false);
    setConfirmed(false);
    setPreviewError(null);
  };

  const runCleanup = async () => {
    if (!preview || running) return;
    const ticket = ++previewRequest.current;
    runningRef.current = true;
    activeRun.current = null;
    setRunning(true);
    setRunError(null);
    setReport(null);
    setProgress(null);

    try {
      unlistenRef.current?.();
      unlistenRef.current = await listen<CleanupProgress>(CLEANUP_EVENT, (event) => {
        if (!runningRef.current) return;
        if (activeRun.current && activeRun.current !== event.payload.runId) return;
        activeRun.current = event.payload.runId;
        setProgress(event.payload);
      });
      const result = await api.startCleanup(
        preview.planId,
        preview.targets.map((target) => target.id),
      );
      if (!mounted.current || previewRequest.current !== ticket) {
        runningRef.current = false;
        return;
      }
      setReport(result);
      setSelected(new Set());
      invalidateCaches();
      void load(true);
      void loadQuarantine();
      void loadLog();
      void loadMode();
    } catch (caught) {
      if (mounted.current && previewRequest.current === ticket) {
        setRunError(errorMessage(caught));
      }
    } finally {
      runningRef.current = false;
      unlistenRef.current?.();
      unlistenRef.current = null;
      if (mounted.current) setRunning(false);
    }
  };

  const confirmPending = async () => {
    if (!pending) return;
    const { kind, batch } = pending;
    setBusyBatch(batch.runId);
    setSideNote(null);
    try {
      if (kind === "restore") {
        const result = await api.restoreQuarantine(batch.runId);
        if (!mounted.current) return;
        setSideNote(
          `Restored ${plural(result.restored, "file")}${result.skipped ? `, ${plural(result.skipped, "skipped")}` : ""}${result.failed ? `, ${plural(result.failed, "failed")}` : ""}.`,
        );
      } else {
        const result = await api.purgeQuarantine(batch.runId);
        if (!mounted.current) return;
        setSideNote(
          `Removed ${plural(result.removedFiles, "quarantined file")} and freed ${formatBytes(result.freedBytes)}.`,
        );
      }
      setPending(null);
      await loadQuarantine();
      await loadLog();
      invalidateCaches();
      void load(true);
    } catch (caught) {
      if (mounted.current) setSideNote(errorMessage(caught));
    } finally {
      if (mounted.current) setBusyBatch(null);
    }
  };

  const lastChecked = inventory
    ? `${error ? "Last successful check" : "Checked"} ${relativeTime(inventory.scannedAt)}${error ? " · stale" : ""}`
    : "not yet";
  const warnings = inventory?.warnings ?? [];
  const batches = quarantine ?? [];
  const confirmReady =
    preview !== null &&
    canConfirmCleanup({
      readOnly: readOnly === true || preview.readOnly,
      selectedCount: preview.targets.length,
      previewing: previewLoading,
      confirmed,
      running,
    });

  const progressRatio =
    progress && progress.totalFiles > 0
      ? Math.min(1, progress.files / progress.totalFiles)
      : undefined;

  return (
    <>
      <ViewHeader
        title="Cleaner"
        description="Review what each cache costs, then move it to a recoverable quarantine. Nothing is deleted without a preview, and protected data is never included."
        meta={
          <Badge tone={readOnly === true ? "neutral" : "accent"}>
            {readOnly === true ? "Read-only mode" : "Cleaning enabled"}
          </Badge>
        }
        actions={
          <>
            <span className={`small faint${error && inventory ? " cl-stale" : ""}`}>
              {lastChecked}
            </span>
            <Button
              size="sm"
              icon={<IconRefresh size={14} />}
              loading={refreshing}
              onClick={() => void load(true)}
            >
              Refresh
            </Button>
          </>
        }
      />

      <div className="view-body">
        {modeError && (
          <Banner tone="danger">
            <strong>Cleanup mode is unknown.</strong> {modeError}
          </Banner>
        )}

        {readOnly === true && (
          <Banner
            tone="accent"
            actions={
              <Button size="sm" variant="primary" loading={modeBusy} onClick={() => void enableCleaning()}>
                Enable cleaning
              </Button>
            }
          >
            <strong>Read-only mode is on.</strong> Cyron measures and previews only until you enable
            cleaning. Enabling it also requires an explicit confirmation for every run.
          </Banner>
        )}

        <Banner tone="info">
          <strong>Files are moved, not deleted.</strong> Each confirmed cleanup moves the selected
          files into Cyron's quarantine, so you can restore them; purge a batch when you are sure.
          Protected data — model weights, histories, settings, credentials, Docker/WSL data and
          browser profiles — is never offered.
        </Banner>

        {error && (
          <Banner
            tone="danger"
            actions={
              <Button size="sm" loading={refreshing} onClick={() => void load(true)}>
                Retry
              </Button>
            }
          >
            <strong>Could not measure caches.</strong> {error}
          </Banner>
        )}

        {warnings.length > 0 && (
          <Banner tone="caution">
            <strong>{warnings.length === 1 ? "One location" : `${warnings.length} locations`}</strong>{" "}
            could not be measured completely. Sizes below are what Windows allowed Cyron to read.
            <ul className="cl-warning-list">
              {warnings.map((warning) => (
                <li key={warning}>{warning}</li>
              ))}
            </ul>
          </Banner>
        )}

        <div className="cl-tabs">
          <Segmented<Tab>
            ariaLabel="Cleaner sections"
            value={tab}
            onChange={setTab}
            options={[
              { value: "cleanup", label: "Cleanup" },
              { value: "quarantine", label: "Quarantine", count: batches.length },
              { value: "history", label: "History", count: log?.length },
            ]}
          />
        </div>

        {tab === "quarantine" && (
          <QuarantinePanel
            batches={batches}
            loading={quarantine === null && !quarantineError}
            error={quarantineError}
            pending={pending}
            busy={busyBatch}
            note={sideNote}
            onAsk={(kind, batch) => {
              setSideNote(null);
              setPending({ kind, batch });
            }}
            onCancel={() => setPending(null)}
            onConfirm={() => void confirmPending()}
            onRetry={() => void loadQuarantine()}
          />
        )}

        {tab === "history" && (
          <Panel
            title="Cleanup history"
            subtitle="One line per run, kept on this PC. Per-file detail stays in the quarantine manifest."
            actions={
              <Button size="sm" icon={<IconRefresh size={14} />} onClick={() => void loadLog()}>
                Refresh
              </Button>
            }
          >
            {logError && (
              <Banner tone="danger">
                <strong>Could not read the cleanup log.</strong> {logError}
              </Banner>
            )}
            {log === null && !logError ? (
              <div className="cl-skeleton">
                {[0, 1, 2].map((index) => (
                  <div className="cl-skeleton__row" key={index}>
                    <Skeleton width={140} height={11} />
                    <Skeleton width="40%" height={10} />
                    <Skeleton width={64} height={10} />
                  </div>
                ))}
              </div>
            ) : (log?.length ?? 0) === 0 ? (
              <EmptyState
                compact
                icon={IconCleaner}
                title="No cleanup runs yet"
                description="Confirmed cleanups appear here with the bytes moved and the per-item outcome."
              />
            ) : (
              <ul className="cl-log">
                {log?.map((entry) => (
                  <li className="cl-log__row" key={entry.id}>
                    <span className="cl-log__when">{formatDateTime(entry.at)}</span>
                    <span className="cl-log__title">{entry.title}</span>
                    <span className="cl-log__detail">{entry.detail}</span>
                    <span className="cl-log__bytes">{formatBytes(entry.bytes)}</span>
                    <Badge
                      tone={
                        entry.status === "completed"
                          ? "safe"
                          : entry.status === "cancelled"
                            ? "caution"
                            : "danger"
                      }
                    >
                      {entry.status}
                    </Badge>
                  </li>
                ))}
              </ul>
            )}
          </Panel>
        )}

        {tab === "cleanup" && status === "loading" && !inventory ? (
          <CleanerSkeleton />
        ) : tab === "cleanup" && status === "error" && !inventory ? (
          <EmptyState
            icon={IconCleaner}
            title="Cache inventory unavailable"
            description="Cyron could not read the cache locations. Start the desktop backend and try again."
            action={
              <Button variant="primary" onClick={() => void load(true)}>
                Try again
              </Button>
            }
          />
        ) : tab === "cleanup" && existing.length === 0 ? (
          <Panel title="Detected locations">
            <EmptyState
              icon={IconCleaner}
              title={
                warnings.length > 0 || error
                  ? "No cache locations could be measured"
                  : "No known cache locations found"
              }
              description={
                warnings.length > 0 || error
                  ? "Some locations were inaccessible or the latest measurement failed. Review the warning above and refresh; this is not a complete inventory."
                  : "None of the catalogued developer, AI or Windows cache folders exist on this PC yet. Run your tools once and refresh."
              }
              action={
                <Button icon={<IconRefresh size={14} />} onClick={() => void load(true)}>
                  Refresh
                </Button>
              }
            />
          </Panel>
        ) : tab === "cleanup" ? (
          <>
            <Panel
              title="Measured storage by risk"
              subtitle={`${formatCount(existing.length)} measured locations in known tool folders`}
            >
              <div className="cl-summary">
                <div className="cl-summary__grid">
                  <Stat
                    label="Detected"
                    value={formatBytes(totals.size)}
                    meta={`${formatCount(existing.length)} locations`}
                    size="lg"
                  />
                  <Stat
                    label="Safe to regenerate"
                    value={formatBytes(totals.safe)}
                    meta={`${formatPercent(totals.size > 0 ? totals.safe / totals.size : 0)} of detected`}
                  />
                  <Stat
                    label="Needs review"
                    value={formatBytes(totals.caution)}
                    meta="May reset state, affect offline use, or need re-download"
                  />
                  <Stat
                    label="Protected · measured only"
                    value={formatBytes(totals.protected)}
                    meta="Never offered for removal"
                  />
                </div>

                <div className="cl-composition">
                  <div
                    className="cl-bar"
                    role="img"
                    aria-label={`Composition: ${formatBytes(totals.safe)} safe, ${formatBytes(totals.caution)} review, ${formatBytes(totals.protected)} protected`}
                  >
                    <span
                      className="cl-bar__seg cl-bar__seg--safe"
                      style={{ width: `${(totals.safe / Math.max(1, totals.size)) * 100}%` }}
                    />
                    <span
                      className="cl-bar__seg cl-bar__seg--caution"
                      style={{ width: `${(totals.caution / Math.max(1, totals.size)) * 100}%` }}
                    />
                    <span
                      className="cl-bar__seg cl-bar__seg--protected"
                      style={{
                        width: `${Math.max(
                          0,
                          100 -
                            (totals.safe / Math.max(1, totals.size)) * 100 -
                            (totals.caution / Math.max(1, totals.size)) * 100,
                        )}%`,
                      }}
                    />
                  </div>
                  <div className="cl-legend">
                    {RISK_ORDER.map((risk) => (
                      <span className="cl-legend__item" key={risk}>
                        <span className={`cl-legend__dot cl-legend__dot--${risk}`} />
                        {RISK_LABEL[risk]}
                        <strong>
                          {formatBytes(
                            risk === "safe"
                              ? totals.safe
                              : risk === "caution"
                                ? totals.caution
                                : totals.protected,
                          )}
                        </strong>
                      </span>
                    ))}
                  </div>
                </div>
              </div>
            </Panel>

            <Panel
              title="Detected locations"
              subtitle={
                appliedFilters
                  ? `${formatCount(visible.length)} of ${formatCount(existing.length)} shown`
                  : "Select the caches you want to review. Protected rows have no checkbox."
              }
              flush
            >
              <div className="cl-toolbar">
                <SearchField
                  value={search}
                  onChange={setSearch}
                  placeholder="Search tool, label or path"
                  ariaLabel="Search cache locations"
                />
                <Segmented<RiskFilter>
                  ariaLabel="Filter by risk"
                  value={riskFilter}
                  onChange={setRiskFilter}
                  options={[
                    { value: "all", label: "All", count: counts.all },
                    { value: "safe", label: "Safe", count: counts.safe },
                    { value: "caution", label: "Review", count: counts.caution },
                    { value: "protected", label: "Protected", count: counts.protected },
                  ]}
                />
                <Select
                  ariaLabel="Filter by category"
                  value={categoryFilter}
                  onChange={setCategoryFilter}
                  options={[
                    { value: "all", label: "All categories" },
                    ...categoriesPresent.map((category) => ({
                      value: category,
                      label: categoryLabel(category),
                    })),
                  ]}
                />
                <span className="cl-toolbar__spacer" />
                <Button
                  size="sm"
                  onClick={() => toggleGroup(selectableVisible, true)}
                  disabled={selectableVisible.length === 0}
                >
                  Select visible
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => setSelected(new Set())}
                  disabled={selection.count === 0}
                >
                  Clear
                </Button>
              </div>

              {appliedFilters && counts.protected > protectedItems.length && counts.protected > 0 && (
                <div className="cl-filter-note" role="status">
                  Protected data remains measured ({formatBytes(totals.protected)} total) and is never
                  offered for removal; active search or category/risk filters hide some protected rows.
                </div>
              )}

              {visible.length === 0 ? (
                <EmptyState
                  compact
                  icon={IconCleaner}
                  title="No locations match these filters"
                  description="Adjust the search or risk filter to see the measured locations again."
                  action={
                    <Button size="sm" onClick={clearFilters}>
                      Clear filters
                    </Button>
                  }
                />
              ) : (
                <>
                  {groups.map((group) => {
                    const items = group.targets.filter(isSelectable);
                    const allSelected =
                      items.length > 0 && items.every((target) => selected.has(target.id));
                    return (
                      <section
                        className="cl-group"
                        key={group.key}
                        aria-label={groupLabel(group.key)}
                      >
                        <header className="cl-group__head">
                          <div className="cl-group__title">
                            <Eyebrow>{groupLabel(group.key)}</Eyebrow>
                            <span className="cl-group__count">{group.targets.length}</span>
                          </div>
                          <div className="cl-group__side">
                            <button
                              type="button"
                              className="cl-link"
                              onClick={() => toggleGroup(items, !allSelected)}
                              disabled={items.length === 0}
                            >
                              {allSelected ? "Deselect all" : "Select all"}
                            </button>
                            <span className="cl-group__total">{formatBytes(group.bytes)}</span>
                          </div>
                        </header>
                        {group.targets.map((target) => {
                          const isOpen = expanded.has(target.id);
                          const selectable = isSelectable(target);
                          return (
                            <div
                              className={`cl-item${isOpen ? " cl-item--open" : ""}`}
                              key={target.id}
                            >
                              <div className={`cl-row${isOpen ? " cl-row--open" : ""}`}>
                                {selectable ? (
                                  <input
                                    type="checkbox"
                                    className="cl-check"
                                    checked={selected.has(target.id)}
                                    onChange={() => toggleTarget(target)}
                                    aria-label={`Select ${target.tool} ${target.label} (${formatBytes(target.size)})`}
                                  />
                                ) : (
                                  <span className="cl-check cl-check--locked" aria-hidden="true">
                                    <IconLock size={11} />
                                  </span>
                                )}
                                <button
                                  type="button"
                                  className="cl-row__toggle"
                                  aria-expanded={isOpen}
                                  aria-controls={detailId(target)}
                                  onClick={() => toggleExpanded(target.id)}
                                >
                                  <span className="cl-row__name">
                                    <span className="cl-row__tool">{target.tool}</span>
                                    <span className="cl-row__label">{target.label}</span>
                                  </span>
                                  <span className="cl-row__path" title={target.path}>
                                    {truncateMiddle(target.path, 58)}
                                  </span>
                                  <span className="cl-row__size">{formatBytes(target.size)}</span>
                                  <span className="cl-row__files">
                                    {formatCount(target.fileCount)} <small>files</small>
                                  </span>
                                  <span className="cl-row__risk">
                                    <RiskBadge risk={target.risk} />
                                  </span>
                                  <IconChevronRight size={14} className="cl-row__chev" />
                                  <span className="cl-row__bar" aria-hidden="true">
                                    <span style={{ width: `${(target.size / totals.max) * 100}%` }} />
                                  </span>
                                </button>
                              </div>
                              <div className="cl-detail" id={detailId(target)} hidden={!isOpen}>
                                <div className="cl-detail__main">
                                  <div className="cl-detail__path">{target.path}</div>
                                  <p className="cl-detail__note">{target.note}</p>
                                  <p className="cl-detail__impact">
                                    <IconInfo size={14} />
                                    <span>
                                      {target.risk === "protected"
                                        ? RISK_DESCRIPTION.protected
                                        : `${target.risk === "safe" ? "If moved" : "Review first"}: ${target.note}`}
                                    </span>
                                  </p>
                                </div>
                                <dl className="cl-detail__meta">
                                  <div className="cl-meta-row">
                                    <dt>Last modified</dt>
                                    <dd>{formatDateTime(target.lastModified)}</dd>
                                  </div>
                                  <div className="cl-meta-row">
                                    <dt>Files</dt>
                                    <dd>{formatCount(target.fileCount)}</dd>
                                  </div>
                                  <div className="cl-meta-row">
                                    <dt>Folders</dt>
                                    <dd>{formatCount(target.dirCount)}</dd>
                                  </div>
                                  <div className="cl-meta-row">
                                    <dt>Allocated</dt>
                                    <dd>{formatBytes(target.allocated)}</dd>
                                  </div>
                                  <div className="cl-meta-row">
                                    <dt>Risk</dt>
                                    <dd>{RISK_LABEL[target.risk]}</dd>
                                  </div>
                                </dl>
                              </div>
                            </div>
                          );
                        })}
                      </section>
                    );
                  })}

                  {protectedItems.length > 0 && (
                    <section className="cl-protected" aria-label="Protected data">
                      <header className="cl-protected__head">
                        <span className="cl-protected__icon">
                          <IconLock size={15} />
                        </span>
                        <div>
                          <h3 className="cl-protected__title">Protected data Cyron will not touch</h3>
                          <p className="cl-protected__description">
                            {formatBytes(totals.protected)} of conversation histories, model weights,
                            configs, credentials, Docker volumes and WSL distros. They are measured so
                            you understand your disk, never offered for removal, and the backend
                            rejects them even if the interface asked.
                          </p>
                        </div>
                      </header>
                      {protectedItems.map((target) => (
                        <div className="cl-protected__row" key={target.id}>
                          <div className="cl-row__name">
                            <span className="cl-row__tool">{target.tool}</span>
                            <span className="cl-row__label">{target.label}</span>
                          </div>
                          <span className="cl-protected__size">{formatBytes(target.size)}</span>
                          <span className="cl-protected__path">{target.path}</span>
                        </div>
                      ))}
                    </section>
                  )}

                  {selection.count > 0 && (
                    <div className="cl-selectbar" role="region" aria-label="Cleanup selection">
                      <div className="cl-selectbar__text">
                        <strong>{plural(selection.count, "location")} selected</strong>
                        <span>{formatBytes(selection.bytes)} measured · moved to quarantine, not deleted</span>
                      </div>
                      <div className="cl-selectbar__actions">
                        <Button size="sm" variant="ghost" onClick={() => setSelected(new Set())}>
                          Clear
                        </Button>
                        <Button
                          size="sm"
                          variant="primary"
                          loading={previewLoading}
                          onClick={() => void openPreview()}
                        >
                          Preview cleanup
                        </Button>
                      </div>
                    </div>
                  )}
                </>
              )}
            </Panel>
          </>
        ) : null}
      </div>

      <Drawer
        open={previewOpen}
        title={report ? "Cleanup result" : "Cleanup preview"}
        subtitle={
          report
            ? `${plural(report.quarantined, "file")} moved to quarantine`
            : "Review the exact files before anything moves"
        }
        onClose={closePreview}
        footer={
          report ? (
            <div className="cl-drawer-foot">
              <Button onClick={closePreview}>Done</Button>
              <Button
                variant="primary"
                onClick={() => {
                  closePreview();
                  setTab("quarantine");
                }}
              >
                Open quarantine
              </Button>
            </div>
          ) : (
            <div className="cl-drawer-foot">
              <Button onClick={closePreview} disabled={running}>
                Cancel
              </Button>
              <Button
                variant="primary"
                disabled={!confirmReady}
                loading={running}
                onClick={() => void runCleanup()}
              >
                {running ? "Moving files" : "Move to quarantine"}
              </Button>
            </div>
          )
        }
      >
        {previewLoading && !preview ? (
          <div className="cl-skeleton">
            {[0, 1, 2, 3].map((index) => (
              <div className="cl-skeleton__row" key={index}>
                <Skeleton width={160} height={11} />
                <Skeleton width="46%" height={10} />
                <Skeleton width={70} height={10} />
              </div>
            ))}
          </div>
        ) : previewError ? (
          <Banner tone="danger">
            <strong>Preview failed.</strong> {previewError}
          </Banner>
        ) : report ? (
          <CleanupResult report={report} runError={runError} onRetry={() => void runCleanup()} />
        ) : preview ? (
          <div className="cl-preview">
            <Banner tone="safe">
              <strong>{formatBytes(preview.bytes)}</strong> in{" "}
              <strong>{plural(preview.fileCount, "file")}</strong> will move to quarantine. Files can
              be restored from the Quarantine tab; purging a batch is permanent.
            </Banner>

            {(readOnly === true || preview.readOnly) && (
              <Banner
                tone="accent"
                actions={
                  <Button size="sm" loading={modeBusy} onClick={() => void enableCleaning()}>
                    Enable cleaning
                  </Button>
                }
              >
                <strong>Read-only mode is on.</strong> Previewing is safe; execution stays blocked
                until you enable cleaning.
              </Banner>
            )}

            {preview.targets.some((target) => !target.includable) && (
              <Banner tone="caution">
                Some selected locations are protected and were excluded by the backend.
              </Banner>
            )}

            <div className="cl-preview__targets">
              {preview.targets.map((target) => (
                <article className="cl-preview__target" key={target.id}>
                  <header className="cl-preview__target-head">
                    <span className="cl-preview__target-name">
                      {target.tool} · {target.label}
                    </span>
                    <RiskBadge risk={target.risk} />
                    <span className="cl-preview__target-size">{formatBytes(target.size)}</span>
                  </header>
                  <p className="cl-preview__path">{target.path}</p>
                  <p className="cl-preview__impact">{target.impact}</p>
                </article>
              ))}
            </div>

            <KeyValue
              items={[
                { label: "Group", value: preview.targets.map((target) => groupLabel(groupForCategory(target.category))).filter((value, index, all) => all.indexOf(value) === index).join(", ") || "—" },
                { label: "Files", value: formatCount(preview.fileCount) },
                { label: "Measured size", value: formatBytes(preview.bytes) },
                { label: "Preview expires", value: formatDateTime(preview.expiresAt) },
              ]}
            />

            <section className="cl-preview__section" aria-labelledby="cl-preview-files">
              <h3 className="cl-preview__title" id="cl-preview-files">
                Planned files
              </h3>
              {preview.files.length === 0 ? (
                <p className="cl-muted">
                  No files were found in the selected locations. They may already be empty or
                  unreadable.
                </p>
              ) : (
                <>
                  <ul className="cl-pathlist">
                    {preview.files.map((file) => (
                      <li key={`${file.targetId}:${file.path}`}>
                        <span className="cl-pathlist__size">{formatBytes(file.size)}</span>
                        <span className="cl-pathlist__path" title={file.path}>
                          {file.path}
                        </span>
                      </li>
                    ))}
                  </ul>
                  {preview.hiddenCount > 0 && (
                    <p className="cl-muted">
                      {plural(preview.hiddenCount, "more file")} not listed in this preview; the counts
                      and size above cover every planned file.
                    </p>
                  )}
                </>
              )}
            </section>

            {preview.skipped.length > 0 && (
              <section className="cl-preview__section" aria-labelledby="cl-preview-skipped">
                <h3 className="cl-preview__title" id="cl-preview-skipped">
                  Skipped
                </h3>
                <ul className="cl-skiplist">
                  {preview.skipped.map((skip) => (
                    <li key={`${skip.path}:${skip.reason}`}>
                      <span className="cl-skiplist__reason">{skip.reason}</span>
                      <span className="cl-pathlist__path">{skip.path}</span>
                    </li>
                  ))}
                </ul>
              </section>
            )}

            {preview.warnings.length > 0 && (
              <Banner tone="caution">
                <ul className="cl-warning-list">
                  {preview.warnings.map((warning) => (
                    <li key={warning}>{warning}</li>
                  ))}
                </ul>
              </Banner>
            )}

            {running && (
              <div className="cl-progress" aria-live="polite">
                <ProgressBar
                  value={progressRatio}
                  label="Cleanup progress"
                  tone="accent"
                />
                <p className="cl-progress__text">
                  {progress
                    ? `${formatCount(progress.files)} of ${formatCount(progress.totalFiles)} files · ${formatBytes(progress.bytes)}`
                    : "Preparing the first file…"}
                </p>
                <p className="cl-progress__path">
                  {progress?.currentPath ? truncateMiddle(progress.currentPath, 74) : "—"}
                </p>
              </div>
            )}

            {runError && (
              <Banner tone="danger">
                <strong>Cleanup did not finish.</strong> {runError}
              </Banner>
            )}

            <label className="cl-confirm">
              <input
                type="checkbox"
                checked={confirmed}
                onChange={(event) => setConfirmed(event.target.checked)}
                disabled={running}
              />
              <span>
                Move <strong>{plural(preview.fileCount, "file")}</strong> (
                <strong>{formatBytes(preview.bytes)}</strong>) from{" "}
                {plural(preview.targets.length, "location")} into Cyron's quarantine. I understand
                this changes those caches, they may be re-downloaded or rebuilt, and purging the
                quarantine later is permanent.
              </span>
            </label>
          </div>
        ) : null}
      </Drawer>
    </>
  );
}

function CleanupResult({
  report,
  runError,
  onRetry,
}: {
  report: CleanupReport;
  runError: string | null;
  onRetry: () => void;
}) {
  const items = report.items.slice(0, RESULT_PAGE);
  return (
    <div className="cl-results" aria-live="polite">
      <Banner
        tone={report.failed > 0 ? "caution" : report.cancelled ? "info" : "safe"}
        actions={
          runError ? (
            <Button size="sm" onClick={onRetry}>
              Retry
            </Button>
          ) : undefined
        }
      >
        <strong>
          {report.cancelled
            ? "Cleanup stopped early."
            : report.failed > 0
              ? "Cleanup finished with items left behind."
              : "Cleanup finished."}
        </strong>{" "}
        {formatBytes(report.bytes)} in {plural(report.quarantined, "file")} moved to quarantine
        {report.dirsRemoved > 0 ? `, ${plural(report.dirsRemoved, "empty folder")} removed` : ""}.
      </Banner>

      <div className="cl-results__stats">
        <Stat label="Moved" value={formatCount(report.quarantined)} meta={formatBytes(report.bytes)} />
        <Stat label="Skipped" value={formatCount(report.skipped)} meta="Locked, changed or excluded" />
        <Stat label="Failed" value={formatCount(report.failed)} meta={report.failed > 0 ? "See reasons below" : "None"} />
        <Stat
          label="Recoverable until"
          value={report.purgeAt ? formatDateTime(report.purgeAt) : "—"}
          meta={report.quarantineDir ? truncateMiddle(report.quarantineDir, 34) : "No quarantine batch"}
        />
      </div>

      {report.errors.length > 0 && (
        <Banner tone="caution">
          <ul className="cl-warning-list">
            {report.errors.map((message) => (
              <li key={message}>{message}</li>
            ))}
          </ul>
        </Banner>
      )}

      <section className="cl-preview__section" aria-labelledby="cl-result-items">
        <h3 className="cl-preview__title" id="cl-result-items">
          Per-item outcome
        </h3>
        {items.length === 0 ? (
          <p className="cl-muted">No files were processed.</p>
        ) : (
          <ul className="cl-resultlist">
            {items.map((item) => (
              <li className={`cl-resultlist__row cl-resultlist__row--${item.status}`} key={`${item.path}:${item.status}`}>
                <Badge
                  tone={item.status === "quarantined" ? "safe" : item.status === "skipped" ? "neutral" : "danger"}
                >
                  {item.status}
                </Badge>
                <span className="cl-pathlist__path" title={item.path}>
                  {item.path}
                </span>
                <span className="cl-pathlist__size">{formatBytes(item.bytes)}</span>
                {item.reason && <span className="cl-resultlist__reason">{item.reason}</span>}
              </li>
            ))}
          </ul>
        )}
        {report.items.length > items.length && (
          <p className="cl-muted">
            Showing {formatCount(RESULT_PAGE)} of {plural(report.items.length, "item")}.
          </p>
        )}
      </section>
    </div>
  );
}

function QuarantinePanel({
  batches,
  loading,
  error,
  pending,
  busy,
  note,
  onAsk,
  onCancel,
  onConfirm,
  onRetry,
}: {
  batches: QuarantineBatch[];
  loading: boolean;
  error: string | null;
  pending: Pending | null;
  busy: string | null;
  note: string | null;
  onAsk: (kind: "restore" | "purge", batch: QuarantineBatch) => void;
  onCancel: () => void;
  onConfirm: () => void;
  onRetry: () => void;
}) {
  return (
    <Panel
      title="Quarantine"
      subtitle="Files moved out of caches stay here until you restore or purge them."
      actions={
        <Button size="sm" icon={<IconRefresh size={14} />} onClick={onRetry}>
          Refresh
        </Button>
      }
    >
      {note && <Banner tone="info">{note}</Banner>}
      {error && (
        <Banner tone="danger">
          <strong>Could not read the quarantine.</strong> {error}
        </Banner>
      )}

      {pending && (
        <Banner
          tone={pending.kind === "purge" ? "danger" : "caution"}
          actions={
            <>
              <Button size="sm" variant="ghost" onClick={onCancel} disabled={busy !== null}>
                Cancel
              </Button>
              <Button
                size="sm"
                variant={pending.kind === "purge" ? "danger" : "primary"}
                loading={busy === pending.batch.runId}
                onClick={onConfirm}
              >
                {pending.kind === "purge" ? "Delete permanently" : "Restore files"}
              </Button>
            </>
          }
        >
          {pending.kind === "purge" ? (
            <>
              <strong>Permanently delete this batch?</strong> {plural(pending.batch.fileCount, "file")} (
              {formatBytes(pending.batch.bytes)}) will be unrecoverable.
            </>
          ) : (
            <>
              <strong>Restore this batch?</strong> {plural(pending.batch.fileCount, "file")} go back to
              their original paths. Existing files are never overwritten.
            </>
          )}
        </Banner>
      )}

      {loading ? (
        <div className="cl-skeleton">
          {[0, 1].map((index) => (
            <div className="cl-skeleton__row" key={index}>
              <Skeleton width={150} height={11} />
              <Skeleton width="34%" height={10} />
              <Skeleton width={80} height={10} />
            </div>
          ))}
        </div>
      ) : batches.length === 0 ? (
        <EmptyState
          compact
          icon={IconCheck}
          title="Quarantine is empty"
          description="Confirmed cleanups appear here with their recoverable files, size and expiry."
        />
      ) : (
        <>
          <p className="cl-muted">
            {plural(batches.length, "batch", "batches")} · {formatBytes(batchTotal(batches))}{" "}
            recoverable right now.
          </p>
          <ul className="cl-batches">
            {batches.map((batch) => (
              <li className="cl-batch" key={batch.runId}>
                <div className="cl-batch__main">
                  <span className="cl-batch__title">
                    {batch.fileCount === 1 ? "1 file" : `${formatCount(batch.fileCount)} files`} ·{" "}
                    {formatBytes(batch.bytes)}
                  </span>
                  <span className="cl-batch__path">{batch.dir}</span>
                  <span className="cl-batch__meta">
                    Created {formatDateTime(batch.createdAt)} · expires{" "}
                    {formatDateTime(batch.expiresAt)}
                  </span>
                </div>
                <div className="cl-batch__actions">
                  <Button
                    size="sm"
                    onClick={() => onAsk("restore", batch)}
                    disabled={busy !== null}
                  >
                    Restore
                  </Button>
                  <Button
                    size="sm"
                    variant="danger"
                    onClick={() => onAsk("purge", batch)}
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
  );
}

function CleanerSkeleton() {
  return (
    <>
      <Panel title="Measured storage by risk">
        <div className="cl-skeleton">
          <div className="cl-skeleton__stats">
            {[0, 1, 2, 3].map((index) => (
              <div className="cl-skeleton__stat" key={index}>
                <Skeleton width={72} height={9} />
                <Skeleton width={104} height={24} />
                <Skeleton width={86} height={9} />
              </div>
            ))}
          </div>
          <Skeleton width="100%" height={10} />
        </div>
      </Panel>
      <Panel title="Detected locations" flush>
        <div className="cl-skeleton">
          {[0, 1, 2, 3, 4].map((index) => (
            <div className="cl-skeleton__row" key={index}>
              <Skeleton width={150} height={12} />
              <Skeleton width="32%" height={10} />
              <Skeleton width={64} height={10} />
              <Skeleton width={72} height={18} />
            </div>
          ))}
        </div>
      </Panel>
    </>
  );
}
