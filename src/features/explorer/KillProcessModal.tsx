import { useEffect, useState } from "react";
import { api, errorMessage } from "../../lib/api";
import { displayPath } from "../../lib/format";
import type { KillProcessPlanPreview, ProcessTerminateResult } from "../../lib/types";
import { Badge, Banner, Button, Modal, Spinner } from "../../components/ui";
import { IconShield, IconStop } from "../../components/Icons";
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
      const prepared = await api.prepareKillProcessPlan(path);
      setPlan(prepared);
      const nonCritical = prepared.matchingProcesses
        .filter((process) => !process.isCritical)
        .map((process) => process.pid);
      setSelectedPids(new Set(nonCritical));
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (open && exePath) {
      void loadPlan(exePath);
    } else {
      setPlan(null);
      setError(null);
      setResults(null);
      setSelectedPids(new Set());
    }
  }, [open, exePath]);

  const togglePid = (pid: number) => {
    setSelectedPids((previous) => {
      const next = new Set(previous);
      if (next.has(pid)) next.delete(pid);
      else next.add(pid);
      return next;
    });
  };

  const handleSelectAll = (select: boolean) => {
    if (!plan) return;
    if (select) {
      const nonCritical = plan.matchingProcesses
        .filter((process) => !process.isCritical)
        .map((process) => process.pid);
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
      const response = await api.executeKillProcessPlan(plan.planId, Array.from(selectedPids), force);
      setResults(response);
    } catch (caught) {
      setActionError(errorMessage(caught));
    } finally {
      setExecuting(false);
    }
  };

  const hasRunning = Boolean(plan && plan.matchingProcesses.length > 0);

  return (
    <Modal
      open={open}
      title={
        <>
          <IconStop size={18} />
          Review process termination
        </>
      }
      subtitle={
        exePath
          ? "Cyron matched running processes by this exact executable path. Windows-critical processes cannot be selected."
          : undefined
      }
      onClose={onClose}
      maxWidth={600}
      footer={
        results ? (
          <Button variant="primary" onClick={onClose}>
            Done
          </Button>
        ) : (
          <>
            <Button variant="ghost" onClick={onClose} disabled={executing}>
              Cancel
            </Button>
            {hasRunning && (
              <Button
                variant={force ? "danger" : "secondary"}
                icon={<IconStop size={14} />}
                loading={executing}
                disabled={selectedPids.size === 0}
                onClick={() => void handleExecute()}
              >
                {force ? "Force close" : "Close gracefully"} ({selectedPids.size})
              </Button>
            )}
          </>
        )
      }
    >
      {exePath && (
        <section className="exp-modal-card">
          <h3 className="exp-section-title">Matched executable</h3>
          <div className="exp-target-path">{displayPath(exePath)}</div>
        </section>
      )}

      {loading && (
        <div className="exp-loading">
          <Spinner size={22} />
          <p>Checking the running process table for this exact path…</p>
        </div>
      )}

      {error && (
        <Banner tone="danger">
          <strong>Cyron refused this request.</strong> {error}
        </Banner>
      )}

      {actionError && (
        <Banner tone="danger">
          <strong>Processes were not closed.</strong> {actionError}
        </Banner>
      )}

      {results && (
        <section className="exp-modal-card" aria-labelledby="exp-kill-results">
          <h3 className="exp-section-title" id="exp-kill-results">
            Results
          </h3>
          <div className="exp-proc-list">
            {results.map((response) => (
              <div className="exp-proc-item" key={response.pid}>
                <div className="exp-proc-info">
                  <span className="exp-proc-title">
                    {response.name}
                    <span className="exp-proc-pid">PID {response.pid}</span>
                  </span>
                  {response.error && <span className="exp-proc-error">{response.error}</span>}
                </div>
                <Badge tone={response.success ? "safe" : "danger"}>
                  {response.success ? "Closed" : "Failed"}
                </Badge>
              </div>
            ))}
          </div>
        </section>
      )}

      {!loading && !error && !results && !hasRunning && (
        <Banner tone="safe">
          <strong>Nothing is running from this path.</strong> There are no active matching processes
          right now.
        </Banner>
      )}

      {!loading && !results && plan && hasRunning && (
        <section className="exp-modal-card" aria-labelledby="exp-kill-list">
          <div className="exp-section-head">
            <div>
              <h3 className="exp-section-title" id="exp-kill-list">
                {plan.matchingProcesses.length}{" "}
                {plan.matchingProcesses.length === 1 ? "matching process" : "matching processes"}
              </h3>
              <p className="exp-note">Only the checked processes will be closed.</p>
            </div>
            <div className="exp-inline-actions">
              <Button size="sm" variant="ghost" onClick={() => handleSelectAll(true)}>
                Select all
              </Button>
              <Button size="sm" variant="ghost" onClick={() => handleSelectAll(false)}>
                Clear
              </Button>
            </div>
          </div>

          <div className="exp-proc-list">
            {plan.matchingProcesses.map((process) => {
              const checked = selectedPids.has(process.pid);
              return (
                <label
                  className={`exp-proc-item exp-select-row${
                    process.isCritical ? " is-disabled" : ""
                  }`}
                  key={process.pid}
                >
                  <input
                    type="checkbox"
                    checked={checked}
                    disabled={process.isCritical}
                    onChange={() => togglePid(process.pid)}
                  />
                  <span className="exp-proc-info">
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
                  </span>
                </label>
              );
            })}
          </div>

          <label className="exp-force-option">
            <input type="checkbox" checked={force} onChange={(event) => setForce(event.target.checked)} />
            <span>
              <strong>Force termination</strong>
              <span className="exp-mode-detail">
                Skip the graceful window-close request and terminate immediately. Unsaved work in the
                selected apps will be lost.
              </span>
            </span>
          </label>
        </section>
      )}
    </Modal>
  );
}
