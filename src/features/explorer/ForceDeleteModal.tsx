import { useEffect, useState } from "react";
import { api, errorMessage } from "../../lib/api";
import { displayPath, formatBytes, formatCount } from "../../lib/format";
import type { ForceDeletePlanPreview, ForceDeleteResult } from "../../lib/types";
import { Badge, Banner, Button, Modal, Spinner } from "../../components/ui";
import { IconRefresh, IconShield, IconTrash } from "../../components/Icons";
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
      const prepared = await api.prepareForceDeletePlan(path);
      setPlan(prepared);
    } catch (caught) {
      setError(errorMessage(caught));
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
      const response = await api.terminateForceDeleteLockingProcess(plan.planId, pid, force);
      if (response.success) {
        setProcessMsg(`Process ${pid} (${response.name}) closed.`);
        if (targetPath) await loadPlan(targetPath);
      } else {
        setProcessMsg(
          `Could not close process ${pid}: ${response.error ?? "access denied or process already exited"}`,
        );
      }
    } catch (caught) {
      setProcessMsg(`Could not close the process. ${errorMessage(caught)}`);
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
      const response = await api.executeForceDeletePlan(plan.planId, mode);
      setResult(response);
      onSuccess?.(response);
    } catch (caught) {
      setExecutionError(errorMessage(caught));
    } finally {
      setExecuting(false);
    }
  };

  const inspection = plan?.inspection;
  const lockedCount = plan?.lockingProcesses.length ?? 0;

  return (
    <Modal
      open={open}
      title={
        <>
          <IconTrash size={18} />
          Review deletion
        </>
      }
      subtitle={
        targetPath
          ? "Check the exact target, resolve any process locks, then choose how it is removed."
          : undefined
      }
      onClose={onClose}
      maxWidth={600}
      footer={
        result ? (
          <Button variant="primary" onClick={onClose}>
            Done
          </Button>
        ) : (
          <>
            <Button variant="ghost" onClick={onClose} disabled={executing}>
              Cancel
            </Button>
            {plan && (
              <Button
                variant={mode === "permanent" ? "danger" : "primary"}
                icon={<IconTrash size={14} />}
                loading={executing}
                disabled={mode === "permanent" && !confirmPermanent}
                onClick={() => void handleExecute()}
              >
                {mode === "quarantine" ? "Move to quarantine" : "Delete permanently"}
              </Button>
            )}
          </>
        )
      }
    >
      {loading && (
        <div className="exp-loading">
          <Spinner size={22} />
          <p>Inspecting the target and checking for file locks…</p>
        </div>
      )}

      {error && (
        <Banner tone="danger">
          <strong>Cyron refused this target.</strong> {error}
        </Banner>
      )}

      {result && (
        <div className="exp-modal-card">
          <Banner tone={result.warnings.length > 0 ? "caution" : "safe"}>
            <strong>{result.message}</strong>
          </Banner>

          <dl className="exp-meta-grid">
            <div className="exp-meta-item">
              <dt className="exp-meta-label">Files removed</dt>
              <dd className="exp-meta-val">{formatCount(result.deletedFiles)}</dd>
            </div>
            <div className="exp-meta-item">
              <dt className="exp-meta-label">Folders removed</dt>
              <dd className="exp-meta-val">{formatCount(result.deletedDirs)}</dd>
            </div>
            <div className="exp-meta-item">
              <dt className="exp-meta-label">Space</dt>
              <dd className="exp-meta-val">{formatBytes(result.freedBytes)}</dd>
            </div>
            <div className="exp-meta-item">
              <dt className="exp-meta-label">Mode</dt>
              <dd className="exp-meta-val">
                {result.mode === "quarantine" ? "Quarantine" : "Permanent deletion"}
              </dd>
            </div>
          </dl>

          {result.quarantineBatchId && (
            <p className="exp-note">
              Batch <span className="mono">{result.quarantineBatchId}</span> can be restored from
              History &amp; Recovery → Recovery.
            </p>
          )}

          {result.warnings.length > 0 && (
            <div>
              <h3 className="exp-section-title">Warnings</h3>
              <ul className="exp-reasons-list">
                {result.warnings.map((warning) => (
                  <li key={warning}>{warning}</li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}

      {!loading && !result && inspection && (
        <>
          <section className="exp-modal-card" aria-labelledby="exp-force-target">
            <div className="exp-section-head">
              <h3 className="exp-section-title" id="exp-force-target">
                Target
              </h3>
              <Badge tone={inspection.isSystem ? "danger" : "neutral"}>
                {inspection.isDirectory ? "Folder" : "File"}
              </Badge>
            </div>
            <div className="exp-target-path">{displayPath(inspection.path)}</div>
            {inspection.canonicalPath !== inspection.path && (
              <p className="exp-note">
                Resolved path: <span className="mono">{displayPath(inspection.canonicalPath)}</span>
              </p>
            )}

            <dl className="exp-meta-grid">
              <div className="exp-meta-item">
                <dt className="exp-meta-label">Size</dt>
                <dd className="exp-meta-val">{formatBytes(inspection.sizeBytes)}</dd>
              </div>
              {inspection.isDirectory && (
                <>
                  <div className="exp-meta-item">
                    <dt className="exp-meta-label">Files</dt>
                    <dd className="exp-meta-val">{formatCount(inspection.fileCount)}</dd>
                  </div>
                  <div className="exp-meta-item">
                    <dt className="exp-meta-label">Subfolders</dt>
                    <dd className="exp-meta-val">{formatCount(inspection.dirCount)}</dd>
                  </div>
                </>
              )}
              {inspection.owner && (
                <div className="exp-meta-item">
                  <dt className="exp-meta-label">Owner</dt>
                  <dd className="exp-meta-val">{inspection.owner}</dd>
                </div>
              )}
            </dl>

            <div className="exp-badges">
              {inspection.isReadonly && <Badge tone="caution">Read-only</Badge>}
              {inspection.isHidden && <Badge tone="neutral">Hidden</Badge>}
              {inspection.isSystem && <Badge tone="danger">System</Badge>}
              {inspection.isReparsePoint && <Badge tone="caution">Reparse point</Badge>}
              {!inspection.isReadonly && !inspection.isSystem && (
                <Badge tone="safe">Normal attributes</Badge>
              )}
            </div>

            {inspection.reasonsForFailure.length > 0 && (
              <div>
                <h3 className="exp-section-title">Why ordinary deletion may fail</h3>
                <ul className="exp-reasons-list">
                  {inspection.reasonsForFailure.map((reason) => (
                    <li key={reason}>{reason}</li>
                  ))}
                </ul>
              </div>
            )}
          </section>

          {lockedCount > 0 && (
            <section className="exp-modal-card" aria-labelledby="exp-force-locks">
              <div className="exp-section-head">
                <div>
                  <h3 className="exp-section-title" id="exp-force-locks">
                    Locked by {lockedCount} {lockedCount === 1 ? "process" : "processes"}
                  </h3>
                  <p className="exp-note">Close the app first. Force termination is a last resort.</p>
                </div>
                <Button
                  size="sm"
                  variant="ghost"
                  icon={<IconRefresh size={13} />}
                  onClick={() => targetPath && void loadPlan(targetPath)}
                >
                  Re-check
                </Button>
              </div>

              {processMsg && <Banner tone="info">{processMsg}</Banner>}

              <div className="exp-proc-list">
                {plan?.lockingProcesses.map((process) => (
                  <div className="exp-proc-item" key={process.pid}>
                    <div className="exp-proc-info">
                      <span className="exp-proc-title">
                        {process.name}
                        <span className="exp-proc-pid">PID {process.pid}</span>
                        {process.isCritical && (
                          <Badge tone="danger">
                            <IconShield size={10} /> Windows critical
                          </Badge>
                        )}
                      </span>
                      {process.windowTitle && (
                        <span className="exp-proc-meta">Window: “{process.windowTitle}”</span>
                      )}
                      {process.path && (
                        <span className="exp-proc-path">{displayPath(process.path)}</span>
                      )}
                    </div>
                    <div className="exp-proc-actions">
                      <Button
                        size="sm"
                        disabled={process.isCritical || terminatingPid === process.pid}
                        loading={terminatingPid === process.pid}
                        onClick={() => void handleTerminateLock(process.pid, false)}
                        title="Ask the app to close its window"
                      >
                        Close app
                      </Button>
                      <Button
                        size="sm"
                        variant="danger"
                        disabled={process.isCritical || terminatingPid === process.pid}
                        loading={terminatingPid === process.pid}
                        onClick={() => void handleTerminateLock(process.pid, true)}
                        title="Terminate the process immediately"
                      >
                        Force close
                      </Button>
                    </div>
                  </div>
                ))}
              </div>
            </section>
          )}

          <fieldset className="exp-delete-modes">
            <legend className="exp-section-title">Removal method</legend>

            <label className={`exp-mode-option${mode === "quarantine" ? " is-selected" : ""}`}>
              <input
                type="radio"
                name="deleteMode"
                checked={mode === "quarantine"}
                onChange={() => setMode("quarantine")}
              />
              <span>
                <strong>Move to quarantine</strong>
                <span className="exp-mode-detail">
                  Recoverable for seven days from History &amp; Recovery. Recommended.
                </span>
              </span>
            </label>

            <label
              className={`exp-mode-option exp-mode-option--danger${
                mode === "permanent" ? " is-selected" : ""
              }`}
            >
              <input
                type="radio"
                name="deleteMode"
                checked={mode === "permanent"}
                onChange={() => setMode("permanent")}
              />
              <span>
                <strong>Delete permanently</strong>
                <span className="exp-mode-detail">
                  Clears read-only attributes and unlinks the target. This cannot be undone.
                </span>
              </span>
            </label>
          </fieldset>

          {mode === "permanent" && (
            <label className="exp-checkbox-label">
              <input
                type="checkbox"
                checked={confirmPermanent}
                onChange={(event) => setConfirmPermanent(event.target.checked)}
              />
              <span>
                I understand this permanently deletes{" "}
                {inspection.isDirectory
                  ? `${formatCount(inspection.fileCount)} files in this folder`
                  : "this file"}{" "}
                without recovery.
              </span>
            </label>
          )}

          {executionError && (
            <Banner tone="danger">
              <strong>Deletion did not finish.</strong> {executionError}
            </Banner>
          )}
        </>
      )}
    </Modal>
  );
}
