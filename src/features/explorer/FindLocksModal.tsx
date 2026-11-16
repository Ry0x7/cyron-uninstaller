import { useEffect, useState } from "react";
import { api, errorMessage } from "../../lib/api";
import { displayPath } from "../../lib/format";
import type { LockingProcessInfo } from "../../lib/types";
import { Badge, Banner, Button, Modal, Spinner } from "../../components/ui";
import { IconRefresh, IconSearch, IconShield } from "../../components/Icons";
import "./explorerModals.css";

interface FindLocksModalProps {
  open: boolean;
  targetPath: string | null;
  onClose: () => void;
}

export function FindLocksModal({ open, targetPath, onClose }: FindLocksModalProps) {
  const [loading, setLoading] = useState(false);
  const [processes, setProcesses] = useState<LockingProcessInfo[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [actionMsg, setActionMsg] = useState<string | null>(null);
  const [terminatingPid, setTerminatingPid] = useState<number | null>(null);

  const loadLocks = async (path: string) => {
    setLoading(true);
    setError(null);
    setActionMsg(null);
    try {
      const list = await api.findLockingProcesses(path);
      setProcesses(list);
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (open && targetPath) {
      void loadLocks(targetPath);
    } else {
      setProcesses([]);
      setError(null);
      setActionMsg(null);
    }
  }, [open, targetPath]);

  const handleTerminate = async (pid: number, force: boolean) => {
    setTerminatingPid(pid);
    setActionMsg(null);
    try {
      if (!targetPath) return;
      const plan = await api.prepareForceDeletePlan(targetPath);
      const response = await api.terminateForceDeleteLockingProcess(plan.planId, pid, force);
      if (response.success) {
        await loadLocks(targetPath);
        setActionMsg(`Process ${pid} (${response.name}) closed.`);
      } else {
        setActionMsg(
          `Could not close process ${pid}: ${response.error ?? "access denied or process already exited"}`,
        );
      }
    } catch (caught) {
      setActionMsg(`Could not close the process. ${errorMessage(caught)}`);
    } finally {
      setTerminatingPid(null);
    }
  };

  const lockedCount = processes.length;

  return (
    <Modal
      open={open}
      title={
        <>
          <IconSearch size={18} />
          Processes using this item
        </>
      }
      subtitle={
        targetPath
          ? "Windows Restart Manager reports any process holding an open handle on the exact path."
          : undefined
      }
      onClose={onClose}
      maxWidth={600}
      footer={
        <>
          {targetPath && (
            <Button
              variant="secondary"
              icon={<IconRefresh size={13} />}
              loading={loading}
              onClick={() => void loadLocks(targetPath)}
            >
              Check again
            </Button>
          )}
          <Button variant="primary" onClick={onClose}>
            Close
          </Button>
        </>
      }
    >
      {targetPath && (
        <section className="exp-modal-card">
          <h3 className="exp-section-title">Inspected path</h3>
          <div className="exp-target-path">{displayPath(targetPath)}</div>
        </section>
      )}

      {loading && (
        <div className="exp-loading">
          <Spinner size={22} />
          <p>Asking Windows which processes hold this path open…</p>
        </div>
      )}

      {error && (
        <Banner tone="danger">
          <strong>Locks could not be read.</strong> {error}
        </Banner>
      )}

      {actionMsg && <Banner tone="info">{actionMsg}</Banner>}

      {!loading && !error && lockedCount === 0 && (
        <Banner tone="safe">
          <strong>No process is holding this item.</strong> Windows Restart Manager found no active
          handle on the exact path.
        </Banner>
      )}

      {!loading && lockedCount > 0 && (
        <section className="exp-modal-card" aria-labelledby="exp-lock-count">
          <div className="exp-section-head">
            <div>
              <h3 className="exp-section-title" id="exp-lock-count">
                {lockedCount} {lockedCount === 1 ? "process is" : "processes are"} using it
              </h3>
              <p className="exp-note">Close the app normally before forcing anything.</p>
            </div>
            <Badge tone="danger">{lockedCount}</Badge>
          </div>

          <div className="exp-proc-list">
            {processes.map((process) => (
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
                    onClick={() => void handleTerminate(process.pid, false)}
                    title="Ask the app to close its window"
                  >
                    Close app
                  </Button>
                  <Button
                    size="sm"
                    variant="danger"
                    disabled={process.isCritical || terminatingPid === process.pid}
                    loading={terminatingPid === process.pid}
                    onClick={() => void handleTerminate(process.pid, true)}
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
    </Modal>
  );
}
