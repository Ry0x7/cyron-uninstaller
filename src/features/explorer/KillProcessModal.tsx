import { useEffect, useState } from "react";
import { api, errorMessage } from "../../lib/api";
import type { KillProcessPlanPreview, ProcessTerminateResult } from "../../lib/types";
import { Badge, Banner, Button, Modal, Spinner } from "../../components/ui";
import { IconStop } from "../../components/Icons";
import "./explorerModals.css";

interface KillProcessModalProps {
  open: boolean;
  exePath: string | null;
  onClose: () => void;
}

export function KillProcessModal({ open, exePath, onClose }: KillProcessModalProps) {
  const [loading, setLoading] = useState(false);
  const [plan, setPlan] = useState<KillProcessPlanPreview | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [selectedPids, setSelectedPids] = useState<Set<number>>(new Set());
  const [force, setForce] = useState(false);
  const [executing, setExecuting] = useState(false);
  const [results, setResults] = useState<ProcessTerminateResult[] | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  const loadPlan = async (path: string) => {
    setLoading(true);
    setError(null);
    setPlan(null);
    setResults(null);
    setActionError(null);
    setSelectedPids(new Set());
    try {
      const p = await api.prepareKillProcessPlan(path);
      setPlan(p);
      // Pre-select all non-critical matching processes by default
      const nonCritical = p.matchingProcesses.filter((proc) => !proc.isCritical).map((proc) => proc.pid);
      setSelectedPids(new Set(nonCritical));
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (open && exePath) {
      loadPlan(exePath);
    } else {
      setPlan(null);
      setError(null);
      setResults(null);
      setSelectedPids(new Set());
    }
  }, [open, exePath]);

  const togglePid = (pid: number) => {
    setSelectedPids((prev) => {
      const next = new Set(prev);
      if (next.has(pid)) next.delete(pid);
      else next.add(pid);
      return next;
    });
  };

  const handleSelectAll = (select: boolean) => {
    if (!plan) return;
    if (select) {
      const nonCritical = plan.matchingProcesses.filter((p) => !p.isCritical).map((p) => p.pid);
      setSelectedPids(new Set(nonCritical));
    } else {
      setSelectedPids(new Set());
    }
  };

  const handleExecute = async () => {
    if (!plan || selectedPids.size === 0 || executing) return;
    setExecuting(true);
    setActionError(null);
    try {
      const pidsArray = Array.from(selectedPids);
      const res = await api.executeKillProcessPlan(plan.planId, pidsArray, force);
      setResults(res);
    } catch (err) {
      setActionError(errorMessage(err));
    } finally {
      setExecuting(false);
    }
  };

  const hasRunning = plan && plan.matchingProcesses.length > 0;

  return (
    <Modal
      open={open}
      title={
        <>
          <IconStop size={16} />
          Kill Running Process Review
        </>
      }
      subtitle={exePath ? "Verify running processes whose executable path exactly matches the target." : undefined}
      onClose={onClose}
      maxWidth={580}
      footer={
        results ? (
          <Button variant="primary" onClick={onClose}>
            Done
          </Button>
        ) : (
          <div className="row" style={{ gap: 8, justifyContent: "flex-end", width: "100%" }}>
            <Button variant="ghost" onClick={onClose} disabled={executing}>
              Cancel
            </Button>
            {hasRunning && (
              <Button
                variant={force ? "danger" : "secondary"}
                icon={<IconStop size={14} />}
                loading={executing}
                disabled={selectedPids.size === 0}
                onClick={handleExecute}
              >
                {force ? "Force Terminate Selected" : "Graceful Close Selected"} ({selectedPids.size})
              </Button>
            )}
          </div>
        )
      }
    >
      {exePath && (
        <div className="exp-modal-card">
          <div className="exp-meta-label">Selected Executable</div>
          <div className="exp-target-path">{exePath}</div>
        </div>
      )}

      {loading && (
        <div style={{ textAlign: "center", padding: 28 }}>
          <Spinner size={22} />
          <p style={{ marginTop: 10, color: "var(--text-muted)", fontSize: 12.5 }}>
            Scanning running process table for exact executable matches…
          </p>
        </div>
      )}

      {error && (
        <Banner tone="danger">
          <strong>Refused:</strong> {error}
        </Banner>
      )}

      {actionError && (
        <Banner tone="danger">
          <strong>Action Failed:</strong> {actionError}
        </Banner>
      )}

      {results && (
        <div className="exp-modal-card">
          <span style={{ fontSize: 12.5, fontWeight: 600, color: "var(--text-strong)" }}>
            Termination Results:
          </span>
          <div className="exp-proc-list" style={{ marginTop: 6 }}>
            {results.map((res) => (
              <div className="exp-proc-item" key={res.pid}>
                <div className="exp-proc-info">
                  <span className="exp-proc-title">
                    {res.name} (PID {res.pid})
                  </span>
                  {res.error && (
                    <span style={{ fontSize: 11, color: "var(--danger)" }}>{res.error}</span>
                  )}
                </div>
                <Badge tone={res.success ? "safe" : "danger"}>
                  {res.success ? "Terminated" : "Failed"}
                </Badge>
              </div>
            ))}
          </div>
        </div>
      )}

      {!loading && !error && !results && !hasRunning && (
        <Banner tone="safe">
          <strong>No matching running process.</strong> There are currently no active processes running from this exact executable path.
        </Banner>
      )}

      {!loading && !results && hasRunning && (
        <div className="exp-modal-card">
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <span style={{ fontSize: 12, fontWeight: 600, color: "var(--text-strong)" }}>
              {plan.matchingProcesses.length} Verified Matching Process{plan.matchingProcesses.length === 1 ? "" : "es"}:
            </span>
            <div style={{ display: "flex", gap: 6 }}>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => handleSelectAll(true)}
              >
                Select all
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => handleSelectAll(false)}
              >
                Deselect all
              </Button>
            </div>
          </div>

          <div className="exp-proc-list" style={{ maxHeight: 220 }}>
            {plan.matchingProcesses.map((proc) => {
              const checked = selectedPids.has(proc.pid);
              return (
                <div
                  className="exp-proc-item"
                  key={proc.pid}
                  style={{ opacity: proc.isCritical ? 0.6 : 1 }}
                >
                  <label style={{ display: "flex", alignItems: "center", gap: 8, cursor: proc.isCritical ? "not-allowed" : "pointer", flex: 1, minWidth: 0 }}>
                    <input
                      type="checkbox"
                      checked={checked}
                      disabled={proc.isCritical}
                      onChange={() => togglePid(proc.pid)}
                    />
                    <div className="exp-proc-info">
                      <span className="exp-proc-title">
                        {proc.name} (PID {proc.pid})
                      </span>
                      {proc.windowTitle && (
                        <span className="exp-proc-meta">Window: &ldquo;{proc.windowTitle}&rdquo;</span>
                      )}
                      {proc.path && <span className="exp-proc-meta">{proc.path}</span>}
                    </div>
                  </label>
                  {proc.isCritical && <Badge tone="danger">Critical</Badge>}
                </div>
              );
            })}
          </div>

          <div style={{ marginTop: 10, display: "flex", flexDirection: "column", gap: 8 }}>
            <label style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 11.5, cursor: "pointer" }}>
              <input
                type="checkbox"
                checked={force}
                onChange={(e) => setForce(e.target.checked)}
              />
              <span>
                <strong>Force termination:</strong> Bypass graceful window closure (WM_CLOSE) and immediately terminate process with TerminateProcess.
              </span>
            </label>
          </div>
        </div>
      )}
    </Modal>
  );
}
