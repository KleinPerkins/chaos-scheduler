import { niceLinearDomain } from "./scales";

export interface LineSeries {
  /** Series name (used in the accessible summary). */
  label: string;
  /** y-values, one per x category (aligned by index). */
  data: number[];
  /** Stroke color; bind to a token (e.g. `var(--chart-1)`). */
  color: string;
  /** Render the line dashed. Defaults to `false`. */
  dashed?: boolean;
}

export interface DualAxisBaseline {
  /** Value on the LEFT y-scale where the dashed reference line sits. */
  value: number;
  /** Line color. Defaults to `var(--border-strong)`. */
  color?: string;
  /** Optional short label drawn above the line's right end. */
  label?: string;
}

export interface DualAxisLineProps {
  /** X categories (labels), shared by both axes. */
  categories: string[];
  /** Series plotted against the left y-scale. */
  leftSeries: LineSeries[];
  /** Series plotted against the right y-scale (independent scale). */
  rightSeries?: LineSeries[];
  /** Dashed reference line(s) on the left scale (e.g. an SLA baseline). */
  baselines?: DualAxisBaseline[];
  /** viewBox width. Defaults to `360`. */
  width?: number;
  /** viewBox height. Defaults to `140`. */
  height?: number;
  /** Draw y grid + tick labels and x category labels. Defaults to `false`. */
  showAxes?: boolean;
  /** Override the auto left domain `[min, max]`. */
  leftDomain?: [number, number];
  /** Override the auto right domain `[min, max]`. */
  rightDomain?: [number, number];
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

const GRID_FRACS = [0, 0.25, 0.5, 0.75, 1] as const;

function polyline(
  data: number[],
  xAt: (i: number) => number,
  y: (v: number) => number,
): string {
  return data
    .map((v, i) => `${xAt(i).toFixed(1)},${y(v).toFixed(1)}`)
    .join(" ");
}

/**
 * Dual-axis line chart — left- and right-scale series drawn on one plot, each
 * against its own independent y-domain, with optional dashed baseline reference
 * line(s). Presentational and props-driven; strokes bind to tokens via props
 * (never raw hex) and it renders in dark/light. Responsive: fills its container
 * at a fixed aspect ratio. Compact by default (no axis chrome, matching the
 * master); pass `showAxes` for gridlines + tick labels. Mirrors the Figma
 * `DualAxisLine` master (node 521:4262).
 */
export default function DualAxisLine({
  categories,
  leftSeries,
  rightSeries = [],
  baselines = [],
  width = 360,
  height = 140,
  showAxes = false,
  leftDomain,
  rightDomain,
  ariaLabel,
  className,
}: DualAxisLineProps) {
  const n = categories.length;
  const pad = showAxes
    ? { l: 40, r: rightSeries.length ? 44 : 12, t: 14, b: 22 }
    : { l: 4, r: 4, t: 8, b: 8 };
  const plotL = pad.l;
  const plotR = width - pad.r;
  const plotT = pad.t;
  const plotB = height - pad.b;
  const plotW = plotR - plotL;
  const plotH = plotB - plotT;

  const xAt = (i: number) =>
    n <= 1 ? (plotL + plotR) / 2 : plotL + (i / (n - 1)) * plotW;

  const leftDom =
    leftDomain ??
    niceLinearDomain([
      ...leftSeries.flatMap((s) => s.data),
      ...baselines.map((b) => b.value),
    ]);
  const rightDom =
    rightDomain ?? niceLinearDomain(rightSeries.flatMap((s) => s.data));

  const spanY = (v: number, dom: [number, number]) => {
    const [lo, hi] = dom;
    const t = hi > lo ? (v - lo) / (hi - lo) : 0;
    return plotB - t * plotH;
  };
  const yL = (v: number) => spanY(v, leftDom);
  const yR = (v: number) => spanY(v, rightDom);

  const allSeries = [...leftSeries, ...rightSeries];
  const summary =
    ariaLabel ??
    (n === 0 || allSeries.length === 0
      ? "No data"
      : `Line chart of ${allSeries
          .map((s) => s.label)
          .join(", ")} across ${n} intervals`);

  return (
    <svg
      className={["cs-dual-axis-line", className].filter(Boolean).join(" ")}
      viewBox={`0 0 ${width} ${height}`}
      style={{
        width: "100%",
        aspectRatio: `${width} / ${height}`,
        display: "block",
      }}
      role="img"
      aria-label={summary}
    >
      {showAxes
        ? GRID_FRACS.map((f, i) => {
            const y = plotB - f * plotH;
            return (
              <g key={`grid-${i}`}>
                <line
                  x1={plotL}
                  y1={y}
                  x2={plotR}
                  y2={y}
                  style={{ stroke: "var(--border)" }}
                  strokeWidth={1}
                />
                <text
                  x={plotL - 6}
                  y={y}
                  textAnchor="end"
                  dominantBaseline="central"
                  style={AXIS_LABEL_STYLE}
                >
                  {Math.round(leftDom[0] + f * (leftDom[1] - leftDom[0]))}
                </text>
                {rightSeries.length ? (
                  <text
                    x={plotR + 6}
                    y={y}
                    textAnchor="start"
                    dominantBaseline="central"
                    style={AXIS_LABEL_STYLE}
                  >
                    {Math.round(rightDom[0] + f * (rightDom[1] - rightDom[0]))}
                  </text>
                ) : null}
              </g>
            );
          })
        : null}

      {showAxes
        ? categories.map((c, i) => (
            <text
              key={`x-${i}`}
              x={xAt(i)}
              y={plotB + 14}
              textAnchor="middle"
              style={AXIS_LABEL_STYLE}
            >
              {c}
            </text>
          ))
        : null}

      {baselines.map((b, i) => (
        <g key={`baseline-${i}`}>
          <line
            x1={plotL}
            y1={yL(b.value)}
            x2={plotR}
            y2={yL(b.value)}
            style={{ stroke: b.color ?? "var(--border-strong)" }}
            strokeWidth={1}
            strokeDasharray="4 3"
          />
          {b.label ? (
            <text
              x={plotR}
              y={yL(b.value) - 4}
              textAnchor="end"
              style={AXIS_LABEL_STYLE}
            >
              {b.label}
            </text>
          ) : null}
        </g>
      ))}

      {leftSeries.map((s, i) => (
        <polyline
          key={`l-${i}`}
          points={polyline(s.data, xAt, yL)}
          fill="none"
          style={{ stroke: s.color }}
          strokeWidth={2}
          strokeLinejoin="round"
          strokeLinecap="round"
          strokeDasharray={s.dashed ? "5 4" : undefined}
        />
      ))}
      {rightSeries.map((s, i) => (
        <polyline
          key={`r-${i}`}
          points={polyline(s.data, xAt, yR)}
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
