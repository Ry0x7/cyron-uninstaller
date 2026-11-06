import { useEffect, useMemo, useState, type KeyboardEvent } from "react";
import { api, errorMessage, invalidateInstalledApps, loadInstalledApps } from "../../lib/api";
import { formatBytes, formatCount, formatDate, formatDateTime, truncateMiddle } from "../../lib/format";
import { useAsync, useDebouncedValue } from "../../lib/hooks";
import type { BatchUninstallPlan, BatchUninstallReport, InstalledApp } from "../../lib/types";
import {
  Badge,
  Banner,
  Button,
  Drawer,
  EmptyState,
  Eyebrow,
  Panel,
  SearchField,
  Segmented,
  Select,
  Skeleton,
  ViewHeader,
} from "../../components/ui";
import { IconApps, IconLock, IconPlay, IconRefresh, IconShield, IconTrash } from "../../components/Icons";
import { AppDrawer } from "./AppDrawer";
import "./uninstaller.css";

type ScopeFilter = "all" | "machine" | "user";
type SourceFilter = "all" | "win32" | "store" | "steam" | "portable" | "broken";
type SortKey = "name" | "publisher" | "installed" | "size";

const SORT_OPTIONS: { value: SortKey; label: string }[] = [
  { value: "name", label: "Name (A–Z)" },
  { value: "publisher", label: "Publisher (A–Z)" },
  { value: "installed", label: "Newest install" },
  { value: "size", label: "Largest estimate" },
];

function compareNullableText(a: string | null, b: string | null): number {
  if (!a && !b) return 0;
  if (!a) return 1;
  if (!b) return -1;
  return a.localeCompare(b, undefined, { sensitivity: "base", numeric: true });
}

function compareNullableNumber(a: number | null, b: number | null): number {
  if (a === null && b === null) return 0;
  if (a === null) return 1;
  if (b === null) return -1;
  return b - a;
}

function installDateSortValue(value: string | null): number | null {
  if (!value) return null;
  const normalized = /^\d{8}$/.test(value)
    ? `${value.slice(0, 4)}-${value.slice(4, 6)}-${value.slice(6, 8)}`
    : value;
  const timestamp = Date.parse(normalized);
  return Number.isFinite(timestamp) ? timestamp : null;
}

function normalizeSearch(value: string): string {
  return value.trim().normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLocaleLowerCase();
}

export function Uninstaller({
  initialAppId,
  initialQuery,
}: {
  initialAppId?: string | null;
  initialQuery?: string | null;
} = {}) {
  const inventory = useAsync(() => loadInstalledApps(), []);
  const [query, setQuery] = useState(initialQuery ?? "");
  const debouncedQuery = useDebouncedValue(query, 120);
  const [scope, setScope] = useState<ScopeFilter>("all");
  const [sourceFilter, setSourceFilter] = useState<SourceFilter>("all");
  const [sort, setSort] = useState<SortKey>("name");
  const [selected, setSelected] = useState<InstalledApp | null>(null);

  // Batch selection state
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [batchOpen, setBatchOpen] = useState(false);
  const [batchQuiet, setBatchQuiet] = useState(true);
  const [batchConfirm, setBatchConfirm] = useState(false);
  const [batchRunning, setBatchRunning] = useState(false);
  const [batchPlan, setBatchPlan] = useState<BatchUninstallPlan | null>(null);
  const [batchPreparing, setBatchPreparing] = useState(false);
  const [batchReport, setBatchReport] = useState<BatchUninstallReport | null>(null);
  const [batchError, setBatchError] = useState<string | null>(null);

  const apps = inventory.data?.apps ?? [];
  const warnings = [...new Set(inventory.data?.warnings ?? [])];

  useEffect(() => {
    if (initialAppId && apps.length > 0) {
      const match = apps.find((a) => a.id === initialAppId);
      if (match) setSelected(match);
    }
  }, [initialAppId, apps]);

  const counts = useMemo(() => {
    const needle = normalizeSearch(debouncedQuery);
    const matching = needle
      ? apps.filter((app) =>
          [app.name, app.publisher, app.installLocation, app.version].some(
            (value) => value !== null && normalizeSearch(value).includes(needle),
          ),
        )
      : apps;
    let machine = 0;
    let user = 0;
    let win32 = 0;
    let store = 0;
    let steam = 0;
    let portable = 0;
    let broken = 0;
    for (const app of matching) {
      if (app.scope === "machine") machine += 1;
      else user += 1;
      if (app.uninstallSupport === "missing") broken += 1;
      if (app.source === "store") store += 1;
      else if (app.source === "steam") steam += 1;
      else if (app.source === "portable") portable += 1;
      else win32 += 1;
    }
    return { all: matching.length, machine, user, win32, store, steam, portable, broken };
  }, [apps, query]);

  const visible = useMemo(() => {
    const needle = normalizeSearch(debouncedQuery);
    const filtered = apps.filter((app) => {
      if (scope !== "all" && app.scope !== scope) return false;
      if (sourceFilter === "win32" && app.source !== "win32") return false;
      if (sourceFilter === "store" && app.source !== "store") return false;
      if (sourceFilter === "steam" && app.source !== "steam") return false;
      if (sourceFilter === "portable" && app.source !== "portable") return false;
      if (sourceFilter === "broken" && app.uninstallSupport !== "missing") return false;
      if (!needle) return true;
      return [app.name, app.publisher, app.installLocation, app.version]
        .some((value) => value !== null && normalizeSearch(value).includes(needle));
    });

    return filtered.sort((a, b) => {
      switch (sort) {
        case "publisher":
          return compareNullableText(a.publisher, b.publisher) || compareNullableText(a.name, b.name);
        case "installed": {
          const aDate = installDateSortValue(a.installDate);
          const bDate = installDateSortValue(b.installDate);
          if (aDate === null && bDate !== null) return 1;
          if (bDate === null && aDate !== null) return -1;
          return (aDate === null || bDate === null ? 0 : bDate - aDate) || compareNullableText(a.name, b.name);
        }
        case "size":
          return compareNullableNumber(a.estimatedSize, b.estimatedSize) || compareNullableText(a.name, b.name);
        default:
          return compareNullableText(a.name, b.name);
      }
    });
  }, [apps, debouncedQuery, scope, sort]);

  const loading = inventory.status === "loading" && apps.length === 0;

  const toggleSelectRow = (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const toggleSelectAll = () => {
    const selectable = visible.filter((a) => !a.isProtected);
    if (selectedIds.size === selectable.length) {
      setSelectedIds(new Set());
    } else {
      setSelectedIds(new Set(selectable.map((a) => a.id)));
    }
  };

  const openRow = (app: InstalledApp) => setSelected(app);
  const onRowKeyDown = (event: KeyboardEvent<HTMLTableRowElement>, app: InstalledApp) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      openRow(app);
    }
  };

  const selectedApps = useMemo(() => {
    return apps.filter((a) => selectedIds.has(a.id));
  }, [apps, selectedIds]);

  useEffect(() => {
    if (!batchOpen || selectedApps.length === 0) {
      setBatchPlan(null);
      setBatchConfirm(false);
      return;
    }
    let cancelled = false;
    setBatchPreparing(true);
    setBatchError(null);
    api.prepareBatchUninstall(Array.from(selectedIds), batchQuiet)
      .then((plan) => {
        if (!cancelled) {
          setBatchPlan(plan);
          setBatchPreparing(false);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setBatchError(errorMessage(err));
          setBatchPlan(null);
          setBatchPreparing(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [batchOpen, selectedIds, batchQuiet]);

  const runBatchUninstall = async () => {
    if (!batchPlan || !batchConfirm || batchRunning) return;
    setBatchRunning(true);
    setBatchError(null);
    setBatchReport(null);
    try {
      const report = await api.executeBatchUninstall(batchPlan.planId);
      setBatchReport(report);
      invalidateInstalledApps();
      inventory.reload();
    } catch (err) {
      setBatchError(errorMessage(err));
    } finally {
      setBatchRunning(false);
    }
  };

  return (
    <>
      <ViewHeader
        title="Uninstaller"
        description="Application discovery, process termination, standard/MSI uninstaller execution, and review-first leftover scanning."
        actions={
          <>
            <Badge tone="safe">Verified Engine</Badge>
            <Button
              icon={<IconRefresh size={15} />}
              loading={inventory.status === "loading"}
              onClick={() => {
                invalidateInstalledApps();
                inventory.reload();
              }}
            >
              Refresh
            </Button>
          </>
        }
      />

      <div className="view-body">
        <Banner tone="accent">
          <strong>Review-First Uninstaller.</strong> Select an application to view live processes,
          launch its registered uninstaller, or scan and safely quarantine leftover files and registry keys.
        </Banner>

        {warnings.length > 0 && (
          <Banner tone="caution">
            Some registry keys could not be read: {warnings.slice(0, 3).join(" · ")}
            {warnings.length > 3 ? ` and ${warnings.length - 3} more` : ""}.
          </Banner>
        )}

        {inventory.status === "error" && (
          <Banner
            tone="danger"
            actions={
              <Button size="sm" onClick={inventory.reload}>
                Try again
              </Button>
            }
          >
            {inventory.error}
          </Banner>
        )}

        <Panel
          title="Installed applications"
          subtitle="Publisher, version, install date and size exactly as reported by each installer."
          flush
        >
          <div className="un-toolbar">
            <SearchField
              value={query}
              onChange={setQuery}
              placeholder="Search name, publisher or path"
              ariaLabel="Search installed applications"
              className="un-search"
            />
            <Segmented
              ariaLabel="Filter by install scope"
              value={scope}
              onChange={setScope}
              options={[
                { value: "all", label: "All scope", count: counts.all },
                { value: "machine", label: "Machine", count: counts.machine },
                { value: "user", label: "User", count: counts.user },
              ]}
            />
            <Segmented
              ariaLabel="Filter by application source"
              value={sourceFilter}
              onChange={(val) => setSourceFilter(val as SourceFilter)}
              options={[
                { value: "all", label: "All sources" },
                { value: "win32", label: "Win32", count: counts.win32 },
                { value: "store", label: "Store", count: counts.store },
                { value: "steam", label: "Steam", count: counts.steam },
                { value: "portable", label: "Portable", count: counts.portable },
                { value: "broken", label: "Broken", count: counts.broken },
              ]}
            />
            <div className="un-toolbar__end">
              <span className="un-count" aria-live="polite">
                {loading ? "Reading registry…" : `${formatCount(visible.length)} shown`}
              </span>
              <Select
                ariaLabel="Sort applications"
                value={sort}
                onChange={(value) => setSort(value as SortKey)}
                options={SORT_OPTIONS.map((option) => ({ value: option.value, label: option.label }))}
              />
            </div>
          </div>

          {loading ? (
            <div className="un-loading" aria-busy="true" aria-label="Loading installed applications">
              {Array.from({ length: 8 }, (_, index) => (
                <div className="un-loading__row" key={index}>
                  <Skeleton width="38%" />
                  <Skeleton width="20%" />
                  <Skeleton width="10%" />
                  <Skeleton width="14%" />
                  <Skeleton width="8%" />
                </div>
              ))}
            </div>
          ) : visible.length === 0 ? (
            <EmptyState
              icon={IconApps}
              title={
                inventory.status === "error" && apps.length === 0
                  ? "Inventory unavailable"
                  : apps.length === 0
                    ? "No applications found"
                    : "No matching applications"
              }
              description={
                inventory.status === "error" && apps.length === 0
                  ? "The registry could not be read. Retry, or check the message above if the problem continues."
                  : apps.length === 0
                    ? "Cyron could not read any uninstall entries from the registry."
                    : "Try a different search term or switch the scope filter back to All."
              }
              action={
                apps.length === 0 ? (
                  <Button onClick={inventory.reload}>Scan registry again</Button>
                ) : (
                  <Button
                    onClick={() => {
                      setQuery("");
                      setScope("all");
                    }}
                  >
                    Clear filters
                  </Button>
                )
              }
            />
          ) : (
            <div className="table-wrap un-table">
              <table className="table un-grid">
                <colgroup>
                  <col style={{ width: "36px" }} />
                  <col style={{ width: "31%" }} />
                  <col style={{ width: "19%" }} />
                  <col style={{ width: "10%" }} />
                  <col style={{ width: "12%" }} />
                  <col style={{ width: "11%" }} />
                  <col style={{ width: "14%" }} />
                </colgroup>
                <thead>
                  <tr>
                    <th scope="col" style={{ textAlign: "center" }}>
                      <input
                        type="checkbox"
                        aria-label="Select all visible non-protected applications"
                        checked={
                          selectedIds.size > 0 &&
                          selectedIds.size === visible.filter((a) => !a.isProtected).length
                        }
                        onChange={toggleSelectAll}
                      />
                    </th>
                    <th scope="col">Application</th>
                    <th scope="col">Publisher</th>
                    <th scope="col">Version</th>
                    <th scope="col">Installed</th>
                    <th scope="col" className="numeric">
                      Size
                    </th>
                    <th scope="col">Scope</th>
                  </tr>
                </thead>
                <tbody>
                  {visible.map((app) => (
                    <tr
                      key={app.id}
                      className="un-row"
                      data-clickable="true"
                      tabIndex={0}
                      aria-selected={selected?.id === app.id}
                      aria-label={`View details for ${app.name}`}
                      onClick={() => openRow(app)}
                      onKeyDown={(event) => onRowKeyDown(event, app)}
                    >
                      <td style={{ textAlign: "center" }} onClick={(e) => e.stopPropagation()}>
                        <input
                          type="checkbox"
                          disabled={app.isProtected}
                          checked={selectedIds.has(app.id)}
                          onChange={() => {}}
                          onClick={(e) => toggleSelectRow(app.id, e)}
                        />
                      </td>
                      <td>
                        <div className="cell-name">
                          <span className="cell-name__title un-name" title={app.name}>
                            {app.isProtected && (
                              <IconLock size={11} className="un-lock" aria-label="System component" />
                            )}
                            {app.name}
                          </span>
                          <span className="cell-name__meta">
                            {app.installLocation
                              ? truncateMiddle(app.installLocation, 58)
                              : "No install location reported"}
                          </span>
                        </div>
                      </td>
                      <td className="un-publisher">{app.publisher ?? "—"}</td>
                      <td className="mono un-version">{app.version ?? "—"}</td>
                      <td className="un-date">{formatDate(app.installDate)}</td>
                      <td
                        className="numeric"
                        title={
                          app.estimatedSize === null
                            ? "This installer did not record an EstimatedSize value"
                            : "EstimatedSize reported by the installer in the registry"
                        }
                      >
                        {app.estimatedSize === null ? "—" : formatBytes(app.estimatedSize)}
                      </td>
                      <td>
                        <div className="un-scope" style={{ display: "flex", gap: "4px", flexWrap: "wrap", alignItems: "center" }}>
                          <Badge tone={app.source === "store" ? "accent" : app.source === "steam" ? "accent" : app.source === "portable" ? "caution" : "neutral"}>
                            {app.source === "store" ? "Store" : app.source === "steam" ? "Steam" : app.source === "portable" ? "Portable" : (app.scope === "machine" ? "Machine" : "User")}
                          </Badge>
                          {app.uninstallSupport === "missing" && (
                            <Badge tone="danger">Broken</Badge>
                          )}
                          <span className="un-arch">{app.architecture === "x64" ? "64-bit" : app.architecture === "x86" ? "32-bit" : ""}</span>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </Panel>

        {/* Selection Bar for Batch Actions */}
        {selectedIds.size > 0 && (
          <div className="un-selection-bar">
            <span className="un-selection-bar__count">
              {formatCount(selectedIds.size)} application{selectedIds.size === 1 ? "" : "s"} selected
            </span>
            <div className="row" style={{ gap: 8 }}>
              <Button size="sm" variant="ghost" onClick={() => setSelectedIds(new Set())}>
                Clear
              </Button>
              <Button
                size="sm"
                variant="primary"
                icon={<IconTrash size={14} />}
                onClick={() => setBatchOpen(true)}
              >
                Batch Uninstall…
              </Button>
            </div>
          </div>
        )}

        {visible.length > 0 && (
          <p className="un-footnote">
            <IconShield size={13} /> Sizes are installer estimates from the registry. Open an
            application to measure its install folder for real and manage processes or leftovers.
          </p>
        )}
      </div>

      {/* Detail Drawer */}
      <AppDrawer
        app={selected}
        onClose={() => setSelected(null)}
        onUninstallSuccess={() => {
          invalidateInstalledApps();
          inventory.reload();
        }}
      />

      {/* Batch Uninstall Drawer */}
      {batchOpen && (
        <Drawer
          open
          title="Batch Uninstall"
          subtitle={`Sequential execution for ${selectedApps.length} applications`}
          onClose={() => setBatchOpen(false)}
          footer={
            <>
              <Button
                variant="danger"
                icon={<IconPlay size={14} />}
                loading={batchRunning}
                disabled={!batchConfirm || batchRunning || !batchPlan || batchPreparing || !batchPlan?.items.some((i) => i.isValid)}
                onClick={runBatchUninstall}
              >
                {batchRunning ? "Executing queue…" : `Start Batch Uninstall (${selectedApps.length})`}
              </Button>
              <Button variant="ghost" disabled={batchRunning} onClick={() => setBatchOpen(false)}>
                Close
              </Button>
            </>
          }
        >
          <div className="stack" style={{ gap: 12 }}>
            <Banner tone="info">
              Applications are uninstalled sequentially. Uninstaller exit codes and failures are
              isolated so one failure does not halt the queue.
            </Banner>

            {batchPreparing ? (
              <div className="stack" style={{ gap: 8, padding: "12px 0" }}>
                <Skeleton width="60%" />
                <Skeleton width="100%" />
                <Skeleton width="80%" />
                <span className="un-footnote">Resolving registry commands and validating executable security…</span>
              </div>
            ) : batchPlan ? (
              <>
                <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
                  <Eyebrow>Queued applications ({batchPlan.items.length})</Eyebrow>
                  <span className="mono" style={{ fontSize: 11, color: "var(--text-dim)" }}>
                    Plan expires {formatDateTime(batchPlan.expiresAt)}
                  </span>
                </div>
                <div className="un-batch-queue">
                  {batchPlan.items.map((item) => {
                    const itemRes = batchReport?.results.find((r) => r.appId === item.appId);
                    return (
                      <div key={item.appId} className="un-batch-item" style={{ flexDirection: "column", alignItems: "stretch", gap: 4 }}>
                        <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
                          <div>
                            <strong>{item.appName}</strong>
                          </div>
                          <div>
                            {itemRes ? (
                              <Badge
                                tone={
                                  itemRes.status === "completed"
                                    ? "safe"
                                    : itemRes.status === "restart_required"
                                    ? "caution"
                                    : "danger"
                                }
                              >
                                {itemRes.status.toUpperCase()}
                              </Badge>
                            ) : item.isValid ? (
                              <Badge tone="safe">Verified</Badge>
                            ) : (
                              <Badge tone="danger">Refused</Badge>
                            )}
                          </div>
                        </div>

                        {item.isValid ? (
                          <div className="mono" style={{ fontSize: 11, color: "var(--text-dim)", wordBreak: "break-all" }}>
                            {item.executable} {item.arguments.join(" ")}
                          </div>
                        ) : (
                          <div style={{ fontSize: 12, color: "var(--danger)" }}>
                            {item.rejectionReason ?? "Invalid uninstaller command"}
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
              </>
            ) : (
              <div className="un-batch-queue">
                {selectedApps.map((a) => (
                  <div key={a.id} className="un-batch-item">
                    <div>
                      <strong>{a.name}</strong>
                      {a.version && <span className="mono"> v{a.version}</span>}
                    </div>
                    <div>
                      <Badge tone="neutral">Queued</Badge>
                    </div>
                  </div>
                ))}
              </div>
            )}

            <label className="un-confirm-box">
              <input
                type="checkbox"
                checked={batchQuiet}
                disabled={batchRunning || batchPreparing}
                onChange={(e) => setBatchQuiet(e.target.checked)}
              />
              <span>Use silent / quiet uninstallation switch where vendor uninstaller supports it</span>
            </label>

            <label className="un-confirm-box">
              <input
                type="checkbox"
                checked={batchConfirm}
                disabled={batchRunning || batchPreparing || !batchPlan || !batchPlan.items.some((i) => i.isValid)}
                onChange={(e) => setBatchConfirm(e.target.checked)}
              />
              <span>
                I have reviewed the exact commands above and approve sequentially uninstalling these{" "}
                {selectedApps.length} applications. Leftover scanning will remain review-first and separate.
              </span>
            </label>

            {batchReport && (
              <Banner
                tone={
                  batchReport.failed === 0
                    ? "safe"
                    : batchReport.completed > 0
                    ? "caution"
                    : "danger"
                }
              >
                <strong>Batch complete:</strong> {batchReport.completed} completed,{" "}
                {batchReport.restartRequired} restart required, {batchReport.failed} failed.
              </Banner>
            )}

            {batchError && <p className="un-inline-error">{batchError}</p>}
          </div>
        </Drawer>
      )}
    </>
  );
}
