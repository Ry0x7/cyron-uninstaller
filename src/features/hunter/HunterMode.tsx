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
  const [target, setTarget] = useState<HunterTargetInfo | null>(null);
  const [targetApp, setTargetApp] = useState<InstalledApp | null>(null);
  const [wizardOpen, setWizardOpen] = useState(false);
  const [terminating, setTerminating] = useState(false);
  const [actionMessage, setActionMessage] = useState<string | null>(null);

  const pollIntervalRef = useRef<number | null>(null);

  // Poll cursor target while targeting is active
  useEffect(() => {
    if (!isTargeting) {
      if (pollIntervalRef.current) {
        window.clearInterval(pollIntervalRef.current);
        pollIntervalRef.current = null;
      }
      return;
    }

    const poll = async () => {
      try {
        const info = await api.resolveCursorTarget();
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
        // Ignore cursor query errors
      }
    };

    pollIntervalRef.current = window.setInterval(poll, 180);
    return () => {
      if (pollIntervalRef.current) {
        window.clearInterval(pollIntervalRef.current);
        pollIntervalRef.current = null;
      }
    };
  }, [isTargeting, installedApps]);

  const handleStartTargeting = () => {
    setIsActive(true);
    setIsTargeting(true);
    setActionMessage("Move cursor over any desktop window or tray icon to target it...");
  };

  const handleLockTarget = () => {
    setIsTargeting(false);
    setActionMessage(null);
  };

  const handleTerminateProcess = async () => {
    if (!target || terminating) return;
    setTerminating(true);
    try {
      const results = await api.terminateAppProcesses(
        target.matchedAppId || target.processName,
        [target.pid],
        true,
      );
      if (results[0]?.success) {
        setActionMessage(`Process ${target.processName} (PID ${target.pid}) terminated.`);
        setTarget(null);
      } else {
        setActionMessage(`Could not terminate process: ${results[0]?.error ?? "Access denied"}`);
      }
    } catch (err) {
      setActionMessage(`Termination error: ${errorMessage(err)}`);
    } finally {
      setTerminating(false);
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
                  onClick={() => {
                    // Create ad-hoc app representation for forced removal
                    const adHocApp: InstalledApp = {
                      id: `hunter-${target.pid}`,
                      name: target.windowTitle || target.processName,
                      version: null,
                      publisher: null,
                      installLocation: target.processPath ? target.processPath.substring(0, target.processPath.lastIndexOf("\\")) : null,
                      installDate: null,
                      uninstallString: null,
                      quietUninstallString: null,
                      displayIcon: target.processPath,
                      estimatedSize: null,
                      hive: "user-64",
                      scope: "user",
                      architecture: "x64",
                      isProtected: target.isCritical,
                      source: "portable",
                      uninstallSupport: "missing",
                      confidence: "heuristic",
                      detectionReason: "Targeted via Hunter Mode",
                    };
                    setTargetApp(adHocApp);
                    setWizardOpen(true);
                  }}
                >
                  Forced Removal of Application…
                </Button>
              ) : null}

              {!target.isCritical && (
                <Button
                  size="sm"
                  variant="danger"
                  icon={<IconStop size={13} />}
                  loading={terminating}
                  onClick={handleTerminateProcess}
                >
                  Kill Process
                </Button>
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

        {/* Floating Toggle Button */}
        <button
          type="button"
          className={`hunter-trigger-btn ${isTargeting ? "hunter-trigger-btn--active" : ""}`}
          onClick={() => {
            if (isTargeting) {
              handleLockTarget();
            } else {
              handleStartTargeting();
            }
          }}
          title={isTargeting ? "Click to lock target window" : "Enter Hunter Mode"}
        >
          <IconCrosshair size={16} />
          <span>{isTargeting ? "Lock Target" : "Hunter Mode"}</span>
        </button>
      </div>

      {/* Guided Uninstall Wizard if target app is uninstalled */}
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
