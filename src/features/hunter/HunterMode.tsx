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

interface HunterModeProps {
  isActive: boolean;
  onExit: () => void;
  installedApps?: InstalledApp[];
}

export function HunterMode({
  isActive,
  onExit,
  installedApps = [],
}: HunterModeProps) {
  const [isTargeting, setIsTargeting] = useState(false);
  const [isDragging, setIsDragging] = useState(false);
  const [target, setTarget] = useState<HunterTargetInfo | null>(null);
  const [targetApp, setTargetApp] = useState<InstalledApp | null>(null);
  const [wizardOpen, setWizardOpen] = useState(false);
  const [terminating, setTerminating] = useState(false);
  const [confirmKill, setConfirmKill] = useState(false);
  const [actionMessage, setActionMessage] = useState<string | null>(null);
  const [drawerOpen, setDrawerOpen] = useState(false);

  // Sync session active state
  useEffect(() => {
    if (isActive) {
      setIsTargeting(true);
      setConfirmKill(false);
      setActionMessage(null);
    } else {
      setIsTargeting(false);
      setTarget(null);
      setTargetApp(null);
      setConfirmKill(false);
      setActionMessage(null);
    }
  }, [isActive]);

  useEffect(() => {
    const onDrawerChange = (e: Event) => {
      const customEvent = e as CustomEvent<{ open: boolean }>;
      setDrawerOpen(Boolean(customEvent.detail?.open));
    };
    window.addEventListener("cyron:drawer-change", onDrawerChange);
    return () => window.removeEventListener("cyron:drawer-change", onDrawerChange);
  }, []);

  const pollIntervalRef = useRef<number | null>(null);
  const inFlightRef = useRef(false);
  const reqSeqRef = useRef(0);

  // Poll cursor target while targeting is active without overlapping calls
  useEffect(() => {
    if (!isActive || !isTargeting) {
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

    // 200ms polling interval for responsive desktop targeting
    pollIntervalRef.current = window.setInterval(poll, 200);
    poll(); // Trigger immediate first check

    return () => {
      if (pollIntervalRef.current) {
        window.clearInterval(pollIntervalRef.current);
        pollIntervalRef.current = null;
      }
    };
  }, [isActive, isTargeting, installedApps]);

  // Keyboard navigation: Escape cancels session cleanly, Space/Enter locks target
  useEffect(() => {
    if (!isActive) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onExit();
      } else if (isTargeting && (e.key === " " || e.key === "Enter")) {
        e.preventDefault();
        setIsTargeting(false);
        setConfirmKill(false);
        setActionMessage(null);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [isActive, isTargeting, onExit]);

  const handleLockTarget = () => {
    setIsTargeting(false);
    setConfirmKill(false);
    setActionMessage(null);
  };

  const handleResumeTargeting = () => {
    setIsTargeting(true);
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

  // NEVER display floating widget unless an active Hunter session is running
  if (!isActive) {
    return null;
  }

  return (
    <>
      <div className={`hunter-floating-widget ${drawerOpen ? "hunter-floating-widget--hidden" : ""}`}>
        {target && (
          <section className="hunter-hud" aria-label="Target window" aria-live="polite">
            <header className="hunter-hud__head">
              <span className="hunter-hud__title">
                <IconCrosshair size={15} />
                Target window
              </span>
              <button
                type="button"
                className="icon-btn"
                onClick={() => {
                  setTarget(null);
                  setTargetApp(null);
                  handleResumeTargeting();
                }}
                aria-label="Clear target and resume targeting"
                title="Clear target and resume targeting"
              >
                <IconClose size={14} />
              </button>
            </header>

            <div className="hunter-hud__body">
              <div className="hunter-target-title" title={target.windowTitle || target.processName}>
                {target.windowTitle || target.processName}
              </div>
              <div className="hunter-target-meta">
                PID {target.pid} · {target.processName}
              </div>

              {target.processPath && (
                <div className="hunter-target-path" title={target.processPath}>
                  {target.processPath}
                </div>
              )}

              <div className="hunter-badges">
                {target.isCritical ? (
                  <Badge tone="danger">Protected by Windows</Badge>
                ) : target.matchedAppName ? (
                  <Badge tone="accent">{target.matchedAppName}</Badge>
                ) : (
                  <Badge tone="neutral">Unregistered application</Badge>
                )}
                {isTargeting && <Badge tone="caution">Targeting</Badge>}
              </div>

              {actionMessage && (
                <p className="hunter-message" role="status">
                  {actionMessage}
                </p>
              )}
            </div>

            <div className="hunter-hud__actions">
              {targetApp ? (
                <Button
                  size="sm"
                  variant="danger"
                  icon={<IconTrash size={13} />}
                  onClick={() => setWizardOpen(true)}
                  className="hunter-action--wide"
                >
                  Review uninstall for {targetApp.name}
                </Button>
              ) : target.processPath ? (
                <Button
                  size="sm"
                  variant="secondary"
                  icon={<IconTrash size={13} />}
                  onClick={handleForcedRemovalOfTarget}
                  className="hunter-action--wide"
                >
                  Review forced removal
                </Button>
              ) : null}

              {!target.isCritical && target.canTerminate && (
                confirmKill ? (
                  <div className="hunter-kill-confirm" role="alert">
                    <p>
                      End <strong>{target.processName}</strong> (PID {target.pid})? Unsaved work will be
                      lost.
                    </p>
                    <div className="hunter-action-row">
                      <Button
                        size="sm"
                        variant="danger"
                        icon={<IconStop size={13} />}
                        loading={terminating}
                        onClick={handleTerminateProcess}
                      >
                        End process
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => setConfirmKill(false)}
                        disabled={terminating}
                      >
                        Keep running
                      </Button>
                    </div>
                  </div>
                ) : (
                  <Button
                    size="sm"
                    variant="danger"
                    icon={<IconStop size={13} />}
                    onClick={() => setConfirmKill(true)}
                    className="hunter-action--wide"
                  >
                    End process
                  </Button>
                )
              )}

              {target.processPath && (
                <div className="hunter-action-row">
                  <Button
                    size="sm"
                    variant="ghost"
                    icon={<IconFolder size={13} />}
                    onClick={handleRevealPath}
                    className="hunter-action--grow"
                  >
                    Open folder
                  </Button>
                  <Button
                    size="sm"
                    variant="ghost"
                    icon={<IconCopy size={13} />}
                    onClick={handleCopyPath}
                  >
                    Copy path
                  </Button>
                </div>
              )}
            </div>
          </section>
        )}

        <div className="hunter-dock" role="region" aria-label="Hunter targeting controls">
          <button
            type="button"
            className={`hunter-lock ${isTargeting ? "hunter-lock--active" : ""} ${isDragging ? "hunter-lock--dragging" : ""}`}
            aria-pressed={isTargeting}
            onPointerDown={(e) => {
              try {
                e.currentTarget.setPointerCapture(e.pointerId);
                setIsDragging(true);
                setIsTargeting(true);
                setActionMessage(null);
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
                  handleResumeTargeting();
                }
              }
            }}
            title={
              isTargeting
                ? "Click to lock the current window (or press Space/Enter)"
                : "Click to resume window targeting"
            }
          >
            <IconCrosshair size={16} />
            <span>{isTargeting ? "Lock target" : "Resume"}</span>
          </button>

          <div className="hunter-dock__text">
            <span className="hunter-dock__title">
              {target ? "Target locked" : isTargeting ? "Move over any desktop window" : "Targeting paused"}
            </span>
            <span className="hunter-dock__hint">
              {isTargeting ? "Space or Enter locks · Esc exits" : "Esc exits Hunter Mode"}
            </span>
          </div>

          <Button size="sm" variant="ghost" onClick={onExit} title="Exit Hunter Mode (Esc)">
            Exit
          </Button>
        </div>
      </div>

      {targetApp && (
        <UninstallWizard
          app={targetApp}
          isOpen={wizardOpen}
          onClose={() => setWizardOpen(false)}
          onComplete={() => {
            setWizardOpen(false);
            setTarget(null);
            onExit();
          }}
        />
      )}
    </>
  );
}
