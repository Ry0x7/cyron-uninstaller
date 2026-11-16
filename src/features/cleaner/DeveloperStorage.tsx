import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Badge,
  Banner,
  Button,
  Drawer,
  EmptyState,
  IconButton,
  Panel,
  RiskBadge,
  SearchField,
  Segmented,
  Skeleton,
  Stat,
} from "../../components/ui";
import {
  IconAlert,
  IconChevronRight,
  IconClose,
  IconFolder,
  IconInfo,
  IconLock,
  IconRefresh,
} from "../../components/Icons";
import { api, errorMessage } from "../../lib/api";
import {
  ACTIVITY_LABEL,
  ACTIVITY_OPTIONS,
  ARTIFACT_KIND_LABEL,
  filterProjects,
  flattenArtifacts,
  selectedArtifactTotals,
  verificationTone,
  type ActivityFilter,
} from "../../lib/developer";
import {
  displayPath,
  formatBytes,
  formatCount,
  formatDate,
  formatDateTime,
  plural,
  truncateMiddle,
} from "../../lib/format";
import { useSettings } from "../../lib/settings";
import type {
  CacheInventory,
  CleanupPreview,
  CleanupReport,
  DeveloperArtifact,
  DeveloperCleanupVerification,
  DeveloperStorageScan,
} from "../../lib/types";
import "./cleaner.css";

type Status = "idle" | "scanning" | "ready" | "error";

function projectTotal(project: DeveloperStorageScan["projects"][number]): number {
  return project.artifacts.reduce((sum, artifact) => sum + artifact.size, 0);
}

function projectProtected(project: DeveloperStorageScan["projects"][number]): number {
  return project.protected.reduce((sum, item) => sum + item.size, 0);
}

function kindLabel(kind: string): string {
  return ARTIFACT_KIND_LABEL[kind] ?? kind;
}

function activityTone(activity: string): "safe" | "caution" | "neutral" {
  if (activity === "active") return "safe";
  if (activity === "old") return "caution";
  return "neutral";
}

export function DeveloperStorage({
  readOnly,
  scanDrives,
  cacheInventory,
  onOpenQuarantine,
}: {
  readOnly: boolean | null;
  scanDrives: string[];
  cacheInventory: CacheInventory | null;
  onOpenQuarantine: () => void;
}) {
  const { settings, setProjectRoots } = useSettings();
  const [scan, setScan] = useState<DeveloperStorageScan | null>(null);
  const [status, setStatus] = useState<Status>("idle");
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [activity, setActivity] = useState<ActivityFilter>("all");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [expanded, setExpanded] = useState<Set<string>>(new Set());

  const [preview, setPreview] = useState<CleanupPreview | null>(null);
  const [previewOpen, setPreviewOpen] = useState(false);
  const [previewLoading, setPreviewLoading] = useState(false);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const [report, setReport] = useState<CleanupReport | null>(null);
  const [runError, setRunError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [verification, setVerification] = useState<DeveloperCleanupVerification | null>(null);
  const [verifying, setVerifying] = useState(false);
  const [verifyError, setVerifyError] = useState<string | null>(null);
  const mounted = useRef(true);
  const scanRequest = useRef(0);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const runScan = useCallback(
    async (roots = settings.projectRoots) => {
      const ticket = ++scanRequest.current;
      setStatus("scanning");
      setError(null);
      try {
        const result = await api.scanDeveloperStorage(roots, scanDrives.length ? scanDrives : undefined);
        if (!mounted.current || scanRequest.current !== ticket) return;
        setScan(result);
        setSelected(new Set());
        setStatus("ready");
      } catch (caught) {
        if (!mounted.current || scanRequest.current !== ticket) return;
        setError(errorMessage(caught));
        setStatus("error");
      }
    },
    [scanDrives, settings.projectRoots],
  );

  useEffect(() => {
    void runScan();
    // Initial discovery only. Later scans are explicit or follow a folder change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const artifacts = useMemo(() => flattenArtifacts(scan), [scan]);
  const visibleProjects = useMemo(
    () => (scan ? filterProjects(scan.projects, query, activity) : []),
    [scan, query, activity],
  );
  const selection = useMemo(() => selectedArtifactTotals(artifacts, selected), [artifacts, selected]);

  const toolCaches = useMemo(() => {
    const existing = (cacheInventory?.targets ?? []).filter((target) => target.exists);
    return {
      count: existing.filter((target) => target.risk !== "protected").length,
      bytes: existing.filter((target) => target.risk !== "protected").reduce((sum, target) => sum + target.size, 0),
      protectedBytes: existing.filter((target) => target.risk === "protected").reduce((sum, target) => sum + target.size, 0),
      orphaned: existing.filter((target) => target.risk !== "protected" && target.installState === "not-detected").length,
    };
  }, [cacheInventory]);

  const addFolder = async () => {
    try {
      const selection = await open({ directory: true, multiple: false, title: "Add a project folder" });
      const picked = Array.isArray(selection) ? selection[0] : selection;
      if (typeof picked !== "string" || !picked.trim()) return;
      const next = Array.from(new Set([...settings.projectRoots, picked.trim()])).slice(0, 24);
      setProjectRoots(next);
      await runScan(next);
    } catch (caught) {
      if (mounted.current) setError(errorMessage(caught));
    }
  };

  const removeFolder = (path: string) => {
    const next = settings.projectRoots.filter((root) => root !== path);
    setProjectRoots(next);
    void runScan(next);
  };

  const toggleArtifact = (artifact: DeveloperArtifact) => {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(artifact.id)) next.delete(artifact.id);
      else next.add(artifact.id);
      return next;
    });
  };

  const toggleExpanded = (id: string) => {
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const openPreview = async () => {
    if (!scan || selection.count === 0) return;
    setPreviewLoading(true);
    setPreviewError(null);
    setReport(null);
    setRunError(null);
    setVerification(null);
    setVerifyError(null);
    setConfirmed(false);
    setPreviewOpen(true);
    try {
      const data = await api.previewDeveloperCleanup(
        scan.scanId,
        Array.from(selected),
        settings.exclusions,
      );
      if (!mounted.current) return;
      setPreview(data);
    } catch (caught) {
      if (!mounted.current) return;
      setPreview(null);
      setPreviewError(errorMessage(caught));
    } finally {
      if (mounted.current) setPreviewLoading(false);
    }
  };

  const closePreview = () => {
    if (running) return;
    setPreviewOpen(false);
    setConfirmed(false);
    setPreviewError(null);
  };

  const verifyAfterRun = async (scanId: string, ids: string[]) => {
    setVerifying(true);
    setVerifyError(null);
    try {
      const check = await api.verifyDeveloperCleanup(scanId, ids);
      if (!mounted.current) return;
      setVerification(check);
      // Refresh the project list so remaining artifacts and sizes reflect disk.
      const fresh = await api.scanDeveloperStorage(
        settings.projectRoots,
        scanDrives.length ? scanDrives : undefined,
      );
      if (!mounted.current) return;
      setScan(fresh);
      setSelected(new Set());
    } catch (caught) {
      if (mounted.current) setVerifyError(errorMessage(caught));
    } finally {
      if (mounted.current) setVerifying(false);
    }
  };

  const runCleanup = async () => {
    if (!preview || !scan || running || readOnly !== false) return;
    const ids = preview.targets.map((target) => target.id);
    setRunning(true);
    setRunError(null);
    try {
      const result = await api.startCleanup(preview.planId, ids);
      if (!mounted.current) return;
      setReport(result);
      setSelected(new Set());
      await verifyAfterRun(scan.scanId, ids);
    } catch (caught) {
      if (mounted.current) setRunError(errorMessage(caught));
    } finally {
      if (mounted.current) setRunning(false);
    }
  };

  return (
    <>
      <Panel
        title="Developer projects"
        subtitle="Find regenerable build and dependency folders without opening every repository. Source, Git data, credentials and model files are never selected."
        actions={
          <>
            <Button size="sm" icon={<IconFolder size={14} />} onClick={() => void addFolder()}>
              Add project folder
            </Button>
            <Button
              size="sm"
              icon={<IconRefresh size={14} />}
              loading={status === "scanning"}
              disabled={status === "scanning"}
              onClick={() => void runScan()}
            >
              Scan projects
            </Button>
          </>
        }
      >
        <div className="cl-dev-roots" aria-label="Project folders">
          {settings.projectRoots.length === 0 && (
            <span className="small muted">
              Automatic candidates in your profile and on the selected drives are included. Add a folder to
              cover projects stored elsewhere.
            </span>
          )}
          {settings.projectRoots.map((root) => (
            <span className="cl-dev-root" key={root}>
              <IconFolder size={12} />
              <span title={root}>{truncateMiddle(root, 48)}</span>
              <IconButton label={`Remove ${root}`} onClick={() => removeFolder(root)}>
                <IconClose size={11} />
              </IconButton>
            </span>
          ))}
        </div>
        {(scan?.roots.some((root) => root.source === "automatic") || toolCaches.count > 0) && (
          <div className="cl-dev-note">
            {scan?.roots.some((root) => root.source === "automatic") && (
              <span>
                Scanned {plural(scan.roots.length, "project root")} on {scanDrives.length ? scanDrives.join(", ") : "local drives"}.
              </span>
            )}
            {toolCaches.count > 0 && (
              <span>
                {" "}
                {plural(toolCaches.count, "tool cache")} ({formatBytes(toolCaches.bytes)}) are in the Cleanup
                section
                {toolCaches.orphaned > 0
                  ? `; ${plural(toolCaches.orphaned, "tool")} not detected and shown as orphaned there`
                  : ""}
                . Protected tool data: {formatBytes(toolCaches.protectedBytes)}.
              </span>
            )}
          </div>
        )}
      </Panel>

      <Banner tone="info">
        <strong>Old does not mean disposable.</strong> Project age and artifact age are evidence, not
        permission. Every cleanup needs a preview and an explicit confirmation. Source, <code>.git</code>,{" "}
        <code>.env</code>, credentials, databases and model weights stay untouched even if they are large.
      </Banner>

      {error && (
        <Banner
          tone="danger"
          actions={
            <Button size="sm" onClick={() => void runScan()}>
              Retry
            </Button>
          }
        >
          <strong>Project scan failed.</strong> {error}
        </Banner>
      )}

      {status === "scanning" && !scan && (
        <Panel title="Scanning projects">
          <div className="cl-dev-skeleton">
            <Skeleton width="32%" height={13} />
            <Skeleton width="54%" height={10} />
            <Skeleton width="70%" height={10} />
            <Skeleton width="44%" height={10} />
          </div>
        </Panel>
      )}

      {scan && (
        <>
          <Panel title="Measured developer storage" subtitle={`Last scan ${formatDateTime(scan.scannedAt)}`}>
            <div className="cl-summary__grid">
              <Stat label="Projects" value={formatCount(scan.totals.projectCount)} meta={`${formatCount(scan.totals.activeProjects)} active · ${formatCount(scan.totals.oldProjects)} old`} size="lg" />
              <Stat label="Regenerable artifacts" value={formatBytes(scan.totals.artifactBytes)} meta={`${formatCount(scan.totals.artifactCount)} folders`} />
              <Stat label="On disk" value={formatBytes(scan.totals.artifactAllocated)} meta="physical estimate before quarantine" />
              <Stat label="Protected data" value={formatBytes(scan.totals.protectedBytes)} meta="Git, credentials, models · measured only" />
            </div>
            <p className="small muted cl-dev-note">
              “On disk” is the allocated size of selected artifact folders. Cleanup moves files into Cyron's
              quarantine; physical space is freed only when you purge that batch. Restore is available for
              seven days and never overwrites existing files.
            </p>
          </Panel>

          {scan.warnings.length > 0 && (
            <Banner tone="caution">
              <strong>{plural(scan.warnings.length, "scan warning")}.</strong>
              <ul className="cl-warning-list">
                {scan.warnings.map((warning) => (
                  <li key={warning}>{warning}</li>
                ))}
              </ul>
            </Banner>
          )}

          <div className="cl-dev-toolbar">
            <SearchField
              value={query}
              onChange={setQuery}
              placeholder="Search project, framework or artifact"
              ariaLabel="Search projects"
            />
            <Segmented<ActivityFilter>
              ariaLabel="Filter projects by activity"
              value={activity}
              onChange={setActivity}
              options={ACTIVITY_OPTIONS}
            />
            <span className="cl-toolbar__spacer" />
            <span className="small muted">{plural(visibleProjects.length, "project")} shown</span>
          </div>

          {visibleProjects.length === 0 ? (
            <Panel title="Projects">
              <EmptyState
                compact
                icon={IconFolder}
                title={scan.projects.length === 0 ? "No projects found" : "No projects match"}
                description={
                  scan.projects.length === 0
                    ? "Add the folder that contains your projects. Cyron recognises package.json, Cargo.toml, pyproject.toml, go.mod, Gradle/Maven files, .NET projects, CMake and .git roots."
                    : "Clear the search or activity filter to see the scanned projects again."
                }
                action={
                  scan.projects.length === 0 ? (
                    <Button size="sm" variant="primary" onClick={() => void addFolder()}>
                      Add project folder
                    </Button>
                  ) : (
                    <Button
                      size="sm"
                      onClick={() => {
                        setQuery("");
                        setActivity("all");
                      }}
                    >
                      Clear filters
                    </Button>
                  )
                }
              />
            </Panel>
          ) : (
            <div className="cl-dev-projects">
              {visibleProjects.map((project) => {
                const total = projectTotal(project);
                const protectedTotal = projectProtected(project);
                return (
                  <section className="cl-dev-project" key={project.id} aria-label={project.name}>
                    <header className="cl-dev-project__head">
                      <div className="cl-dev-project__identity">
                        <div className="cl-dev-project__titleline">
                          <h3>{project.name}</h3>
                          <Badge tone="neutral">{project.kind}</Badge>
                          <Badge tone={activityTone(project.activity)}>{project.activity}</Badge>
                        </div>
                        <span className="cl-dev-project__path" title={displayPath(project.path)}>
                          {displayPath(project.path)}
                        </span>
                        <div className="cl-dev-project__markers">
                          {project.frameworks.length > 0 && (
                            <span>Frameworks: {project.frameworks.join(", ")}</span>
                          )}
                          {project.markers.slice(0, 5).map((marker) => (
                            <span className="cl-dev-marker" key={marker}>
                              {marker}
                            </span>
                          ))}
                          {project.markers.length > 5 && <span>+{project.markers.length - 5}</span>}
                        </div>
                      </div>
                      <div className="cl-dev-project__totals">
                        <strong>{formatBytes(total)}</strong>
                        <span>
                          {plural(project.artifacts.length, "artifact")} · {formatBytes(protectedTotal)} protected
                        </span>
                        <span className="small faint">{ACTIVITY_LABEL[project.activity] ?? project.activity}</span>
                      </div>
                    </header>

                    {project.artifacts.length === 0 ? (
                      <p className="cl-dev-project__empty">
                        No regenerable dependency or build folders were measured. Source remains protected.
                      </p>
                    ) : (
                      <div className="cl-dev-artifacts">
                        {project.artifacts.map((artifact) => {
                          const isSelected = selected.has(artifact.id);
                          const isOpen = expanded.has(artifact.id);
                          return (
                            <div className={`cl-dev-artifact${isSelected ? " is-selected" : ""}`} key={artifact.id}>
                              <input
                                type="checkbox"
                                className="cl-check"
                                checked={isSelected}
                                onChange={() => toggleArtifact(artifact)}
                                aria-label={`Select ${artifact.label} in ${project.name}`}
                              />
                              <div className="cl-dev-artifact__main">
                                <button
                                  type="button"
                                  className="cl-dev-artifact__title"
                                  aria-expanded={isOpen}
                                  onClick={() => toggleExpanded(artifact.id)}
                                >
                                  <span className="cl-dev-artifact__name">{artifact.label}</span>
                                  <Badge tone="neutral">{kindLabel(artifact.kind)}</Badge>
                                  <RiskBadge risk={artifact.risk} />
                                  {artifact.stale && <Badge tone="caution">Not written in 90+ days</Badge>}
                                  <IconChevronRight size={13} className={isOpen ? "is-open" : ""} />
                                </button>
                                <span className="cl-dev-artifact__path" title={displayPath(artifact.path)}>
                                  {truncateMiddle(displayPath(artifact.path), 92)}
                                </span>
                                <div className="cl-dev-artifact__meta">
                                  <span>{formatBytes(artifact.size)} logical</span>
                                  <span>{formatBytes(artifact.allocated)} on disk</span>
                                  <span>{plural(artifact.fileCount, "file")}</span>
                                  <span>Modified {formatDate(artifact.lastModified)}</span>
                                  {artifact.skippedLinks > 0 && <span>{plural(artifact.skippedLinks, "link")} skipped</span>}
                                  {artifact.truncated && <span className="cl-dev-truncated">measurement capped</span>}
                                </div>
                                {isOpen && (
                                  <div className="cl-dev-artifact__detail">
                                    <p>
                                      <strong>What is lost:</strong> {artifact.loss}
                                    </p>
                                    <p>
                                      <strong>How to rebuild:</strong> {artifact.rebuild}
                                    </p>
                                    <p className="small muted">
                                      <IconInfo size={12} /> {artifact.evidence} Confidence: {artifact.confidence}.
                                    </p>
                                  </div>
                                )}
                              </div>
                              <span className="cl-dev-artifact__size">{formatBytes(artifact.size)}</span>
                            </div>
                          );
                        })}
                      </div>
                    )}

                    {project.protected.length > 0 && (
                      <details className="cl-dev-protected">
                        <summary>
                          <IconLock size={13} /> {plural(project.protected.length, "protected item")} · {" "}
                          {formatBytes(protectedTotal)}
                        </summary>
                        <ul>
                          {project.protected.map((item) => (
                            <li key={item.path}>
                              <span className="cl-dev-protected__kind">{item.kind}</span>
                              <span className="cl-dev-protected__path" title={displayPath(item.path)}>
                                {truncateMiddle(displayPath(item.path), 82)}
                              </span>
                              <span>{formatBytes(item.size)}</span>
                              <span className="small muted">{item.reason}</span>
                            </li>
                          ))}
                        </ul>
                      </details>
                    )}
                  </section>
                );
              })}
            </div>
          )}

          {selection.count > 0 && (
            <div className="cl-selectbar" role="region" aria-label="Project cleanup selection">
              <div className="cl-selectbar__text">
                <strong>{plural(selection.count, "artifact")} selected</strong>
                <span>{formatBytes(selection.bytes)} logical · {formatBytes(selection.allocated)} on disk · moved to quarantine, not deleted</span>
              </div>
              <div className="cl-selectbar__actions">
                <Button size="sm" variant="ghost" onClick={() => setSelected(new Set())}>
                  Clear
                </Button>
                <Button size="sm" variant="primary" loading={previewLoading} onClick={() => void openPreview()}>
                  Preview project cleanup
                </Button>
              </div>
            </div>
          )}
        </>
      )}

      <Drawer
        open={previewOpen}
        title={report ? "Project cleanup result" : "Project cleanup preview"}
        subtitle={
          report
            ? `${plural(report.quarantined, "file")} moved to quarantine`
            : "Review the exact project artifacts before anything moves"
        }
        onClose={closePreview}
        footer={
          report ? (
            <div className="cl-drawer-foot">
              <Button onClick={closePreview}>Done</Button>
              {report.quarantined > 0 && (
                <Button
                  variant="primary"
                  onClick={() => {
                    closePreview();
                    onOpenQuarantine();
                  }}
                >
                  Open quarantine
                </Button>
              )}
            </div>
          ) : (
            <div className="cl-drawer-foot">
              <Button onClick={closePreview} disabled={running}>
                Cancel
              </Button>
              <Button
                variant="primary"
                loading={running}
                disabled={readOnly !== false || !confirmed || !preview || previewLoading}
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
            {[0, 1, 2].map((index) => (
              <div className="cl-skeleton__row" key={index}>
                <Skeleton width={170} height={11} />
                <Skeleton width="52%" height={10} />
                <Skeleton width={64} height={10} />
              </div>
            ))}
          </div>
        ) : previewError ? (
          <Banner tone="danger">
            <strong>Preview failed.</strong> {previewError}
          </Banner>
        ) : report ? (
          <div className="cl-dev-result">
            <Banner tone={report.failed > 0 || runError ? "caution" : "safe"}>
              <strong>
                {formatBytes(report.bytes)} in {plural(report.quarantined, "file")} moved to quarantine.
              </strong>{" "}
              {report.skipped > 0 && `${formatCount(report.skipped)} skipped. `}
              {report.failed > 0 && `${formatCount(report.failed)} failed; review the errors below. `}
              {report.dirsRemoved > 0 && `${plural(report.dirsRemoved, "empty folder")} removed. `}
              Nothing was permanently deleted.
            </Banner>
            {runError && (
              <Banner tone="danger">
                <strong>The cleanup did not finish.</strong> {runError}
              </Banner>
            )}
            {verifying && <Banner tone="info">Re-scanning the selected artifacts and quarantine…</Banner>}
            {verifyError && (
              <Banner tone="caution">
                <strong>Cleanup finished, but verification failed.</strong> {verifyError} Use the Quarantine tab
                to inspect recoverable files.
              </Banner>
            )}
            {verification && (
              <div className="cl-dev-verification">
                <h3>Verification</h3>
                <p className="small muted">
                  {plural(verification.removedArtifacts, "artifact")} no longer contains files;{" "}
                  {plural(verification.remainingArtifacts, "artifact")} still has{" "}
                  {formatBytes(verification.remainingBytes)}. Files created or changed after the preview are
                  deliberately left alone.
                </p>
                <ul>
                  {verification.artifacts.map((item) => (
                    <li key={item.id}>
                      <Badge tone={verificationTone(item.status)}>{item.status}</Badge>
                      <span>{item.label}</span>
                      <span className="mono">{formatBytes(item.remainingBytes)} remaining</span>
                    </li>
                  ))}
                </ul>
                {verification.errors.length > 0 && (
                  <p className="small muted">{verification.errors[0]}</p>
                )}
                {verification.restorableRunIds.length > 0 && (
                  <p className="small muted">
                    Recoverable quarantine batch{verification.restorableRunIds.length === 1 ? "" : "es"}:{" "}
                    {verification.restorableRunIds.join(", ")}. Restore from the Quarantine tab.
                  </p>
                )}
              </div>
            )}
            {report.items.length > 0 && (
              <div className="cl-dev-result__items">
                <h3>Items</h3>
                <ul>
                  {report.items.slice(0, 40).map((item) => (
                    <li key={`${item.targetId}-${item.path}`}>
                      <Badge tone={item.status === "quarantined" ? "safe" : item.status === "skipped" ? "caution" : "danger"}>
                        {item.status}
                      </Badge>
                      <span title={displayPath(item.path)}>
                        {truncateMiddle(displayPath(item.path), 74)}
                      </span>
                      <span>{formatBytes(item.bytes)}</span>
                      {item.reason && <span className="small muted">{item.reason}</span>}
                    </li>
                  ))}
                </ul>
                {report.items.length > 40 && (
                  <p className="small muted">Showing the first 40 of {formatCount(report.items.length)} item results.</p>
                )}
              </div>
            )}
            {report.errors.length > 0 && (
              <Banner tone="caution">
                <strong>Some files could not be moved.</strong>
                <ul className="cl-warning-list">
                  {report.errors.slice(0, 8).map((message) => (
                    <li key={message}>{message}</li>
                  ))}
                </ul>
              </Banner>
            )}
          </div>
        ) : preview ? (
          <div className="cl-dev-preview">
            {readOnly !== false && (
              <Banner tone="caution">
                <strong>Read-only mode is on.</strong> Reviewing is allowed. Enable cleaning in the Cleaner
                banner above before moving files.
              </Banner>
            )}
            <Banner tone="safe">
              <strong>{formatBytes(preview.bytes)}</strong> in{" "}
              <strong>{plural(preview.fileCount, "file")}</strong> will move to quarantine. Files can be
              restored; purging is permanent.
            </Banner>
            <div className="cl-dev-preview__targets">
              {preview.targets.map((target) => (
                <div className="cl-dev-preview__target" key={target.id}>
                  <div>
                    <strong>{target.label}</strong>
                    <span className="mono" title={displayPath(target.path)}>
                      {displayPath(target.path)}
                    </span>
                  </div>
                  <span>{formatBytes(target.size)}</span>
                  <span className="small muted">{plural(target.fileCount, "file")}</span>
                  <RiskBadge risk={target.risk} />
                </div>
              ))}
            </div>
            {preview.warnings.length > 0 && (
              <Banner tone="caution">
                <strong>Preview notes.</strong>
                <ul className="cl-warning-list">
                  {preview.warnings.map((warning) => (
                    <li key={warning}>{warning}</li>
                  ))}
                </ul>
              </Banner>
            )}
            <details className="cl-dev-preview__files">
              <summary>
                <IconChevronRight size={12} /> Exact affected files (first {preview.files.length})
              </summary>
              <ul>
                {preview.files.slice(0, 30).map((file) => (
                  <li key={file.path}>
                    <span title={displayPath(file.path)}>
                      {truncateMiddle(displayPath(file.path), 82)}
                    </span>
                    <span>{formatBytes(file.size)}</span>
                  </li>
                ))}
              </ul>
              {preview.hiddenCount > 0 && (
                <p className="small muted">
                  {formatCount(preview.hiddenCount)} more files are counted in the totals and will also be
                  moved. The preview is capped for readability.
                </p>
              )}
            </details>
            <label className="cl-dev-confirm">
              <input type="checkbox" checked={confirmed} onChange={(event) => setConfirmed(event.target.checked)} />
              <span>
                I understand these files move to Cyron's quarantine, not the Recycle Bin. I can restore them
                from Quarantine; purging is permanent.
              </span>
            </label>
          </div>
        ) : (
          <EmptyState
            compact
            icon={IconAlert}
            title="No preview available"
            description="Close this panel and select at least one artifact."
          />
        )}
      </Drawer>
    </>
  );
}
