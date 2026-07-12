import ThresholdBand from "./ThresholdBand";
import type { LineSeries } from "./DualAxisLine";

export type { LineSeries };

export interface QueueLineProps {
  /** X categories (labels), shared by every series. */
  categories: string[];
  /** Occupancy series, each a list of percentages (0–100) by category index. */
  series: LineSeries[];
  /** Capacity threshold %. Draws a dashed line + a near-capacity band above it. */
  capacity?: number;
  /** Render the near-capacity band (capacity → 100%). Defaults to `true`. */
  showCapacityBand?: boolean;
  /** Band + capacity-line color. Defaults to `var(--warning)`. */
  capacityColor?: string;
  /** Optional label drawn in the near-capacity band. */
  capacityLabel?: string;
  /** viewBox width. Defaults to `360`. */
  width?: number;
  /** viewBox height. Defaults to `140`. */
  height?: number;
  /** Draw the plot frame, y `%` ticks, and x category labels. Default `false`. */
  showAxes?: boolean;
  /** Accessible summary; auto-generated from the series when omitted. */
  ariaLabel?: string;
  /** Extra class(es) merged onto the root `<svg>`. */
  className?: string;
}

const AXIS_LABEL_STYLE = {
  fill: "var(--text-muted)",
  fontFamily: "var(--font-sans)",
  fontSize: "var(--font-size-xs)",
} as const;

const Y_TICKS = [0, 25, 50, 75, 100] as const;

/**
 * Queue-occupancy line chart — multiple `0–100%` series over a shared plot, with
 * a near-capacity threshold band and a dashed capacity line (both via the shared
 * `ThresholdBand` primitive). Presentational and props-driven; strokes bind to
 * tokens via props (never raw hex) and it renders in dark/light. Responsive:
 * fills its container at a fixed aspect ratio. Compact by default; pass
 * `showAxes` for the frame + tick labels. Mirrors the Figma `QueueLine` master
 * (node 525:4262).
 */
export default function QueueLine({
  categories,
  series,
  capacity = 85,
  showCapacityBand = true,
  capacityColor = "var(--warning)",
  capacityLabel,
  width = 360,
  height = 140,
  showAxes = false,
  ariaLabel,
  className,
}: QueueLineProps) {
  const n = categories.length;
  const pad = showAxes
    ? { l: 34, r: 12, t: 12, b: 22 }
    : { l: 4, r: 4, t: 6, b: 6 };
  const plotL = pad.l;
  const plotR = width - pad.r;
  const plotT = pad.t;
  const plotB = height - pad.b;
  const plotW = plotR - plotL;
  const plotH = plotB - plotT;

  const xAt = (i: number) =>
    n <= 1 ? (plotL + plotR) / 2 : plotL + (i / (n - 1)) * plotW;
  const yAt = (pct: number) =>
    plotB - (Math.max(0, Math.min(100, pct)) / 100) * plotH;

  const summary =
    ariaLabel ??
    (n === 0 || series.length === 0
      ? "No data"
      : `Queue occupancy for ${series
          .map((s) => s.label)
          .join(", ")} against a ${capacity}% capacity threshold`);

  return (
    <svg
      className={["cs-queue-line", className].filter(Boolean).join(" ")}
      viewBox={`0 0 ${width} ${height}`}
      style={{
        width: "100%",
        aspectRatio: `${width} / ${height}`,
        display: "block",
      }}
      role="img"
      aria-label={summary}
    >
      {showCapacityBand && capacity < 100 ? (
        <ThresholdBand
          x={plotL}
          width={plotW}
          y1={yAt(100)}
          y2={yAt(capacity)}
          color={capacityColor}
          boundary="bottom"
          label={capacityLabel}
        />
      ) : null}

      {showAxes ? (
        <>
          <rect
            x={plotL}
            y={plotT}
            width={plotW}
            height={plotH}
            fill="none"
            style={{ stroke: "var(--border)" }}
            strokeWidth={1}
          />
          {Y_TICKS.map((t) => (
            <text
              key={`yt-${t}`}
              x={plotL - 6}
              y={yAt(t)}
              textAnchor="end"
              dominantBaseline="central"
              style={AXIS_LABEL_STYLE}
            >
              {t}%
            </text>
          ))}
          {categories.map((c, i) => (
            <text
              key={`x-${i}`}
              x={xAt(i)}
              y={plotB + 14}
              textAnchor="middle"
              style={AXIS_LABEL_STYLE}
            >
              {c}
            </text>
          ))}
        </>
      ) : null}

      {series.map((s, i) => (
        <polyline
          key={`line-${i}`}
          points={s.data
            .map((v, j) => `${xAt(j).toFixed(1)},${yAt(v).toFixed(1)}`)
            .join(" ")}
          fill="none"
          style={{ stroke: s.color }}
          strokeWidth={2}
          strokeLinejoin="round"
          strokeLinecap="round"
          strokeDasharray={s.dashed ? "5 4" : undefined}
        />
      ))}
    </svg>
  );
}
