import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Badge,
  Banner,
  Button,
  EmptyState,
  Eyebrow,
  Panel,
  RiskBadge,
  SearchField,
  Segmented,
  Select,
  Skeleton,
  Stat,
  ViewHeader,
} from "../../components/ui";
import {
  IconChevronRight,
  IconCleaner,
  IconInfo,
  IconLock,
  IconRefresh,
} from "../../components/Icons";
import { errorMessage, loadCaches } from "../../lib/api";
import {
  formatBytes,
  formatCount,
  formatDateTime,
  formatPercent,
  relativeTime,
  truncateMiddle,
} from "../../lib/format";
import { RISK_DESCRIPTION, RISK_LABEL, RISK_ORDER } from "../../lib/risk";
import type { CacheInventory, CacheTarget, Risk } from "../../lib/types";
import "./cleaner.css";

const CATEGORY_ORDER = ["ai", "ide", "packages", "containers", "windows"] as const;

const CATEGORY_LABEL: Record<string, string> = {
  ai: "AI tools",
  ide: "Editors & IDEs",
  packages: "Package managers",
  containers: "Containers & WSL",
  windows: "Windows & browsers",
};

type RiskFilter = "all" | Risk;
type Status = "loading" | "ready" | "error";

function categoryOf(target: CacheTarget): string {
  return target.category || "other";
}

function categoryLabel(category: string): string {
  return CATEGORY_LABEL[category] ?? `Other · ${category}`;
}

function categoryRank(category: string): number {
  const rank = (CATEGORY_ORDER as readonly string[]).indexOf(category);
  return rank < 0 ? CATEGORY_ORDER.length : rank;
}

function percentOf(part: number, total: number): number {
  if (!(total > 0) || !Number.isFinite(part) || !Number.isFinite(total)) return 0;
  return Math.max(0, Math.min(100, (part / total) * 100));
}

function bySize(a: CacheTarget, b: CacheTarget): number {
  return b.size - a.size || a.tool.localeCompare(b.tool) || a.label.localeCompare(b.label);
}

function detailId(target: CacheTarget): string {
  return `cl-detail-${target.id.replace(/[^a-zA-Z0-9_-]/g, "-")}`;
}

export function Cleaner() {
  const [inventory, setInventory] = useState<CacheInventory | null>(null);
  const [status, setStatus] = useState<Status>("loading");
  const [error, setError] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [search, setSearch] = useState("");
  const [riskFilter, setRiskFilter] = useState<RiskFilter>("all");
  const [categoryFilter, setCategoryFilter] = useState<string>("all");
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const mounted = useRef(true);
  const request = useRef(0);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
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

  useEffect(() => {
    void load(false);
  }, [load]);

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
    return Array.from(present).sort(
      (a, b) => categoryRank(a) - categoryRank(b) || a.localeCompare(b),
    );
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

  const groups = useMemo(() => {
    const buckets = new Map<string, CacheTarget[]>();
    for (const target of visible) {
      if (target.risk === "protected") continue;
      const key = categoryOf(target);
      const bucket = buckets.get(key);
      if (bucket) bucket.push(target);
      else buckets.set(key, [target]);
    }
    const extra = Array.from(buckets.keys())
      .filter((key) => !(CATEGORY_ORDER as readonly string[]).includes(key))
      .sort();
    return [...CATEGORY_ORDER, ...extra]
      .filter((key) => buckets.has(key))
      .map((key) => ({ key, label: categoryLabel(key), items: [...buckets.get(key)!].sort(bySize) }));
  }, [visible]);

  const protectedItems = useMemo(
    () => visible.filter((target) => target.risk === "protected").sort(bySize),
    [visible],
  );

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

  const lastChecked = inventory
    ? `${error ? "Last successful check" : "Checked"} ${relativeTime(inventory.scannedAt)}${error ? " · stale" : ""}`
    : "not yet";
  const compositionTotal = totals.size;
  const warnings = inventory?.warnings ?? [];

  return (
    <>
      <ViewHeader
        title="Cleaner"
        description="Developer, AI and Windows caches with an honest cost for each one. Nothing here is removed in this phase."
        meta={<Badge tone="accent">Preview only</Badge>}
        actions={
          <>
            <span className={`small faint${error && inventory ? " cl-stale" : ""}`}>{lastChecked}</span>
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
        <Banner tone="accent">
          <strong>This build measures and classifies.</strong> Cleaning with previews, rollback and an
          operation log arrives in Phase 3. No control on this page deletes a file, and Cyron never
          offers protected data for removal.
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

        {status === "loading" && !inventory ? (
          <CleanerSkeleton />
        ) : status === "error" && !inventory ? (
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
        ) : existing.length === 0 ? (
          <Panel title="Detected locations">
            <EmptyState
              icon={IconCleaner}
              title={warnings.length > 0 || error ? "No cache locations could be measured" : "No known cache locations found"}
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
        ) : (
          <>
            <Panel
              title="Measured storage by risk"
              subtitle={`${formatCount(existing.length)} measured locations in known tool folders; this view does not remove files`}
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
                    meta={`${formatPercent(compositionTotal > 0 ? totals.safe / compositionTotal : 0)} of detected`}
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
                      style={{ width: `${percentOf(totals.safe, compositionTotal)}%` }}
                    />
                    <span
                      className="cl-bar__seg cl-bar__seg--caution"
                      style={{ width: `${percentOf(totals.caution, compositionTotal)}%` }}
                    />
                    <span
                      className="cl-bar__seg cl-bar__seg--protected"
                      style={{ width: `${Math.max(0, 100 - percentOf(totals.safe, compositionTotal) - percentOf(totals.caution, compositionTotal))}%` }}
                    />
                  </div>
                  <div className="cl-legend">
                    {RISK_ORDER.map((risk) => (
                      <span className="cl-legend__item" key={risk}>
                        <span className={`cl-legend__dot cl-legend__dot--${risk}`} />
                        {RISK_LABEL[risk]}
                        <strong>
                          {formatBytes(
                            risk === "safe" ? totals.safe : risk === "caution" ? totals.caution : totals.protected,
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
                  : "Grouped by tool family. Expand a row to see the exact path and the cost of removing it."
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
              </div>

              {appliedFilters && counts.protected > protectedItems.length && counts.protected > 0 && (
                <div className="cl-filter-note" role="status">
                  Protected data remains measured ({formatBytes(totals.protected)} total) and is never offered for removal; active search or category/risk filters hide some protected rows.
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
                    const groupSize = group.items.reduce((acc, target) => acc + target.size, 0);
                    return (
                      <section className="cl-group" key={group.key} aria-label={group.label}>
                        <header className="cl-group__head">
                          <div className="cl-group__title">
                            <Eyebrow>{group.label}</Eyebrow>
                            <span className="cl-group__count">{group.items.length}</span>
                          </div>
                          <span className="cl-group__total">{formatBytes(groupSize)}</span>
                        </header>
                        {group.items.map((target) => {
                          const isOpen = expanded.has(target.id);
                          return (
                            <div className="cl-item" key={target.id}>
                              <button
                                type="button"
                                className="cl-row"
                                aria-label={`${target.tool}, ${target.label}, ${formatBytes(target.size)}, ${RISK_LABEL[target.risk]}`}
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
                              <div className="cl-detail" id={detailId(target)} hidden={!isOpen}>
                                  <div className="cl-detail__main">
                                    <div className="cl-detail__path">{target.path}</div>
                                    <p className="cl-detail__note">{target.note}</p>
                                    <p className="cl-detail__impact">
                                      <IconInfo size={14} />
                                      <span>
                                        {target.risk === "protected"
                                          ? RISK_DESCRIPTION.protected
                                          : `${target.risk === "safe" ? "If removed" : "Review first"}: ${target.note}`}
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
                            you understand your disk, never offered for removal.
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
                </>
              )}
            </Panel>
          </>
        )}
      </div>
    </>
  );
}

function CleanerSkeleton() {
  return (
    <>
      <Panel title="What could be reclaimed">
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
