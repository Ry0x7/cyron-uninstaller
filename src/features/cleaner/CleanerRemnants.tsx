import { useEffect, useRef, useState } from "react";
import { Badge, Banner, Button, Panel, RiskBadge } from "../../components/ui";
import { api, errorMessage } from "../../lib/api";
import { formatBytes, formatDateTime } from "../../lib/format";
import type {
  CleanupPreview,
  InstalledApp,
  LeftoverCandidate,
  LeftoverScanReport,
} from "../../lib/types";

type Review = { appId: string; report: LeftoverScanReport };

export function CleanerRemnants({
  readOnly,
  driveLetters,
  exclusions,
  onPreview,
}: {
  readOnly: boolean | null;
  driveLetters: string[];
  exclusions: string[];
  onPreview: (preview: CleanupPreview) => void;
}) {
  const [apps, setApps] = useState<InstalledApp[] | null>(null);
  const [review, setReview] = useState<Review | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const driveScope = driveLetters.join(",");
  const activeDriveScope = useRef(driveScope);

  const refresh = async () => {
    try {
      setApps(await api.listRemovedApps());
    } catch (caught) {
      setError(errorMessage(caught));
    }
  };

  useEffect(() => {
    void refresh();
  }, []);

  useEffect(() => {
    activeDriveScope.current = driveScope;
    setReview(null);
    setSelected(new Set());
  }, [driveScope]);

  const scan = async (app: InstalledApp) => {
    const scanScope = activeDriveScope.current;
    setBusy(app.id);
    setError(null);
    setReview(null);
    setSelected(new Set());
    try {
      const report = await api.scanAppLeftovers(app.id);
      if (activeDriveScope.current !== scanScope) return;
      const candidates = report.candidates.filter((candidate) => {
        if (candidate.kind === "registry_key") return false;
        const drive = candidate.path.match(/^([a-z]):\\/i)?.[1]?.toUpperCase();
        return drive !== undefined && driveLetters.includes(drive);
      });
      setReview({
        appId: app.id,
        report: {
          ...report,
          candidates,
          totalSize: candidates.reduce((sum, candidate) => sum + candidate.size, 0),
          totalFiles: candidates.reduce((sum, candidate) => sum + candidate.fileCount, 0),
          registryKeysCount: 0,
        },
      });
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setBusy(null);
    }
  };

  const toggle = (candidate: LeftoverCandidate) => {
    if (candidate.risk === "protected") return;
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(candidate.id)) next.delete(candidate.id);
      else next.add(candidate.id);
      return next;
    });
  };

  const preview = async () => {
    if (!review || selected.size === 0) return;
    const previewScope = activeDriveScope.current;
    setBusy(review.appId);
    setError(null);
    try {
      const cleanupPreview = await api.previewRemnantCleanup(
        review.appId,
        review.report.planId,
        Array.from(selected),
        exclusions,
        driveLetters,
      );
      if (activeDriveScope.current !== previewScope) return;
      onPreview(cleanupPreview);
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setReview(null);
      setSelected(new Set());
      setBusy(null);
    }
  };

  return (
    <Panel
      title="Known removed applications"
      subtitle="Review filesystem remnants tied to a verified removal, limited to the selected local drives. Registry review stays in Uninstaller."
      flush
    >
      <div className="cl-remnants">
        {error && <Banner tone="danger">{error}</Banner>}
        {readOnly === true && (
          <Banner tone="accent">Read-only mode is on. You can scan and preview remnants, but the final quarantine action remains disabled.</Banner>
        )}
        {apps === null ? (
          <p className="cl-muted">Loading verified removals…</p>
        ) : apps.length === 0 ? (
          <p className="cl-muted">
            No verified app removals in this session. Cyron does not infer stale installers or app remnants from old file names or age; Windows Installer data stays under Windows management.
          </p>
        ) : (
          <ul className="cl-remnants__apps">
            {apps.map((app) => (
              <li key={app.id}>
                <div>
                  <strong>{app.name}</strong>
                  <span>{app.publisher || "Publisher unknown"}</span>
                  <small>Removed this session · install evidence: {app.installLocation || "registered app identity"}</small>
                </div>
                <Button size="sm" loading={busy === app.id} disabled={busy !== null || driveLetters.length === 0} onClick={() => void scan(app)}>
                  Review remnants
                </Button>
              </li>
            ))}
          </ul>
        )}

        {review && (
          <section className="cl-remnants__review" aria-label={`Remnants for ${review.report.appName}`}>
            <div className="cl-remnants__summary">
              <strong>{review.report.appName}</strong>
              <span>{review.report.candidates.length} candidates · {formatBytes(review.report.totalSize)}</span>
              <span>Review expires {formatDateTime(review.report.expiresAt)}</span>
            </div>
            {review.report.candidates.length === 0 ? (
              <p className="cl-muted">
                No filesystem candidates under the selected drives met the evidence rules. Registry candidates and other drives remain available through Uninstaller review.
              </p>
            ) : (
              <ul className="cl-remnants__candidates">
                {review.report.candidates.map((candidate) => (
                  <li key={candidate.id}>
                    <input
                      type="checkbox"
                      checked={selected.has(candidate.id)}
                      disabled={candidate.risk === "protected" || busy !== null}
                      onChange={() => toggle(candidate)}
                      aria-label={`Select ${candidate.kind} ${candidate.path}`}
                    />
                    <div className="cl-remnants__candidate">
                      <div className="cl-remnants__badges">
                        <Badge tone="neutral">{candidate.kind.replace("_", " ")}</Badge>
                        <Badge tone={candidate.confidence === "high" ? "safe" : "neutral"}>{candidate.confidence} confidence</Badge>
                        <RiskBadge risk={candidate.risk} />
                        <span>{formatBytes(candidate.size)}</span>
                        <span>Age unavailable</span>
                      </div>
                      <code>{candidate.path}</code>
                      <p>{candidate.reason}</p>
                    </div>
                  </li>
                ))}
              </ul>
            )}
            <div className="cl-remnants__actions">
              <Button size="sm" variant="ghost" disabled={busy !== null} onClick={() => setReview(null)}>Close review</Button>
              <Button
                size="sm"
                variant="primary"
                loading={busy === review.appId}
                disabled={selected.size === 0 || busy !== null}
                onClick={() => void preview()}
              >
                Preview {selected.size} for quarantine
              </Button>
            </div>
          </section>
        )}
      </div>
    </Panel>
  );
}
