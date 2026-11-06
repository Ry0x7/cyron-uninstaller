import { memo, useMemo, useState } from "react";
import {
  IconChevronDown,
  IconCopy,
  IconExternal,
  IconFolder,
  IconInfo,
  IconSearch,
} from "../../components/Icons";
import { formatBytes, formatCount, formatDate, formatPercent } from "../../lib/format";
import type { ScanNode } from "../../lib/types";
import "./detailedListing.css";

export type ColumnKey = "name" | "size" | "allocated" | "percent" | "files" | "dirs" | "modified";

interface DetailedListingProps {
  node: ScanNode;
  selectedPath: string;
  onSelect: (node: ScanNode) => void;
  onDrill: (node: ScanNode) => void;
  onReveal: (path: string) => void;
  onShowProperties: (node: ScanNode) => void;
}

export const DetailedListing = memo(function DetailedListing({
  node,
  selectedPath,
  onSelect,
  onDrill,
  onReveal,
  onShowProperties,
}: DetailedListingProps) {
  const [sortField, setSortField] = useState<ColumnKey>("size");
  const [sortAsc, setSortAsc] = useState(false);
  const [filterText, setFilterText] = useState("");
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; item: ScanNode } | null>(null);

  const parentSize = node.size || 1;

  const handleHeaderClick = (key: ColumnKey) => {
    if (sortField === key) {
      setSortAsc((prev) => !prev);
    } else {
      setSortField(key);
      setSortAsc(key === "name"); // Default name to ascending, numbers to descending
    }
  };

  const items = useMemo(() => {
    let list = node.children.slice();
    if (filterText.trim()) {
      const q = filterText.trim().toLowerCase();
      list = list.filter((child) => child.name.toLowerCase().includes(q));
    }

    list.sort((a, b) => {
      // Folders first
      if (a.kind !== b.kind) {
        return a.kind === "dir" ? -1 : 1;
      }

      let diff = 0;
      switch (sortField) {
        case "name":
          diff = a.name.localeCompare(b.name, undefined, { sensitivity: "base", numeric: true });
          break;
        case "allocated":
          diff = a.allocated - b.allocated;
          break;
        case "percent":
        case "size":
          diff = a.size - b.size;
          break;
        case "files":
          diff = a.fileCount - b.fileCount;
          break;
        case "dirs":
          diff = a.dirCount - b.dirCount;
          break;
        case "modified":
          diff = (a.modified ?? 0) - (b.modified ?? 0);
          break;
      }
      return sortAsc ? diff : -diff;
    });

    return list;
  }, [node.children, filterText, sortField, sortAsc]);

  const copyPath = (path: string) => {
    if (path) {
      void navigator.clipboard.writeText(path);
    }
  };

  return (
    <div className="az-details-wrap" onClick={() => setContextMenu(null)}>
      <div className="az-details-toolbar">
        <div className="az-details-search">
          <IconSearch size={14} className="az-details-search-icon" />
          <input
            type="search"
            placeholder="Filter files in this folder..."
            value={filterText}
            onChange={(e) => setFilterText(e.target.value)}
            className="az-details-search-input"
          />
        </div>
        <div className="az-details-summary">
          {formatCount(items.length)} item{items.length === 1 ? "" : "s"} &middot;{" "}
          {formatBytes(items.reduce((acc, cur) => acc + cur.size, 0))}
        </div>
      </div>

      <div className="az-details-table-scroll">
        <table className="az-details-table" role="grid">
          <thead>
            <tr>
              <th
                className={`az-col-name ${sortField === "name" ? "is-sorted" : ""}`}
                onClick={() => handleHeaderClick("name")}
                aria-sort={sortField === "name" ? (sortAsc ? "ascending" : "descending") : undefined}
              >
                <span>Name</span>
                {sortField === "name" && (
                  <IconChevronDown
                    size={12}
                    className={`sort-chevron ${sortAsc ? "sort-chevron--asc" : ""}`}
                  />
                )}
              </th>
              <th
                className={`az-col-size ${sortField === "size" ? "is-sorted" : ""}`}
                onClick={() => handleHeaderClick("size")}
                aria-sort={sortField === "size" ? (sortAsc ? "ascending" : "descending") : undefined}
              >
                <span>Size</span>
                {sortField === "size" && (
                  <IconChevronDown
                    size={12}
                    className={`sort-chevron ${sortAsc ? "sort-chevron--asc" : ""}`}
                  />
                )}
              </th>
              <th
                className={`az-col-allocated ${sortField === "allocated" ? "is-sorted" : ""}`}
                onClick={() => handleHeaderClick("allocated")}
                aria-sort={sortField === "allocated" ? (sortAsc ? "ascending" : "descending") : undefined}
              >
                <span>Allocated</span>
                {sortField === "allocated" && (
                  <IconChevronDown
                    size={12}
                    className={`sort-chevron ${sortAsc ? "sort-chevron--asc" : ""}`}
                  />
                )}
              </th>
              <th
                className={`az-col-percent ${sortField === "percent" ? "is-sorted" : ""}`}
                onClick={() => handleHeaderClick("percent")}
                aria-sort={sortField === "percent" ? (sortAsc ? "ascending" : "descending") : undefined}
              >
                <span>% Parent</span>
                {sortField === "percent" && (
                  <IconChevronDown
                    size={12}
                    className={`sort-chevron ${sortAsc ? "sort-chevron--asc" : ""}`}
                  />
                )}
              </th>
              <th
                className={`az-col-files ${sortField === "files" ? "is-sorted" : ""}`}
                onClick={() => handleHeaderClick("files")}
                aria-sort={sortField === "files" ? (sortAsc ? "ascending" : "descending") : undefined}
              >
                <span>Items</span>
                {sortField === "files" && (
                  <IconChevronDown
                    size={12}
                    className={`sort-chevron ${sortAsc ? "sort-chevron--asc" : ""}`}
                  />
                )}
              </th>
              <th
                className={`az-col-modified ${sortField === "modified" ? "is-sorted" : ""}`}
                onClick={() => handleHeaderClick("modified")}
                aria-sort={sortField === "modified" ? (sortAsc ? "ascending" : "descending") : undefined}
              >
                <span>Modified</span>
                {sortField === "modified" && (
                  <IconChevronDown
                    size={12}
                    className={`sort-chevron ${sortAsc ? "sort-chevron--asc" : ""}`}
                  />
                )}
              </th>
            </tr>
          </thead>
          <tbody>
            {items.length === 0 ? (
              <tr>
                <td colSpan={6} className="az-details-empty">
                  No items in this folder match the filter.
                </td>
              </tr>
            ) : (
              items.map((child, idx) => {
                const isSelected = child.path && selectedPath === child.path;
                const ratio = parentSize > 0 ? child.size / parentSize : 0;
                return (
                  <tr
                    key={child.path || `${child.name}-${idx}`}
                    className={`az-details-row ${isSelected ? "is-selected" : ""}`}
                    onClick={(e) => {
                      e.stopPropagation();
                      onSelect(child);
                    }}
                    onDoubleClick={() => {
                      if (child.kind === "dir") {
                        onDrill(child);
                      }
                    }}
                    onContextMenu={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      onSelect(child);
                      setContextMenu({ x: e.clientX, y: e.clientY, item: child });
                    }}
                  >
                    <td className="az-col-name">
                      <div className="az-name-cell">
                        <span className={`az-item-icon az-item-icon--${child.kind}`}>
                          {child.kind === "dir" ? <IconFolder size={14} /> : "·"}
                        </span>
                        <span className="az-item-title" title={child.path || child.name}>
                          {child.name}
                        </span>
                      </div>
                    </td>
                    <td className="az-col-size">{formatBytes(child.size)}</td>
                    <td className="az-col-allocated">{formatBytes(child.allocated)}</td>
                    <td className="az-col-percent">
                      <div className="az-percent-cell">
                        <span className="az-percent-bar-wrap">
                          <span
                            className="az-percent-bar"
                            style={{ width: `${Math.min(100, ratio * 100).toFixed(1)}%` }}
                          />
                        </span>
                        <span className="az-percent-label">{formatPercent(ratio)}</span>
                      </div>
                    </td>
                    <td className="az-col-files">
                      {child.kind === "dir" ? formatCount(child.fileCount) : "—"}
                    </td>
                    <td className="az-col-modified">
                      {child.modified ? formatDate(child.modified) : "—"}
                    </td>
                  </tr>
                );
              })
            )}
          </tbody>
        </table>
      </div>

      {contextMenu && (
        <div
          className="az-context-menu"
          style={{ top: contextMenu.y, left: contextMenu.x }}
          onClick={(e) => e.stopPropagation()}
        >
          {contextMenu.item.kind === "dir" && (
            <button
              type="button"
              className="az-context-item"
              onClick={() => {
                onDrill(contextMenu.item);
                setContextMenu(null);
              }}
            >
              <IconFolder size={14} /> Drill into folder
            </button>
          )}
          {contextMenu.item.path && (
            <>
              <button
                type="button"
                className="az-context-item"
                onClick={() => {
                  onReveal(contextMenu.item.path);
                  setContextMenu(null);
                }}
              >
                <IconExternal size={14} /> Show in Explorer
              </button>
              <button
                type="button"
                className="az-context-item"
                onClick={() => {
                  copyPath(contextMenu.item.path);
                  setContextMenu(null);
                }}
              >
                <IconCopy size={14} /> Copy full path
              </button>
            </>
          )}
          <button
            type="button"
            className="az-context-item"
            onClick={() => {
              onShowProperties(contextMenu.item);
              setContextMenu(null);
            }}
          >
            <IconInfo size={14} /> Properties
          </button>
        </div>
      )}
    </div>
  );
});
