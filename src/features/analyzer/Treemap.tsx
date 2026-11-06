import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { colorForFile } from "../../lib/categories";
import { formatBytes, formatPercent } from "../../lib/format";
import { squarify } from "../../lib/treemap";
import type { ScanNode } from "../../lib/types";

interface TreemapProps {
  node: ScanNode;
  selectedPath: string;
  onSelect: (node: ScanNode) => void;
  onDrill: (node: ScanNode) => void;
}

const DIR_FILL = "#3d4c52";
const GROUP_FILL = "#8a7a5e";

const KIND_LABEL: Record<ScanNode["kind"], string> = {
  dir: "Folder",
  file: "File",
  group: "Grouped items",
};

/**
 * Interactive squarified treemap. Tiles are real buttons: click selects,
 * double-click or Enter on a folder drills in, arrow keys move between tiles.
 */
export function Treemap({ node, selectedPath, onSelect, onDrill }: TreemapProps) {
  const surfaceRef = useRef<HTMLDivElement>(null);
  const tileRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const [hoveredKey, setHoveredKey] = useState<string | null>(null);

  // Index keys are only meaningful for the current node.
  useEffect(() => setHoveredKey(null), [node.path]);

  useEffect(() => {
    const element = surfaceRef.current;
    if (!element) return;
    const measure = () => {
      const rect = element.getBoundingClientRect();
      setSize({ width: Math.round(rect.width), height: Math.round(rect.height) });
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const tiles = useMemo(() => {
    const children = node.children.filter((child) => child.size > 0 || child.allocated > 0);
    if (children.length > 0) return children;
    if (node.size > 0 || node.allocated > 0) return [node];
    return [];
  }, [node]);

  const rectangles = useMemo(() => {
    const layout = squarify(
      tiles.map((tile, index) => ({
        key: String(index),
        value: tile.size || tile.allocated,
      })),
      size.width,
      size.height,
    );
    return layout.flatMap((rect) => {
      const tile = tiles[Number(rect.key)];
      // Sub-pixel tiles would still be focusable buttons with no visible area.
      if (!tile || rect.w < 0.75 || rect.h < 0.75) return [];
      return [{ ...rect, tile }];
    });
  }, [tiles, size]);

  const measured = size.width > 0 && size.height > 0;

  const total = useMemo(
    () => tiles.reduce((sum, tile) => sum + (tile.size || tile.allocated), 0),
    [tiles],
  );

  const hovered = rectangles.find((rect) => rect.key === hoveredKey) ?? null;

  const moveFocus = (index: number, dx: number, dy: number) => {
    const current = rectangles[index];
    if (!current) return;
    const originX = current.x + current.w / 2;
    const originY = current.y + current.h / 2;
    let best = -1;
    let bestScore = Number.POSITIVE_INFINITY;
    rectangles.forEach((rect, candidateIndex) => {
      if (candidateIndex === index) return;
      const offsetX = rect.x + rect.w / 2 - originX;
      const offsetY = rect.y + rect.h / 2 - originY;
      const primary = dx !== 0 ? offsetX * dx : offsetY * dy;
      if (primary <= 1) return;
      const secondary = dx !== 0 ? Math.abs(offsetY) : Math.abs(offsetX);
      const score = primary + secondary * 2.4;
      if (score < bestScore) {
        bestScore = score;
        best = candidateIndex;
      }
    });
    if (best >= 0) tileRefs.current[best]?.focus();
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    const rect = rectangles[index];
    if (!rect) return;
    if (event.key === "Enter") {
      event.preventDefault();
      if (rect.tile.kind === "dir" && rect.tile.children.length > 0) onDrill(rect.tile);
      else onSelect(rect.tile);
      return;
    }
    const directions: Record<string, [number, number]> = {
      ArrowRight: [1, 0],
      ArrowLeft: [-1, 0],
      ArrowUp: [0, -1],
      ArrowDown: [0, 1],
    };
    const direction = directions[event.key];
    if (!direction) return;
    event.preventDefault();
    moveFocus(index, direction[0], direction[1]);
  };

  const tipLeft = hovered ? Math.min(Math.max(hovered.x + 8, 6), Math.max(size.width - 214, 6)) : 0;
  const tipTop = hovered ? Math.min(hovered.y + 8, Math.max(size.height - 62, 6)) : 0;

  return (
    <div className="treemap">
      <div
        className="treemap__surface"
        ref={surfaceRef}
        role="group"
        aria-label={`Treemap of ${node.name}`}
      >
        {rectangles.map((rect, index) => {
          const tile = rect.tile;
          const selected = Boolean(tile.path) && tile.path === selectedPath;
          const fill =
            tile.kind === "file"
              ? colorForFile(tile.name)
              : tile.kind === "group"
                ? GROUP_FILL
                : DIR_FILL;
          const showName = rect.w >= 58 && rect.h >= 24;
          const showSize = rect.w >= 86 && rect.h >= 48;
          return (
            <button
              key={rect.key}
              ref={(element) => {
                tileRefs.current[index] = element;
              }}
              type="button"
              className={`treemap__tile${selected ? " is-selected" : ""}`}
              style={{
                left: rect.x + 1,
                top: rect.y + 1,
                width: Math.max(rect.w - 2, 0),
                height: Math.max(rect.h - 2, 0),
                background: fill,
                // Light tiles take ink labels; the folder fill is dark, so it takes paper.
                color: tile.kind === "dir" ? "#eef1ea" : "#0b1314",
              }}
              aria-label={`${tile.name}, ${KIND_LABEL[tile.kind]}, ${formatBytes(tile.size || tile.allocated)}`}
              aria-pressed={selected}
              onClick={() => onSelect(tile)}
              onDoubleClick={() => {
                if (tile.kind === "dir" && tile.children.length > 0) onDrill(tile);
              }}
              onKeyDown={(event) => handleKeyDown(event, index)}
              onMouseEnter={() => setHoveredKey(rect.key)}
              onMouseLeave={() => setHoveredKey(null)}
              onFocus={() => setHoveredKey(rect.key)}
              onBlur={() => setHoveredKey(null)}
            >
              {showName && (
                <span className="treemap__label">
                  <span className="treemap__name">{tile.name}</span>
                  {showSize && (
                    <span className="treemap__size">{formatBytes(tile.size || tile.allocated)}</span>
                  )}
                </span>
              )}
            </button>
          );
        })}

        {measured && rectangles.length === 0 && (
          <div className="treemap__empty">
            <p className="treemap__empty-title">Nothing to measure here</p>
            <p className="treemap__empty-text">
              This folder contains no readable files or is empty. Try drilling up a level.
            </p>
          </div>
        )}

        {measured && hovered && total > 0 && (
          <div className="treemap__tip" style={{ left: tipLeft, top: tipTop }} aria-hidden="true">
            <span className="treemap__tip-name">{hovered.tile.name}</span>
            <span className="treemap__tip-meta">
              {formatBytes(hovered.tile.size || hovered.tile.allocated)} ·{" "}
              {formatPercent((hovered.tile.size || hovered.tile.allocated) / total)} of{" "}
              {node.name}
            </span>
          </div>
        )}
      </div>
    </div>
  );
}
