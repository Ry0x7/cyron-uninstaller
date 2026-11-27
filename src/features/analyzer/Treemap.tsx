import {
  memo,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type PointerEvent,
} from "react";
import { colorForFile } from "../../lib/categories";
import { formatBytes, formatPercent } from "../../lib/format";
import { hitTest, nestedSquarify, type NestedTile } from "../../lib/treemap";
import type { ScanNode } from "../../lib/types";

interface TreemapProps {
  node: ScanNode;
  selectedPath: string;
  /** Bumped when folder contents are loaded on demand, so the map redraws. */
  version?: number;
  onSelect: (node: ScanNode) => void;
  onDrill: (node: ScanNode) => void;
}

/** Folder frames get lighter with depth so each level reads as its own box. */
const FRAME_FILL = ["#26313a", "#2f3c47", "#394855", "#435463"];
const FRAME_TEXT = "#e8edf2";
const LEAF_DIR_FILL = "#4a5b66";
const GROUP_FILL = "#7d7262";
const CANVAS_FILL = "#151a1f";
const FONT = '600 11px "Segoe UI Variable Text", "Segoe UI", system-ui, -apple-system, sans-serif';
const FONT_SMALL = '500 10px "Cascadia Mono", Consolas, ui-monospace, monospace';

const KIND_LABEL: Record<ScanNode["kind"], string> = {
  dir: "Folder",
  file: "File",
  group: "Grouped items",
};

type Tile = NestedTile<ScanNode>;

const valueOf = (node: ScanNode) => node.size || node.allocated;

function fillFor(tile: Tile): string {
  if (tile.node.kind === "file") return colorForFile(tile.node.name);
  if (tile.node.kind === "group") return GROUP_FILL;
  if (tile.nested) return FRAME_FILL[Math.min(tile.depth, FRAME_FILL.length - 1)];
  return LEAF_DIR_FILL;
}

/** Fits a label into a width with a trailing ellipsis, measuring at most a few times. */
function fitText(context: CanvasRenderingContext2D, text: string, width: number): string {
  if (width <= 8) return "";
  if (context.measureText(text).width <= width) return text;
  let low = 0;
  let high = text.length;
  while (low < high) {
    const middle = Math.ceil((low + high) / 2);
    if (context.measureText(`${text.slice(0, middle)}…`).width <= width) low = middle;
    else high = middle - 1;
  }
  return low > 0 ? `${text.slice(0, low)}…` : "";
}

function draw(canvas: HTMLCanvasElement, tiles: Tile[], width: number, height: number) {
  const ratio = window.devicePixelRatio || 1;
  const pixelWidth = Math.max(1, Math.round(width * ratio));
  const pixelHeight = Math.max(1, Math.round(height * ratio));
  if (canvas.width !== pixelWidth) canvas.width = pixelWidth;
  if (canvas.height !== pixelHeight) canvas.height = pixelHeight;
  const context = canvas.getContext("2d");
  if (!context) return;
  context.setTransform(ratio, 0, 0, ratio, 0, 0);
  context.fillStyle = CANVAS_FILL;
  context.fillRect(0, 0, width, height);
  context.textBaseline = "middle";

  for (const tile of tiles) {
    const gap = tile.nested ? 1 : tile.w > 4 && tile.h > 4 ? 1 : 0.5;
    const x = tile.x + gap / 2;
    const y = tile.y + gap / 2;
    const w = Math.max(tile.w - gap, 0.5);
    const h = Math.max(tile.h - gap, 0.5);
    context.fillStyle = fillFor(tile);
    if (w > 6 && h > 6) {
      context.beginPath();
      context.roundRect(x, y, w, h, tile.nested ? 4 : 2.5);
      context.fill();
    } else {
      context.fillRect(x, y, w, h);
    }

    if (tile.nested) {
      if (tile.header > 0) {
        context.font = FONT;
        context.fillStyle = FRAME_TEXT;
        const size = formatBytes(valueOf(tile.node));
        context.font = FONT_SMALL;
        const sizeWidth = context.measureText(size).width;
        const room = w - 12;
        const showSize = room > sizeWidth + 48;
        if (showSize) {
          context.globalAlpha = 0.66;
          context.fillText(size, x + w - 6 - sizeWidth, y + tile.header / 2 + 0.5);
          context.globalAlpha = 1;
        }
        context.font = FONT;
        const label = fitText(context, tile.node.name, showSize ? room - sizeWidth - 10 : room);
        if (label) context.fillText(label, x + 6, y + tile.header / 2 + 0.5);
      }
      continue;
    }

    // A one-pixel top light gives flat tiles a little depth without a gradient.
    if (w > 10 && h > 10) {
      context.fillStyle = "rgba(255,255,255,0.14)";
      context.fillRect(x + 2, y + 0.5, w - 4, 1);
    }

    if (w >= 52 && h >= 22) {
      const ink = tile.node.kind === "dir" ? FRAME_TEXT : "#0b1314";
      context.fillStyle = ink;
      context.font = FONT;
      const label = fitText(context, tile.node.name, w - 12);
      const twoLines = h >= 40 && w >= 72;
      if (label) context.fillText(label, x + 6, twoLines ? y + h - 24 : y + h / 2);
      if (twoLines) {
        context.font = FONT_SMALL;
        context.globalAlpha = 0.72;
        context.fillText(formatBytes(valueOf(tile.node)), x + 6, y + h - 10);
        context.globalAlpha = 1;
      }
    }
  }
}

/**
 * Nested, canvas-drawn treemap. Folders are drawn as framed boxes with their
 * name, and their contents inside, so sibling folders stay visibly separate.
 * Drawing on a canvas keeps thousands of tiles cheap; hover only moves an
 * outline. The top-level tiles are also real, invisible buttons for keyboard
 * and screen-reader use.
 */
export function Treemap({ node, selectedPath, version = 0, onSelect, onDrill }: TreemapProps) {
  const surfaceRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const [hovered, setHovered] = useState(-1);
  const frame = useRef(0);

  useEffect(() => setHovered(-1), [node.path, version]);

  useEffect(() => {
    const element = surfaceRef.current;
    if (!element) return;
    let pending = 0;
    const measure = () => {
      cancelAnimationFrame(pending);
      // Coalesce resize bursts (dragging the splitter) into one layout per frame.
      pending = requestAnimationFrame(() => {
        const rect = element.getBoundingClientRect();
        setSize((previous) => {
          const width = Math.round(rect.width);
          const height = Math.round(rect.height);
          return previous.width === width && previous.height === height ? previous : { width, height };
        });
      });
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => {
      cancelAnimationFrame(pending);
      observer.disconnect();
    };
  }, []);

  const tiles = useMemo(() => {
    if (size.width <= 0 || size.height <= 0) return [] as Tile[];
    const hasChildren = node.children.some((child) => valueOf(child) > 0);
    // A folder without loaded contents still shows as one tile.
    const root = hasChildren ? node : ({ ...node, name: "", children: [node] } as ScanNode);
    return nestedSquarify(root, size.width, size.height);
    // version: contents can be grafted in place after a lazy load.
  }, [node, size, version]);

  useLayoutEffect(() => {
    const canvas = canvasRef.current;
    if (canvas && size.width > 0) draw(canvas, tiles, size.width, size.height);
  }, [tiles, size]);

  // Redraw on DPI changes (moving between monitors).
  useEffect(() => {
    const query = window.matchMedia(`(resolution: ${window.devicePixelRatio || 1}dppx)`);
    const redraw = () => {
      const canvas = canvasRef.current;
      if (canvas) draw(canvas, tiles, size.width, size.height);
    };
    query.addEventListener?.("change", redraw);
    return () => query.removeEventListener?.("change", redraw);
  }, [tiles, size]);

  const topLevel = useMemo(
    () => tiles.map((tile, index) => ({ tile, index })).filter((entry) => entry.tile.parent === -1),
    [tiles],
  );

  const total = useMemo(
    () => node.children.reduce((sum, child) => sum + valueOf(child), 0) || valueOf(node),
    [node, version],
  );

  const pointAt = useCallback(
    (event: PointerEvent<HTMLDivElement>) => {
      const rect = surfaceRef.current?.getBoundingClientRect();
      if (!rect) return -1;
      return hitTest(tiles, event.clientX - rect.left, event.clientY - rect.top);
    },
    [tiles],
  );

  const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
    const clientX = event.clientX;
    const clientY = event.clientY;
    cancelAnimationFrame(frame.current);
    frame.current = requestAnimationFrame(() => {
      const rect = surfaceRef.current?.getBoundingClientRect();
      if (!rect) return;
      const index = hitTest(tiles, clientX - rect.left, clientY - rect.top);
      setHovered((previous) => (previous === index ? previous : index));
    });
  };

  useEffect(() => () => cancelAnimationFrame(frame.current), []);

  const drillTarget = (index: number): ScanNode | null => {
    // Double-clicking a file drills into the folder that holds it.
    let cursor = index;
    while (cursor >= 0) {
      const candidate = tiles[cursor];
      if (candidate.node.kind === "dir" && candidate.node.path) return candidate.node;
      cursor = candidate.parent;
    }
    return null;
  };

  const moveFocus = (position: number, dx: number, dy: number) => {
    const current = topLevel[position]?.tile;
    if (!current) return;
    const originX = current.x + current.w / 2;
    const originY = current.y + current.h / 2;
    let best = -1;
    let bestScore = Number.POSITIVE_INFINITY;
    topLevel.forEach(({ tile }, candidate) => {
      if (candidate === position) return;
      const offsetX = tile.x + tile.w / 2 - originX;
      const offsetY = tile.y + tile.h / 2 - originY;
      const primary = dx !== 0 ? offsetX * dx : offsetY * dy;
      if (primary <= 1) return;
      const secondary = dx !== 0 ? Math.abs(offsetY) : Math.abs(offsetX);
      const score = primary + secondary * 2.4;
      if (score < bestScore) {
        bestScore = score;
        best = candidate;
      }
    });
    if (best >= 0) {
      surfaceRef.current
        ?.querySelectorAll<HTMLButtonElement>(".treemap__hit")
        [best]?.focus();
    }
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLButtonElement>, position: number) => {
    const tile = topLevel[position]?.tile;
    if (!tile) return;
    if (event.key === "Enter") {
      event.preventDefault();
      if (tile.node.kind === "dir") onDrill(tile.node);
      else onSelect(tile.node);
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
    moveFocus(position, direction[0], direction[1]);
  };

  const hoveredTile = hovered >= 0 ? tiles[hovered] : null;
  const selectedTile = useMemo(() => {
    if (!selectedPath) return null;
    const key = selectedPath.toLowerCase();
    return tiles.find((tile) => tile.node.path && tile.node.path.toLowerCase() === key) ?? null;
  }, [tiles, selectedPath]);

  const crumbs = useMemo(() => {
    if (hovered < 0) return [] as string[];
    const names: string[] = [];
    let cursor = tiles[hovered]?.parent ?? -1;
    while (cursor >= 0) {
      names.unshift(tiles[cursor].node.name);
      cursor = tiles[cursor].parent;
    }
    return names;
  }, [hovered, tiles]);

  const tipLeft = hoveredTile
    ? Math.min(Math.max(hoveredTile.x + 10, 6), Math.max(size.width - 250, 6))
    : 0;
  const tipTop = hoveredTile
    ? Math.min(Math.max(hoveredTile.y + (hoveredTile.nested ? hoveredTile.header : 0) + 10, 6), Math.max(size.height - 76, 6))
    : 0;

  const measured = size.width > 0 && size.height > 0;

  return (
    <div className="treemap">
      <div
        className="treemap__surface"
        ref={surfaceRef}
        role="group"
        aria-label={`Treemap of ${node.name}`}
        onPointerMove={onPointerMove}
        onPointerLeave={() => {
          cancelAnimationFrame(frame.current);
          setHovered(-1);
        }}
        onClick={(event) => {
          const index = pointAt(event as unknown as PointerEvent<HTMLDivElement>);
          if (index >= 0) onSelect(tiles[index].node);
        }}
        onDoubleClick={(event) => {
          const index = pointAt(event as unknown as PointerEvent<HTMLDivElement>);
          if (index < 0) return;
          const target = drillTarget(index);
          if (target && (target !== node || node.children.length === 0)) onDrill(target);
        }}
      >
        <canvas className="treemap__canvas" ref={canvasRef} aria-hidden="true" />

        {topLevel.map(({ tile }, position) => (
          <button
            key={`${tile.node.path || tile.node.name}-${position}`}
            type="button"
            className="treemap__hit"
            tabIndex={position === 0 ? 0 : -1}
            style={{ left: tile.x, top: tile.y, width: tile.w, height: tile.h }}
            aria-label={`${tile.node.name}, ${KIND_LABEL[tile.node.kind]}, ${formatBytes(valueOf(tile.node))}`}
            aria-pressed={Boolean(tile.node.path) && tile.node.path === selectedPath}
            onFocus={() => onSelect(tile.node)}
            onKeyDown={(event) => handleKeyDown(event, position)}
          />
        ))}

        {selectedTile && (
          <span
            className="treemap__outline treemap__outline--selected"
            style={{ left: selectedTile.x, top: selectedTile.y, width: selectedTile.w, height: selectedTile.h }}
            aria-hidden="true"
          />
        )}
        {hoveredTile && hoveredTile !== selectedTile && (
          <span
            className="treemap__outline"
            style={{ left: hoveredTile.x, top: hoveredTile.y, width: hoveredTile.w, height: hoveredTile.h }}
            aria-hidden="true"
          />
        )}

        {measured && tiles.length === 0 && (
          <div className="treemap__empty">
            <p className="treemap__empty-title">Nothing to measure here</p>
            <p className="treemap__empty-text">
              This folder contains no readable files or is empty. Try going up a level.
            </p>
          </div>
        )}

        {measured && hoveredTile && total > 0 && (
          <TreemapTip
            left={tipLeft}
            top={tipTop}
            tile={hoveredTile}
            crumbs={crumbs}
            share={valueOf(hoveredTile.node) / total}
            scope={node.name}
          />
        )}
      </div>
    </div>
  );
}

const TreemapTip = memo(function TreemapTip({
  left,
  top,
  tile,
  crumbs,
  share,
  scope,
}: {
  left: number;
  top: number;
  tile: Tile;
  crumbs: string[];
  share: number;
  scope: string;
}) {
  const node = tile.node;
  return (
    <div className="treemap__tip" style={{ left, top }} aria-hidden="true">
      {crumbs.length > 0 && <span className="treemap__tip-path">{crumbs.join(" › ")}</span>}
      <span className="treemap__tip-name">{node.name}</span>
      <span className="treemap__tip-meta">
        {KIND_LABEL[node.kind]} · {formatBytes(valueOf(node))} · {formatPercent(share)} of {scope}
      </span>
      {node.kind === "dir" && (
        <span className="treemap__tip-hint">
          {node.children.length > 0 || node.fileCount > 0 ? "Double-click to open" : "Empty folder"}
        </span>
      )}
    </div>
  );
});
