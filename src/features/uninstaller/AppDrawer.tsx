import { useEffect, useRef, useState } from "react";
import { api, errorMessage } from "../../lib/api";
import { formatBytes, formatCount, formatDate, formatDuration } from "../../lib/format";
import type { InstalledApp, Measured } from "../../lib/types";
import { Badge, Button, Drawer, Eyebrow, KeyValue } from "../../components/ui";
import { IconFolder, IconShield } from "../../components/Icons";

const HIVE_LABEL: Record<string, string> = {
  "machine-64": "HKLM · 64-bit view",
  "machine-32": "HKLM · 32-bit view",
  "user-64": "HKCU · 64-bit view",
  "user-32": "HKCU · 32-bit view",
};

export function AppDrawer({
  app,
  onClose,
}: {
  app: InstalledApp | null;
  onClose: () => void;
}) {
  const [measurement, setMeasurement] = useState<{ appId: string; value: Measured } | null>(null);
  const measured = measurement && measurement.appId === app?.id ? measurement.value : null;
  const [measuring, setMeasuring] = useState(false);
  const [measureError, setMeasureError] = useState<string | null>(null);
  const [revealing, setRevealing] = useState(false);
  const [revealError, setRevealError] = useState<string | null>(null);
  const activeId = useRef<string | null>(null);
  activeId.current = app?.id ?? null;

  useEffect(() => {
    setMeasurement(null);
    setMeasureError(null);
    setRevealError(null);
    setMeasuring(false);
    setRevealing(false);
  }, [app?.id]);

  if (!app) return null;

  const location = app.installLocation;
  const subtitle =
    [app.version ? `Version ${app.version}` : null, app.publisher].filter(Boolean).join(" · ") ||
    "Installed application";

  const measure = async () => {
    if (!location || measuring) return;
    const id = app.id;
    setMeasuring(true);
    setMeasureError(null);
    try {
      const result = await api.measurePath(location);
      if (activeId.current === id) setMeasurement({ appId: id, value: result });
    } catch (error) {
      if (activeId.current === id) setMeasureError(errorMessage(error));
    } finally {
      if (activeId.current === id) setMeasuring(false);
    }
  };

  const reveal = async () => {
    if (!location || revealing) return;
    setRevealing(true);
    setRevealError(null);
    try {
      await api.revealPath(location);
    } catch (error) {
      if (activeId.current === app.id) setRevealError(errorMessage(error));
    } finally {
      if (activeId.current === app.id) setRevealing(false);
    }
  };

  return (
    <Drawer
      open
      title={app.name}
      subtitle={subtitle}
      onClose={onClose}
      footer={
        <>
          <Button
            icon={<IconFolder size={15} />}
            loading={revealing}
            disabled={!location}
            onClick={reveal}
            title={location ? `Open ${location}` : "No install location recorded"}
          >
            Open install location
          </Button>
          <Button variant="ghost" onClick={onClose}>
            Close
          </Button>
        </>
      }
    >
      <div className="un-drawer-badges">
        <Badge tone="neutral">{app.scope === "machine" ? "Machine-wide" : "Current user"}</Badge>
        <Badge tone="neutral">{app.architecture === "x64" ? "64-bit" : "32-bit"}</Badge>
        {app.isProtected && <Badge tone="caution">System component</Badge>}
      </div>

      {app.isProtected && (
        <div className="un-protected">
          <IconShield size={16} />
          <div>
            <strong>Reported as a Windows component.</strong> Cyron marks it protected and never
            proposes removing it, even once uninstall support ships.
          </div>
        </div>
      )}

      <KeyValue
        items={[
          { label: "Publisher", value: app.publisher ?? "—" },
          { label: "Version", value: app.version ?? "—" },
          { label: "Installed", value: formatDate(app.installDate) },
          { label: "Registry view", value: HIVE_LABEL[app.hive] ?? app.hive },
          {
            label: "Install location",
            value: location ? <span className="mono un-code">{location}</span> : "Not reported",
          },
          {
            label: "Estimated size",
            value:
              app.estimatedSize === null
                ? "Not reported"
                : formatBytes(app.estimatedSize),
          },
        ]}
      />

      <section className="un-section" aria-label="Measured install folder size">
        <div className="un-section__head">
          <Eyebrow>Measured on disk</Eyebrow>
          {measured && <span className="un-elapsed">walked in {formatDuration(measured.elapsedMs)}</span>}
        </div>

        {measured ? (
          <>
            <div className="un-measure">
              <div>
                <span className="un-measure__label">Logical size</span>
                <span className="un-measure__value">{formatBytes(measured.size)}</span>
              </div>
              <div>
                <span className="un-measure__label">Allocated</span>
                <span className="un-measure__value">{formatBytes(measured.allocated)}</span>
              </div>
              <div>
                <span className="un-measure__label">Files</span>
                <span className="un-measure__value">{formatCount(measured.fileCount)}</span>
              </div>
              <div>
                <span className="un-measure__label">Folders</span>
                <span className="un-measure__value">{formatCount(measured.dirCount)}</span>
              </div>
            </div>
            {measured.errors.length > 0 && (
              <p className="un-hint">
                {measured.errors.length} location{measured.errors.length === 1 ? "" : "s"} could not be
                read and may be missing from this total.
              </p>
            )}
            {app.estimatedSize !== null && (
              <p className="un-hint">
                Registry estimate was {formatBytes(app.estimatedSize)} — the real folder can differ,
                and shared folders may include files used by other software.
              </p>
            )}
          </>
        ) : (
          <p className="un-hint">
            {measureError
              ? "The folder could not be measured. Check that the recorded location still exists and is accessible, then retry."
              : "Walk the install folder to see the real logical and allocated size instead of the installer's estimate."}
          </p>
        )}

        {measureError && <p className="un-inline-error">{measureError}</p>}

        <div className="row">
          <Button
            size="sm"
            loading={measuring}
            disabled={!location}
            onClick={measure}
            title={location ? "Measure the install folder" : "No install location recorded"}
          >
            {measured ? "Measure again" : "Measure folder"}
          </Button>
        </div>

        {!location && (
          <p className="un-hint">
            This entry has no install location, so there is no folder to measure.
          </p>
        )}
      </section>

      <section className="un-section" aria-label="Uninstall command">
        <Eyebrow>Uninstall command</Eyebrow>
        {app.uninstallString ? (
          <>
            <pre className="un-command" tabIndex={0}>
              {app.uninstallString}
            </pre>
            {app.quietUninstallString && app.quietUninstallString !== app.uninstallString && (
              <>
                <span className="un-command__label">Silent variant</span>
                <pre className="un-command" tabIndex={0}>
                  {app.quietUninstallString}
                </pre>
              </>
            )}
          </>
        ) : (
          <p className="un-hint">This entry records no uninstall command.</p>
        )}
        <p className="un-hint">
          Shown for transparency only. Cyron never executes uninstall commands in Phase 1.
        </p>
      </section>

      {revealError && <p className="un-inline-error">{revealError}</p>}
    </Drawer>
  );
}
