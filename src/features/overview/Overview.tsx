import { useEffect, useState, type ReactNode } from "react";
import { useAsync } from "../../lib/hooks";
import { api, BACKEND_OFFLINE, invalidateCaches, invalidateInstalledApps, loadCaches, loadInstalledApps } from "../../lib/api";
import {
  formatBytes,
  formatCount,
  formatDate,
  formatPercent,
  plural,
  relativeTime,
} from "../../lib/format";
import type { CacheTarget, HistoryEntry, ViewId, VolumeInfo } from "../../lib/types";
import {
  Banner,
  Button,
  EmptyState,
  Eyebrow,
  Panel,
  ProgressBar,
  RiskBadge,
  Skeleton,
  Stat,
  ViewHeader,
} from "../../components/ui";
import {
  IconActivity,
  IconAnalyzer,
  IconArrowUpRight,
  IconApps,
  IconCleaner,
  IconClock,
  IconCpu,
  IconDrive,
  IconRefresh,
  type IconProps,
} from "../../components/Icons";
import "./overview.css";

const VOLUME_KIND: Record<string, string> = {
  fixed: "Fixed disk",
  removable: "Removable",
  network: "Network",
  optical: "Optical",
  other: "Volume",
};

function kindRank(kind: string): number {
  return { fixed: 0, removable: 1, network: 2, optical: 3 }[kind] ?? 4;
}

function volumeName(volume: VolumeInfo): string {
  return volume.label.trim() || "Local disk";
}

function LoadingRows({ rows = 3 }: { rows?: number }) {
  return (
    <div aria-busy="true">
      {Array.from({ length: rows }, (_, index) => (
        <div className="ov-skeleton-row" key={index}>
          <Skeleton width={28} height={24} />
          <div className="grow stack" style={{ gap: 5 }}>
            <Skeleton width={`${64 - index * 9}%`} height={11} />
            <Skeleton width="42%" height={9} />
          </div>
          <Skeleton width={48} height={11} />
        </div>
      ))}
    </div>
  );
}

function LoadError({ message, onRetry }: { message: string; onRetry: () => void }) {
  return (
    <Banner
      tone="danger"
      actions={
        <Button size="sm" variant="ghost" onClick={onRetry}>
          Retry
        </Button>
      }
    >
      {message}
    </Banner>
  );
}

function historyIcon(kind: string): (props: IconProps) => ReactNode {
  if (kind === "cleanup" || kind === "cache-scan") return IconCleaner;
  if (kind === "app-scan") return IconApps;
  return IconAnalyzer;
}

function StatusBadge({ entry }: { entry: HistoryEntry }) {
  const tone =
    entry.status === "completed" ? "safe" : entry.status === "cancelled" ? "caution" : "danger";
  const label =
    entry.status === "completed" ? "Done" : entry.status === "cancelled" ? "Cancelled" : "Failed";
  return <span className={`badge badge--${tone}`}>{label}</span>;
}

function formatUptime(seconds: number): string {
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (days > 0) return `${days}d ${hours}h ${minutes}m`;
  if (hours > 0) return `${hours}h ${minutes}m`;
  return `${minutes}m`;
}

export function Overview({ onNavigate }: { onNavigate: (view: ViewId) => void }) {
  const specs = useAsync(() => api.getSystemSpecs(), []);
  const volumes = useAsync(() => api.listVolumes(), []);
  const applications = useAsync(() => loadInstalledApps(), []);
  const caches = useAsync(() => loadCaches(), []);
  const history = useAsync(() => api.getHistory(), []);
  const [lastSpecs, setLastSpecs] = useState<typeof specs.data>(null);
  const [lastVolumes, setLastVolumes] = useState<VolumeInfo[] | null>(null);
  const [lastApplications, setLastApplications] = useState<typeof applications.data>(null);
  const [lastCaches, setLastCaches] = useState<typeof caches.data>(null);
  const [lastHistory, setLastHistory] = useState<typeof history.data>(null);

  useEffect(() => {
    if (specs.data) setLastSpecs(specs.data);
  }, [specs.data]);
  useEffect(() => {
    if (volumes.data) setLastVolumes(volumes.data);
  }, [volumes.data]);
  useEffect(() => {
    if (applications.data) setLastApplications(applications.data);
  }, [applications.data]);
  useEffect(() => {
    if (caches.data) setLastCaches(caches.data);
  }, [caches.data]);
  useEffect(() => {
    if (history.data) setLastHistory(history.data);
  }, [history.data]);

  const specsData = specs.data ?? lastSpecs;
  const volumeData = volumes.data ?? lastVolumes;
  const applicationData = applications.data ?? lastApplications;
  const cacheData = caches.data ?? lastCaches;
  const historyData = history.data ?? lastHistory;
  const readyVolumes = [...new Map(
    (volumeData ?? [])
      .filter(
        (volume) =>
          volume.total > 0 &&
          volume.free >= 0 &&
          volume.free <= volume.total &&
          volume.kind !== "network" &&
          volume.kind !== "optical",
      )
      .map((volume) => [volume.mount.trim().toLowerCase(), volume] as const),
  ).values()].sort(
    (a, b) => kindRank(a.kind) - kindRank(b.kind) || a.mount.localeCompare(b.mount),
  );
  const totalCapacity = readyVolumes.reduce((sum, volume) => sum + volume.total, 0);
  const totalFree = readyVolumes.reduce((sum, volume) => sum + volume.free, 0);
  const totalUsed = Math.max(totalCapacity - totalFree, 0);
  const usedRatio = totalCapacity > 0 ? totalUsed / totalCapacity : 0;

  const apps = applicationData?.apps ?? [];
  const warnings = applicationData?.warnings ?? [];
  const machineCount = apps.filter((app) => app.scope === "machine").length;
  // Sort by the parsed timestamp so unknown date formats cannot reorder rows.
  const recentInstalls = apps
    .map((app) => ({ app, at: app.installDate ? Date.parse(app.installDate) : Number.NaN }))
    .filter((entry) => Number.isFinite(entry.at))
    .sort((a, b) => b.at - a.at)
    .slice(0, 5)
    .map((entry) => entry.app);

  const inventory = cacheData;
  const existingTargets = (inventory?.targets ?? []).filter((target) => target.exists);
  const cacheTargets = existingTargets
    .filter((target) => target.size > 0)
    .sort((a, b) => b.size - a.size);
  const topTools: CacheTarget[] = cacheTargets.slice(0, 4);
  const safeBytes = inventory?.totalSafe ?? 0;
  const cautionBytes = inventory?.totalCaution ?? 0;
  const protectedBytes = inventory?.totalProtected ?? 0;
  const measuredLocations = existingTargets.length;
  const compositionBytes = safeBytes + cautionBytes + protectedBytes;
  const cleanupTotal = compositionBytes;
  const compositionTotal = Math.max(compositionBytes, 1);

  const recentHistory = [...(historyData ?? [])].sort((a, b) => b.at - a.at).slice(0, 5);

  const offline = [volumes, applications, caches, history, specs].some(
    (state) => state.error === BACKEND_OFFLINE,
  );
  const refreshing = [volumes, applications, caches, history, specs].some(
    (state) => state.status === "loading",
  );

  // Activity clears history in this same window; drop the stale copy here.
  const reloadHistory = history.reload;
  useEffect(() => {
    const handler = () => reloadHistory();
    window.addEventListener("cyron:history-changed", handler);
    return () => window.removeEventListener("cyron:history-changed", handler);
  }, [reloadHistory]);

  const refreshAll = () => {
    invalidateInstalledApps();
    invalidateCaches();
    specs.reload();
    volumes.reload();
    applications.reload();
    caches.reload();
    history.reload();
  };

  return (
    <div>
      <ViewHeader
        title="Overview"
        meta={<span className="badge badge--neutral">Review-first</span>}
        description="What is installed, where the space went, and what the known cache locations could give back. Cleaning is previewed before anything moves."
        actions={
          <>
            <Button
              variant="secondary"
              icon={<IconRefresh size={14} />}
              loading={refreshing}
              onClick={refreshAll}
            >
              Refresh
            </Button>
            <Button
              variant="primary"
              icon={<IconAnalyzer size={15} />}
              onClick={() => onNavigate("analyzer")}
            >
              Scan a folder
            </Button>
          </>
        }
      />

      <div className="view-body">
        {offline && (
          <Banner
            tone="danger"
            actions={
              <Button size="sm" variant="ghost" onClick={refreshAll}>
                Retry
              </Button>
            }
          >
            <strong>The desktop backend is not connected.</strong> Launch Cyron with{" "}
            <code>npm run dev:desktop</code> or the built app. Measurements below cannot be read
            from a browser tab alone.
          </Banner>
        )}

        <div className="ov-grid">
          <Panel
            className="ov-specs"
            title="System & Hardware"
            subtitle="Platform architecture, processor, memory pressure, and display adapters"
            actions={
              <div className="ov-panel-status">
                {specs.status === "loading" && specsData && <span className="badge badge--neutral">Updating</span>}
                {specs.status === "error" && specsData && specs.error !== BACKEND_OFFLINE && (
                  <span className="badge badge--caution">Stale</span>
                )}
                {specsData && (
                  <span className="badge badge--safe">
                    Uptime: {formatUptime(specsData.uptimeSeconds)}
                  </span>
                )}
              </div>
            }
          >
            {!specsData && specs.status === "loading" && <LoadingRows rows={2} />}
            {specs.status === "error" && !specsData && specs.error && specs.error !== BACKEND_OFFLINE && (
              <LoadError message={specs.error} onRetry={specs.reload} />
            )}
            {specsData && (
              <div className="ov-specs__grid">
                <div className="ov-specs__card">
                  <span className="ov-specs__card-title">
                    <IconApps size={14} /> Operating System
                  </span>
                  <span className="ov-specs__card-value">{specsData.osName || "Windows"}</span>
                  <span className="ov-specs__card-sub">
                    Build {specsData.osBuild || "Unknown"} ({specsData.osArchitecture})
                  </span>
                </div>

                <div className="ov-specs__card">
                  <span className="ov-specs__card-title">
                    <IconCpu size={14} /> Processor
                  </span>
                  <span className="ov-specs__card-value">{specsData.cpuBrand || "Generic CPU"}</span>
                  <span className="ov-specs__card-sub">
                    {specsData.cpuLogicalCores} Logical Cores · {specsData.osArchitecture}
                  </span>
                </div>

                <div className="ov-specs__card">
                  <span className="ov-specs__card-title">
                    <IconActivity size={14} /> Memory Pressure
                  </span>
                  <div className="spread" style={{ alignItems: "baseline" }}>
                    <span className="ov-specs__card-value">
                      {formatBytes(specsData.totalMemoryBytes - specsData.availableMemoryBytes)} / {formatBytes(specsData.totalMemoryBytes)}
                    </span>
                    <span className="mono small faint">{specsData.memoryInUsePercent}%</span>
                  </div>
                  <ProgressBar
                    value={specsData.memoryInUsePercent / 100}
                    label="Physical memory load"
                    tone={specsData.memoryInUsePercent >= 85 ? "danger" : specsData.memoryInUsePercent >= 70 ? "caution" : "accent"}
                  />
                  <span className="ov-specs__card-sub">
                    {formatBytes(specsData.availableMemoryBytes)} available
                  </span>
                </div>

                <div className="ov-specs__card">
                  <span className="ov-specs__card-title">
                    <IconDrive size={14} /> Graphics Adapter
                  </span>
                  <span className="ov-specs__card-value truncate">
                    {specsData.gpuBrand || "Standard Display Adapter"}
                  </span>
                  <span className="ov-specs__card-sub truncate">
                    Direct3D / Vulkan hardware acceleration
                  </span>
                </div>
              </div>
            )}
          </Panel>

          <Panel
            className="ov-storage"
            title="Storage"
            subtitle="Capacity and free space on ready local volumes"
            actions={
              <div className="ov-panel-status">
                <Eyebrow>{volumeData ? `${formatCount(readyVolumes.length)} volumes` : "reading"}</Eyebrow>
                {volumes.status === "loading" && volumeData && <span className="badge badge--neutral">Updating</span>}
                {volumes.status === "error" && volumeData && volumes.error !== BACKEND_OFFLINE && <span className="badge badge--caution">Stale</span>}
              </div>
            }
          >
            {!volumeData && volumes.status === "loading" && <LoadingRows rows={3} />}
            {volumes.status === "error" && volumeData && volumes.error !== BACKEND_OFFLINE && (
              <Banner tone="caution">
                <strong>Showing the last successful volume reading.</strong> Refresh failed. {volumes.error}
              </Banner>
            )}
            {volumes.status === "error" && !volumeData && volumes.error && volumes.error !== BACKEND_OFFLINE && (
              <LoadError message={volumes.error} onRetry={volumes.reload} />
            )}
            {volumeData && readyVolumes.length === 0 && (
              <EmptyState
                compact
                icon={IconDrive}
                title="No ready volumes"
                description="Windows did not report a mounted disk with a capacity."
                action={
                  <Button size="sm" onClick={volumes.reload} icon={<IconRefresh size={13} />}>
                    Check again
                  </Button>
                }
              />
            )}
            {readyVolumes.length > 0 && (
              <>
                <div className="ov-storage__summary">
                  <Stat
                    size="lg"
                    label="Free space"
                    value={formatBytes(totalFree)}
                    meta={`across ${plural(readyVolumes.length, "volume")}`}
                  />
                  <div className="ov-storage__meter">
                    <div className="spread">
                      <Eyebrow>Used</Eyebrow>
                      <span className="mono small muted">
                        {formatBytes(totalUsed)} · {formatPercent(usedRatio)}
                      </span>
                    </div>
                    <ProgressBar
                      value={usedRatio}
                      label="Used space across all volumes"
                      tone={usedRatio >= 0.9 ? "caution" : "accent"}
                    />
                    <span className="faint" style={{ fontSize: 10.5 }}>
                      {formatBytes(totalCapacity)} total capacity
                    </span>
                  </div>
                </div>

                <ul className="ov-volumes">
                  {readyVolumes.map((volume) => {
                    const volumeUsed = Math.max(volume.total - volume.free, 0);
                    const ratio = volumeUsed / volume.total;
                    const letter = volume.mount.replace(/[\\/]/g, "").replace(/:$/, "") || "?";
                    return (
                      <li className="ov-volume" key={volume.mount}>
                        <span className="ov-drive" aria-hidden="true">
                          {letter}:
                        </span>
                        <div className="ov-volume__main">
                          <div className="ov-volume__title">
                            <span className="truncate">{volumeName(volume)}</span>
                            <span className="ov-volume__kind">
                              {VOLUME_KIND[volume.kind] ?? "Volume"} · {volume.fileSystem || "unknown"}
                            </span>
                          </div>
                          <ProgressBar
                            value={ratio}
                            tone={ratio >= 0.9 ? "caution" : "accent"}
                            label={`${letter}: usage`}
                          />
                        </div>
                        <div className="ov-volume__figures">
                          <span className="mono strong">{formatBytes(volume.free)} free</span>
                          <span className="mono faint">{formatBytes(volume.total)}</span>
                        </div>
                      </li>
                    );
                  })}
                </ul>
              </>
            )}
          </Panel>

          <Panel
            className="ov-apps"
            title="Applications"
            subtitle="Discovered from the Windows uninstall registry"
            actions={
              <>
                {applications.status === "loading" && applicationData && (
                  <span className="badge badge--neutral">Updating</span>
                )}
                {applications.status === "error" && applicationData && applications.error !== BACKEND_OFFLINE && (
                  <span className="badge badge--caution">Stale</span>
                )}
                <Button
                  size="sm"
                  variant="ghost"
                  icon={<IconArrowUpRight size={13} />}
                  onClick={() => onNavigate("uninstaller")}
                >
                  Open uninstaller
                </Button>
              </>
            }
          >
            {!applicationData && applications.status === "loading" && <LoadingRows rows={4} />}
            {applications.status === "error" && applicationData && applications.error && applications.error !== BACKEND_OFFLINE && (
              <Banner tone="caution">
                <strong>Showing the last successful inventory.</strong> Refresh failed. {applications.error}
              </Banner>
            )}
            {applications.status === "error" && !applicationData &&
              applications.error && applications.error !== BACKEND_OFFLINE && (
                <LoadError message={applications.error} onRetry={applications.reload} />
              )}

            {applicationData && apps.length === 0 && (
              <EmptyState
                compact
                icon={IconApps}
                title="No applications reported"
                description="No visible entries were found in the uninstall registry keys."
              />
            )}

            {applicationData && warnings.length > 0 && (
              <div style={{ paddingTop: 12 }}>
                <Banner tone="caution">
                  <strong>Partial inventory.</strong> {[...new Set(warnings)].join(" ")}
                </Banner>
              </div>
            )}

            {apps.length > 0 && (
              <>
                <div className="ov-apps__stats">
                  <Stat
                    size="lg"
                    label="Installed"
                    value={formatCount(apps.length)}
                    meta="visible entries"
                  />
                  <Stat label="Machine" value={formatCount(machineCount)} meta="all users" />
                  <Stat
                    label="User"
                    value={formatCount(apps.length - machineCount)}
                    meta="current account"
                  />
                </div>
                <div className="ov-subhead">
                  <Eyebrow>Recently installed</Eyebrow>
                  <span className="faint" style={{ fontSize: 10.5 }}>
                    InstallDate ·{" "}
                    {
                      apps.filter(
                        (app) => app.installDate && Number.isFinite(Date.parse(app.installDate)),
                      ).length
                    }{" "}
                    of {apps.length} entries
                  </span>
                </div>
                {recentInstalls.length > 0 ? (
                  <ul className="ov-list">
                    {recentInstalls.map((app) => (
                      <li className="ov-row" key={app.id}>
                        <div className="ov-row__main">
                          <span className="ov-row__title truncate">{app.name}</span>
                          <span className="ov-row__meta truncate">
                            {app.publisher ?? "Unknown publisher"}
                          </span>
                        </div>
                        <span className="ov-row__end mono">{formatDate(app.installDate)}</span>
                      </li>
                    ))}
                  </ul>
                ) : (
                  <p className="muted small" style={{ padding: "10px 0 2px" }}>
                    No install dates were published by these entries.
                  </p>
                )}
              </>
            )}
          </Panel>

          <Panel
            className="ov-cleanup"
            title="Cleanup potential"
            subtitle="Developer, AI and Windows caches measured on this PC"
            actions={
              <>
                {caches.status === "loading" && cacheData && <span className="badge badge--neutral">Updating</span>}
                {caches.status === "error" && cacheData && caches.error !== BACKEND_OFFLINE && (
                  <span className="badge badge--caution">Stale</span>
                )}
                <Button
                  size="sm"
                  variant="ghost"
                  icon={<IconArrowUpRight size={13} />}
                  onClick={() => onNavigate("cleaner")}
                >
                  Review &amp; clean
                </Button>
              </>
            }
          >
            {!cacheData && caches.status === "loading" && <LoadingRows rows={3} />}
            {caches.status === "error" && cacheData && caches.error && caches.error !== BACKEND_OFFLINE && (
              <Banner tone="caution">
                <strong>Showing the last successful measurement.</strong> Refresh failed. {caches.error}
              </Banner>
            )}
            {caches.status === "error" && !cacheData && caches.error && caches.error !== BACKEND_OFFLINE && (
              <LoadError message={caches.error} onRetry={caches.reload} />
            )}

            {inventory && existingTargets.length === 0 && (
              <EmptyState
                compact
                icon={IconCleaner}
                title="No cache locations found"
                description="None of the known developer or AI tool cache folders exist for this account."
              />
            )}

            {inventory && inventory.warnings.length > 0 && (
              <div style={{ paddingTop: 10 }}>
                <Banner tone="caution">
                  <strong>Some cache locations could not be measured.</strong>
                  <ul className="ov-warning-list">
                    {[...new Set(inventory.warnings)].map((warning) => (
                      <li key={warning}>{warning}</li>
                    ))}
                  </ul>
                </Banner>
              </div>
            )}

            {inventory && existingTargets.length > 0 && (
              <>
                <div className="ov-cleanup__head">
                  <Stat
                    size="lg"
                    tone="accent"
                    label="Regenerable"
                    value={formatBytes(safeBytes)}
                    meta={`${formatBytes(cleanupTotal)} measured across ${plural(measuredLocations, "location")}`}
                  />
                  <div className="ov-cleanup__side">
                    <Stat label="Review" value={formatBytes(cautionBytes)} />
                    <Stat label="Protected · measured only" value={formatBytes(protectedBytes)} />
                  </div>
                </div>

                <div
                  className="ov-split"
                  role="img"
                  aria-label={`${formatBytes(safeBytes)} safe, ${formatBytes(cautionBytes)} needs review, ${formatBytes(protectedBytes)} protected`}
                >
                  {safeBytes > 0 && (
                    <span
                      style={{
                        width: `${(safeBytes / compositionTotal) * 100}%`,
                        background: "var(--safe)",
                      }}
                    />
                  )}
                  {cautionBytes > 0 && (
                    <span
                      style={{
                        width: `${(cautionBytes / compositionTotal) * 100}%`,
                        background: "var(--caution)",
                      }}
                    />
                  )}
                  {protectedBytes > 0 && (
                    <span
                      style={{
                        width: `${(protectedBytes / compositionTotal) * 100}%`,
                        background: "var(--text-faint)",
                      }}
                    />
                  )}
                </div>

                <div className="ov-legend">
                  <span className="ov-legend__item">
                    <span className="ov-dot" style={{ background: "var(--safe)" }} />
                    Regenerates
                  </span>
                  <span className="ov-legend__item">
                    <span className="ov-dot" style={{ background: "var(--caution)" }} />
                    Re-download
                  </span>
                  <span className="ov-legend__item">
                    <span className="ov-dot" style={{ background: "var(--text-faint)" }} />
                    Protected · measured only
                  </span>
                </div>

                {compositionBytes === 0 && (
                  <p className="ov-note">
                    Cache locations exist but hold no measurable bytes yet.
                  </p>
                )}

                <ul className="ov-list ov-cleanup__list">
                  {topTools.length === 0 && (
                    <li className="ov-row muted small">
                      Cache locations exist but are currently empty.
                    </li>
                  )}
                  {topTools.map((target) => (
                    <li className="ov-row" key={target.id}>
                      <div className="ov-row__main">
                        <span className="ov-row__title truncate">{target.tool}</span>
                        <span className="ov-row__meta truncate">{target.label}</span>
                      </div>
                      <span className="ov-row__end">
                        <RiskBadge risk={target.risk} />
                        <span className="mono">{formatBytes(target.size)}</span>
                      </span>
                    </li>
                  ))}
                </ul>

                <p className="ov-note">
                  Measured {relativeTime(inventory.scannedAt).toLowerCase()}. Cleaning is previewed
                  first, then kept in a 7-day quarantine you can restore. Protected locations
                  (histories, models, volumes) are never offered.
                </p>
              </>
            )}
          </Panel>

          <Panel
            className="ov-activity"
            title="Recent activity"
            subtitle="Scan and cleanup history from this computer"
            actions={
              <>
                {history.status === "loading" && historyData && (
                  <span className="badge badge--neutral">Updating</span>
                )}
                {history.status === "error" && historyData && history.error !== BACKEND_OFFLINE && (
                  <span className="badge badge--caution">Stale</span>
                )}
                <Button
                  size="sm"
                  variant="ghost"
                  icon={<IconArrowUpRight size={13} />}
                  onClick={() => onNavigate("activity")}
                >
                  View all
                </Button>
              </>
            }
          >
            {!historyData && history.status === "loading" && <LoadingRows rows={4} />}
            {history.status === "error" && historyData && history.error && history.error !== BACKEND_OFFLINE && (
              <Banner tone="caution">
                <strong>Showing the last successful history.</strong> Refresh failed. {history.error}
              </Banner>
            )}
            {history.status === "error" && !historyData && history.error && history.error !== BACKEND_OFFLINE && (
              <LoadError message={history.error} onRetry={history.reload} />
            )}

            {historyData && recentHistory.length === 0 && (
              <EmptyState
                compact
                icon={IconClock}
                title="No activity yet"
                description="Run a disk analysis or a cleanup and Cyron records what happened, how long it took and what changed."
                action={
                  <Button
                    size="sm"
                    icon={<IconActivity size={13} />}
                    onClick={() => onNavigate("analyzer")}
                  >
                    Open disk analyzer
                  </Button>
                }
              />
            )}

            {recentHistory.length > 0 && (
              <ul className="ov-list">
                {recentHistory.map((entry) => {
                  const Icon = historyIcon(entry.kind);
                  return (
                    <li className="ov-row" key={entry.id}>
                      <span className="ov-row__icon" aria-hidden="true">
                        <Icon size={14} />
                      </span>
                      <div className="ov-row__main">
                        <span className="ov-row__title truncate">{entry.title}</span>
                        <span className="ov-row__meta truncate">
                          {entry.detail} · {relativeTime(entry.at)}
                        </span>
                      </div>
                      <span className="ov-row__end">
                        {entry.bytes > 0 && <span className="mono">{formatBytes(entry.bytes)}</span>}
                        <StatusBadge entry={entry} />
                      </span>
                    </li>
                  );
                })}
              </ul>
            )}
          </Panel>
        </div>
      </div>
    </div>
  );
}
