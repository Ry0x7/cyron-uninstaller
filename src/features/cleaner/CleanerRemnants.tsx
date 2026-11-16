import { useEffect, useRef, useState } from "react";
import { Badge, Banner, Button, Panel, RiskBadge } from "../../components/ui";
import { api, errorMessage } from "../../lib/api";
import { formatBytes, formatDateTime } from "../../lib/format";
import type {
  AppHistoryRecord,
  CleanupPreview,
  InstalledApp,
  LeftoverCandidate,
  LeftoverScanReport,
} from "../../lib/types";

type Review = { appId: string; report: LeftoverScanReport };

/** The Cleaner only quarantines filesystem candidates on drives in scope. */
function narrowReport(report: LeftoverScanReport, driveLetters: string[]): LeftoverScanReport {
  const candidates = report.candidates.filter((candidate) => {
    if (candidate.kind === "registry_key") return false;
    const drive = candidate.path.match(/^([a-z]):\\/i)?.[1]?.toUpperCase();
    return drive !== undefined && driveLetters.includes(drive);
  });
  return {
    ...report,
    candidates,
    totalSize: candidates.reduce((sum, candidate) => sum + candidate.size, 0),
    totalFiles: candidates.reduce((sum, candidate) => sum + candidate.fileCount, 0),
    registryKeysCount: 0,
  };
}

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
  const [history, setHistory] = useState<AppHistoryRecord[] | null>(null);
  const [review, setReview] = useState<Review | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Explicit evidence for an application removed before Cyron was installed.
  const [manualName, setManualName] = useState("");
  const [manualPublisher, setManualPublisher] = useState("");
  const [manualPath, setManualPath] = useState("");
  const [manualBusy, setManualBusy] = useState(false);
  const [manualError, setManualError] = useState<string | null>(null);

  const driveScope = driveLetters.join(",");
  const activeDriveScope = useRef(driveScope);

  const refresh = async () => {
    try {
      const [removed, records] = await Promise.all([
        api.listRemovedApps(),
        api.listAppHistory(),
      ]);
      setApps(removed);
      setHistory(records);
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
      setReview({ appId: app.id, report: narrowReport(report, driveLetters) });
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setBusy(null);
    }
  };

  const scanNamed = async () => {
    const name = manualName.trim();
    if (!name || manualBusy) return;
    const scanScope = activeDriveScope.current;
    setManualBusy(true);
    setManualError(null);
    setReview(null);
    setSelected(new Set());
    try {
      const publisher = manualPublisher.trim();
      const installLocation = manualPath.trim();
      const report = await api.scanNamedRemnants(
        name,
        publisher.length > 0 ? publisher : null,
        installLocation.length > 0 ? installLocation : null,
      );
      if (activeDriveScope.current !== scanScope) return;
      setReview({ appId: report.appId, report: narrowReport(report, driveLetters) });
      // Explicit scans never pre-select anything, even an exact supplied path.
      setSelected(new Set());
    } catch (caught) {
      setManualError(errorMessage(caught));
    } finally {
      setManualBusy(false);
    }
  };

  const useKnownPaths = (record: AppHistoryRecord) => {
    setManualName(record.name);
    setManualPublisher(record.publisher ?? "");
    setManualPath(record.installLocation ?? "");
    setManualError(null);
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

  const removedHistory = (history ?? []).filter((record) => record.removedAt !== null);
  const evidenceCount = (apps?.length ?? 0) + removedHistory.length;
  const sessionIds = new Set((apps ?? []).map((app) => app.id));

  return (
    <details className="cl-remnants-block">
      <summary className="cl-remnants-block__summary">
        <span className="cl-remnants-block__title">Removed applications</span>
        <span className="cl-remnants-block__detail">
          Review filesystem evidence from a verified removal, or scan for an app removed before Cyron
          was installed. Registry review stays in Applications.
        </span>
        <span className="cl-remnants-block__meta">
          {evidenceCount > 0
            ? `${evidenceCount} removal record${evidenceCount === 1 ? "" : "s"}`
            : "No removal evidence yet"}
        </span>
      </summary>
      <div className="cl-remnants-block__body">
        <Panel
          title="Known removed applications"
          subtitle="Only reviewed removal evidence is scanned. Name-only matches stay caution and are never selected automatically."
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
            No verified app removals in this session. Cyron does not infer stale installers or app remnants from old file names or age; use the explicit scan below only when you know the application. Windows Installer data stays under Windows management.
          </p>
        ) : (
          <ul className="cl-remnants__apps">
            {apps.map((app) => (
              <li key={app.id}>
                <div>
                  <strong>{app.name}</strong>
                  <span>{app.publisher || "Publisher unknown"}</span>
                  <small>
                    {app.hive === "history" ? "Persistent lifecycle record" : "Removed this session"} · install evidence:{" "}
                    {app.installLocation || "registered app identity"}
                  </small>
                </div>
                <Button size="sm" loading={busy === app.id} disabled={busy !== null || driveLetters.length === 0} onClick={() => void scan(app)}>
                  Review remnants
                </Button>
              </li>
            ))}
          </ul>
        )}

        <section aria-label="Persistent removal history">
          <h4>Persistent removal history</h4>
          <p className="cl-muted">
            These records survive restarts. History is evidence that an application was removed, not proof that any matching folder or registry key belongs to it.
          </p>
          {history === null ? (
            <p className="cl-muted">Loading removal history…</p>
          ) : removedHistory.length === 0 ? (
            <p className="cl-muted">No stored removals yet. Installing and refreshing the app list starts the lifecycle record.</p>
          ) : (
            <ul className="cl-remnants__apps">
              {removedHistory.slice(0, 20).map((record) => (
                <li key={`history-${record.id}`}>
                  <div>
                    <strong>{record.name}</strong>
                    <span>{record.publisher || "Publisher unknown"}</span>
                    <small>
                      Removed {record.removedAt ? formatDateTime(record.removedAt) : "at an unknown time"}
                      {record.removalMethod ? ` · ${record.removalMethod}` : ""}
                      {" · "}
                      install evidence: {record.installLocation || "name only"}
                    </small>
                  </div>
                  {!sessionIds.has(record.id) && record.installLocation ? (
                    <Button size="sm" variant="secondary" onClick={() => useKnownPaths(record)}>
                      Review known paths
                    </Button>
                  ) : null}
                </li>
              ))}
            </ul>
          )}
        </section>

        <section aria-label="Scan an application removed earlier">
          <h4>Scan an app removed earlier</h4>
          <p className="cl-muted">
            Use a name you know, plus a publisher or exact install path when you have one. Name-only matches remain caution and are never selected automatically; an exact path still needs your review. Cyron never deletes files from this step — selected candidates move to quarantine.
          </p>
          <div className="cl-remnants__form">
            <label className="cl-remnants__field">
              <span>Application name (required)</span>
              <input
                value={manualName}
                onChange={(event) => setManualName(event.target.value)}
                placeholder="Example: Old Photo Editor"
                disabled={manualBusy}
              />
            </label>
            <div className="cl-remnants__row">
              <label className="cl-remnants__field">
                <span>Publisher (optional)</span>
                <input
                  value={manualPublisher}
                  onChange={(event) => setManualPublisher(event.target.value)}
                  placeholder="Example: Contoso"
                  disabled={manualBusy}
                />
              </label>
              <label className="cl-remnants__field">
                <span>Exact install path (optional)</span>
                <input
                  value={manualPath}
                  onChange={(event) => setManualPath(event.target.value)}
                  placeholder="D:\\Apps\\Old Photo Editor"
                  disabled={manualBusy}
                />
              </label>
            </div>
            {manualError && <Banner tone="danger">{manualError}</Banner>}
            <div className="cl-remnants__actions">
              <Button
                size="sm"
                variant="secondary"
                loading={manualBusy}
                disabled={!manualName.trim() || manualBusy || busy !== null || driveLetters.length === 0}
                onClick={() => void scanNamed()}
              >
                Scan explicit evidence
              </Button>
            </div>
          </div>
        </section>

        {review && (
          <section className="cl-remnants__review" aria-label={`Remnants for ${review.report.appName}`}>
            <div className="cl-remnants__summary">
              <strong>{review.report.appName}</strong>
              <span>{review.report.candidates.length} candidates · {formatBytes(review.report.totalSize)}</span>
              <span>Review expires {formatDateTime(review.report.expiresAt)}</span>
            </div>
            {review.report.candidates.length === 0 ? (
              <p className="cl-muted">
                No filesystem candidates under the selected drives met the evidence rules. Registry candidates and other drives remain available through Applications review.
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
      </div>
    </details>
  );
}
