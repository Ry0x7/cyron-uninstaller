import { useEffect, useState } from "react";
import { api, errorMessage } from "../../lib/api";
import { formatBytes, formatCount } from "../../lib/format";
import type { ForceDeletePlanPreview, ForceDeleteResult } from "../../lib/types";
import { Badge, Banner, Button, Modal, Spinner } from "../../components/ui";
import { IconRefresh, IconTrash } from "../../components/Icons";
import "./explorerModals.css";

interface ForceDeleteModalProps {
  open: boolean;
  targetPath: string | null;
  onClose: () => void;
  onSuccess?: (result: ForceDeleteResult) => void;
}

export function ForceDeleteModal({
  open,
  targetPath,
  onClose,
  onSuccess,
}: ForceDeleteModalProps) {
  const [loading, setLoading] = useState(false);
  const [plan, setPlan] = useState<ForceDeletePlanPreview | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [mode, setMode] = useState<"quarantine" | "permanent">("quarantine");
  const [confirmPermanent, setConfirmPermanent] = useState(false);

  const [terminatingPid, setTerminatingPid] = useState<number | null>(null);
  const [processMsg, setProcessMsg] = useState<string | null>(null);

  const [executing, setExecuting] = useState(false);
  const [result, setResult] = useState<ForceDeleteResult | null>(null);
  const [executionError, setExecutionError] = useState<string | null>(null);

  const loadPlan = async (path: string) => {
    setLoading(true);
    setError(null);
    setPlan(null);
    setResult(null);
    setExecutionError(null);
    setConfirmPermanent(false);
    try {
      const p = await api.prepareForceDeletePlan(path);
      setPlan(p);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (open && targetPath) {
      loadPlan(targetPath);
    } else {
      setPlan(null);
      setError(null);
      setResult(null);
      setExecutionError(null);
    }
  }, [open, targetPath]);

  const handleTerminateLock = async (pid: number, force: boolean) => {
    if (!plan) return;
    setTerminatingPid(pid);
    setProcessMsg(null);
    try {
      const res = await api.terminateForceDeleteLockingProcess(plan.planId, pid, force);
      if (res.success) {
        setProcessMsg(`Process ${pid} (${res.name}) closed successfully.`);
        // Reload plan to refresh locking process list and inspection
        if (targetPath) {
          await loadPlan(targetPath);
        }
      } else {
        setProcessMsg(`Could not terminate process ${pid}: ${res.error ?? "Access denied or process already exited"}`);
      }
    } catch (err) {
      setProcessMsg(`Error: ${errorMessage(err)}`);
    } finally {
      setTerminatingPid(null);
    }
  };

  const handleExecute = async () => {
    if (!plan || executing) return;
    if (mode === "permanent" && !confirmPermanent) return;

    setExecuting(true);
    setExecutionError(null);
    try {
      const res = await api.executeForceDeletePlan(plan.planId, mode);
      setResult(res);
      onSuccess?.(res);
    } catch (err) {
      setExecutionError(errorMessage(err));
    } finally {
      setExecuting(false);
    }
  };

  const inspection = plan?.inspection;

  return (
    <Modal
      open={open}
      title={
        <>
          <IconTrash size={16} />
          Force Delete Review
        </>
      }
      subtitle={targetPath ? "Inspect target object, resolve file locks, and choose recovery mode." : undefined}
      onClose={onClose}
      maxWidth={580}
      footer={
        result ? (
          <Button variant="primary" onClick={onClose}>
            Done
          </Button>
        ) : (
          <div className="row" style={{ gap: 8, justifyContent: "flex-end", width: "100%" }}>
            <Button variant="ghost" onClick={onClose} disabled={executing}>
              Cancel
            </Button>
            {plan && (
              <Button
                variant={mode === "permanent" ? "danger" : "primary"}
                icon={<IconTrash size={14} />}
                loading={executing}
                disabled={mode === "permanent" && !confirmPermanent}
                onClick={handleExecute}
              >
                {mode === "quarantine" ? "Quarantine Target (7-Day)" : "Permanently Delete Target"}
              </Button>
            )}
          </div>
        )
      }
    >
      {loading && (
        <div style={{ textAlign: "center", padding: 32 }}>
          <Spinner size={24} />
          <p style={{ marginTop: 12, color: "var(--text-muted)", fontSize: 13 }}>
            Inspecting filesystem target and scanning for file locks…
          </p>
        </div>
      )}

      {error && (
        <Banner tone="danger">
          <strong>Refused:</strong> {error}
        </Banner>
      )}

      {result && (
        <div className="exp-modal-card">
          <Banner tone={result.warnings.length > 0 ? "caution" : "safe"}>
            <strong>{result.message}</strong>
          </Banner>
          <div className="exp-meta-grid" style={{ marginTop: 8 }}>
            <div className="exp-meta-item">
              <span className="exp-meta-label">Deleted Files</span>
              <span className="exp-meta-val">{formatCount(result.deletedFiles)}</span>
            </div>
            <div className="exp-meta-item">
              <span className="exp-meta-label">Deleted Folders</span>
              <span className="exp-meta-val">{formatCount(result.deletedDirs)}</span>
            </div>
            <div className="exp-meta-item">
              <span className="exp-meta-label">Freed Storage</span>
              <span className="exp-meta-val">{formatBytes(result.freedBytes)}</span>
            </div>
            <div className="exp-meta-item">
              <span className="exp-meta-label">Mode</span>
              <span className="exp-meta-val">
                {result.mode === "quarantine" ? "7-Day Quarantine" : "Permanent Deletion"}
              </span>
            </div>
          </div>
          {result.quarantineBatchId && (
            <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 6 }}>
              Quarantine Batch ID: <code>{result.quarantineBatchId}</code> (Restorable in Settings &gt; Safety).
            </p>
          )}
          {result.warnings.length > 0 && (
            <div style={{ marginTop: 8 }}>
              <span style={{ fontSize: 11, fontWeight: 600, color: "var(--caution)" }}>Warnings:</span>
              <ul className="exp-reasons-list">
                {result.warnings.map((w, idx) => (
                  <li key={idx}>{w}</li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}

      {!loading && !result && inspection && (
        <>
          {/* Target Overview Card */}
          <div className="exp-modal-card">
            <div className="exp-target-path">{inspection.path}</div>
            {inspection.canonicalPath !== inspection.path && (
              <div style={{ fontSize: 10, color: "var(--text-muted)" }}>
                Canonical: {inspection.canonicalPath}
              </div>
            )}

            <div className="exp-meta-grid" style={{ marginTop: 4 }}>
              <div className="exp-meta-item">
                <span className="exp-meta-label">Type</span>
                <span className="exp-meta-val">
                  {inspection.isDirectory ? "Folder" : "File"}
                </span>
              </div>
              <div className="exp-meta-item">
                <span className="exp-meta-label">Size</span>
                <span className="exp-meta-val">{formatBytes(inspection.sizeBytes)}</span>
              </div>
              {inspection.isDirectory && (
                <>
                  <div className="exp-meta-item">
                    <span className="exp-meta-label">Files</span>
                    <span className="exp-meta-val">{formatCount(inspection.fileCount)}</span>
                  </div>
                  <div className="exp-meta-item">
                    <span className="exp-meta-label">Subfolders</span>
                    <span className="exp-meta-val">{formatCount(inspection.dirCount)}</span>
                  </div>
                </>
              )}
              {inspection.owner && (
                <div className="exp-meta-item">
                  <span className="exp-meta-label">Owner</span>
                  <span className="exp-meta-val">{inspection.owner}</span>
                </div>
              )}
            </div>

            <div className="exp-badges" style={{ marginTop: 6 }}>
              {inspection.isReadonly && <Badge tone="caution">Read-Only Attribute</Badge>}
              {inspection.isHidden && <Badge tone="neutral">Hidden</Badge>}
              {inspection.isSystem && <Badge tone="danger">System Flagged</Badge>}
              {inspection.isReparsePoint && <Badge tone="caution">Reparse Point / Symlink</Badge>}
              {!inspection.isReadonly && !inspection.isSystem && (
                <Badge tone="safe">Normal Attributes</Badge>
              )}
            </div>

            {inspection.reasonsForFailure.length > 0 && (
              <div style={{ marginTop: 6 }}>
                <span style={{ fontSize: 11, fontWeight: 600, color: "var(--caution)" }}>
                  Why ordinary deletion may fail:
                </span>
                <ul className="exp-reasons-list">
                  {inspection.reasonsForFailure.map((r, i) => (
                    <li key={i}>{r}</li>
                  ))}
                </ul>
              </div>
            )}
          </div>

          {/* Locking Processes */}
          {plan?.lockingProcesses && plan.lockingProcesses.length > 0 && (
            <div className="exp-modal-card">
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <span style={{ fontSize: 12, fontWeight: 600, color: "var(--danger)" }}>
                  Locked by {plan.lockingProcesses.length} Running Process{plan.lockingProcesses.length === 1 ? "" : "es"}:
                </span>
                <Button
                  size="sm"
                  variant="ghost"
                  icon={<IconRefresh size={12} />}
                  onClick={() => targetPath && loadPlan(targetPath)}
                >
                  Re-check
                </Button>
              </div>

              {processMsg && <Banner tone="info">{processMsg}</Banner>}

              <div className="exp-proc-list">
                {plan.lockingProcesses.map((proc) => (
                  <div className="exp-proc-item" key={proc.pid}>
                    <div className="exp-proc-info">
                      <span className="exp-proc-title">
                        {proc.name} (PID {proc.pid})
                      </span>
                      {proc.windowTitle && (
                        <span className="exp-proc-meta">Window: &ldquo;{proc.windowTitle}&rdquo;</span>
                      )}
                      {proc.path && <span className="exp-proc-meta">{proc.path}</span>}
                    </div>
                    <div style={{ display: "flex", gap: 6, flexShrink: 0 }}>
                      <Button
                        size="sm"
                        variant="secondary"
                        disabled={proc.isCritical || terminatingPid === proc.pid}
                        loading={terminatingPid === proc.pid}
                        onClick={() => handleTerminateLock(proc.pid, false)}
                        title="Attempt graceful closure (WM_CLOSE)"
                      >
                        Close
                      </Button>
                      <Button
                        size="sm"
                        variant="danger"
                        disabled={proc.isCritical || terminatingPid === proc.pid}
                        loading={terminatingPid === proc.pid}
                        onClick={() => handleTerminateLock(proc.pid, true)}
                        title="Force terminate process"
                      >
                        Kill
                      </Button>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Deletion Mode Choice */}
          <div className="exp-delete-modes">
            <label
              className={`exp-mode-option ${mode === "quarantine" ? "exp-mode-option--selected" : ""}`}
              onClick={() => setMode("quarantine")}
            >
              <input
                type="radio"
                name="deleteMode"
                checked={mode === "quarantine"}
                onChange={() => setMode("quarantine")}
                style={{ marginTop: 2 }}
              />
              <div>
                <div style={{ fontWeight: 600, fontSize: 12.5, color: "var(--text-strong)" }}>
                  Recoverable Quarantine (Recommended)
                </div>
                <div style={{ fontSize: 11.5, color: "var(--text-muted)", marginTop: 2 }}>
                  Moves the files to Cyron&apos;s 7-day secure quarantine directory. Can be restored at any time
                  from the Activity / Quarantine tab if needed.
                </div>
              </div>
            </label>

            <label
              className={`exp-mode-option ${mode === "permanent" ? "exp-mode-option--selected-danger" : ""}`}
              onClick={() => setMode("permanent")}
            >
              <input
                type="radio"
                name="deleteMode"
                checked={mode === "permanent"}
                onChange={() => setMode("permanent")}
                style={{ marginTop: 2 }}
              />
              <div>
                <div style={{ fontWeight: 600, fontSize: 12.5, color: "var(--danger)" }}>
                  Permanent Deletion (Irreversible)
                </div>
                <div style={{ fontSize: 11.5, color: "var(--text-muted)", marginTop: 2 }}>
                  Clears read-only attributes, unlinks files, and deletes directories permanently. This cannot be
                  undone.
                </div>
              </div>
            </label>
          </div>

          {mode === "permanent" && (
            <label className="exp-checkbox-label">
              <input
                type="checkbox"
                checked={confirmPermanent}
                onChange={(e) => setConfirmPermanent(e.target.checked)}
              />
              <span>
                I confirm I want to permanently delete {inspection.isDirectory ? `${inspection.fileCount} files in this folder` : "this file"} without recovery.
              </span>
            </label>
          )}

          {executionError && (
            <Banner tone="danger">
              <strong>Execution Error:</strong> {executionError}
            </Banner>
          )}
        </>
      )}
    </Modal>
  );
}
