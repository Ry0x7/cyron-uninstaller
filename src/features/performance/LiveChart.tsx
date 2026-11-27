import { memo, useId, useRef, useState, type PointerEvent } from "react";
import { cx } from "../../components/ui";
import { HISTORY_LENGTH } from "./monitorModel";

export interface ChartSeries {
  label: string;
  values: number[];
  /** Primary series is filled; secondary is a dashed line over it. */
  tone?: "primary" | "secondary";
}

interface LiveChartProps {
  series: ChartSeries[];
  /** Value at the top of the chart. */
  max: number;
  formatValue: (value: number) => string;
  ariaLabel: string;
  /** Top-left caption, e.g. "% Utilization". */
  caption?: string;
  /** Top-right caption, e.g. "100%". */
  maxLabel?: string;
  height?: number;
  compact?: boolean;
  className?: string;
}

const VIEW_WIDTH = 600;

function pointsFor(values: number[], max: number, height: number): string {
  const count = values.length;
  if (count === 0) return "";
  const step = VIEW_WIDTH / (HISTORY_LENGTH - 1);
  const offset = HISTORY_LENGTH - count;
  const ceiling = max > 0 ? max : 1;
  return values
    .map((value, index) => {
      const x = (offset + index) * step;
      const ratio = Math.min(Math.max(value / ceiling, 0), 1);
      const y = height - ratio * height;
      return `${x.toFixed(2)},${y.toFixed(2)}`;
    })
    .join(" ");
}

/**
 * A scrolling 60-second history chart. Newest sample on the right, like Task
 * Manager. Hover or focus reads exact values; the drawing never animates, so
 * the numbers being read do not move for style.
 */
export const LiveChart = memo(function LiveChart({
  series,
  max,
  formatValue,
  ariaLabel,
  caption,
  maxLabel,
  height = 220,
  compact = false,
  className,
}: LiveChartProps) {
  const surfaceRef = useRef<HTMLDivElement>(null);
  const [hover, setHover] = useState<number | null>(null);
  const id = useId();
  const viewHeight = 100;
  const longest = series.reduce((length, entry) => Math.max(length, entry.values.length), 0);

  const onMove = (event: PointerEvent<HTMLDivElement>) => {
    if (compact || longest === 0) return;
    const rect = surfaceRef.current?.getBoundingClientRect();
    if (!rect || rect.width <= 0) return;
    const ratio = (event.clientX - rect.left) / rect.width;
    const slot = Math.round(ratio * (HISTORY_LENGTH - 1));
    const index = slot - (HISTORY_LENGTH - longest);
    setHover(index >= 0 && index < longest ? index : null);
  };

  const hoverX =
    hover === null ? null : ((HISTORY_LENGTH - longest + hover) / (HISTORY_LENGTH - 1)) * 100;
  const secondsAgo = hover === null ? 0 : longest - 1 - hover;
  const latest = series.map((entry) => entry.values[entry.values.length - 1] ?? 0);

  return (
    <figure
      className={cx("live-chart", compact && "live-chart--compact", className)}
      aria-label={ariaLabel}
    >
      {!compact && (caption || maxLabel) && (
        <figcaption className="live-chart__top">
          <span>{caption}</span>
          <span className="numeric">{maxLabel}</span>
        </figcaption>
      )}
      <div
        className="live-chart__surface"
        ref={surfaceRef}
        style={{ height }}
        onPointerMove={onMove}
        onPointerLeave={() => setHover(null)}
        role="img"
        aria-label={`${ariaLabel}: ${series
          .map((entry, index) => `${entry.label} ${formatValue(latest[index])}`)
          .join(", ")}`}
      >
        <svg
          className="live-chart__svg"
          viewBox={`0 0 ${VIEW_WIDTH} ${viewHeight}`}
          preserveAspectRatio="none"
          aria-hidden="true"
        >
          {!compact &&
            [25, 50, 75].map((line) => (
              <line
                key={line}
                className="live-chart__grid"
                x1={0}
                x2={VIEW_WIDTH}
                y1={line}
                y2={line}
                vectorEffect="non-scaling-stroke"
              />
            ))}
          {series.map((entry) => {
            const points = pointsFor(entry.values, max, viewHeight);
            if (!points) return null;
            const primary = (entry.tone ?? "primary") === "primary";
            const first = points.split(" ")[0]?.split(",")[0] ?? "0";
            const last = points.split(" ").pop()?.split(",")[0] ?? "0";
            return (
              <g key={`${id}-${entry.label}`}>
                {primary && (
                  <polygon
                    className="live-chart__area"
                    points={`${first},${viewHeight} ${points} ${last},${viewHeight}`}
                  />
                )}
                <polyline
                  className={cx(
                    "live-chart__line",
                    !primary && "live-chart__line--secondary",
                  )}
                  points={points}
                  vectorEffect="non-scaling-stroke"
                />
              </g>
            );
          })}
        </svg>
        {hoverX !== null && (
          <>
            <span className="live-chart__cursor" style={{ left: `${hoverX}%` }} aria-hidden="true" />
            <div
              className={cx("live-chart__tip", hoverX > 62 && "live-chart__tip--left")}
              style={{ left: `${hoverX}%` }}
              aria-hidden="true"
            >
              <span className="live-chart__tip-time">
                {secondsAgo === 0 ? "Now" : `${secondsAgo}s ago`}
              </span>
              {series.map((entry) => (
                <span className="live-chart__tip-row" key={entry.label}>
                  <span
                    className={cx(
                      "live-chart__swatch",
                      entry.tone === "secondary" && "live-chart__swatch--secondary",
                    )}
                  />
                  {entry.label}
                  <strong className="numeric">{formatValue(entry.values[hover ?? 0] ?? 0)}</strong>
                </span>
              ))}
            </div>
          </>
        )}
      </div>
      {!compact && (
        <div className="live-chart__bottom" aria-hidden="true">
          <span>60 seconds</span>
          <span>0</span>
        </div>
      )}
    </figure>
  );
});
