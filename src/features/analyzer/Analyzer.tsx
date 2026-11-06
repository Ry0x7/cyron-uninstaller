import { useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {
  IconChevronRight,
  IconDrive,
  IconExternal,
  IconFolder,
  IconRefresh,
  IconStop,
} from "../../components/Icons";
import {
  Banner,
  Button,
  EmptyState,
  KeyValue,
  Panel,
  ProgressBar,
  Segmented,
  Skeleton,
  Stat,
  ViewHeader,
  cx,
} from "../../components/ui";
import { api, errorMessage } from "../../lib/api";
import {
  CATEGORY_COLOR,
  CATEGORY_LABEL,
  categoryForExtension,
  categoryForFile,
  type CategoryKey,
} from "../../lib/categories";
import {
  formatBytes,
  formatCount,
  formatDate,
  formatDuration,
  formatPercent,
  parentPath,
  truncateMiddle,
} from "../../lib/format";
import { useAsync } from "../../lib/hooks";
import { useSettings } from "../../lib/settings";
import { SCAN_EVENT, type ScanNode, type ScanProgress, type ScanResult } from "../../lib/types";
import { Treemap } from "./Treemap";
import { TreeView } from "./TreeView";
import { AnalyzerFilterBar } from "./AnalyzerFilters";
import { DuplicatesPanel } from "./DuplicatesPanel";
import { StoragePanel } from "./StoragePanel";
import {
  defaultFilters,
  filteredTotals,
  matchesFile,
  type AnalyzerFilters,
} from "../../lib/analyzerFilters";
import "./analyzer.css";

const CATEGORY_ORDER: CategoryKey[] = [
  "code",
  "image",
  "video",
  "audio",
  "archive",
  "document",
  "app",
  "other",
];

const VOLUME_KIND: Record<string, string> = {
  fixed: "Local disk",
  removable: "Removable",
  network: "Network",
  optical: "Optical",
  other: "Drive",
};

const RAIL_TABS = [
  { value: "selection" as const, label: "Selection" },
  { value: "largest" as const, label: "Largest" },
  { value: "types" as const, label: "Types" },
];

type RailTab = (typeof RAIL_TABS)[number]["value"];

const MODES = [
  { value: "map" as const, label: "Map" },
  { value: "duplicates" as const, label: "Duplicates" },
  { value: "storage" as const, label: "Storage" },
];

type AnalyzerMode = (typeof MODES)[number]["value"];

const TREE_WIDTH_MIN = 220;
const TREE_WIDTH_MAX = 520;
const TREE_WIDTH_DEFAULT = 300;

function clampTreeWidth(width: number): number {
  if (!Number.isFinite(width)) return TREE_WIDTH_DEFAULT;
  return Math.min(TREE_WIDTH_MAX, Math.max(TREE_WIDTH_MIN, Math.round(width)));
}

function readTreeWidth(): number {
  try {
    const stored = Number(window.localStorage.getItem("cyron.analyzer.treeWidth"));
    return stored ? clampTreeWidth(stored) : TREE_WIDTH_DEFAULT;
  } catch {
    return TREE_WIDTH_DEFAULT;
  }
}

const KIND_LABEL: Record<ScanNode["kind"], string> = {
  dir: "Folder",
  file: "File",
  group: "Grouped items",
};

interface ScanFailure {
  message: string;
  retryable: boolean;
}

/** crypto.randomUUID only exists in secure contexts; the fallback keeps scans working. */
function createJobId(): string {
  if (typeof crypto.randomUUID === "function") return crypto.randomUUID();
  const bytes = new Uint8Array(16);
  if (typeof crypto.getRandomValues === "function") crypto.getRandomValues(bytes);
  else for (let index = 0; index < bytes.length; index += 1) bytes[index] = Math.floor(Math.random() * 256);
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function isAbsoluteWindowsPath(value: string): boolean {
  if (/^[a-zA-Z]:[\\/]/.test(value)) return true;
  // UNC share (\\server\share), including extended-length paths.
  return value.startsWith("\\\\") && value.length > 2 && value[2] !== "\\";
}

export function Analyzer() {
  const { settings } = useSettings();
  const volumes = useAsync(() => api.listVolumes(), []);
  const [phase, setPhase] = useState<"idle" | "scanning" | "result">("idle");
  const [path, setPath] = useState("");
  const [lastPath, setLastPath] = useState("");
  const [result, setResult] = useState<ScanResult | null>(null);
  const [progress, setProgress] = useState<ScanProgress | null>(null);
  const [error, setError] = useState<ScanFailure | null>(null);
  const [stack, setStack] = useState<ScanNode[]>([]);
  const [selected, setSelected] = useState<ScanNode | null>(null);
  const [railTab, setRailTab] = useState<RailTab>("selection");
  const [mode, setMode] = useState<AnalyzerMode>("map");
  const [filters, setFilters] = useState<AnalyzerFilters>(defaultFilters);
  const [treeWidth, setTreeWidth] = useState(readTreeWidth);
  const [treeOpen, setTreeOpen] = useState(false);
  const [elapsed, setElapsed] = useState(0);
  const [cancelling, setCancelling] = useState(false);
  // Frozen at scan start so the result describes the scan that actually ran.
  const [appliedOptions, setAppliedOptions] = useState({
    includeHidden: settings.includeHidden,
    exclusions: settings.exclusions.length,
  });

  const jobRef = useRef<string | null>(null);
  const startedRef = useRef(0);
  const aliveRef = useRef(true);

  useEffect(() => {
    let subscribe = true;
    let unlisten: (() => void) | undefined;
    listen<ScanProgress>(SCAN_EVENT, (event) => {
      const active = jobRef.current;
      if (!active || event.payload.jobId !== active) return;
      // Events are throttled, not sequenced: never let a late event shrink the counts.
      setProgress((previous) =>
        previous
          ? {
              ...event.payload,
              files: Math.max(previous.files, event.payload.files),
              dirs: Math.max(previous.dirs, event.payload.dirs),
              bytes: Math.max(previous.bytes, event.payload.bytes),
              currentDir: event.payload.currentDir || previous.currentDir,
            }
          : event.payload,
      );
    })
      .then((stop) => {
        if (subscribe) unlisten = stop;
        else stop();
      })
      .catch(() => {
        // Events are optional; the scan still returns its full result.
      });
    return () => {
      subscribe = false;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (phase !== "scanning") return;
    const timer = window.setInterval(() => {
      setElapsed(performance.now() - startedRef.current);
    }, 250);
    return () => window.clearInterval(timer);
  }, [phase]);

  // Cancellation normally returns within a directory read. If the backend is
  // slow to settle, re-enable the control instead of disabling it forever.
  useEffect(() => {
    if (!cancelling) return;
    const timer = window.setTimeout(() => setCancelling(false), 8000);
    return () => window.clearTimeout(timer);
  }, [cancelling]);

  useEffect(() => {
    aliveRef.current = true;
    return () => {
      aliveRef.current = false;
      const active = jobRef.current;
      if (active) void api.cancelScan(active).catch(() => undefined);
    };
  }, []);

  const current = stack.length > 0 ? stack[stack.length - 1] : result?.root ?? null;

  const nodesByPath = useMemo(() => {
    const map = new Map<string, ScanNode>();
    if (!result) return map;
    const walk = (node: ScanNode) => {
      if (node.path) map.set(node.path, node);
      node.children.forEach(walk);
    };
    walk(result.root);
    return map;
  }, [result]);

  const visibleTotals = useMemo(
    () => (result ? filteredTotals(result.root, filters) : { bytes: 0, files: 0 }),
    [result, filters],
  );
  const visibleLargest = useMemo(
    () => (result ? result.largestFiles.filter((file) => matchesFile(file, filters)) : []),
    [result, filters],
  );
  const visibleTypes = useMemo(() => {
    if (!result) return [];
    const extension = filters.extension.trim().replace(/^\./, "").toLocaleLowerCase();
    return result.extensionStats.filter((stat) => {
      if (filters.minSize > 0 && stat.size < filters.minSize) return false;
      if (extension && stat.extension.toLocaleLowerCase() !== extension) return false;
      if (filters.category !== "all" && categoryForExtension(stat.extension) !== filters.category) {
        return false;
      }
      return true;
    });
  }, [result, filters]);

  const applyTreeWidth = (next: number) => {
    const width = clampTreeWidth(next);
    setTreeWidth(width);
    try {
      window.localStorage.setItem("cyron.analyzer.treeWidth", String(width));
    } catch {
      // Persistence is optional.
    }
  };

  const runScan = async (target: string) => {
    const trimmed = target.trim();
    if (!trimmed) {
      setError({ message: "Choose a folder or drive before scanning.", retryable: false });
      return;
    }
    if (!isAbsoluteWindowsPath(trimmed)) {
      setError({
        message: "Enter an absolute Windows path, for example C:\\Users\\you\\Downloads.",
        retryable: false,
      });
      return;
    }

    // A second scan must stop the first one; otherwise two walks compete for disk.
    const previousJob = jobRef.current;
    if (previousJob) void api.cancelScan(previousJob).catch(() => undefined);

    const jobId = createJobId();
    jobRef.current = jobId;
    startedRef.current = performance.now();
    setPhase("scanning");
    setProgress(null);
    setResult(null);
    setError(null);
    setSelected(null);
    setStack([]);
    setElapsed(0);
    setCancelling(false);
    setAppliedOptions({
      includeHidden: settings.includeHidden,
      exclusions: settings.exclusions.length,
    });
    setLastPath(trimmed);
    setMode("map");

    try {
      const scan = await api.startScan(jobId, trimmed, {
        includeHidden: settings.includeHidden,
        exclusions: settings.exclusions,
        collectTree: true,
        maxDepth: 48,
        method: "auto",
      });
      if (!aliveRef.current || jobRef.current !== jobId) return;
      jobRef.current = null;
      setProgress(null);
      setCancelling(false);
      setResult(scan);
      setStack([scan.root]);
      setPath(scan.rootPath);
      setPhase("result");
    } catch (scanError) {
      if (!aliveRef.current || jobRef.current !== jobId) return;
      jobRef.current = null;
      setProgress(null);
      setCancelling(false);
      setPhase("idle");
      setError({ message: errorMessage(scanError), retryable: true });
    }
  };

  const cancel = () => {
    const jobId = jobRef.current;
    if (!jobId || cancelling) return;
    setCancelling(true);
    api.cancelScan(jobId).catch((cancelError: unknown) => {
      setCancelling(false);
      setError({ message: errorMessage(cancelError), retryable: false });
    });
  };

  const chooseFolder = async () => {
    try {
      const selection = await open({ directory: true, multiple: false, title: "Scan a folder" });
      const picked = Array.isArray(selection) ? selection[0] : selection;
      if (typeof picked !== "string" || !picked.trim()) return;
      const target = picked.trim();
      setPath(target);
      void runScan(target);
    } catch (pickerError) {
      setError({ message: errorMessage(pickerError), retryable: false });
    }
  };

  const reveal = async (target: string) => {
    if (!target) return;
    try {
      await api.revealPath(target);
    } catch (revealError) {
      setError({ message: errorMessage(revealError), retryable: false });
    }
  };

  const selectNode = (node: ScanNode) => {
    setSelected(node);
    setRailTab("selection");
  };

  const chainToNode = (node: ScanNode): ScanNode[] => {
    const chain: ScanNode[] = [];
    const find = (currentNode: ScanNode): boolean => {
      chain.push(currentNode);
      if (currentNode === node || (node.path && currentNode.path.toLocaleLowerCase() === node.path.toLocaleLowerCase())) return true;
      for (const child of currentNode.children) {
        if (find(child)) return true;
      }
      chain.pop();
      return false;
    };
    return result && find(result.root) ? chain : [];
  };

  const drill = (node: ScanNode) => {
    if (node.kind !== "dir" || node.children.length === 0) return;
    const chain = chainToNode(node);
    if (chain.length) setStack(chain);
    setSelected(node);
  };

  const selectTreeNode = (node: ScanNode) => {
    setSelected(node);
    setRailTab("selection");
    const chain = chainToNode(node);
    if (chain.length) {
      // Keep the selected item visible in the treemap by showing its parent
      // folder; the selection itself is highlighted there when it is a tile.
      setStack(chain.length > 1 ? chain.slice(0, -1) : chain);
    }
  };

  const selectFileParent = (filePath: string) => {
    // The deep tree is summarized, so the exact parent may be absent. Walk up
    // until a node the treemap actually contains is found instead of doing
    // nothing when a largest file is clicked.
    let parent = parentPath(filePath);
    while (parent) {
      const node = nodesByPath.get(parent);
      if (node) {
        selectNode(node);
        return;
      }
      parent = parentPath(parent);
    }
    setSelected(null);
    setRailTab("selection");
  };

  const activeExclusions = settings.exclusions.length;

  const headerActions =
    phase === "scanning" ? (
      <Button
        variant="secondary"
        icon={cancelling ? undefined : <IconStop size={14} />}
        loading={cancelling}
        onClick={cancel}
      >
        {cancelling ? "Cancelling" : "Cancel scan"}
      </Button>
    ) : (
      <>
        {result && (
          <Button
            icon={<IconRefresh size={15} />}
            onClick={() => void runScan(lastPath || result.rootPath)}
          >
            Rescan
          </Button>
        )}
        <Button variant="primary" icon={<IconFolder size={15} />} onClick={() => void chooseFolder()}>
          Choose folder
        </Button>
      </>
    );

  return (
    <>
      <ViewHeader
        title="Disk Analyzer"
        description="See where the space actually went. Cyron reads file metadata only — nothing is moved, renamed or deleted."
        actions={headerActions}
      />
      <div className="view-body">
        {error && (
          <Banner
            tone="danger"
            actions={
              <>
                {error.retryable && lastPath && (
                  <Button size="sm" variant="ghost" onClick={() => void runScan(lastPath)}>
                    Retry scan
                  </Button>
                )}
                <Button size="sm" variant="ghost" onClick={() => setError(null)}>
                  Dismiss
                </Button>
              </>
            }
          >
            {error.message}
          </Banner>
        )}

        {phase === "scanning" && (
          <ScanningPanel progress={progress} elapsed={elapsed} options={appliedOptions} />
        )}

        {phase === "idle" && (
          <div className="az-start">
            <Panel
              title="Volumes"
              subtitle="Choose a drive to map everything on it."
              className="az-start__volumes"
              flush
            >
              {volumes.status === "loading" && (
                <div className="az-volume-list">
                  {[0, 1, 2].map((row) => (
                    <div className="az-volume az-volume--skeleton" key={row}>
                      <Skeleton width={34} height={34} />
                      <div className="stack">
                        <Skeleton width={150} height={12} />
                        <Skeleton width={220} height={9} />
                      </div>
                    </div>
                  ))}
                </div>
              )}
              {volumes.status === "error" && (
                <EmptyState
                  compact
                  icon={IconDrive}
                  title="Volumes could not be read"
                  description={volumes.error ?? "The desktop backend returned an error."}
                  action={
                    <Button size="sm" onClick={volumes.reload}>
                      Try again
                    </Button>
                  }
                />
              )}
              {volumes.status === "ready" && (volumes.data?.length ?? 0) === 0 && (
                <EmptyState
                  compact
                  icon={IconDrive}
                  title="No volumes found"
                  description="Windows did not report any mounted drives. You can still scan a folder by path."
                />
              )}
              {volumes.status === "ready" && (volumes.data?.length ?? 0) > 0 && (
                <div className="az-volume-list">
                  {volumes.data?.map((volume) => {
                    const used = Math.max(volume.total - volume.free, 0);
                    const ratio = volume.total > 0 ? used / volume.total : 0;
                    return (
                      <div className="az-volume" key={volume.mount}>
                        <span className="az-volume__icon">
                          <IconDrive size={17} />
                        </span>
                        <div className="az-volume__info">
                          <div className="az-volume__title">
                            <span className="az-volume__mount">{volume.mount}</span>
                            {volume.label && <span className="az-volume__label">{volume.label}</span>}
                            <span className="az-volume__kind">
                              {VOLUME_KIND[volume.kind] ?? "Drive"} · {volume.fileSystem || "unknown"}
                            </span>
                          </div>
                          <div className="az-volume__bar">
                            <ProgressBar value={ratio} label={`${volume.mount} usage`} />
                            <div className="az-volume__figures">
                              <span>{formatBytes(used)} used</span>
                              <span className="faint">{formatBytes(volume.free)} free</span>
                              <span className="faint">{formatBytes(volume.total)} total</span>
                            </div>
                          </div>
                        </div>
                        <Button size="sm" onClick={() => void runScan(volume.mount)}>
                          Scan
                        </Button>
                      </div>
                    );
                  })}
                </div>
              )}
            </Panel>

            <Panel title="Scan a folder" subtitle="Folders are measured recursively." className="az-start__path">
              <div className="az-path">
                <Button variant="primary" icon={<IconFolder size={15} />} onClick={() => void chooseFolder()}>
                  Choose folder…
                </Button>
                <form
                  className="az-path__form"
                  onSubmit={(event) => {
                    event.preventDefault();
                    void runScan(path);
                  }}
                >
                  <input
                    className="input az-path__input"
                    value={path}
                    onChange={(event) => setPath(event.target.value)}
                    placeholder={"C:\\Users\\you\\Downloads"}
                    aria-label="Folder path to scan"
                    spellCheck={false}
                    autoComplete="off"
                  />
                  <Button type="submit" disabled={!path.trim()}>
                    Scan
                  </Button>
                </form>
                <ul className="az-notes">
                  <li>Read-only: sizes come from filesystem metadata.</li>
                  <li>Junctions and symlinks are skipped, so loops and double counts are avoided.</li>
                  <li>
                    {activeExclusions > 0
                      ? `${formatCount(activeExclusions)} exclusion${activeExclusions === 1 ? "" : "s"} from Settings will be skipped.`
                      : "No folder exclusions are set in Settings."}{" "}
                    {settings.includeHidden
                      ? "Hidden and system files are included."
                      : "Hidden and system files are skipped."}
                  </li>
                </ul>
              </div>
            </Panel>
          </div>
        )}

        {phase === "result" && result && current && (
          <>
            {result.cancelled && (
              <Banner
                tone="caution"
                actions={
                  <Button size="sm" onClick={() => void runScan(lastPath || result.rootPath)}>
                    Scan again
                  </Button>
                }
              >
                <strong>Scan cancelled.</strong> These figures only cover what was measured before
                you stopped it; the tree is a partial snapshot, not a broken scan.
              </Banner>
            )}
            {result.errors.length > 0 && (
              <Banner tone="caution">
                <strong>
                  {formatCount(result.errors.length)} folder
                  {result.errors.length === 1 ? "" : "s"} could not be read.
                </strong>{" "}
                Sizes for those folders are incomplete.{" "}
                <span className="faint">{result.errors[0]}</span>
              </Banner>
            )}
            {result.truncated && (
              <Banner tone="info">
                This tree is summarized at the deepest levels so the view stays responsive. Sizes are
                still complete.
              </Banner>
            )}

            <div className="az-summary">
              <Stat label="Measured" value={formatBytes(result.totalSize)} meta="logical size" />
              <Stat label="On disk" value={formatBytes(result.totalAllocated)} meta="allocated" />
              <Stat label="Files" value={formatCount(result.fileCount)} />
              <Stat label="Folders" value={formatCount(result.dirCount)} />
              <Stat
                label="Hard links"
                value={formatCount(result.dedupedFiles)}
                meta="bytes counted once"
              />
              <Stat
                label="Reparse points"
                value={formatCount(result.skippedReparse)}
                meta="junctions not followed"
              />
              <Stat label="Took" value={formatDuration(result.elapsedMs)} meta="wall clock" />
            </div>

            <p className="az-scan-note">
              {appliedOptions.includeHidden
                ? "Hidden and system files included"
                : "Hidden and system files skipped"}
              {appliedOptions.exclusions > 0 &&
                ` · ${formatCount(appliedOptions.exclusions)} exclusion${appliedOptions.exclusions === 1 ? "" : "s"} applied`}
              {" · "}logical size and allocated size are reported separately.
            </p>

            <div className="az-modes">
              <Segmented
                size="sm"
                options={MODES}
                value={mode}
                onChange={(value) => setMode(value as AnalyzerMode)}
                ariaLabel="Analyzer mode"
              />
            </div>

            {mode === "duplicates" && (
              <DuplicatesPanel
                rootPath={result.rootPath}
                includeHidden={appliedOptions.includeHidden}
                exclusions={settings.exclusions}
              />
            )}

            {mode === "storage" && <StoragePanel result={result} />}

            {mode === "map" && (
              <>
                <AnalyzerFilterBar
                  filters={filters}
                  onChange={setFilters}
                  visible={visibleTotals}
                  total={{ bytes: result.totalSize, files: result.fileCount }}
                  method={result.method}
                  methodNote={result.methodNote}
                  treeOpen={treeOpen}
                  onToggleTree={() => setTreeOpen((open) => !open)}
                />

                <div
                  className={cx("az-split", treeOpen && "az-split--tree-open")}
                  style={{ "--az-tree-width": `${treeWidth}px` } as React.CSSProperties}
                >
                  <aside className="az-split__tree" aria-label="Folder tree">
                    <div className="az-tree-panel">
                      <div className="az-tree-panel__head">
                        <span>Folder tree</span>
                        <span className="az-tree-panel__meta">
                          {formatCount(visibleTotals.files)} files
                        </span>
                      </div>
                      <TreeView
                        root={result.root}
                        selectedNode={selected}
                        currentPath={current.path}
                        filters={filters}
                        onSelect={selectTreeNode}
                        onDrill={drill}
                      />
                    </div>
                  </aside>

                  <div
                    className="az-split__divider"
                    role="separator"
                    aria-orientation="vertical"
                    aria-label="Resize folder tree"
                    aria-valuemin={TREE_WIDTH_MIN}
                    aria-valuemax={TREE_WIDTH_MAX}
                    aria-valuenow={treeWidth}
                    tabIndex={0}
                    onPointerDown={(event) => {
                      event.preventDefault();
                      const startX = event.clientX;
                      const startWidth = treeWidth;
                      const target = event.currentTarget;
                      target.setPointerCapture(event.pointerId);
                      const move = (moveEvent: PointerEvent) =>
                        applyTreeWidth(startWidth + moveEvent.clientX - startX);
                      const stop = () => {
                        target.removeEventListener("pointermove", move);
                        target.removeEventListener("pointerup", stop);
                        target.removeEventListener("pointercancel", stop);
                      };
                      target.addEventListener("pointermove", move);
                      target.addEventListener("pointerup", stop);
                      target.addEventListener("pointercancel", stop);
                    }}
                    onKeyDown={(event) => {
                      if (event.key === "ArrowLeft") {
                        event.preventDefault();
                        applyTreeWidth(treeWidth - 16);
                      } else if (event.key === "ArrowRight") {
                        event.preventDefault();
                        applyTreeWidth(treeWidth + 16);
                      } else if (event.key === "Home") {
                        event.preventDefault();
                        applyTreeWidth(TREE_WIDTH_MIN);
                      } else if (event.key === "End") {
                        event.preventDefault();
                        applyTreeWidth(TREE_WIDTH_MAX);
                      }
                    }}
                  >
                    <span className="az-split__grip" aria-hidden="true" />
                  </div>

                  <div className="az-grid az-grid--split">
                    <div className="az-main">
                      <Panel
                        className="az-tree"
                  flush
                  title={
                    <nav className="az-breadcrumb" aria-label="Folder path">
                      {stack.map((node, index) => (
                        <span className="az-crumb-wrap" key={`${node.path}-${index}`}>
                          {index > 0 && <IconChevronRight size={12} className="az-crumb-sep" />}
                          <button
                            type="button"
                            className={cx("az-crumb", index === stack.length - 1 && "az-crumb--current")}
                            aria-current={index === stack.length - 1 ? "true" : undefined}
                            title={node.path || node.name}
                            onClick={() => {
                              setStack((previous) => previous.slice(0, index + 1));
                              setSelected(node);
                            }}
                          >
                            {node.name || node.path}
                          </button>
                        </span>
                      ))}
                    </nav>
                  }
                  actions={
                    <>
                      <span className="az-tree__count">
                        {formatCount(current.fileCount)} files
                      </span>
                      <Button
                        size="sm"
                        variant="ghost"
                        icon={<IconExternal size={14} />}
                        disabled={!current.path}
                        onClick={() => void reveal(current.path)}
                      >
                        Open in Explorer
                      </Button>
                    </>
                  }
                >
                  <div className="az-canvas">
                    <Treemap
                      node={current}
                      selectedPath={selected?.path ?? ""}
                      onSelect={selectNode}
                      onDrill={drill}
                    />
                  </div>
                  <div className="az-legend">
                    {CATEGORY_ORDER.map((category) => (
                      <span className="az-legend__item" key={category}>
                        <span
                          className="az-legend__dot"
                          style={{ background: CATEGORY_COLOR[category] }}
                        />
                        {CATEGORY_LABEL[category]}
                      </span>
                    ))}
                    <span className="az-legend__item">
                      <span className="az-legend__dot az-legend__dot--folder" />
                      Folders
                    </span>
                  </div>
                </Panel>
              </div>

              <aside className="az-rail">
                <Panel flush className="az-rail-panel">
                  <div className="az-rail__tabs">
                    <Segmented
                      size="sm"
                      options={RAIL_TABS}
                      value={railTab}
                      onChange={setRailTab}
                      ariaLabel="Analyzer details"
                    />
                  </div>
                  <div
                    className={cx(
                      "az-rail__body",
                      railTab !== "selection" && "az-rail__body--flush",
                    )}
                  >
                    {railTab === "selection" && (
                      <SelectionDetails
                        node={selected ?? current}
                        viewNode={current}
                        totalSize={result.totalSize}
                        onDrill={drill}
                        onReveal={reveal}
                      />
                    )}

                    {railTab === "largest" && (
                      <div className="az-list" role="list">
                        {visibleLargest.length === 0 && (
                          <EmptyState
                            compact
                            icon={IconDrive}
                            title="No large files"
                            description={
                              result.largestFiles.length === 0
                                ? "This scan did not record any sizable files."
                                : "No recorded large file matches the current filters."
                            }
                          />
                        )}
                        {visibleLargest.slice(0, 40).map((file) => (
                          <button
                            type="button"
                            className="az-list__row"
                            key={file.path}
                            onClick={() => selectFileParent(file.path)}
                            title={`${file.path} — ${formatBytes(file.size)}`}
                          >
                            <span
                              className="az-row__dot"
                              style={{ background: CATEGORY_COLOR[categoryForFile(file.name)] }}
                            />
                            <span className="az-row__text">
                              <span className="az-row__name">{file.name}</span>
                              <span className="az-row__meta" title={file.path}>
                                {truncateMiddle(file.path, 46)}
                              </span>
                            </span>
                            <span className="az-row__size">{formatBytes(file.size)}</span>
                          </button>
                        ))}
                        {visibleLargest.length > 40 && (
                          <p className="az-list__foot">
                            Showing the 40 largest of {formatCount(visibleLargest.length)} matching
                            files. The backend returns the top 100 files from the walk.
                          </p>
                        )}
                      </div>
                    )}

                    {railTab === "types" && (
                      <div className="az-types" role="list">
                        {visibleTypes.length === 0 && (
                          <EmptyState
                            compact
                            icon={IconDrive}
                            title="No file types"
                            description={
                              result.extensionStats.length === 0
                                ? "Nothing measurable was found in this scan."
                                : "No recorded file type matches the current filters."
                            }
                          />
                        )}
                        {visibleTypes.map((stat) => {
                          const category = categoryForExtension(stat.extension);
                          const share = result.totalSize > 0 ? stat.size / result.totalSize : 0;
                          return (
                            <div className="az-type" key={stat.extension || "no-extension"}>
                              <span
                                className="az-row__dot"
                                style={{ background: CATEGORY_COLOR[category] }}
                              />
                              <span className="az-type__name">
                                {stat.extension ? `.${stat.extension}` : "no extension"}
                              </span>
                              <span className="az-type__count">{formatCount(stat.fileCount)} files</span>
                              <span className="az-type__size">{formatBytes(stat.size)}</span>
                              <span className="az-type__bar">
                                <span
                                  style={{
                                    width: `${Math.min(share * 100, 100)}%`,
                                    background: CATEGORY_COLOR[category],
                                  }}
                                />
                              </span>
                            </div>
                          );
                        })}
                      </div>
                    )}
                  </div>
                </Panel>
              </aside>
                  </div>
                </div>
              </>
            )}
          </>
        )}
      </div>
    </>
  );
}

function ScanningPanel({
  progress,
  elapsed,
  options,
}: {
  progress: ScanProgress | null;
  elapsed: number;
  options: { includeHidden: boolean; exclusions: number };
}) {
  return (
    <Panel className="az-scanning" title="Scanning" subtitle="Reading file metadata in the background.">
      <div className="az-scanning__body">
        <div className="az-scanning__path" title={progress?.currentDir ?? ""}>
          {progress?.currentDir ? truncateMiddle(progress.currentDir, 92) : "Starting scan…"}
        </div>
        <ProgressBar label="Scanning folders" />
        <div className="az-live" aria-busy="true">
          <div>
            <span className="stat__label">Files</span>
            <span className="az-live__value">{formatCount(progress?.files ?? 0)}</span>
          </div>
          <div>
            <span className="stat__label">Folders</span>
            <span className="az-live__value">{formatCount(progress?.dirs ?? 0)}</span>
          </div>
          <div>
            <span className="stat__label">Measured</span>
            <span className="az-live__value">{formatBytes(progress?.bytes ?? 0)}</span>
          </div>
          <div>
            <span className="stat__label">Elapsed</span>
            <span className="az-live__value">{formatDuration(elapsed)}</span>
          </div>
        </div>
        <p className="az-scan-options">
          {options.includeHidden
            ? "Hidden and system files included"
            : "Hidden and system files skipped"}
          {options.exclusions > 0 &&
            ` · ${formatCount(options.exclusions)} exclusion${options.exclusions === 1 ? "" : "s"} active`}
        </p>
        <p className="small muted">
          Large folders can take a while. The window stays responsive and you can cancel at any
          time.
        </p>
      </div>
    </Panel>
  );
}

interface SelectionProps {
  node: ScanNode;
  viewNode: ScanNode;
  totalSize: number;
  onDrill: (node: ScanNode) => void;
  onReveal: (path: string) => void;
}

function SelectionDetails({ node, viewNode, totalSize, onDrill, onReveal }: SelectionProps) {
  const isView = node.path === viewNode.path;
  // Share percentages are always based on logical bytes; allocated bytes are
  // presented separately and must not substitute when logical size is zero.
  const logical = node.size;
  const viewTotal = viewNode.size;
  const denominator = isView ? totalSize : viewTotal;
  const share = denominator > 0 ? logical / denominator : 0;
  const canDrill = node.kind === "dir" && node.children.length > 0 && !isView;

  return (
    <div className="az-node">
      <div className="az-node__head">
        <span className={cx("badge", node.kind === "file" ? "badge--accent" : "badge--neutral")}>
          {KIND_LABEL[node.kind]}
        </span>
        {isView && <span className="az-node__hint">This view</span>}
      </div>
      <span className="az-node__name">{node.name}</span>
      <span className="az-node__path" title={node.path || undefined}>
        {node.path || "Synthetic group of smaller items"}
      </span>

      <KeyValue
        items={[
          { label: "Size", value: formatBytes(node.size) },
          { label: "On disk", value: formatBytes(node.allocated) },
          { label: isView ? "Share of scan" : "Share of view", value: formatPercent(share) },
          { label: "Files", value: formatCount(node.fileCount) },
          { label: "Folders", value: formatCount(node.dirCount) },
          { label: "Modified", value: formatDate(node.modified) },
        ]}
      />

      <div className="az-node__actions">
        {canDrill && (
          <Button size="sm" onClick={() => onDrill(node)}>
            Drill into folder
          </Button>
        )}
        {node.path && (
          <Button
            size="sm"
            variant="ghost"
            icon={<IconExternal size={13} />}
            onClick={() => onReveal(node.path)}
          >
            Open in Explorer
          </Button>
        )}
      </div>

      <div className="az-bar-block">
        <div className="spread small muted">
          <span>{isView ? "Share of scan" : "Share of this view"}</span>
          <span className="mono">{formatPercent(share)}</span>
        </div>
        <span className="az-share-bar">
          <span
            style={{
              width: `${Math.min(share * 100, 100)}%`,
              background:
                node.kind === "file" ? CATEGORY_COLOR[categoryForFile(node.name)] : "var(--accent)",
            }}
          />
        </span>
      </div>

      <p className="small faint">
        Sizes are logical unless marked on disk. Junctions and reparse points are never followed.
      </p>
    </div>
  );
}
