import { useEffect, useState, type ReactNode } from "react";
import { useAsync } from "../../lib/hooks";
import {
  api,
  BACKEND_OFFLINE,
  invalidateCaches,
  invalidateInstalledApps,
  loadCaches,
  loadInstalledApps,
} from "../../lib/api";
import {
  formatBytes,
  formatCount,
  formatDate,
  displayPath,
  formatPercent,
  friendlyError,
  operationTitle,
  plural,
  relativeTime,
} from "../../lib/format";
import type { CacheTarget, HistoryEntry, ViewId, VolumeInfo } from "../../lib/types";
import {
  Badge,
  Banner,
  Button,
  EmptyState,
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
        <div className="home-skeleton-row" key={index}>
          <Skeleton width={30} height={26} />
          <div className="grow stack" style={{ gap: 5 }}>
            <Skeleton width={`${62 - index * 8}%`} height={11} />
            <Skeleton width="40%" height={9} />
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
  return <Badge tone={tone}>{label}</Badge>;
}

function formatUptime(seconds: number): string {
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (days > 0) return `${days}d ${hours}h`;
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
  const readyVolumes = [
    ...new Map(
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
    ).values(),
  ].sort((a, b) => kindRank(a.kind) - kindRank(b.kind) || a.mount.localeCompare(b.mount));
  const totalCapacity = readyVolumes.reduce((sum, volume) => sum + volume.total, 0);
  const totalFree = readyVolumes.reduce((sum, volume) => sum + volume.free, 0);
  const totalUsed = Math.max(totalCapacity - totalFree, 0);
  const usedRatio = totalCapacity > 0 ? totalUsed / totalCapacity : 0;

  const apps = applicationData?.apps ?? [];
  const warnings = applicationData?.warnings ?? [];
  const machineCount = apps.filter((app) => app.scope === "machine").length;
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
  const compositionTotal = Math.max(compositionBytes, 1);

  const recentHistory = [...(historyData ?? [])].sort((a, b) => b.at - a.at).slice(0, 5);

  const offline = [volumes, applications, caches, history, specs].some(
    (state) => state.error === BACKEND_OFFLINE,
  );
  const refreshing = [volumes, applications, caches, history, specs].some(
    (state) => state.status === "loading",
  );

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

  const systemItems = specsData
    ? [
        {
          label: "OS",
          value: `${specsData.osName || "Windows"}${
            specsData.osBuild ? ` · ${specsData.osBuild}` : ""
          }`,
        },
        { label: "Processor", value: specsData.cpuBrand || "Unknown" },
        {
          label: "Memory",
          value: `${formatBytes(
            specsData.totalMemoryBytes - specsData.availableMemoryBytes,
          )} of ${formatBytes(specsData.totalMemoryBytes)} (${specsData.memoryInUsePercent}%)`,
        },
        { label: "Graphics", value: specsData.gpuBrand || "Standard display" },
        { label: "Uptime", value: formatUptime(specsData.uptimeSeconds) },
      ]
    : [];

  return (
    <div>
      <ViewHeader
        title="Home"
        description="A clear summary of this PC: what is installed, where space goes, and what can be reclaimed safely."
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
              Open disk space
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

        <section className="home-system" aria-label="This PC">
          {systemItems.length > 0 &&
            systemItems.map((item) => (
              <div className="home-system__item" key={item.label}>
                <span className="home-system__label">{item.label}</span>
                <span className="home-system__value" title={item.value}>
                  {item.value}
                </span>
              </div>
            ))}
          {!specsData && specs.status === "loading" && (
            <div className="home-system__loading" aria-busy="true">
              <Skeleton width={220} height={11} />
              <Skeleton width={180} height={11} />
              <Skeleton width={140} height={11} />
            </div>
          )}
          {!specsData && specs.status === "error" && specs.error !== BACKEND_OFFLINE && (
            <div className="home-system__error">
              <span>System details are unavailable.</span>
              <Button size="sm" variant="ghost" onClick={specs.reload}>
                Retry
              </Button>
            </div>
          )}
        </section>

        <section className="home-stats" aria-label="At a glance">
          <Stat
            label="Free space"
            value={
              volumeData ? formatBytes(totalFree) : <Skeleton width={92} height={28} />
            }
            meta={volumeData ? `${formatPercent(usedRatio)} used of ${formatBytes(totalCapacity)}` : "Reading volumes"}
          />
          <Stat
            label="Installed applications"
            value={
              applicationData ? formatCount(apps.length) : <Skeleton width={62} height={28} />
            }
            meta={
              applicationData
                ? `${formatCount(machineCount)} for all users · ${formatCount(
                    apps.length - machineCount,
                  )} for you`
                : "Reading the registry"
            }
          />
          <Stat
            label="Can be regenerated"
            value={
              inventory ? formatBytes(safeBytes) : <Skeleton width={86} height={28} />
            }
            meta={
              inventory
                ? `across ${plural(measuredLocations, "measured location")}`
                : "Measuring known caches"
            }
            tone="accent"
          />
        </section>

        <div className="home-grid">
          <Panel
            className="home-storage"
            title="Storage"
            subtitle="Capacity and free space on ready local volumes"
            actions={
              <>
                {volumes.status === "error" && volumeData && volumes.error !== BACKEND_OFFLINE && (
                  <Badge tone="caution">Stale</Badge>
                )}
                <Button
                  size="sm"
                  variant="ghost"
                  icon={<IconArrowUpRight size={13} />}
                  onClick={() => onNavigate("analyzer")}
                >
                  Open disk space
                </Button>
              </>
            }
          >
            {!volumeData && volumes.status === "loading" && <LoadingRows rows={3} />}
            {volumes.status === "error" && volumeData && volumes.error !== BACKEND_OFFLINE && (
              <Banner tone="caution">
                <strong>Showing the last successful reading.</strong> Refresh failed.{" "}
                {volumes.error}
              </Banner>
            )}
            {volumes.status === "error" &&
              !volumeData &&
              volumes.error &&
              volumes.error !== BACKEND_OFFLINE && (
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
                <div className="home-storage__summary">
                  <div className="home-storage__label">
                    <span className="home-subhead__title">
                      {plural(readyVolumes.length, "volume")}
                    </span>
                    <span className="home-caption">
                      {formatBytes(totalUsed)} used of {formatBytes(totalCapacity)}
                    </span>
                  </div>
                  <div className="home-storage__meter">
                    <div className="spread">
                      <span className="home-muted">Used</span>
                      <span className="mono small muted">{formatPercent(usedRatio)}</span>
                    </div>
                    <ProgressBar
                      value={usedRatio}
                      label="Used space across all volumes"
                      tone={usedRatio >= 0.9 ? "caution" : "accent"}
                    />
                  </div>
                </div>

                <ul className="home-volumes">
                  {readyVolumes.map((volume) => {
                    const volumeUsed = Math.max(volume.total - volume.free, 0);
                    const ratio = volumeUsed / volume.total;
                    const letter = volume.mount.replace(/[\\/]/g, "").replace(/:$/, "") || "?";
                    return (
                      <li className="home-volume" key={volume.mount}>
                        <span className="home-drive" aria-hidden="true">
                          {letter}:
                        </span>
                        <div className="home-volume__main">
                          <div className="home-volume__title">
                            <span className="truncate">{volumeName(volume)}</span>
                            <span className="home-volume__kind">
                              {VOLUME_KIND[volume.kind] ?? "Volume"} ·{" "}
                              {volume.fileSystem || "unknown"}
                            </span>
                          </div>
                          <ProgressBar
                            value={ratio}
                            tone={ratio >= 0.9 ? "caution" : "accent"}
                            label={`${letter}: usage`}
                          />
                        </div>
                        <div className="home-volume__figures">
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
            className="home-cleanup"
            title="Cleanup potential"
            subtitle="Known developer, AI and Windows caches measured on this PC"
            actions={
              <>
                {caches.status === "error" && cacheData && caches.error !== BACKEND_OFFLINE && (
                  <Badge tone="caution">Stale</Badge>
                )}
                <Button
                  size="sm"
                  variant="ghost"
                  icon={<IconArrowUpRight size={13} />}
                  onClick={() => onNavigate("cleaner")}
                >
                  Review cleanup
                </Button>
              </>
            }
          >
            {!cacheData && caches.status === "loading" && <LoadingRows rows={3} />}
            {caches.status === "error" && cacheData && caches.error && caches.error !== BACKEND_OFFLINE && (
              <Banner tone="caution">
                <strong>Showing the last successful measurement.</strong> Refresh failed.{" "}
                {caches.error}
              </Banner>
            )}
            {caches.status === "error" &&
              !cacheData &&
              caches.error &&
              caches.error !== BACKEND_OFFLINE && (
                <LoadError message={caches.error} onRetry={caches.reload} />
              )}

            {inventory && existingTargets.length === 0 && (
              <EmptyState
                compact
                icon={IconCleaner}
                title="No cache locations found"
                description="None of the known developer or AI tool cache folders exist for this account yet."
              />
            )}

            {inventory && inventory.warnings.length > 0 && (
              <Banner tone="caution">
                <strong>Some locations could not be measured.</strong>
                <details className="home-warning-details">
                  <summary>Review the blocked folders</summary>
                  <ul className="home-warning-list">
                    {[...new Set(inventory.warnings)].map((warning) => (
                      <li key={warning}>{friendlyError(warning)}</li>
                    ))}
                  </ul>
                </details>
              </Banner>
            )}

            {inventory && existingTargets.length > 0 && (
              <>
                <div className="home-cleanup__head">
                  <div className="home-cleanup__label">
                    <span className="home-subhead__title">
                      {formatBytes(compositionBytes)} measured
                    </span>
                    <span className="home-caption">
                      Across {plural(measuredLocations, "location")}
                    </span>
                  </div>
                  <div className="home-cleanup__side">
                    <Stat label="Needs review" value={formatBytes(cautionBytes)} />
                    <Stat label="Protected" value={formatBytes(protectedBytes)} />
                  </div>
                </div>

                <div
                  className="home-split"
                  role="img"
                  aria-label={`${formatBytes(safeBytes)} safe, ${formatBytes(
                    cautionBytes,
                  )} needs review, ${formatBytes(protectedBytes)} protected`}
                >
                  {safeBytes > 0 && (
                    <span
                      className="home-split__safe"
                      style={{ width: `${(safeBytes / compositionTotal) * 100}%` }}
                    />
                  )}
                  {cautionBytes > 0 && (
                    <span
                      className="home-split__caution"
                      style={{ width: `${(cautionBytes / compositionTotal) * 100}%` }}
                    />
                  )}
                  {protectedBytes > 0 && (
                    <span
                      className="home-split__protected"
                      style={{ width: `${(protectedBytes / compositionTotal) * 100}%` }}
                    />
                  )}
                </div>

                <div className="home-legend">
                  <span className="home-legend__item">
                    <span className="home-dot home-dot--safe" />
                    Regenerates
                  </span>
                  <span className="home-legend__item">
                    <span className="home-dot home-dot--caution" />
                    Re-download
                  </span>
                  <span className="home-legend__item">
                    <span className="home-dot home-dot--protected" />
                    Protected
                  </span>
                </div>

                {compositionBytes === 0 && (
                  <p className="home-note">
                    Cache locations exist but hold no measurable bytes yet.
                  </p>
                )}

                <ul className="home-list">
                  {topTools.length === 0 && (
                    <li className="home-row muted small">
                      Cache locations exist but are currently empty.
                    </li>
                  )}
                  {topTools.map((target) => (
                    <li className="home-row" key={target.id}>
                      <div className="home-row__main">
                        <span className="home-row__title truncate">{target.tool}</span>
                        <span className="home-row__meta truncate">{target.label}</span>
                      </div>
                      <span className="home-row__end">
                        <RiskBadge risk={target.risk} />
                        <span className="mono">{formatBytes(target.size)}</span>
                      </span>
                    </li>
                  ))}
                </ul>

                <p className="home-note">
                  Measured {relativeTime(inventory.scannedAt).toLowerCase()}. Preview first; moved
                  files stay restorable for seven days.
                </p>
              </>
            )}
          </Panel>

          <Panel
            className="home-apps"
            title="Applications"
            subtitle="Discovered from the Windows uninstall registry"
            actions={
              <>
                {applications.status === "error" &&
                  applicationData &&
                  applications.error !== BACKEND_OFFLINE && <Badge tone="caution">Stale</Badge>}
                <Button
                  size="sm"
                  variant="ghost"
                  icon={<IconArrowUpRight size={13} />}
                  onClick={() => onNavigate("uninstaller")}
                >
                  Open applications
                </Button>
              </>
            }
          >
            {!applicationData && applications.status === "loading" && <LoadingRows rows={4} />}
            {applications.status === "error" &&
              applicationData &&
              applications.error &&
              applications.error !== BACKEND_OFFLINE && (
                <Banner tone="caution">
                  <strong>Showing the last successful inventory.</strong> Refresh failed.{" "}
                  {applications.error}
                </Banner>
              )}
            {applications.status === "error" &&
              !applicationData &&
              applications.error &&
              applications.error !== BACKEND_OFFLINE && (
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
              <Banner tone="caution">
                <strong>Partial inventory.</strong> {[...new Set(warnings)].join(" ")}
              </Banner>
            )}

            {apps.length > 0 && (
              <>
                <div className="home-apps__stats">
                  <Stat label="All users" value={formatCount(machineCount)} />
                  <Stat label="This account" value={formatCount(apps.length - machineCount)} />
                  <Stat label="With install date" value={formatCount(
                    apps.filter(
                      (app) => app.installDate && Number.isFinite(Date.parse(app.installDate)),
                    ).length,
                  )} />
                </div>
                <div className="home-subhead">
                  <span className="home-subhead__title">Recently installed</span>
                  <span className="home-caption">
                    {apps.filter(
                      (app) => app.installDate && Number.isFinite(Date.parse(app.installDate)),
                    ).length}{" "}
                    of {apps.length} entries publish a date
                  </span>
                </div>
                {recentInstalls.length > 0 ? (
                  <ul className="home-list">
                    {recentInstalls.map((app) => (
                      <li className="home-row" key={app.id}>
                        <div className="home-row__main">
                          <span className="home-row__title truncate">{app.name}</span>
                          <span className="home-row__meta truncate">
                            {app.publisher ?? "Unknown publisher"}
                          </span>
                        </div>
                        <span className="home-row__end mono">{formatDate(app.installDate)}</span>
                      </li>
                    ))}
                  </ul>
                ) : (
                  <p className="home-note home-note--pad">
                    No install dates were published by these entries.
                  </p>
                )}
              </>
            )}
          </Panel>

          <Panel
            className="home-activity"
            title="Recent activity"
            subtitle="Scan and cleanup history from this computer"
            actions={
              <>
                {history.status === "error" && historyData && history.error !== BACKEND_OFFLINE && (
                  <Badge tone="caution">Stale</Badge>
                )}
                <Button
                  size="sm"
                  variant="ghost"
                  icon={<IconArrowUpRight size={13} />}
                  onClick={() => onNavigate("activity")}
                >
                  View history
                </Button>
              </>
            }
          >
            {!historyData && history.status === "loading" && <LoadingRows rows={4} />}
            {history.status === "error" &&
              historyData &&
              history.error &&
              history.error !== BACKEND_OFFLINE && (
                <Banner tone="caution">
                  <strong>Showing the last successful history.</strong> Refresh failed.{" "}
                  {history.error}
                </Banner>
              )}
            {history.status === "error" &&
              !historyData &&
              history.error &&
              history.error !== BACKEND_OFFLINE && (
                <LoadError message={history.error} onRetry={history.reload} />
              )}

            {historyData && recentHistory.length === 0 && (
              <EmptyState
                compact
                icon={IconClock}
                title="No activity yet"
                description="Run a disk scan or a cleanup and Cyron records what happened, how long it took and what changed."
                action={
                  <Button
                    size="sm"
                    icon={<IconActivity size={13} />}
                    onClick={() => onNavigate("analyzer")}
                  >
                    Open disk space
                  </Button>
                }
              />
            )}

            {recentHistory.length > 0 && (
              <ul className="home-list">
                {recentHistory.map((entry) => {
                  const Icon = historyIcon(entry.kind);
                  const title = operationTitle(
                    entry.title,
                    entry.kind === "scan" ? "Disk scan" : "Cleanup",
                  );
                  return (
                    <li className="home-row" key={entry.id}>
                      <span className="home-row__icon" aria-hidden="true">
                        <Icon size={14} />
                      </span>
                      <div className="home-row__main">
                        <span className="home-row__title truncate" title={displayPath(entry.title)}>
                          {title}
                        </span>
                        <span
                          className="home-row__meta truncate"
                          title={`${entry.detail} · ${relativeTime(entry.at)}`}
                        >
                          {entry.detail} · {relativeTime(entry.at)}
                        </span>
                      </div>
                      <span className="home-row__end">
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
