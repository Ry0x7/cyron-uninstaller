import { useEffect, useRef, useState } from "react";
import { api, errorMessage, invalidateInstalledApps } from "../../lib/api";
import { formatBytes, formatCount } from "../../lib/format";
import type {
  AppProcess,
  BrokenEntryPlan,
  InstalledApp,
  LeftoverCleanReport,
  LeftoverScanReport,
  UninstallExecutionResult,
} from "../../lib/types";
import { Badge, Banner, Button, Drawer, KeyValue, Spinner, type BadgeTone } from "../../components/ui";
import {
  IconCheck,
  IconFile,
  IconFolder,
  IconRefresh,
  IconShield,
  IconStop,
  IconTrash,
} from "../../components/Icons";
import "./uninstallWizard.css";

export type WizardStep = "mode" | "processes" | "execute" | "leftovers" | "complete";
export type UninstallMode = "standard" | "forced" | "clean_entry";

interface UninstallWizardProps {
  app: InstalledApp | null;
  isOpen: boolean;
  onClose: () => void;
  onComplete?: () => void;
}

export function UninstallWizard({
  app,
  isOpen,
  onClose,
  onComplete,
}: UninstallWizardProps) {
  const [step, setStep] = useState<WizardStep>("mode");
  const [mode, setMode] = useState<UninstallMode>("standard");
  const [isQuiet, setIsQuiet] = useState(false);

  // Process manager state
  const [processes, setProcesses] = useState<AppProcess[]>([]);
  const [loadingProcesses, setLoadingProcesses] = useState(false);
  const [processError, setProcessError] = useState<string | null>(null);
  const [terminating, setTerminating] = useState(false);
  const [terminateForce, setTerminateForce] = useState(false);

  // Execution state
  const [uninstalling, setUninstalling] = useState(false);
  const [uninstallResult, setUninstallResult] = useState<UninstallExecutionResult | null>(null);
  const [uninstallError, setUninstallError] = useState<string | null>(null);
  const [elapsedSecs, setElapsedSecs] = useState(0);

  // Leftovers state
  const [leftoverReport, setLeftoverReport] = useState<LeftoverScanReport | null>(null);
  const [scanningLeftovers, setScanningLeftovers] = useState(false);
  const [leftoverError, setLeftoverError] = useState<string | null>(null);
  const [selectedCandidateIds, setSelectedCandidateIds] = useState<Set<string>>(new Set());
  const [cleaning, setCleaning] = useState(false);
  const [cleanReport, setCleanReport] = useState<LeftoverCleanReport | null>(null);
  const [cleanError, setCleanError] = useState<string | null>(null);
  const [brokenPlan, setBrokenPlan] = useState<BrokenEntryPlan | null>(null);
  const [preparingBrokenPlan, setPreparingBrokenPlan] = useState(false);
  const [restoringRegistry, setRestoringRegistry] = useState(false);
  const [registryRestoreResult, setRegistryRestoreResult] = useState<string | null>(null);
  const [registryConflict, setRegistryConflict] = useState(false);

  const activeAppId = useRef<string | null>(null);
  activeAppId.current = app?.id ?? null;

  // Initialize state when app changes or opens
  useEffect(() => {
    if (!isOpen || !app) return;

    setStep("mode");
    setProcessError(null);
    setUninstallError(null);
    setLeftoverError(null);
    setCleanError(null);
    setUninstallResult(null);
    setLeftoverReport(null);
    setCleanReport(null);
    setRegistryRestoreResult(null);
    setSelectedCandidateIds(new Set());
    setElapsedSecs(0);

    // Initial default mode
    if (app.uninstallSupport === "missing") {
      setMode("forced");
    } else {
      setMode("standard");
    }

    // Cache backend identity snapshot immediately
    api.snapshotAppIdentity(app.id).catch(() => {});

    // Discover processes immediately so count is available
    loadProcesses(app.id);
  }, [isOpen, app?.id]);

  // Elapsed timer during uninstallation
  useEffect(() => {
    if (!uninstalling) return;
    const interval = setInterval(() => {
      setElapsedSecs((s) => s + 1);
    }, 1000);
    return () => clearInterval(interval);
  }, [uninstalling]);

  if (!isOpen || !app) return null;

  const loadProcesses = async (appId?: string) => {
    const id = appId ?? app.id;
    setLoadingProcesses(true);
    setProcessError(null);
    try {
      const procs = await api.discoverAppProcesses(id);
      if (activeAppId.current === id) {
        setProcesses(procs);
      }
    } catch (err) {
      if (activeAppId.current === id) {
        setProcessError(errorMessage(err));
      }
    } finally {
      if (activeAppId.current === id) {
        setLoadingProcesses(false);
      }
    }
  };

  const handleTerminateProcesses = async (force: boolean) => {
    if (processes.length === 0 || terminating) return;
    setTerminating(true);
    setProcessError(null);
    try {
      const pids = processes.map((p) => p.pid);
      const results = await api.terminateAppProcesses(app.id, pids, force);
      const failed = results.filter((r) => !r.success);
      if (failed.length > 0) {
        setProcessError(failed.map((f) => `${f.name}: ${f.error}`).join(" · "));
      }
      await loadProcesses(app.id);
    } catch (err) {
      setProcessError(errorMessage(err));
    } finally {
      setTerminating(false);
    }
  };

  const handleExecuteStandard = async () => {
    setUninstalling(true);
    setUninstallError(null);
    setElapsedSecs(0);
    try {
      const plan = await api.prepareUninstall(app.id, isQuiet);
      const result = await api.executeUninstall(app.id, plan.planId);
      setUninstallResult(result);
      // Invalidate inventory cache
      invalidateInstalledApps();
    } catch (err) {
      setUninstallError(errorMessage(err));
    } finally {
      setUninstalling(false);
    }
  };

  const handleScanLeftovers = async () => {
    setScanningLeftovers(true);
    setLeftoverError(null);
    try {
      const report = await api.scanAppLeftovers(app.id);
      if (activeAppId.current === app.id) {
        setLeftoverReport(report);
        // Pre-select all safe candidates
        const safeIds = new Set(
          report.candidates.filter((c) => c.risk === "safe" && c.selected).map((c) => c.id),
        );
        setSelectedCandidateIds(safeIds);
        setStep("leftovers");
      }
    } catch (err) {
      if (activeAppId.current === app.id) {
        setLeftoverError(errorMessage(err));
      }
    } finally {
      if (activeAppId.current === app.id) {
        setScanningLeftovers(false);
      }
    }
  };

  const handleCleanLeftovers = async () => {
    if (!leftoverReport || selectedCandidateIds.size === 0 || cleaning) return;
    setCleaning(true);
    setCleanError(null);
    try {
      const report = await api.cleanAppLeftovers(
        app.id,
        leftoverReport.planId,
        Array.from(selectedCandidateIds),
      );
      setCleanReport(report);
      invalidateInstalledApps();
      setStep("complete");
      onComplete?.();
    } catch (err) {
      setCleanError(errorMessage(err));
    } finally {
      setCleaning(false);
    }
  };

  const handlePrepareCleanEntry = async () => {
    setPreparingBrokenPlan(true);
    setCleanError(null);
    try {
      const plan = await api.prepareBrokenInventoryCleanup(app.id);
      setBrokenPlan(plan);
    } catch (err) {
      setCleanError(errorMessage(err));
    } finally {
      setPreparingBrokenPlan(false);
    }
  };

  const handleExecuteCleanEntry = async () => {
    if (!brokenPlan) return;
    setCleaning(true);
    setCleanError(null);
    try {
      const report = await api.executeBrokenInventoryCleanup(
        brokenPlan.planId,
        brokenPlan.registryKeyPath,
      );
      setCleanReport(report);
      invalidateInstalledApps();
      setStep("complete");
      onComplete?.();
    } catch (err) {
      setCleanError(errorMessage(err));
    } finally {
      setCleaning(false);
    }
  };

  const toggleCandidate = (id: string) => {
    setSelectedCandidateIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const selectAllCandidates = () => {
    if (!leftoverReport) return;
    setSelectedCandidateIds(new Set(leftoverReport.candidates.map((c) => c.id)));
  };

  const selectSafeCandidatesOnly = () => {
    if (!leftoverReport) return;
    setSelectedCandidateIds(
      new Set(leftoverReport.candidates.filter((c) => c.risk === "safe").map((c) => c.id)),
    );
  };

  const clearSelection = () => {
    setSelectedCandidateIds(new Set());
  };

  // Group candidates for structured display
  const groupedCandidates = {
    dirs: (leftoverReport?.candidates ?? []).filter((c) => c.kind === "directory"),
    files: (leftoverReport?.candidates ?? []).filter((c) => c.kind === "file"),
    registry: (leftoverReport?.candidates ?? []).filter((c) => c.kind === "registry_key"),
  };

  const getRiskTone = (risk: string): BadgeTone => {
    if (risk === "safe") return "safe";
    if (risk === "caution") return "caution";
    if (risk === "protected") return "danger";
    return "neutral";
  };

  const handleRestoreRegistry = async (overwrite = false) => {
    if (!cleanReport?.runId || restoringRegistry) return;
    setRestoringRegistry(true);
    setRegistryRestoreResult(null);
    try {
      const rep = await api.restoreRegistryBackup(cleanReport.runId, overwrite);
      setRegistryRestoreResult(
        `Successfully restored ${rep.keysRestored} registry key(s) and ${rep.valuesRestored} value(s) from backup.`
      );
      setRegistryConflict(false);
    } catch (err) {
      const msg = errorMessage(err);
      setRegistryRestoreResult(`Registry restoration failed: ${msg}`);
      if (msg.toLowerCase().includes("conflict") || msg.toLowerCase().includes("already exists")) {
        setRegistryConflict(true);
      }
    } finally {
      setRestoringRegistry(false);
    }
  };

  const selectedCandidatesList = (leftoverReport?.candidates ?? []).filter((c) =>
    selectedCandidateIds.has(c.id),
  );
  const selectedBytes = selectedCandidatesList.reduce((sum, c) => sum + c.size, 0);

  return (
    <Drawer
      open={isOpen}
      title={`Uninstall ${app.name}`}
      subtitle={
        [app.version ? `v${app.version}` : null, app.publisher].filter(Boolean).join(" · ") ||
        "Guided uninstallation wizard"
      }
      onClose={onClose}
    >
      {/* Wizard Steps Header */}
      <div className="un-wizard-steps" aria-label="Uninstallation steps">
        <div
          className={`un-wizard-step ${
            step === "mode" ? "un-wizard-step--active" : "un-wizard-step--done"
          }`}
        >
          <span className="un-wizard-step-badge">1</span>
          <span>Mode</span>
        </div>
        <div className="un-wizard-step-divider" />
        <div
          className={`un-wizard-step ${
            step === "processes"
              ? "un-wizard-step--active"
              : step === "execute" || step === "leftovers" || step === "complete"
              ? "un-wizard-step--done"
              : ""
          }`}
        >
          <span className="un-wizard-step-badge">2</span>
          <span>Processes</span>
        </div>
        <div className="un-wizard-step-divider" />
        <div
          className={`un-wizard-step ${
            step === "execute"
              ? "un-wizard-step--active"
              : step === "leftovers" || step === "complete"
              ? "un-wizard-step--done"
              : ""
          }`}
        >
          <span className="un-wizard-step-badge">3</span>
          <span>{mode === "forced" ? "Scan" : "Uninstall"}</span>
        </div>
        <div className="un-wizard-step-divider" />
        <div
          className={`un-wizard-step ${
            step === "leftovers"
              ? "un-wizard-step--active"
              : step === "complete"
              ? "un-wizard-step--done"
              : ""
          }`}
        >
          <span className="un-wizard-step-badge">4</span>
          <span>Leftovers</span>
        </div>
        <div className="un-wizard-step-divider" />
        <div className={`un-wizard-step ${step === "complete" ? "un-wizard-step--active" : ""}`}>
          <span className="un-wizard-step-badge">5</span>
          <span>Done</span>
        </div>
      </div>

      {/* STEP 1: Verification & Removal Mode */}
      {step === "mode" && (
        <div className="stack" style={{ gap: 16 }}>
          <KeyValue
            items={[
              { label: "Application Name", value: app.name },
              { label: "Publisher", value: app.publisher ?? "Unknown" },
              { label: "Install Location", value: app.installLocation ?? "Not recorded" },
              {
                label: "Architecture / Scope",
                value: `${app.architecture === "x64" ? "64-bit" : "32-bit"} · ${
                  app.scope === "machine" ? "Machine" : "User"
                }`,
              },
            ]}
          />

          {app.isProtected && (
            <Banner tone="caution">
              <strong>Windows System Component.</strong> Cyron marks this entry protected to
              prevent accidental damage to the operating system. Standard uninstallation is
              disabled.
            </Banner>
          )}

          <div style={{ marginTop: 8 }}>
            <h4 style={{ fontSize: 13, marginBottom: 8, color: "var(--text-base)" }}>
              Choose Removal Mode:
            </h4>

            {/* Standard Mode Card */}
            {app.uninstallSupport !== "missing" && !app.isProtected && (
              <label
                className={`un-mode-card ${mode === "standard" ? "un-mode-card--selected" : ""}`}
                onClick={() => setMode("standard")}
              >
                <input
                  type="radio"
                  name="un-mode"
                  checked={mode === "standard"}
                  onChange={() => setMode("standard")}
                />
                <div className="un-mode-card-content">
                  <div className="un-mode-card-title">
                    <span>Standard Uninstall (Recommended)</span>
                    <Badge tone="accent">Vendor Built-in</Badge>
                  </div>
                  <div className="un-mode-card-desc">
                    Runs the official registered uninstaller, monitors completion, then performs an
                    expanded scan for remaining files, folders, and registry keys.
                  </div>
                  {mode === "standard" && app.quietUninstallString && (
                    <div style={{ marginTop: 8 }}>
                      <label style={{ fontSize: 12, display: "flex", alignItems: "center", gap: 6, cursor: "pointer" }}>
                        <input
                          type="checkbox"
                          checked={isQuiet}
                          onChange={(e) => setIsQuiet(e.target.checked)}
                        />
                        <span>Quiet Mode (Run silent uninstaller without wizard GUI)</span>
                      </label>
                    </div>
                  )}
                </div>
              </label>
            )}

            {/* Forced Removal Card */}
            <label
              className={`un-mode-card ${mode === "forced" ? "un-mode-card--selected" : ""}`}
              onClick={() => setMode("forced")}
            >
              <input
                type="radio"
                name="un-mode"
                checked={mode === "forced"}
                onChange={() => setMode("forced")}
              />
              <div className="un-mode-card-content">
                <div className="un-mode-card-title">
                  <span>Forced Removal</span>
                  {app.uninstallSupport === "missing" && <Badge tone="caution">Recommended</Badge>}
                </div>
                <div className="un-mode-card-desc">
                  Bypasses missing, corrupted, or unresponsive vendor uninstallers. Performs deep
                  evidence-based discovery across folders, shortcuts, and registry keys, moving files
                  to quarantine with automatic verified .reg registry backups.
                </div>
              </div>
            </label>

            {/* Clean Entry Only Card */}
            {app.uninstallSupport === "missing" && (
              <label
                className={`un-mode-card ${mode === "clean_entry" ? "un-mode-card--selected" : ""}`}
                onClick={() => {
                  setMode("clean_entry");
                  setBrokenPlan(null);
                }}
              >
                <input
                  type="radio"
                  name="un-mode"
                  checked={mode === "clean_entry"}
                  onChange={() => {
                    setMode("clean_entry");
                    setBrokenPlan(null);
                  }}
                />
                <div className="un-mode-card-content">
                  <div className="un-mode-card-title">
                    <span>Clean Invalid Inventory Entry Only</span>
                    <Badge tone="neutral">Registry Only</Badge>
                  </div>
                  <div className="un-mode-card-desc">
                    Only removes this broken ghost entry from the Windows Uninstall registry. Does
                    not touch other filesystem folders.
                  </div>
                </div>
              </label>
            )}

            {mode === "clean_entry" && brokenPlan && (
              <div style={{ marginTop: 12, padding: 12, background: "var(--bg-subtle)", borderRadius: 6, border: "1px solid var(--border)" }}>
                <div style={{ fontWeight: 600, fontSize: 13, marginBottom: 4 }}>
                  Verified Registry Entry Confirmation:
                </div>
                <code style={{ fontSize: 11, wordBreak: "break-all", display: "block", marginBottom: 8, color: "var(--accent)" }}>
                  {brokenPlan.registryKeyPath}
                </code>
                <div style={{ fontSize: 11, color: "var(--text-muted)" }}>
                  A full multi-type .reg backup will be created in quarantine before deletion.
                </div>
              </div>
            )}

            {cleanError && (
              <div style={{ marginTop: 8 }}>
                <Banner tone="danger">
                  {cleanError}
                </Banner>
              </div>
            )}
          </div>

          <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 12 }}>
            <Button variant="ghost" onClick={onClose}>
              Cancel
            </Button>
            {mode === "clean_entry" ? (
              brokenPlan ? (
                <Button
                  variant="danger"
                  loading={cleaning}
                  onClick={handleExecuteCleanEntry}
                >
                  Confirm & Clean Entry
                </Button>
              ) : (
                <Button
                  variant="primary"
                  loading={preparingBrokenPlan}
                  onClick={handlePrepareCleanEntry}
                >
                  Review Ghost Entry →
                </Button>
              )
            ) : (
              <Button
                variant="primary"
                disabled={app.isProtected}
                onClick={() => setStep("processes")}
              >
                Continue →
              </Button>
            )}
          </div>
        </div>
      )}

      {/* STEP 2: Running Processes */}
      {step === "processes" && (
        <div className="stack" style={{ gap: 16 }}>
          <div>
            <h4 style={{ fontSize: 13, marginBottom: 4, color: "var(--text-base)" }}>
              Associated Running Processes
            </h4>
            <p style={{ fontSize: 12, color: "var(--text-muted)" }}>
              Running processes must be closed before removal to prevent locked files. Critical
              system processes are unconditionally protected.
            </p>
          </div>

          {loadingProcesses ? (
            <div style={{ padding: 24, textAlign: "center", color: "var(--text-muted)" }}>
              <Spinner size={20} />
              <p style={{ marginTop: 8, fontSize: 12 }}>Scanning for associated processes...</p>
            </div>
          ) : processes.length === 0 ? (
            <Banner tone="safe">
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <IconCheck size={16} />
                <span>No active processes found for this application. Safe to proceed.</span>
              </div>
            </Banner>
          ) : (
            <div className="stack" style={{ gap: 8 }}>
              <Banner tone="caution">
                <strong>{processes.length} active process(es) detected.</strong> Please terminate
                them to prevent uninstallation failure or locked files.
              </Banner>
              <div
                style={{
                  maxHeight: 220,
                  overflowY: "auto",
                  border: "1px solid var(--border-subtle)",
                  borderRadius: 6,
                }}
              >
                {processes.map((proc) => (
                  <div
                    key={proc.pid}
                    style={{
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "space-between",
                      padding: "8px 12px",
                      borderBottom: "1px solid var(--border-subtle)",
                      fontSize: 12,
                    }}
                  >
                    <div>
                      <strong>{proc.name}</strong> (PID {proc.pid})
                      {proc.title && (
                        <span style={{ color: "var(--text-muted)", marginLeft: 6 }}>
                          · {proc.title}
                        </span>
                      )}
                    </div>
                    {proc.isCritical && <Badge tone="danger">Protected System Process</Badge>}
                  </div>
                ))}
              </div>

              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginTop: 8 }}>
                <label style={{ fontSize: 12, display: "flex", alignItems: "center", gap: 6, cursor: "pointer" }}>
                  <input
                    type="checkbox"
                    checked={terminateForce}
                    onChange={(e) => setTerminateForce(e.target.checked)}
                  />
                  <span>Force terminate (TerminateProcess)</span>
                </label>
                <div style={{ display: "flex", gap: 8 }}>
                  <Button
                    variant="ghost"
                    icon={<IconRefresh size={14} />}
                    onClick={() => loadProcesses(app.id)}
                    disabled={terminating}
                  >
                    Refresh
                  </Button>
                  <Button
                    variant="danger"
                    icon={<IconStop size={14} />}
                    loading={terminating}
                    onClick={() => handleTerminateProcesses(terminateForce)}
                  >
                    Close Processes
                  </Button>
                </div>
              </div>
            </div>
          )}

          {processError && <Banner tone="danger">{processError}</Banner>}

          <div style={{ display: "flex", justifyContent: "space-between", marginTop: 12 }}>
            <Button variant="ghost" onClick={() => setStep("mode")}>
              ← Back
            </Button>
            <Button
              variant="primary"
              onClick={() => {
                if (mode === "standard") {
                  setStep("execute");
                  handleExecuteStandard();
                } else {
                  // Forced removal moves directly to leftover scanning
                  handleScanLeftovers();
                }
              }}
            >
              {mode === "standard" ? "Launch Uninstaller →" : "Scan Residuals →"}
            </Button>
          </div>
        </div>
      )}

      {/* STEP 3: Execute Standard Uninstaller */}
      {step === "execute" && (
        <div className="stack" style={{ gap: 16 }}>
          <div>
            <h4 style={{ fontSize: 13, marginBottom: 4, color: "var(--text-base)" }}>
              Running Vendor Uninstaller
            </h4>
            <p style={{ fontSize: 12, color: "var(--text-muted)" }}>
              Monitoring uninstaller process execution. Standard uninstaller exit codes and child
              processes are tracked server-side.
            </p>
          </div>

          {uninstalling ? (
            <div
              style={{
                padding: 32,
                textAlign: "center",
                background: "var(--bg-panel)",
                borderRadius: 8,
                border: "1px solid var(--border-subtle)",
              }}
            >
              <Spinner size={24} />
              <p style={{ marginTop: 12, fontWeight: 600, color: "var(--text-base)" }}>
                Uninstaller is running...
              </p>
              <p style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 4 }}>
                Elapsed time: {elapsedSecs}s
              </p>
              <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 8 }}>
                Please complete any prompts inside the vendor uninstaller window.
              </p>
            </div>
          ) : uninstallResult ? (
            <div className="stack" style={{ gap: 12 }}>
              <Banner tone={uninstallResult.status === "completed" ? "safe" : "caution"}>
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                  <IconCheck size={16} />
                  <div>
                    <strong>{uninstallResult.message}</strong>
                    <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>
                      Exit code: {uninstallResult.exitCode ?? "N/A"} · Duration:{" "}
                      {(uninstallResult.elapsedMs / 1000).toFixed(1)}s
                    </div>
                  </div>
                </div>
              </Banner>
              {uninstallResult.restartRequired && (
                <Banner tone="caution">
                  A Windows restart is required by this uninstaller to finalize removal of locked
                  in-use files.
                </Banner>
              )}
            </div>
          ) : null}

          {uninstallError && (
            <div className="stack" style={{ gap: 8 }}>
              <Banner tone="danger">
                <strong>Uninstallation Failed:</strong> {uninstallError}
              </Banner>
              <p style={{ fontSize: 12, color: "var(--text-muted)" }}>
                The uninstaller may be corrupted or missing. You can switch to Forced Removal to
                clean associated files and registry keys directly.
              </p>
              <Button
                variant="primary"
                onClick={() => {
                  setMode("forced");
                  handleScanLeftovers();
                }}
              >
                Switch to Forced Removal →
              </Button>
            </div>
          )}

          {uninstallResult && (
            <div style={{ display: "flex", justifyContent: "flex-end", marginTop: 12 }}>
              <Button
                variant="primary"
                loading={scanningLeftovers}
                onClick={handleScanLeftovers}
              >
                Scan for Leftovers →
              </Button>
            </div>
          )}
        </div>
      )}

      {/* STEP 4: Residual & Leftover Review */}
      {step === "leftovers" && (
        <div className="stack" style={{ gap: 16 }}>
          <div>
            <h4 style={{ fontSize: 13, marginBottom: 4, color: "var(--text-base)" }}>
              Residual & Leftover Scan
            </h4>
            <p style={{ fontSize: 12, color: "var(--text-muted)" }}>
              Review residual files, folders, shortcuts, and registry entries. Files will be moved to
              quarantine and registry entries backed up to .reg before removal.
            </p>
          </div>

          {scanningLeftovers ? (
            <div style={{ padding: 32, textAlign: "center", color: "var(--text-muted)" }}>
              <Spinner size={24} />
              <p style={{ marginTop: 12, fontSize: 12 }}>
                Scanning filesystem, shortcuts, and registry for leftovers...
              </p>
            </div>
          ) : leftoverReport ? (
            leftoverReport.candidates.length === 0 ? (
              <Banner tone="safe">
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                  <IconCheck size={16} />
                  <span>
                    No residual files or registry keys were found. The application was completely
                    removed!
                  </span>
                </div>
              </Banner>
            ) : (
              <div className="stack" style={{ gap: 12 }}>
                {/* Selection Toolbar */}
                <div
                  style={{
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "space-between",
                    padding: "8px 12px",
                    background: "var(--bg-subtle)",
                    borderRadius: 6,
                    fontSize: 12,
                  }}
                >
                  <span style={{ color: "var(--text-muted)" }}>
                    {selectedCandidateIds.size} of {leftoverReport.candidates.length} items selected ·{" "}
                    <strong>{formatBytes(selectedBytes)}</strong> to quarantine
                  </span>
                  <div style={{ display: "flex", gap: 6 }}>
                    <button
                      type="button"
                      className="btn btn--sm btn--ghost"
                      onClick={selectSafeCandidatesOnly}
                    >
                      Safe Only
                    </button>
                    <button
                      type="button"
                      className="btn btn--sm btn--ghost"
                      onClick={selectAllCandidates}
                    >
                      All
                    </button>
                    <button
                      type="button"
                      className="btn btn--sm btn--ghost"
                      onClick={clearSelection}
                    >
                      None
                    </button>
                  </div>
                </div>

                {/* Candidate Groups */}
                <div style={{ maxHeight: 300, overflowY: "auto" }}>
                  {/* Folders */}
                  {groupedCandidates.dirs.length > 0 && (
                    <div className="un-leftover-group">
                      <div className="un-leftover-group-head">
                        <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
                          <IconFolder size={14} />
                          Installation & Data Folders ({groupedCandidates.dirs.length})
                        </span>
                      </div>
                      {groupedCandidates.dirs.map((cand) => (
                        <label key={cand.id} className="un-leftover-item">
                          <input
                            type="checkbox"
                            checked={selectedCandidateIds.has(cand.id)}
                            onChange={() => toggleCandidate(cand.id)}
                          />
                          <div style={{ flex: 1 }}>
                            <div className="un-leftover-item-path">{cand.path}</div>
                            <div className="un-leftover-item-reason">
                              {cand.reason} · {formatBytes(cand.size)} ({formatCount(cand.fileCount)}{" "}
                              files)
                            </div>
                          </div>
                          <Badge tone={getRiskTone(cand.risk)}>
                            {cand.risk}
                          </Badge>
                        </label>
                      ))}
                    </div>
                  )}

                  {/* Shortcuts */}
                  {groupedCandidates.files.length > 0 && (
                    <div className="un-leftover-group">
                      <div className="un-leftover-group-head">
                        <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
                          <IconFile size={14} />
                          Application Shortcuts ({groupedCandidates.files.length})
                        </span>
                      </div>
                      {groupedCandidates.files.map((cand) => (
                        <label key={cand.id} className="un-leftover-item">
                          <input
                            type="checkbox"
                            checked={selectedCandidateIds.has(cand.id)}
                            onChange={() => toggleCandidate(cand.id)}
                          />
                          <div style={{ flex: 1 }}>
                            <div className="un-leftover-item-path">{cand.path}</div>
                            <div className="un-leftover-item-reason">{cand.reason}</div>
                          </div>
                          <Badge tone="safe">safe</Badge>
                        </label>
                      ))}
                    </div>
                  )}

                  {/* Registry Keys */}
                  {groupedCandidates.registry.length > 0 && (
                    <div className="un-leftover-group">
                      <div className="un-leftover-group-head">
                        <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
                          <IconShield size={14} />
                          Registry Keys ({groupedCandidates.registry.length})
                        </span>
                      </div>
                      {groupedCandidates.registry.map((cand) => (
                        <label key={cand.id} className="un-leftover-item">
                          <input
                            type="checkbox"
                            checked={selectedCandidateIds.has(cand.id)}
                            onChange={() => toggleCandidate(cand.id)}
                          />
                          <div style={{ flex: 1 }}>
                            <div className="un-leftover-item-path">{cand.path}</div>
                            <div className="un-leftover-item-reason">{cand.reason}</div>
                          </div>
                          <Badge tone={getRiskTone(cand.risk)}>
                            {cand.risk}
                          </Badge>
                        </label>
                      ))}
                    </div>
                  )}
                </div>
              </div>
            )
          ) : null}

          {cleanError && <Banner tone="danger">{cleanError}</Banner>}
          {leftoverError && <Banner tone="danger">{leftoverError}</Banner>}

          <div style={{ display: "flex", justifyContent: "space-between", marginTop: 12 }}>
            <Button variant="ghost" onClick={onClose}>
              Skip Leftovers & Close
            </Button>
            <Button
              variant="danger"
              icon={<IconTrash size={14} />}
              loading={cleaning}
              disabled={selectedCandidateIds.size === 0}
              onClick={handleCleanLeftovers}
            >
              Clean Selected ({selectedCandidateIds.size})
            </Button>
          </div>
        </div>
      )}

      {/* STEP 5: Complete */}
      {step === "complete" && (
        <div className="stack" style={{ gap: 16 }}>
          <Banner tone="safe">
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <IconCheck size={18} />
              <div>
                <strong style={{ fontSize: 14 }}>Uninstallation & Cleanup Completed</strong>
                <p style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 2 }}>
                  {app.name} has been removed and verified.
                </p>
              </div>
            </div>
          </Banner>

          {cleanReport && (
            <KeyValue
              items={[
                { label: "Quarantined Files", value: formatCount(cleanReport.quarantinedFiles) },
                { label: "Quarantined Space", value: formatBytes(cleanReport.quarantinedBytes) },
                {
                  label: "Deleted Registry Keys",
                  value: formatCount(cleanReport.deletedRegistryKeys),
                },
                ...(cleanReport.registryBackupPath
                  ? [{ label: "Registry Backup File", value: cleanReport.registryBackupPath }]
                  : []),
                ...(cleanReport.quarantineDir
                  ? [{ label: "Quarantine Folder", value: cleanReport.quarantineDir }]
                  : []),
              ]}
            />
          )}

          {registryRestoreResult && (
            <Banner tone={registryRestoreResult.includes("failed") ? "danger" : "safe"}>
              {registryRestoreResult}
            </Banner>
          )}

          <div
            style={{
              padding: 12,
              background: "var(--bg-subtle)",
              borderRadius: 6,
              fontSize: 12,
              color: "var(--text-muted)",
              display: "flex",
              alignItems: "center",
              gap: 8,
            }}
          >
            <IconShield size={16} />
            <span>
              All quarantined items and registry backups are safely preserved for 7 days in your
              Activity log and can be restored at any time.
            </span>
          </div>

          <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 12 }}>
            {cleanReport?.registryBackupPath && !registryRestoreResult?.includes("Successfully") && (
              registryConflict ? (
                <Button
                  variant="danger"
                  onClick={() => handleRestoreRegistry(true)}
                  loading={restoringRegistry}
                >
                  Overwrite & Restore
                </Button>
              ) : (
                <Button
                  variant="secondary"
                  onClick={() => handleRestoreRegistry(false)}
                  loading={restoringRegistry}
                >
                  Restore Registry Now
                </Button>
              )
            )}
            <Button variant="primary" onClick={onClose}>
              Done
            </Button>
          </div>
        </div>
      )}
    </Drawer>
  );
}
