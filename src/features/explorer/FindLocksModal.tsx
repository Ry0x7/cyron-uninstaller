import { useEffect, useState } from "react";
import { api, errorMessage } from "../../lib/api";
import type { LockingProcessInfo } from "../../lib/types";
import { Badge, Banner, Button, Modal, Spinner } from "../../components/ui";
import { IconRefresh, IconSearch } from "../../components/Icons";
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
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (open && targetPath) {
      loadLocks(targetPath);
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
      const res = await api.terminateForceDeleteLockingProcess(plan.planId, pid, force);
      if (res.success) {
        setActionMsg(`Process ${pid} (${res.name}) closed successfully.`);
        await loadLocks(targetPath);
      } else {
        setActionMsg(`Could not terminate process ${pid}: ${res.error ?? "Access denied or process already exited"}`);
      }
    } catch (err) {
      setActionMsg(`Termination error: ${errorMessage(err)}`);
    } finally {
      setTerminatingPid(null);
    }
  };

  return (
    <Modal
      open={open}
      title={
        <>
          <IconSearch size={16} />
          Locking Processes
        </>
      }
      subtitle={targetPath ? "Inspect active processes holding open handles on this filesystem object." : undefined}
      onClose={onClose}
      maxWidth={560}
      footer={
        <div className="row" style={{ gap: 8, justifyContent: "flex-end", width: "100%" }}>
          {targetPath && (
            <Button
              variant="secondary"
              size="sm"
              icon={<IconRefresh size={13} />}
              loading={loading}
              onClick={() => loadLocks(targetPath)}
            >
              Refresh Locks
            </Button>
          )}
          <Button variant="ghost" onClick={onClose}>
            Close
          </Button>
        </div>
      }
    >
      {targetPath && (
        <div className="exp-modal-card">
          <div className="exp-meta-label">Inspected Object</div>
          <div className="exp-target-path">{targetPath}</div>
        </div>
      )}

      {loading && (
        <div style={{ textAlign: "center", padding: 28 }}>
          <Spinner size={22} />
          <p style={{ marginTop: 10, color: "var(--text-muted)", fontSize: 12.5 }}>
            Querying Windows Restart Manager for open handles…
          </p>
        </div>
      )}

      {error && (
        <Banner tone="danger">
          <strong>Error querying locks:</strong> {error}
        </Banner>
      )}

      {actionMsg && <Banner tone="info">{actionMsg}</Banner>}

      {!loading && !error && processes.length === 0 && (
        <Banner tone="safe">
          <strong>No active locks detected.</strong> No running process currently holds an exclusive handle
          on this path through Windows Restart Manager.
        </Banner>
      )}

      {!loading && processes.length > 0 && (
        <div className="exp-modal-card">
          <div style={{ fontSize: 12, fontWeight: 600, color: "var(--danger)" }}>
            {processes.length} Active Process{processes.length === 1 ? "" : "es"} Holding Locks:
          </div>

          <div className="exp-proc-list" style={{ maxHeight: 280 }}>
            {processes.map((proc) => (
              <div className="exp-proc-item" key={proc.pid}>
                <div className="exp-proc-info">
                  <span className="exp-proc-title">
                    {proc.name} (PID {proc.pid})
                  </span>
                  {proc.windowTitle && (
                    <span className="exp-proc-meta">Window: &ldquo;{proc.windowTitle}&rdquo;</span>
                  )}
                  {proc.path && <span className="exp-proc-meta">{proc.path}</span>}
                  <div style={{ marginTop: 2 }}>
                    {proc.isCritical ? (
                      <Badge tone="danger">Windows Critical</Badge>
                    ) : (
                      <Badge tone="neutral">Standard Process</Badge>
                    )}
                  </div>
                </div>

                <div style={{ display: "flex", gap: 6, flexShrink: 0 }}>
                  <Button
                    size="sm"
                    variant="secondary"
                    disabled={proc.isCritical || terminatingPid === proc.pid}
                    loading={terminatingPid === proc.pid}
                    onClick={() => handleTerminate(proc.pid, false)}
                    title="Attempt graceful closure"
                  >
                    Close
                  </Button>
                  <Button
                    size="sm"
                    variant="danger"
                    disabled={proc.isCritical || terminatingPid === proc.pid}
                    loading={terminatingPid === proc.pid}
                    onClick={() => handleTerminate(proc.pid, true)}
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
    </Modal>
  );
}
