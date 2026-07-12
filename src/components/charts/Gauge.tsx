import type { ReactNode } from "react";
import "./Gauge.css";

/** Utilization % thresholds that switch the value-arc color. */
export interface GaugeThresholds {
  /** At/above this utilization %, the arc turns `warning`. Defaults to `70`. */
  warning: number;
  /** At/above this utilization %, the arc turns `danger`. Defaults to `90`. */
  danger: number;
}

export interface GaugeProps {
  /** Current filled amount (e.g. used execution slots). */
  value: number;
  /** Capacity `value` is measured against. Defaults to `100`. */
  max?: number;
  /** Rendered square size in px (viewBox side + intrinsic width). Default 160. */
  size?: number;
  /** Arc stroke width in user units. Defaults to `14`. */
  thickness?: number;
  /** Utilization thresholds for the value-arc color. Defaults `70` / `90`. */
  thresholds?: GaugeThresholds;
  /** Unit noun for the sub-label (`{value} of {max} {unit}`). Default `slots`. */
  unit?: string;
  /**
   * Override the auto (threshold-derived) value-arc color. Bind to a token
   * (e.g. `var(--chart-1)`); leave unset to use the green/amber/red status ramp.
   */
  valueColor?: string;
  /** Color of the unfilled track. Defaults to `var(--bg-tertiary)`. */
  trackColor?: string;
  /** Override the center sub-label entirely (replaces `{value} of {max} …`). */
  label?: ReactNode;
  /** Accessible summary; auto-generated from the data when omitted. */
  ariaLabel?: string;
  /** Extra class(es) merged onto the root `<svg>`. */
  className?: string;
}

// The gauge is a 270° arc with the opening centered at the bottom: the drawn
// sweep runs from the bottom-left (135°) clockwise over the top to the
// bottom-right (405° ≡ 45°). Angles use screen convention (y grows down):
// 0°→right, 90°→bottom, 180°→left, 270°→top.
const START_ANGLE = 135;
const SWEEP = 270;
const ARC_PAD = 2;

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

function polar(
  cx: number,
  cy: number,
  r: number,
  angleDeg: number,
): [number, number] {
  const rad = (angleDeg * Math.PI) / 180;
  return [cx + r * Math.cos(rad), cy + r * Math.sin(rad)];
}

function arcPath(
  cx: number,
  cy: number,
  r: number,
  fromFrac: number,
  toFrac: number,
): string {
  const a0 = START_ANGLE + fromFrac * SWEEP;
  const a1 = START_ANGLE + toFrac * SWEEP;
  const [x0, y0] = polar(cx, cy, r, a0);
  const [x1, y1] = polar(cx, cy, r, a1);
  const largeArc = a1 - a0 > 180 ? 1 : 0;
  return `M ${x0.toFixed(2)} ${y0.toFixed(2)} A ${r} ${r} 0 ${largeArc} 1 ${x1.toFixed(2)} ${y1.toFixed(2)}`;
}

/**
 * Execution-slots gauge — a 270° open-bottom arc whose value portion is colored
 * by utilization (green `< warning`, amber `warning–danger`, red `≥ danger`) over
 * a neutral track, with a center `N%` + `x of y {unit}` label. Presentational and
 * props-driven; all colors bind to tokens (never raw hex) and it renders in dark
 * and light. Responsive: scales down to its container, capped at `size`. Mirrors
 * the Figma `Gauge` master (node 516:4262).
 */
export default function Gauge({
  value,
  max = 100,
  size = 160,
  thickness = 14,
  thresholds = { warning: 70, danger: 90 },
  unit = "slots",
  valueColor,
  trackColor = "var(--bg-tertiary)",
  label,
  ariaLabel,
  className,
}: GaugeProps) {
  const safeMax = max > 0 ? max : 0;
  const pct = safeMax > 0 ? (value / safeMax) * 100 : 0;
  const valueFrac = clamp(pct / 100, 0, 1);
  const roundedPct = Math.round(pct);

  const arcColor =
    valueColor ??
    (pct >= thresholds.danger
      ? "var(--error)"
      : pct >= thresholds.warning
        ? "var(--warning)"
        : "var(--success)");

  const cx = size / 2;
  const cy = size / 2;
  const r = size / 2 - thickness / 2 - ARC_PAD;

  const subLabel = label ?? `${value} of ${safeMax} ${unit}`;
  const summary =
    ariaLabel ?? `${roundedPct}% utilized — ${value} of ${safeMax} ${unit}`;

  return (
    <svg
      className={["cs-gauge", className].filter(Boolean).join(" ")}
      viewBox={`0 0 ${size} ${size}`}
      style={{ maxWidth: size }}
      role="img"
      aria-label={summary}
    >
      <path
        d={arcPath(cx, cy, r, 0, 1)}
        fill="none"
        style={{ stroke: trackColor }}
        strokeWidth={thickness}
        strokeLinecap="round"
      />
      {valueFrac > 0 ? (
        <path
          d={arcPath(cx, cy, r, 0, valueFrac)}
          fill="none"
          style={{ stroke: arcColor }}
          strokeWidth={thickness}
          strokeLinecap="round"
        />
      ) : null}
      <text
        className="cs-gauge__value"
        x={cx}
        y={cy - 4}
        textAnchor="middle"
        dominantBaseline="central"
      >
        {roundedPct}%
      </text>
      <text
        className="cs-gauge__sub"
        x={cx}
        y={cy + 15}
        textAnchor="middle"
        dominantBaseline="central"
      >
        {subLabel}
      </text>
    </svg>
  );
}
