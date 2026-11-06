import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { IconChevronRight, IconFolder } from "../../components/Icons";
import { formatBytes, formatCount } from "../../lib/format";
import { filterTree, sortNodes, type AnalyzerFilters } from "../../lib/analyzerFilters";
import type { ScanNode } from "../../lib/types";

const ROW_HEIGHT = 30;
const OVERSCAN = 6;
/** A hard ceiling for the flattened list, not a display limit: rows are virtualized. */
const MAX_FLAT_ROWS = 200_000;

interface TreeRow {
  node: ScanNode;
  key: string;
  parentKey: string | null;
  level: number;
  index: number;
  siblings: number;
  hasChildren: boolean;
}

interface TreeViewProps {
  root: ScanNode;
  selectedNode: ScanNode | null;
  currentPath: string;
  filters: AnalyzerFilters;
  onSelect: (node: ScanNode) => void;
  onDrill: (node: ScanNode) => void;
}

function pathKey(path: string): string {
  // Windows paths are case-insensitive; locale-sensitive casing can differ
  // across machines (notably for I/i) and break cross-view identity.
  return path.replace(/[\\/]+$/, "").toLowerCase();
}

function nodeKey(node: ScanNode, parentKey: string | null, index: number): string {
  return node.path ? `path:${pathKey(node.path)}` : `${parentKey ?? "root"}/synthetic-${index}-${node.kind}`;
}

function findAncestors(root: ScanNode, target: ScanNode): string[] {
  const targetPath = target.path ? pathKey(target.path) : "";
  const visit = (
    node: ScanNode,
    parentKey: string | null,
    index: number,
    ancestors: string[],
  ): string[] | null => {
    const key = nodeKey(node, parentKey, index);
    if (node === target || (targetPath && node.path && pathKey(node.path) === targetPath)) {
      return ancestors;
    }
    for (let childIndex = 0; childIndex < node.children.length; childIndex += 1) {
      const found = visit(node.children[childIndex], key, childIndex, [...ancestors, key]);
      if (found) return found;
    }
    return null;
  };
  return visit(root, null, 0, []) ?? [];
}

function flattenVisible(
  root: ScanNode,
  expanded: Set<string>,
  filters: AnalyzerFilters,
): { rows: TreeRow[]; capped: boolean; indexByKey: Map<string, number> } {
  const rows: TreeRow[] = [];
  const indexByKey = new Map<string, number>();
  let capped = false;
  const visit = (node: ScanNode, parentKey: string | null, level: number, index: number, siblings: number) => {
    if (rows.length >= MAX_FLAT_ROWS) {
      capped = true;
      return;
    }
    const key = nodeKey(node, parentKey, index);
    indexByKey.set(key, rows.length);
    rows.push({
      node,
      key,
      parentKey,
      level,
      index,
      siblings,
      hasChildren: node.children.length > 0,
    });
    if (!expanded.has(key) || node.children.length === 0) return;
    const children = sortNodes(node.children, filters);
    for (let childIndex = 0; childIndex < children.length; childIndex += 1) {
      visit(children[childIndex], key, level + 1, childIndex, children.length);
      if (capped) break;
    }
  };
  visit(root, null, 1, 0, 1);
  return { rows, capped, indexByKey };
}

export function TreeView({
  root,
  selectedNode,
  currentPath,
  filters,
  onSelect,
  onDrill,
}: TreeViewProps) {
  const rootKey = nodeKey(root, null, 0);
  const treeId = useMemo(() => `scan-tree-${Math.random().toString(36).slice(2)}`, []);
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set([rootKey]));
  const [focusedKey, setFocusedKey] = useState(rootKey);
  const [viewport, setViewport] = useState({ top: 0, height: 480 });
  const containerRef = useRef<HTMLDivElement>(null);
  const pendingReveal = useRef<string | null>(null);

  const view = useMemo(() => {
    const filtered = filterTree(root, filters, Date.now());
    return flattenVisible(filtered, expanded, filters);
  }, [root, filters, expanded]);
  const { rows, capped, indexByKey } = view;

  const previousRootKey = useRef(rootKey);
  useEffect(() => {
    if (previousRootKey.current === rootKey) return;
    previousRootKey.current = rootKey;
    pendingReveal.current = null;
    setExpanded(new Set([rootKey]));
    setFocusedKey(rootKey);
    if (containerRef.current) containerRef.current.scrollTop = 0;
    setViewport({ top: 0, height: containerRef.current?.clientHeight ?? 480 });
  }, [rootKey]);

  useEffect(() => {
    if (!selectedNode) return;
    const ancestors = findAncestors(root, selectedNode);
    if (!ancestors.length) return;
    setExpanded((previous) => {
      let changed = false;
      const next = new Set(previous);
      ancestors.forEach((key) => {
        if (!next.has(key)) {
          next.add(key);
          changed = true;
        }
      });
      return changed ? next : previous;
    });
    pendingReveal.current = selectedNode.path ? pathKey(selectedNode.path) : null;
  }, [root, selectedNode]);

  const selectedIndex = useMemo(() => {
    if (!selectedNode) return -1;
    const selectedPath = selectedNode.path ? pathKey(selectedNode.path) : "";
    return rows.findIndex(
      (row) =>
        row.node === selectedNode ||
        (selectedPath && row.node.path && pathKey(row.node.path) === selectedPath),
    );
  }, [rows, selectedNode]);

  const focusedIndex = indexByKey.get(focusedKey) ?? 0;

  useEffect(() => {
    const element = containerRef.current;
    if (!element) return;
    const measure = () => setViewport({ top: element.scrollTop, height: element.clientHeight });
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const scrollToIndex = (index: number, block: "nearest" | "center" = "nearest") => {
    const element = containerRef.current;
    if (!element || index < 0) return;
    const top = index * ROW_HEIGHT;
    const bottom = top + ROW_HEIGHT;
    if (block === "center") {
      element.scrollTop = Math.max(0, top - element.clientHeight / 2 + ROW_HEIGHT / 2);
      setViewport({ top: element.scrollTop, height: element.clientHeight });
      return;
    }
    if (top < element.scrollTop) element.scrollTop = top;
    else if (bottom > element.scrollTop + element.clientHeight) {
      element.scrollTop = bottom - element.clientHeight;
    }
    setViewport({ top: element.scrollTop, height: element.clientHeight });
  };

  useEffect(() => {
    if (selectedIndex < 0) return;
    // Only reveal a selection that arrived from another view; keyboard moves stay put.
    if (!pendingReveal.current) return;
    pendingReveal.current = null;
    setFocusedKey(rows[selectedIndex].key);
    scrollToIndex(selectedIndex, "center");
  }, [selectedIndex, rows]);

  const setFocus = (index: number) => {
    const row = rows[index];
    if (!row) return;
    setFocusedKey(row.key);
    scrollToIndex(index);
    containerRef.current?.focus({ preventScroll: true });
  };

  const toggle = (key: string) => {
    setExpanded((previous) => {
      const next = new Set(previous);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (rows.length === 0) return;
    const row = rows[focusedIndex];
    const isExpanded = row ? expanded.has(row.key) : false;
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        setFocus(Math.min(focusedIndex + 1, rows.length - 1));
        break;
      case "ArrowUp":
        event.preventDefault();
        setFocus(Math.max(focusedIndex - 1, 0));
        break;
      case "ArrowRight":
        event.preventDefault();
        if (row?.hasChildren && !isExpanded) toggle(row.key);
        else if (row?.hasChildren) setFocus(Math.min(focusedIndex + 1, rows.length - 1));
        break;
      case "ArrowLeft":
        event.preventDefault();
        if (row?.hasChildren && isExpanded) toggle(row.key);
        else if (row?.parentKey) {
          const parentIndex = rows.findIndex((candidate) => candidate.key === row.parentKey);
          if (parentIndex >= 0) setFocus(parentIndex);
        }
        break;
      case "Home":
        event.preventDefault();
        setFocus(0);
        break;
      case "End":
        event.preventDefault();
        setFocus(rows.length - 1);
        break;
      case "Enter":
      case " ":
        event.preventDefault();
        if (!row) break;
        onSelect(row.node);
        if (event.key === "Enter" && row.node.kind === "dir" && row.node.children.length > 0) {
          onDrill(row.node);
        }
        break;
      default:
        break;
    }
  };

  const start = Math.max(0, Math.floor(viewport.top / ROW_HEIGHT) - OVERSCAN);
  const visibleCount = Math.ceil(viewport.height / ROW_HEIGHT) + OVERSCAN * 2;
  const end = Math.min(rows.length, start + visibleCount);
  const visible = rows.slice(start, end);
  const activeRow = rows[focusedIndex];

  const noMatches = root.children.length > 0 && rows.length <= 1 && rows[0]?.key === rootKey;
  if (rows.length === 0 || noMatches) {
    return (
      <div className="az-tree-view-wrap">
        <p className="az-tree-view__empty">
          {root.children.length === 0
            ? "This scan has no directory tree to display."
            : "No items match the current filters."}
        </p>
      </div>
    );
  }

  return (
    <div className="az-tree-view-wrap">
      <div
        ref={containerRef}
        className="az-tree-view"
        role="tree"
        aria-label="Scanned folder tree"
        aria-activedescendant={activeRow ? `${treeId}-${focusedIndex}` : undefined}
        tabIndex={0}
        onKeyDown={onKeyDown}
        onScroll={(event) => {
          const element = event.currentTarget;
          setViewport({ top: element.scrollTop, height: element.clientHeight });
        }}
      >
        <div className="az-tree-spacer" style={{ height: rows.length * ROW_HEIGHT }}>
          {visible.map((row, offset) => {
            const index = start + offset;
            const { node, key, level, hasChildren } = row;
            const selectedPath = selectedNode?.path ? pathKey(selectedNode.path) : "";
            const isSelected = Boolean(
              selectedNode &&
                (node === selectedNode || (selectedPath && node.path && pathKey(node.path) === selectedPath)),
            );
            const isCurrent = Boolean(
              node.path && currentPath && pathKey(node.path) === pathKey(currentPath),
            );
            const isExpanded = expanded.has(key);
            const share = node.size || node.allocated;
            return (
              <div
                key={key}
                id={`${treeId}-${index}`}
                className={`az-tree-row${isSelected ? " is-selected" : ""}${isCurrent ? " is-current" : ""}`}
                role="treeitem"
                aria-level={level}
                aria-posinset={row.index + 1}
                aria-setsize={row.siblings}
                aria-expanded={hasChildren ? isExpanded : undefined}
                aria-selected={isSelected}
                style={{
                  top: index * ROW_HEIGHT,
                  height: ROW_HEIGHT,
                  paddingLeft: 7 + Math.min(level - 1, 40) * 13,
                }}
                title={node.path || "Summarized items — no single path"}
                onClick={() => {
                  setFocusedKey(key);
                  containerRef.current?.focus({ preventScroll: true });
                  onSelect(node);
                }}
                onDoubleClick={() => {
                  if (node.kind === "dir" && node.children.length) onDrill(node);
                }}
              >
                {hasChildren ? (
                  <button
                    type="button"
                    className={`az-tree-row__toggle${isExpanded ? " is-open" : ""}`}
                    aria-label={`${isExpanded ? "Collapse" : "Expand"} ${node.name}`}
                    tabIndex={-1}
                    onClick={(event) => {
                      event.stopPropagation();
                      setFocusedKey(key);
                      containerRef.current?.focus({ preventScroll: true });
                      toggle(key);
                    }}
                  >
                    <IconChevronRight size={12} />
                  </button>
                ) : (
                  <span className="az-tree-row__toggle-placeholder" aria-hidden="true" />
                )}
                <span className={`az-tree-row__icon az-tree-row__icon--${node.kind}`} aria-hidden="true">
                  {node.kind === "dir" ? <IconFolder size={13} /> : node.kind === "group" ? "…" : "·"}
                </span>
                <span className="az-tree-row__name">{node.name || "Scanned folder"}</span>
                {node.kind === "group" && (
                  <span className="az-tree-row__count">{formatCount(node.fileCount)}</span>
                )}
                <span className="az-tree-row__size">{formatBytes(share)}</span>
              </div>
            );
          })}
        </div>
      </div>
      {capped && (
        <p className="az-tree-view__note">
          Showing the first {formatCount(MAX_FLAT_ROWS)} flattened rows. Collapse branches or filter to
          explore the rest.
        </p>
      )}
      <p className="az-tree-view__note">
        Summarized scan tree · sizes are measured, not affected by filters · grouped items have no
        individual paths.
      </p>
    </div>
  );
}
