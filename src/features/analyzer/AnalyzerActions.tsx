import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Banner, Button, Drawer, KeyValue, ProgressBar } from "../../components/ui";
import { api, errorMessage, invalidateCaches } from "../../lib/api";
import { formatBytes, formatCount, formatDateTime } from "../../lib/format";
import type { AnalyzerAction, AnalyzerActionPreview, AnalyzerActionProgress, AnalyzerActionReport, ScanNode } from "../../lib/types";

export function AnalyzerActions({ selected, onChanged, disabled = false, scope }: {
  selected: ScanNode | null;
  onChanged: () => void;
  disabled?: boolean;
  scope: string;
}) {
  const [paths, setPaths] = useState<string[]>([]);
  const [open, setOpen] = useState(false);
  const [operation, setOperation] = useState<AnalyzerAction>("recycle");
  const [preview, setPreview] = useState<AnalyzerActionPreview | null>(null);
  const [report, setReport] = useState<AnalyzerActionReport | null>(null);
  const [progress, setProgress] = useState<AnalyzerActionProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [running, setRunning] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [confirmed, setConfirmed] = useState(false);
  const planRef = useRef<string | null>(null);
  const generation = useRef(0);

  // A rescan/root change invalidates both the selection basket and any pending review.
  useEffect(() => {
    generation.current += 1;
    setPaths([]);
    setPreview(null);
  }, [disabled, scope]);

  useEffect(() => {
    let active = true;
    let stop: (() => void) | undefined;
    listen<AnalyzerActionProgress>("cyron-analyzer-action-progress", ({ payload }) => {
      if (payload.planId === planRef.current) setProgress(payload);
    }).then((unlisten) => { if (active) stop = unlisten; else unlisten(); }).catch(() => undefined);
    return () => { active = false; stop?.(); };
  }, []);

  const review = async (operation: AnalyzerAction) => {
    if (!paths.length || busy || running) return;
    const version = ++generation.current;
    setOperation(operation);
    setOpen(true);
    setBusy(true);
    setPreview(null);
    setReport(null);
    setError(null);
    setConfirmed(false);
    try {
      const next = await api.prepareAnalyzerAction(paths, operation);
      if (version === generation.current) setPreview(next);
    } catch (err) {
      if (version === generation.current) setError(errorMessage(err));
    } finally { setBusy(false); }
  };

  const execute = async () => {
    if (!preview || !confirmed || running) return;
    planRef.current = preview.planId;
    setRunning(true);
    setError(null);
    setProgress(null);
    setReport(null);
    try { setReport(await api.executeAnalyzerAction(preview.planId)); }
    catch (err) { setError(errorMessage(err)); }
    finally {
      // Even an interrupted IPC may have changed files; discard every old snapshot.
      setRunning(false);
      setCancelling(false);
      setPreview(null);
      setPaths([]);
      planRef.current = null;
      invalidateCaches();
      onChanged();
    }
  };

  const cancel = async () => {
    if (!planRef.current || cancelling) return;
    setCancelling(true);
    try { await api.cancelAnalyzerAction(planRef.current); }
    catch (err) { setError(errorMessage(err)); setCancelling(false); }
  };

  return <>
    <div className="az-actions" style={disabled ? { display: "none" } : undefined} role="region" aria-label="Selected filesystem items">
      <Button size="sm" disabled={disabled || busy || running || !selected?.path || selected.kind === "group"}
        onClick={() => selected?.path && setPaths((previous) => [...new Set([...previous, selected.path])])}>
        Add selected item
      </Button>
      <span>{formatCount(paths.length)} selected</span>
      <Button size="sm" disabled={disabled || !paths.length || busy || running} onClick={() => void review("recycle")}>Move to Recycle Bin…</Button>
      <Button size="sm" variant="danger" disabled={disabled || !paths.length || busy || running} onClick={() => void review("permanent")}>Permanently delete…</Button>
      {paths.length > 0 && <Button size="sm" variant="ghost" disabled={busy || running} onClick={() => setPaths([])}>Clear selection</Button>}
      <p className="small muted">Select an item in the listing or treemap, then add it. Folders include their contents.</p>
      {paths.length > 0 && <details><summary>Review selected paths</summary><ul>{paths.map((path) => <li key={path}><span className="mono">{path}</span>{" "}<Button size="sm" variant="ghost" disabled={busy || running} aria-label={`Exclude ${path}`} onClick={() => setPaths((previous) => previous.filter((entry) => entry !== path))}>Exclude</Button></li>)}</ul></details>}
    </div>
    <Drawer open={open} title={report ? "Filesystem results" : operation === "permanent" ? "Review permanent deletion" : "Review Recycle Bin move"}
      onClose={() => { if (!running) { generation.current += 1; setOpen(false); } }}
      footer={running ? <Button onClick={() => void cancel()} disabled={cancelling}>{cancelling ? "Stopping safely…" : "Cancel remaining items"}</Button> : <>
        {preview && <Button variant={preview.operation === "permanent" ? "danger" : "primary"} disabled={!confirmed || busy} onClick={() => void execute()}>{preview.operation === "permanent" ? "Permanently delete" : "Move to Recycle Bin"}</Button>}
        <Button variant="ghost" onClick={() => { generation.current += 1; setOpen(false); }}>Close</Button>
      </>}>
      {busy && <ProgressBar label="Inspecting selected paths" />}
      {error && <Banner tone="danger">{error}</Banner>}
      {preview && !running && <div className="stack">
        <Banner tone="caution">{preview.operation === "permanent" ? "Cyron cannot restore permanently deleted data. This bypasses quarantine and the Recycle Bin." : "Windows manages recovery from the Recycle Bin. Recycling does not free disk space until the bin is emptied. If recycling is unavailable, Cyron reports the failure and does not permanently delete instead."}</Banner>
        <KeyValue items={[
          { label: "Operation", value: preview.operation === "permanent" ? "Permanent deletion" : "Move to Recycle Bin" },
          { label: "Affected items", value: `${formatCount(preview.totalFiles)} files · ${formatCount(preview.totalDirs)} folders` },
          { label: "Estimated logical size", value: formatBytes(preview.totalBytes) },
          { label: "Review expires", value: formatDateTime(preview.expiresAt) },
        ]} />
        <ul>{preview.targets.map((target) => <li key={target.path}><p className="mono az-action-path">{target.path}</p><span className="small">{formatBytes(target.bytes)} · {formatCount(target.files)} files · {formatCount(target.dirs)} folders</span></li>)}</ul>
        {preview.warnings.map((warning) => <Banner key={warning} tone="caution">{warning}</Banner>)}
        <label className="row"><input type="checkbox" checked={confirmed} onChange={(event) => setConfirmed(event.target.checked)} />I reviewed these paths, contents and the selected operation.</label>
      </div>}
      {running && <div className="stack" aria-live="polite"><ProgressBar label="Changing selected items" value={progress?.total ? progress.completed / progress.total : undefined} /><p>{progress ? `${progress.completed} of ${progress.total} items processed` : "Starting reviewed operation…"}</p><p className="mono az-action-path">{progress?.currentPath}</p><p>Cancellation stops between safe operations. Completed changes remain in effect.</p></div>}
      {report && <div className="stack" aria-live="polite">
        <Banner tone={report.cancelled || report.items.some((item) => item.status !== "completed") ? "caution" : "safe"}>{report.cancelled ? "Stopped safely. " : "Operation finished. "}{formatCount(report.completedFiles)} files {report.operation === "recycle" ? "recycled" : "permanently deleted"}; {formatCount(report.completedDirs)} empty folders removed ({formatBytes(report.completedBytes)} logical bytes). Views are being rescanned.</Banner>
        {report.operation === "recycle" && <p>Recycled bytes remain on disk until Windows empties the Recycle Bin.</p>}
        <ul>{report.items.map((item) => <li key={item.path}><p className="mono az-action-path">{item.path}</p><strong>{item.status}</strong>{item.error && <p>{item.error}</p>}</li>)}</ul>
      </div>}
    </Drawer>
  </>;
}
