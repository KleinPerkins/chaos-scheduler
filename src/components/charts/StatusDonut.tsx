import type { ReactNode } from "react";
import "./StatusDonut.css";

export interface StatusDonutSegment {
  /** Segment name (e.g. `Succeeded`). Used in the accessible summary. */
  label: string;
  /** Raw count/value for the segment. */
  value: number;
  /** Segment color; bind to a token (e.g. `var(--success)`). */
  color: string;
}

export interface StatusDonutProps {
  /** Segments in draw order (clockwise from 12 o'clock). */
  segments: StatusDonutSegment[];
  /** Rendered square size in px (viewBox side + intrinsic width). Default 180. */
  size?: number;
  /** Ring thickness in user units. Defaults to `22`. */
  thickness?: number;
  /** Override the center big number. Defaults to the segment total. */
  centerValue?: ReactNode;
  /** Sub-label under the center number. Defaults to `total`. */
  centerLabel?: ReactNode;
  /** Track color shown when the segments sum to zero. `var(--bg-tertiary)`. */
  trackColor?: string;
  /** Accessible summary; auto-generated from the data when omitted. */
  ariaLabel?: string;
  /** Extra class(es) merged onto the root `<svg>`. */
  className?: string;
}

const RING_PAD = 4;

/**
 * Status-distribution donut — a full ring split into value-weighted segments
 * with a center total. Presentational and props-driven; segment colors bind to
 * tokens via props (status vars like `var(--success)` / `var(--error)`, never raw
 * hex) and it renders in dark and light. Responsive: scales down to its
 * container, capped at `size`. Mirrors the Figma `StatusDonut` master
 * (node 524:4262).
 */
export default function StatusDonut({
  segments,
  size = 180,
  thickness = 22,
  centerValue,
  centerLabel = "total",
  trackColor = "var(--bg-tertiary)",
  ariaLabel,
  className,
}: StatusDonutProps) {
  const total = segments.reduce((sum, s) => sum + Math.max(0, s.value), 0);
  const cx = size / 2;
  const cy = size / 2;
  const r = size / 2 - thickness / 2 - RING_PAD;
  const circumference = 2 * Math.PI * r;

  let acc = 0;
  const arcs =
    total > 0
      ? segments.map((s, i) => {
          const len = (Math.max(0, s.value) / total) * circumference;
          const el = (
            <circle
              key={i}
              cx={cx}
              cy={cy}
              r={r}
              fill="none"
              style={{ stroke: s.color }}
              strokeWidth={thickness}
              strokeDasharray={`${len} ${circumference - len}`}
              strokeDashoffset={-acc}
            />
          );
          acc += len;
          return el;
        })
      : null;

  const bigNumber = centerValue ?? total.toLocaleString();
  const summary =
    ariaLabel ??
    (total > 0
      ? `${total.toLocaleString()} ${String(centerLabel)}: ${segments
          .map((s) => `${s.value} ${s.label}`)
          .join(", ")}`
      : `No data`);

  return (
    <svg
      className={["cs-donut", className].filter(Boolean).join(" ")}
      viewBox={`0 0 ${size} ${size}`}
      style={{ maxWidth: size }}
      role="img"
      aria-label={summary}
    >
      {/* rotate so the first segment starts at 12 o'clock */}
      <g transform={`rotate(-90 ${cx} ${cy})`}>
        {total > 0 ? (
          arcs
        ) : (
          <circle
            cx={cx}
            cy={cy}
            r={r}
            fill="none"
            style={{ stroke: trackColor }}
            strokeWidth={thickness}
          />
        )}
      </g>
      <text
        className="cs-donut__value"
        x={cx}
        y={cy - 4}
        textAnchor="middle"
        dominantBaseline="central"
      >
        {bigNumber}
      </text>
      <text
        className="cs-donut__sub"
        x={cx}
        y={cy + 16}
        textAnchor="middle"
        dominantBaseline="central"
      >
        {centerLabel}
      </text>
    </svg>
  );
}
