import { useEffect, useRef, useState } from "react";
import { api, errorMessage } from "../../lib/api";
import { formatBytes, formatCount, formatDate, formatDuration } from "../../lib/format";
import type {
  AppProcess,
  InstalledApp,
  LeftoverCleanReport,
  LeftoverScanReport,
  Measured,
  ParsedUninstallCommand,
  UninstallExecutionResult,
  UninstallPlan,
} from "../../lib/types";
import {
  Badge,
  Banner,
  Button,
  Drawer,
  Eyebrow,
  KeyValue,
  Segmented,
} from "../../components/ui";
import {
  IconFolder,
  IconPlay,
  IconRefresh,
  IconShield,
  IconStop,
  IconTrash,
} from "../../components/Icons";
import { UninstallWizard, type UninstallMode } from "./UninstallWizard";

const HIVE_LABEL: Record<string, string> = {
  "machine-64": "HKLM · 64-bit view",
  "machine-32": "HKLM · 32-bit view",
  "user-64": "HKCU · 64-bit view",
  "user-32": "HKCU · 32-bit view",
};

type DrawerTab = "overview" | "processes" | "uninstall" | "leftovers";

export function AppDrawer({
  app,
  onClose,
  onUninstallSuccess,
}: {
  app: InstalledApp | null;
  onClose: () => void;
  onUninstallSuccess?: () => void;
}) {
  const [tab, setTab] = useState<DrawerTab>("overview");
  const [wizardOpen, setWizardOpen] = useState(false);
  const [wizardInitialMode, setWizardInitialMode] = useState<UninstallMode | undefined>(undefined);

  // Measurement state
  const [measurement, setMeasurement] = useState<{ appId: string; value: Measured } | null>(null);
  const measured = measurement && measurement.appId === app?.id ? measurement.value : null;
  const [measuring, setMeasuring] = useState(false);
  const [measureError, setMeasureError] = useState<string | null>(null);
  const [revealing, setRevealing] = useState(false);
  const [revealError, setRevealError] = useState<string | null>(null);

  // Process manager state
  const [processes, setProcesses] = useState<AppProcess[]>([]);
  const [loadingProcesses, setLoadingProcesses] = useState(false);
  const [processError, setProcessError] = useState<string | null>(null);
  const [terminating, setTerminating] = useState(false);
  const [terminateForce, setTerminateForce] = useState(false);

  // Uninstaller execution state
  const [parsedCmd, setParsedCmd] = useState<ParsedUninstallCommand | null>(null);
  const [uninstallPlan, setUninstallPlan] = useState<UninstallPlan | null>(null);
  const [uninstallConfirm, setUninstallConfirm] = useState(false);
  const [uninstalling, setUninstalling] = useState(false);
  const [uninstallResult, setUninstallResult] = useState<UninstallExecutionResult | null>(null);
  const [uninstallError, setUninstallError] = useState<string | null>(null);

  // Leftover scanner state
  const [leftoverReport, setLeftoverReport] = useState<LeftoverScanReport | null>(null);
  const [scanningLeftovers, setScanningLeftovers] = useState(false);
  const [leftoverError, setLeftoverError] = useState<string | null>(null);
  const [selectedCandidates, setSelectedCandidates] = useState<Set<string>>(new Set());
  const [cleanConfirm, setCleanConfirm] = useState(false);
  const [cleaning, setCleaning] = useState(false);
  const [cleanReport, setCleanReport] = useState<LeftoverCleanReport | null>(null);

  const activeId = useRef<string | null>(null);
  activeId.current = app?.id ?? null;

  useEffect(() => {
    setTab("overview");
    setMeasurement(null);
    setMeasureError(null);
    setRevealError(null);
    setMeasuring(false);
    setRevealing(false);

    setProcesses([]);
    setLoadingProcesses(false);
    setProcessError(null);
    setTerminating(false);

    setParsedCmd(null);
    setUninstallPlan(null);
    setUninstallConfirm(false);
    setUninstalling(false);
    setUninstallResult(null);
    setUninstallError(null);

    setLeftoverReport(null);
    setScanningLeftovers(false);
    setLeftoverError(null);
    setSelectedCandidates(new Set());
    setCleanConfirm(false);
    setCleaning(false);
    setCleanReport(null);

    if (app?.id) {
      loadProcesses(app.id);

      if (!app.isProtected) {
        api.prepareUninstall(app.id, false)
          .then((plan) => {
            if (activeId.current === app.id) setUninstallPlan(plan);
          })
          .catch((err) => {
            if (activeId.current === app.id) {
              setUninstallError(errorMessage(err));
              if (app.uninstallString) {
                api.parseUninstallCommand(app.uninstallString)
                  .then((parsed) => {
                    if (activeId.current === app.id) setParsedCmd(parsed);
                  })
                  .catch(() => {});
              }
            }
          });
      }
    }
  }, [app?.id]);

  if (!app) return null;

  const location = app.installLocation;
  const subtitle =
    [app.version ? `Version ${app.version}` : null, app.publisher].filter(Boolean).join(" · ") ||
    "Installed application";

  const loadProcesses = async (appId?: string) => {
    const id = appId ?? app?.id;
    if (!id) return;
    setLoadingProcesses(true);
    setProcessError(null);
    try {
      const res = await api.discoverAppProcesses(id);
      if (activeId.current === id) setProcesses(res);
    } catch (err) {
      if (activeId.current === id) setProcessError(errorMessage(err));
    } finally {
      if (activeId.current === id) setLoadingProcesses(false);
    }
  };

  const terminateAllProcesses = async () => {
    if (processes.length === 0 || terminating) return;
    setTerminating(true);
    setProcessError(null);
    try {
      const pids = processes.map((p) => p.pid);
      const results = await api.terminateAppProcesses(app.id, pids, terminateForce);
      const failed = results.filter((r) => !r.success);
      if (failed.length > 0) {
        setProcessError(failed.map((f) => `${f.name}: ${f.error}`).join(" · "));
      }
      await loadProcesses();
    } catch (err) {
      setProcessError(errorMessage(err));
    } finally {
      setTerminating(false);
    }
  };

  const measure = async () => {
    if (!location || measuring) return;
    const id = app.id;
    setMeasuring(true);
    setMeasureError(null);
    try {
      const result = await api.measurePath(location);
      if (activeId.current === id) setMeasurement({ appId: id, value: result });
    } catch (error) {
      if (activeId.current === id) setMeasureError(errorMessage(error));
    } finally {
      if (activeId.current === id) setMeasuring(false);
    }
  };

  const reveal = async () => {
    if (!location || revealing) return;
    setRevealing(true);
    setRevealError(null);
    try {
      await api.revealPath(location);
    } catch (error) {
      if (activeId.current === app.id) setRevealError(errorMessage(error));
    } finally {
      if (activeId.current === app.id) setRevealing(false);
    }
  };

  const launchUninstall = async () => {
    if (!uninstallPlan || !uninstallConfirm || uninstalling) return;
    setUninstalling(true);
    setUninstallError(null);
    try {
      const res = await api.executeUninstall(app.id, uninstallPlan.planId);
      if (activeId.current === app.id) {
        setUninstallResult(res);
        if (res.status === "completed" && onUninstallSuccess) {
          onUninstallSuccess();
        }
      }
    } catch (err) {
      if (activeId.current === app.id) setUninstallError(errorMessage(err));
    } finally {
      if (activeId.current === app.id) setUninstalling(false);
    }
  };

  const startLeftoverScan = async () => {
    setScanningLeftovers(true);
    setLeftoverError(null);
    try {
      const report = await api.scanAppLeftovers(app.id);
      if (activeId.current === app.id) {
        setLeftoverReport(report);
        const defaults = new Set(
          report.candidates.filter((c) => c.selected && c.risk !== "protected").map((c) => c.id),
        );
        setSelectedCandidates(defaults);
      }
    } catch (err) {
      if (activeId.current === app.id) setLeftoverError(errorMessage(err));
    } finally {
      if (activeId.current === app.id) setScanningLeftovers(false);
    }
  };

  const toggleCandidate = (id: string) => {
    setSelectedCandidates((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const cleanSelectedLeftovers = async () => {
    if (!leftoverReport || selectedCandidates.size === 0 || !cleanConfirm || cleaning) return;
    setCleaning(true);
    setLeftoverError(null);
    try {
      const report = await api.cleanAppLeftovers(
        app.id,
        leftoverReport.planId,
        Array.from(selectedCandidates),
      );
      if (activeId.current === app.id) {
        setCleanReport(report);
        await startLeftoverScan();
      }
    } catch (err) {
      if (activeId.current === app.id) setLeftoverError(errorMessage(err));
    } finally {
      if (activeId.current === app.id) setCleaning(false);
    }
  };

  return (
    <Drawer
      open
      title={app.name}
      subtitle={subtitle}
      onClose={onClose}
      footer={
        <>
          <Button
            icon={<IconFolder size={15} />}
            loading={revealing}
            disabled={!location}
            onClick={reveal}
            title={location ? `Open ${location}` : "No install location recorded"}
          >
            Open install location
          </Button>
          <Button variant="ghost" onClick={onClose}>
            Close
          </Button>
        </>
      }
    >
      <div className="un-drawer-badges">
        <Badge tone={app.source === "store" ? "accent" : app.source === "steam" ? "accent" : app.source === "portable" ? "caution" : "neutral"}>
          {app.source === "store" ? "Microsoft Store (MSIX)" : app.source === "steam" ? "Steam Library" : app.source === "portable" ? "Portable App" : (app.scope === "machine" ? "Machine-wide" : "Current user")}
        </Badge>
        <Badge tone="neutral">{app.architecture === "x64" ? "64-bit" : app.architecture === "x86" ? "32-bit" : "Neutral"}</Badge>
        {app.uninstallSupport === "missing" && <Badge tone="danger">Broken Uninstaller</Badge>}
        {app.isProtected && <Badge tone="caution">System component</Badge>}
        {processes.length > 0 && <Badge tone="accent">{processes.length} running</Badge>}
      </div>

      <div className="un-drawer-nav">
        <Segmented
          ariaLabel="Application detail view"
          value={tab}
          onChange={(val) => setTab(val as DrawerTab)}
          options={[
            { value: "overview", label: "Overview" },
            { value: "processes", label: "Processes", count: processes.length || undefined },
            { value: "uninstall", label: "Uninstall" },
            { value: "leftovers", label: "Leftovers" },
          ]}
        />
      </div>

      {app.isProtected && (
        <div className="un-protected">
          <IconShield size={16} />
          <div>
            <strong>Windows system component.</strong> Cyron marks this entry protected to safeguard
            system integrity; standard uninstallation and residual modification are disabled.
          </div>
        </div>
      )}

      {/* Tab 1: Overview */}
      {tab === "overview" && (
        <>
          <div style={{ marginBottom: 14, display: "flex", gap: 8 }}>
            {app.source === "store" ? (
              <Button size="sm" onClick={() => setTab("uninstall")} style={{ width: "100%", justifyContent: "center" }}>
                Store removal guidance
              </Button>
            ) : app.uninstallSupport === "missing" ? (
              <Button
                variant="danger"
                icon={<IconTrash size={14} />}
                disabled={app.isProtected}
                onClick={() => {
                  setWizardInitialMode("forced");
                  setWizardOpen(true);
                }}
                style={{ width: "100%", justifyContent: "center" }}
              >
                Clean Broken Entry (Forced Removal)…
              </Button>
            ) : (
              <>
                <Button
                  variant="danger"
                  icon={<IconTrash size={14} />}
                  disabled={app.isProtected}
                  onClick={() => {
                    setWizardInitialMode("standard");
                    setWizardOpen(true);
                  }}
                  style={{ flex: 1, justifyContent: "center" }}
                >
                  Safe Uninstall…
                </Button>
                <Button
                  variant="secondary"
                  icon={<IconTrash size={14} />}
                  disabled={app.isProtected}
                  onClick={() => {
                    setWizardInitialMode("forced");
                    setWizardOpen(true);
                  }}
                  style={{ flex: 1, justifyContent: "center" }}
                >
                  Forced Removal…
                </Button>
              </>
            )}
          </div>

          <KeyValue
            items={[
              { label: "Publisher", value: app.publisher ?? "—" },
              { label: "Version", value: app.version ?? "—" },
              { label: "Installed", value: formatDate(app.installDate) },
              { label: "Registry view", value: HIVE_LABEL[app.hive] ?? app.hive },
              {
                label: "Application source",
                value:
                  app.source === "store"
                    ? "Microsoft Store / MSIX"
                    : app.source === "steam"
                    ? "Steam Game Library"
                    : app.source === "portable"
                    ? "Portable Heuristic"
                    : "Win32 Registry",
              },
              {
                label: "Uninstall support",
                value:
                  app.uninstallSupport === "standard"
                    ? "Standard uninstaller"
                    : app.uninstallSupport === "silent_only"
                    ? "Silent uninstaller only"
                    : app.uninstallSupport === "store_managed"
                    ? "Store-managed lifecycle"
                    : app.uninstallSupport === "manual_only"
                    ? "Manual removal only"
                    : "Missing uninstaller executable",
              },
              {
                label: "Detection details",
                value: app.detectionReason ?? "Windows Registry entry",
              },
              {
                label: "Install location",
                value: location ? <span className="mono un-code">{location}</span> : "Not reported",
              },
              {
                label: "Estimated size",
                value:
                  app.estimatedSize === null
                    ? "Not reported"
                    : formatBytes(app.estimatedSize),
              },
            ]}
          />

          <section className="un-section" aria-label="Measured install folder size">
            <div className="un-section__head">
              <Eyebrow>Measured on disk</Eyebrow>
              {measured && (
                <span className="un-elapsed">walked in {formatDuration(measured.elapsedMs)}</span>
              )}
            </div>

            {measured ? (
              <>
                <div className="un-measure">
                  <div>
                    <span className="un-measure__label">Logical size</span>
                    <span className="un-measure__value">{formatBytes(measured.size)}</span>
                  </div>
                  <div>
                    <span className="un-measure__label">Allocated</span>
                    <span className="un-measure__value">{formatBytes(measured.allocated)}</span>
                  </div>
                  <div>
                    <span className="un-measure__label">Files</span>
                    <span className="un-measure__value">{formatCount(measured.fileCount)}</span>
                  </div>
                  <div>
                    <span className="un-measure__label">Folders</span>
                    <span className="un-measure__value">{formatCount(measured.dirCount)}</span>
                  </div>
                </div>
                {measured.errors.length > 0 && (
                  <p className="un-hint">
                    {measured.errors.length} location{measured.errors.length === 1 ? "" : "s"} could
                    not be read and may be missing from this total.
                  </p>
                )}
              </>
            ) : (
              <p className="un-hint">
                {measureError
                  ? "The folder could not be measured. Check that the recorded location still exists and is accessible."
                  : "Walk the install folder to see the real logical and allocated size instead of the installer's estimate."}
              </p>
            )}

            {measureError && <p className="un-inline-error">{measureError}</p>}

            <div className="row">
              <Button
                size="sm"
                loading={measuring}
                disabled={!location}
                onClick={measure}
                title={location ? "Measure the install folder" : "No install location recorded"}
              >
                {measured ? "Measure again" : "Measure folder"}
              </Button>
            </div>
          </section>
        </>
      )}

      {/* Tab 2: Processes */}
      {tab === "processes" && (
        <section className="un-section" aria-label="Process manager">
          <div className="un-section__head">
            <Eyebrow>Associated processes</Eyebrow>
            <Button
              size="sm"
              variant="ghost"
              icon={<IconRefresh size={13} />}
              loading={loadingProcesses}
              onClick={() => loadProcesses()}
            >
              Scan
            </Button>
          </div>

          <p className="un-hint">
            Active processes executing from this application&apos;s installation directory. Critical Windows
            processes are protected and can never be terminated.
          </p>

          {processError && <p className="un-inline-error">{processError}</p>}

          {processes.length === 0 ? (
            <Banner tone="info">No running processes found for this application.</Banner>
          ) : (
            <div className="un-proc-list">
              {processes.map((proc) => (
                <div key={proc.pid} className="un-proc-item">
                  <div className="un-proc-info">
                    <strong>
                      {proc.name} <span className="mono">PID {proc.pid}</span>
                    </strong>
                    {proc.title && <span className="un-proc-title">{proc.title}</span>}
                    {proc.path && <span className="mono un-code">{proc.path}</span>}
                  </div>
                  <div>
                    {proc.isCritical ? (
                      <Badge tone="caution">System Critical</Badge>
                    ) : (
                      <Badge tone="accent">Running</Badge>
                    )}
                  </div>
                </div>
              ))}
            </div>
          )}

          {processes.length > 0 && (
            <div className="stack" style={{ gap: 8, marginTop: 8 }}>
              <label className="un-confirm-box">
                <input
                  type="checkbox"
                  checked={terminateForce}
                  onChange={(e) => setTerminateForce(e.target.checked)}
                />
                <span>Force terminate if graceful window closure is unhandled</span>
              </label>

              <Button
                variant="danger"
                size="sm"
                icon={<IconStop size={14} />}
                loading={terminating}
                onClick={terminateAllProcesses}
              >
                Terminate {processes.length} process{processes.length === 1 ? "" : "es"}
              </Button>
            </div>
          )}
        </section>
      )}

      {/* Tab 3: Uninstall */}
      {tab === "uninstall" && (
        <section className="un-section" aria-label="Standard uninstaller">
          <Eyebrow>Vendor uninstaller</Eyebrow>

          {app.isProtected ? (
            <Banner tone="caution">
              This application is registered as a protected Windows component. Cyron refuses to launch
              uninstallation for system software.
            </Banner>
          ) : app.source === "store" ? (
            <Banner tone="info">
              <strong>Microsoft Store / MSIX application.</strong>
              <p style={{ marginTop: 6 }}>
                This package is managed by Windows. Remove it in Settings → Apps → Installed apps,
                or use <code>winget uninstall &quot;{app.name}&quot;</code>. Cyron can scan for
                residual data, but cannot uninstall Store apps directly.
              </p>
              <div style={{ marginTop: 10 }}>
                <Button size="sm" onClick={() => setTab("leftovers")}>
                  Scan for leftover data
                </Button>
              </div>
            </Banner>
          ) : app.source === "portable" ? (
            <Banner tone="caution">
              <strong>Portable application.</strong>
              <p style={{ marginTop: 6 }}>
                This executable was identified via heuristic scanning in your user program directory.
                Portable applications do not have a registered Windows uninstaller.
              </p>
              <div style={{ marginTop: 10 }}>
                <Button size="sm" onClick={() => setTab("leftovers")}>
                  Scan for residual files
                </Button>
              </div>
            </Banner>
          ) : app.uninstallSupport === "missing" ? (
            <Banner tone="danger">
              <strong>Broken uninstaller record.</strong>
              <p style={{ marginTop: 6 }}>
                The uninstaller executable registered in the Windows registry was not found on disk.
                Use Forced Removal to cleanly purge this entry and any residual files.
              </p>
              <div style={{ marginTop: 10, display: "flex", gap: 8 }}>
                <Button
                  size="sm"
                  variant="danger"
                  onClick={() => {
                    setWizardInitialMode("forced");
                    setWizardOpen(true);
                  }}
                >
                  Forced Removal (Purge Entry)
                </Button>
                <Button size="sm" onClick={() => setTab("leftovers")}>
                  Scan for leftovers
                </Button>
              </div>
            </Banner>
          ) : uninstallPlan ? (
            <>
              <p className="un-hint">
                Validated command line registered by the software vendor. Shell interpreters and
                arbitrary scripts are rejected.
              </p>

              <div className="stack" style={{ gap: 6 }}>
                <div>
                  <span className="un-command__label">Executable</span>
                  <pre className="un-command" tabIndex={0}>
                    {uninstallPlan.executable}
                  </pre>
                </div>
                {uninstallPlan.arguments.length > 0 && (
                  <div>
                    <span className="un-command__label">Arguments</span>
                    <pre className="un-command" tabIndex={0}>
                      {uninstallPlan.arguments.join(" ")}
                    </pre>
                  </div>
                )}
                {uninstallPlan.isMsi && (
                  <Badge tone="accent">Windows Installer (MSI) package</Badge>
                )}
              </div>

              {uninstallResult && (
                <Banner
                  tone={
                    uninstallResult.status === "completed"
                      ? "safe"
                      : uninstallResult.restartRequired
                      ? "caution"
                      : "danger"
                  }
                >
                  <strong>{uninstallResult.message}</strong>
                  {uninstallResult.exitCode !== null && (
                    <span> (Exit code: {uninstallResult.exitCode})</span>
                  )}
                  {uninstallResult.status === "completed" && (
                    <div style={{ marginTop: 8 }}>
                      <Button size="sm" onClick={() => setTab("leftovers")}>
                        Scan for leftovers now
                      </Button>
                    </div>
                  )}
                </Banner>
              )}

              {uninstallError && <p className="un-inline-error">{uninstallError}</p>}

              <label className="un-confirm-box">
                <input
                  type="checkbox"
                  checked={uninstallConfirm}
                  onChange={(e) => setUninstallConfirm(e.target.checked)}
                />
                <span>
                  I understand that launching this uninstaller runs the software vendor&apos;s removal
                  process.
                </span>
              </label>

              <div className="row">
                <Button
                  variant="primary"
                  icon={<IconPlay size={14} />}
                  loading={uninstalling}
                  disabled={!uninstallConfirm || uninstalling}
                  onClick={launchUninstall}
                >
                  {uninstalling ? "Running uninstaller…" : "Launch uninstaller"}
                </Button>
              </div>
            </>
          ) : (
            <Banner tone="caution">
              {uninstallError ??
                parsedCmd?.rejectionReason ??
                app.uninstallString ??
                "No registered uninstall command found for this application."}
            </Banner>
          )}
        </section>
      )}

      {/* Tab 4: Leftovers */}
      {tab === "leftovers" && (
        <section className="un-section" aria-label="Leftover scanner">
          <div className="un-section__head">
            <Eyebrow>Leftover scanner</Eyebrow>
            <Button
              size="sm"
              icon={<IconRefresh size={13} />}
              loading={scanningLeftovers}
              disabled={app.isProtected}
              onClick={startLeftoverScan}
            >
              {leftoverReport ? "Scan again" : "Scan for leftovers"}
            </Button>
          </div>

          <p className="un-hint">
            Discovers remaining folders and registry keys created by this program. Selected items are
            safely quarantined with an automated .reg backup before any registry deletion.
          </p>

          {leftoverError && <p className="un-inline-error">{leftoverError}</p>}

          {cleanReport && (
            <Banner tone="safe">
              Quarantined {formatCount(cleanReport.quarantinedFiles)} files (
              {formatBytes(cleanReport.quarantinedBytes)}) and removed{" "}
              {formatCount(cleanReport.deletedRegistryKeys)} registry keys.
              {cleanReport.registryBackupPath && (
                <div style={{ marginTop: 4 }} className="mono un-code">
                  Registry backup saved: {cleanReport.registryBackupPath}
                </div>
              )}
            </Banner>
          )}

          {leftoverReport && (
            <>
              {leftoverReport.candidates.length === 0 ? (
                <Banner tone="safe">No leftover files or registry keys detected.</Banner>
              ) : (
                <>
                  <div className="un-candidates-list">
                    {leftoverReport.candidates.map((cand) => (
                      <label key={cand.id} className="un-candidate-row">
                        <input
                          type="checkbox"
                          disabled={cand.risk === "protected"}
                          checked={selectedCandidates.has(cand.id)}
                          onChange={() => toggleCandidate(cand.id)}
                        />
                        <div className="un-candidate-body">
                          <div className="un-candidate-head">
                            <Badge tone={cand.kind === "directory" ? "neutral" : "accent"}>
                              {cand.kind === "directory" ? "Folder" : "Registry"}
                            </Badge>
                            <Badge tone={cand.confidence === "high" ? "safe" : "neutral"}>
                              {cand.confidence.toUpperCase()} confidence
                            </Badge>
                            {cand.size > 0 && <span className="mono">{formatBytes(cand.size)}</span>}
                          </div>
                          <span className="un-candidate-path">{cand.path}</span>
                          <span className="un-candidate-reason">{cand.reason}</span>
                        </div>
                      </label>
                    ))}
                  </div>

                  <div className="un-measure" style={{ marginTop: 6 }}>
                    <div>
                      <span className="un-measure__label">Selected candidates</span>
                      <span className="un-measure__value">
                        {selectedCandidates.size} of {leftoverReport.candidates.length}
                      </span>
                    </div>
                    <div>
                      <span className="un-measure__label">Registry backup</span>
                      <span className="un-measure__value">Automated .reg</span>
                    </div>
                  </div>

                  <label className="un-confirm-box">
                    <input
                      type="checkbox"
                      checked={cleanConfirm}
                      onChange={(e) => setCleanConfirm(e.target.checked)}
                    />
                    <span>
                      I confirm moving selected residual files to quarantine and backing up registry keys.
                    </span>
                  </label>

                  <div className="row">
                    <Button
                      variant="danger"
                      icon={<IconTrash size={14} />}
                      loading={cleaning}
                      disabled={selectedCandidates.size === 0 || !cleanConfirm || cleaning}
                      onClick={cleanSelectedLeftovers}
                    >
                      {cleaning ? "Cleaning leftovers…" : `Clean ${selectedCandidates.size} leftovers`}
                    </Button>
                  </div>
                </>
              )}
            </>
          )}
        </section>
      )}

      {revealError && <p className="un-inline-error">{revealError}</p>}

      <UninstallWizard
        key={app.id}
        app={app}
        isOpen={wizardOpen}
        initialMode={wizardInitialMode}
        onClose={() => {
          setWizardOpen(false);
          setWizardInitialMode(undefined);
        }}
        onComplete={() => {
          setWizardOpen(false);
          setWizardInitialMode(undefined);
          onClose();
          onUninstallSuccess?.();
        }}
      />
    </Drawer>
  );
}
