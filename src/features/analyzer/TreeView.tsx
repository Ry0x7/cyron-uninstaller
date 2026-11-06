import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { IconChevronRight, IconFolder } from "../../components/Icons";
import { formatBytes, formatCount } from "../../lib/format";
import type { ScanNode } from "../../lib/types";

const MAX_VISIBLE_ROWS = 500;

interface TreeRow {
  node: ScanNode;
  key: string;
  parentKey: string | null;
  level: number;
  hasChildren: boolean;
}

interface TreeViewProps {
  root: ScanNode;
  selectedNode: ScanNode | null;
  currentPath: string;
  onSelect: (node: ScanNode) => void;
  onDrill: (node: ScanNode) => void;
}

function pathKey(path: string): string {
  return path.replace(/[\\/]+$/, "").toLocaleLowerCase();
}

function nodeKey(node: ScanNode, parentKey: string | null, index: number): string {
  return node.path ? `path:${pathKey(node.path)}` : `${parentKey ?? "root"}/synthetic-${index}-${node.kind}`;
}

function findAncestors(root: ScanNode, target: ScanNode): string[] {
  const targetPath = target.path ? pathKey(target.path) : "";
  const visit = (node: ScanNode, parentKey: string | null, index: number, ancestors: string[]): string[] | null => {
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
  limit: number,
): { rows: TreeRow[]; capped: boolean } {
  const rows: TreeRow[] = [];
  let capped = false;
  const visit = (node: ScanNode, parentKey: string | null, level: number, index: number) => {
    if (rows.length >= limit) {
      capped = true;
      return;
    }
    const key = nodeKey(node, parentKey, index);
    const visibleChildren = node.children;
    rows.push({
      node,
      key,
      parentKey,
      level,
      hasChildren: visibleChildren.length > 0,
    });
    if (!expanded.has(key)) return;
    for (let childIndex = 0; childIndex < visibleChildren.length; childIndex += 1) {
      visit(visibleChildren[childIndex], key, level + 1, childIndex);
      if (capped) break;
    }
  };
  visit(root, null, 1, 0);
  return { rows, capped };
}

export function TreeView({ root, selectedNode, currentPath, onSelect, onDrill }: TreeViewProps) {
  const rootKey = nodeKey(root, null, 0);
  const treeId = useMemo(() => `scan-tree-${Math.random().toString(36).slice(2)}`, []);
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set([rootKey]));
  const [focusedKey, setFocusedKey] = useState(rootKey);
  const rowRefs = useRef(new Map<string, HTMLDivElement>());
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!selectedNode) return;
    const ancestors = findAncestors(root, selectedNode);
    if (ancestors.length) {
      setExpanded((previous) => {
        const next = new Set(previous);
        ancestors.forEach((key) => next.add(key));
        return next;
      });
    }
  }, [root, selectedNode]);

  const { rows, capped } = useMemo(
    () => flattenVisible(root, expanded, MAX_VISIBLE_ROWS),
    [root, expanded],
  );

  const selectedKey = useMemo(() => {
    if (!selectedNode) return null;
    const exact = rows.find((row) => row.node === selectedNode);
    if (exact) return exact.key;
    if (selectedNode.path) {
      const selectedPath = pathKey(selectedNode.path);
      return rows.find((row) => row.node.path && pathKey(row.node.path) === selectedPath)?.key ?? null;
    }
    return null;
  }, [rows, selectedNode]);

  useEffect(() => {
    if (!selectedKey) return;
    const frame = requestAnimationFrame(() => {
      rowRefs.current.get(selectedKey)?.scrollIntoView({ block: "nearest" });
    });
    return () => cancelAnimationFrame(frame);
  }, [selectedKey, rows]);

  const toggle = (key: string) => {
    setExpanded((previous) => {
      const next = new Set(previous);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  const focusRow = (row: TreeRow | undefined) => {
    if (!row) return;
    setFocusedKey(row.key);
    rowRefs.current.get(row.key)?.focus();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>, index: number, row: TreeRow) => {
    const isExpanded = expanded.has(row.key);
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        focusRow(rows[index + 1]);
        break;
      case "ArrowUp":
        event.preventDefault();
        focusRow(rows[index - 1]);
        break;
      case "ArrowRight":
        event.preventDefault();
        if (row.hasChildren && !isExpanded) toggle(row.key);
        else if (row.hasChildren) focusRow(rows[index + 1]);
        break;
      case "ArrowLeft":
        event.preventDefault();
        if (row.hasChildren && isExpanded) toggle(row.key);
        else if (row.parentKey) focusRow(rows.find((candidate) => candidate.key === row.parentKey));
        break;
      case "Enter":
      case " ":
        event.preventDefault();
        onSelect(row.node);
        if (event.key === "Enter" && row.node.kind === "dir" && row.node.children.length > 0) {
          onDrill(row.node);
        }
        break;
      case "Home":
        event.preventDefault();
        focusRow(rows[0]);
        break;
      case "End":
        event.preventDefault();
        focusRow(rows[rows.length - 1]);
        break;
    }
  };

  if (!root.children.length) {
    return <p className="az-tree-view__empty">This scan has no directory tree to display.</p>;
  }

  return (
    <div className="az-tree-view-wrap">
      <div className="az-tree-view" ref={scrollRef} role="tree" aria-label="Scanned folder tree">
        {rows.map((row, index) => {
        const { node, key, level, hasChildren } = row;
        const isSelected = selectedKey === key;
        const isCurrent = Boolean(node.path && currentPath && pathKey(node.path) === pathKey(currentPath));
        const share = node.size || node.allocated;
        const parent = row.parentKey ? rows.find((item) => item.key === row.parentKey)?.node : null;
        const parentSize = parent ? parent.size || parent.allocated : share;
        const sharePercent = parentSize > 0 ? Math.min((share / parentSize) * 100, 100) : 0;
        const isExpanded = expanded.has(key);
        return (
          <div
            key={key}
            ref={(element) => {
              if (element) rowRefs.current.set(key, element);
              else rowRefs.current.delete(key);
            }}
            className={`az-tree-row${isSelected ? " is-selected" : ""}${isCurrent ? " is-current" : ""}`}
            id={`${treeId}-${index}`}
            role="treeitem"
            aria-level={level}
            aria-posinset={rows.slice(0, index).filter((candidate) => candidate.parentKey === row.parentKey).length + 1}
            aria-setsize={rows.filter((candidate) => candidate.parentKey === row.parentKey).length}
            aria-expanded={hasChildren ? isExpanded : undefined}
            aria-selected={isSelected}
            tabIndex={focusedKey === key ? 0 : -1}
            onFocus={() => setFocusedKey(key)}
            onKeyDown={(event) => onKeyDown(event, index, row)}
            onClick={(event) => {
              setFocusedKey(key);
              event.currentTarget.focus();
              onSelect(node);
            }}
            onDoubleClick={() => {
              if (node.kind === "dir" && node.children.length) onDrill(node);
            }}
            title={node.path || "Summarized items — no single path"}
            style={{ "--tree-level": Math.min(level - 1, 24) } as React.CSSProperties}
          >
            <span className="az-tree-row__indent" aria-hidden="true" />
            {hasChildren ? (
              <button
                type="button"
                className={`az-tree-row__toggle${isExpanded ? " is-open" : ""}`}
                aria-label={`${isExpanded ? "Collapse" : "Expand"} ${node.name}`}
                tabIndex={-1}
                onClick={(event) => {
                  event.stopPropagation();
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
            {node.kind === "group" && <span className="az-tree-row__count">{formatCount(node.fileCount)}</span>}
            <span className="az-tree-row__size" title={`${sharePercent.toFixed(1)}% of parent`}>
              {formatBytes(node.size || node.allocated)}
            </span>
          </div>
        );
        })}
      </div>
      {capped && (
        <p className="az-tree-view__note">
          Showing the first {formatCount(MAX_VISIBLE_ROWS)} visible rows. Collapse branches or use the treemap to explore other summarized items.
        </p>
      )}
      <p className="az-tree-view__note">
        This is the scanner's summarized tree, not a complete file listing. Grouped items do not have individual paths.
      </p>
    </div>
  );
}
