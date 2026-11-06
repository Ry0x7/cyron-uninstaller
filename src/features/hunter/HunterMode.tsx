import { useEffect, useRef, useState } from "react";
import { api, errorMessage } from "../../lib/api";
import type { HunterTargetInfo, InstalledApp } from "../../lib/types";
import { Badge, Button } from "../../components/ui";
import {
  IconClose,
  IconCopy,
  IconCrosshair,
  IconFolder,
  IconStop,
  IconTrash,
} from "../../components/Icons";
import { UninstallWizard } from "../uninstaller/UninstallWizard";
import "./hunter.css";

export function HunterMode({
  installedApps = [],
}: {
  installedApps?: InstalledApp[];
}) {
  const [isActive, setIsActive] = useState(false);
  const [isTargeting, setIsTargeting] = useState(false);
  const [isDragging, setIsDragging] = useState(false);
  const [target, setTarget] = useState<HunterTargetInfo | null>(null);
  const [targetApp, setTargetApp] = useState<InstalledApp | null>(null);
  const [wizardOpen, setWizardOpen] = useState(false);
  const [terminating, setTerminating] = useState(false);
  const [confirmKill, setConfirmKill] = useState(false);
  const [actionMessage, setActionMessage] = useState<string | null>(null);

  const pollIntervalRef = useRef<number | null>(null);
  const inFlightRef = useRef(false);
  const reqSeqRef = useRef(0);

  // Poll cursor target while targeting is active without overlapping calls
  useEffect(() => {
    if (!isTargeting) {
      if (pollIntervalRef.current) {
        window.clearInterval(pollIntervalRef.current);
        pollIntervalRef.current = null;
      }
      return;
    }

    const poll = async () => {
      if (inFlightRef.current) return;
      inFlightRef.current = true;
      const curSeq = ++reqSeqRef.current;
      try {
        const info = await api.resolveCursorTarget();
        if (curSeq !== reqSeqRef.current) return; // Discard stale response

        if (info && info.pid > 4 && info.processName.toLowerCase() !== "cyron.exe") {
          setTarget(info);
          // Match installed app
          if (info.matchedAppId) {
            const matched = installedApps.find((a) => a.id === info.matchedAppId);
            setTargetApp(matched ?? null);
          } else {
            setTargetApp(null);
          }
        }
      } catch {
        // Fail silent on cursor resolution errors
      } finally {
        inFlightRef.current = false;
      }
    };

    // 250ms polling interval for minimal CPU overhead
    pollIntervalRef.current = window.setInterval(poll, 250);
    poll(); // Trigger immediate first check

    return () => {
      if (pollIntervalRef.current) {
        window.clearInterval(pollIntervalRef.current);
        pollIntervalRef.current = null;
      }
    };
  }, [isTargeting, installedApps]);

  // Keyboard navigation: Space/Enter locks target, Escape cancels
  useEffect(() => {
    if (!isTargeting) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setIsTargeting(false);
        setIsActive(false);
        setTarget(null);
        setActionMessage(null);
      } else if (e.key === " " || e.key === "Enter") {
        e.preventDefault();
        handleLockTarget();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [isTargeting]);

  const handleStartTargeting = () => {
    setIsActive(true);
    setIsTargeting(true);
    setConfirmKill(false);
    setActionMessage("Move cursor over any desktop window or drag the crosshair icon to lock target (Space/Enter to lock, Esc to cancel)...");
  };

  const handleLockTarget = () => {
    setIsTargeting(false);
    setConfirmKill(false);
    setActionMessage(null);
  };

  const handleTerminateProcess = async () => {
    if (!target || terminating) return;
    setTerminating(true);
    setActionMessage(null);
    try {
      const results = await api.terminateAppProcesses(
        target.hunterTargetId,
        [target.pid],
        true,
      );
      const res = results[0];
      if (res?.success) {
        setActionMessage(`Process ${target.processName} (PID ${target.pid}) terminated.`);
        setTarget(null);
        setConfirmKill(false);
      } else {
        setActionMessage(`Could not terminate: ${res?.error ?? "Access denied or process already exited"}`);
      }
    } catch (err) {
      setActionMessage(`Termination error: ${errorMessage(err)}`);
    } finally {
      setTerminating(false);
      setConfirmKill(false);
    }
  };

  const handleForcedRemovalOfTarget = async () => {
    if (!target) return;
    try {
      // Resolve backend-registered identity snapshot
      const app = await api.snapshotAppIdentity(target.hunterTargetId);
      setTargetApp(app);
      setWizardOpen(true);
    } catch (err) {
      setActionMessage(`Could not prepare target for removal: ${errorMessage(err)}`);
    }
  };

  const handleRevealPath = async () => {
    if (!target?.processPath) return;
    try {
      await api.revealPath(target.processPath);
    } catch (err) {
      setActionMessage(`Could not reveal path: ${errorMessage(err)}`);
    }
  };

  const handleCopyPath = () => {
    if (!target?.processPath) return;
    navigator.clipboard.writeText(target.processPath);
    setActionMessage("Path copied to clipboard.");
  };

  return (
    <>
      <div className="hunter-floating-widget">
        {/* Hunter HUD Panel when target is acquired */}
        {isActive && target && (
          <div className="hunter-hud">
            <div className="hunter-hud__head">
              <span className="hunter-hud__title">
                <IconCrosshair size={15} />
                Target Acquired
              </span>
              <button
                type="button"
                className="btn btn--sm btn--ghost"
                onClick={() => {
                  setTarget(null);
                  setIsTargeting(false);
                  setConfirmKill(false);
                }}
                title="Dismiss"
              >
                <IconClose size={13} />
              </button>
            </div>

            <div className="hunter-hud__body">
              <div>
                <div className="hunter-target-title">
                  {target.windowTitle || target.processName}
                </div>
                <div className="hunter-target-meta">
                  PID {target.pid} · {target.processName}
                </div>
              </div>

              {target.processPath && (
                <div style={{ wordBreak: "break-all", fontSize: 11, color: "var(--text-muted)" }}>
                  {target.processPath}
                </div>
              )}

              <div style={{ display: "flex", gap: 6, flexWrap: "wrap", alignItems: "center" }}>
                {target.isCritical ? (
                  <Badge tone="danger">Windows Critical Process</Badge>
                ) : target.matchedAppName ? (
                  <Badge tone="accent">{target.matchedAppName}</Badge>
                ) : (
                  <Badge tone="neutral">Unregistered Application</Badge>
                )}
                {isTargeting && <Badge tone="caution">Targeting Active</Badge>}
              </div>

              {actionMessage && (
                <div style={{ fontSize: 11, color: "var(--accent)" }}>
                  {actionMessage}
                </div>
              )}
            </div>

            <div className="hunter-hud__actions">
              {targetApp ? (
                <Button
                  size="sm"
                  variant="danger"
                  icon={<IconTrash size={13} />}
                  onClick={() => setWizardOpen(true)}
                >
                  Uninstall {targetApp.name}…
                </Button>
              ) : target.processPath ? (
                <Button
                  size="sm"
                  variant="secondary"
                  icon={<IconTrash size={13} />}
                  onClick={handleForcedRemovalOfTarget}
                >
                  Forced Removal of Application…
                </Button>
              ) : null}

              {!target.isCritical && target.canTerminate && (
                confirmKill ? (
                  <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                    <Button
                      size="sm"
                      variant="danger"
                      icon={<IconStop size={13} />}
                      loading={terminating}
                      onClick={handleTerminateProcess}
                    >
                      Confirm Kill PID {target.pid}
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() => setConfirmKill(false)}
                    >
                      Cancel
                    </Button>
                  </div>
                ) : (
                  <Button
                    size="sm"
                    variant="danger"
                    icon={<IconStop size={13} />}
                    onClick={() => setConfirmKill(true)}
                  >
                    Kill Process
                  </Button>
                )
              )}

              {target.processPath && (
                <div style={{ display: "flex", gap: 6 }}>
                  <Button
                    size="sm"
                    variant="ghost"
                    icon={<IconFolder size={13} />}
                    onClick={handleRevealPath}
                    style={{ flex: 1 }}
                  >
                    Open Folder
                  </Button>
                  <Button
                    size="sm"
                    variant="ghost"
                    icon={<IconCopy size={13} />}
                    onClick={handleCopyPath}
                    title="Copy executable path"
                  >
                    Copy
                  </Button>
                </div>
              )}
            </div>
          </div>
        )}

        {/* Floating Crosshair Button supporting Click or Drag-and-Drop Targeting */}
        <button
          type="button"
          className={`hunter-trigger-btn ${isTargeting ? "hunter-trigger-btn--active" : ""} ${isDragging ? "hunter-trigger-btn--dragging" : ""}`}
          onPointerDown={(e) => {
            try {
              e.currentTarget.setPointerCapture(e.pointerId);
              setIsDragging(true);
              setIsActive(true);
              setIsTargeting(true);
              setActionMessage("Drag cursor over any desktop window, then release to lock target…");
            } catch {
              // Ignore pointer capture errors
            }
          }}
          onPointerUp={(e) => {
            if (isDragging) {
              try {
                e.currentTarget.releasePointerCapture(e.pointerId);
              } catch {
                // Ignore
              }
              setIsDragging(false);
              handleLockTarget();
            }
          }}
          onClick={() => {
            if (!isDragging) {
              if (isTargeting) {
                handleLockTarget();
              } else {
                handleStartTargeting();
              }
            }
          }}
          title={isTargeting ? "Click to lock target window (or press Space/Enter)" : "Click or drag crosshair over any desktop window"}
        >
          <IconCrosshair size={16} />
          <span>{isTargeting ? "Lock Target" : "Hunter Mode"}</span>
        </button>
      </div>

      {/* Guided Uninstall Wizard if target app is selected for uninstall/forced removal */}
      {targetApp && (
        <UninstallWizard
          app={targetApp}
          isOpen={wizardOpen}
          onClose={() => setWizardOpen(false)}
          onComplete={() => {
            setWizardOpen(false);
            setTarget(null);
            setIsActive(false);
          }}
        />
      )}
    </>
  );
}
