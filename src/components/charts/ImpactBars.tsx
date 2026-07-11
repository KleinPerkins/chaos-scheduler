import type { CSSProperties, ReactNode } from "react";
import "./ImpactBars.css";

export interface ImpactBarItem {
  /** Row label (e.g. `Resource lock`). */
  label: ReactNode;
  /** Numeric magnitude that drives the bar length and ranking. */
  value: number;
  /** Bar color. Defaults to the categorical palette by rank. */
  color?: string;
  /** Preformatted value label (e.g. `4h 12m`). Defaults to `value`. */
  valueLabel?: ReactNode;
}

export interface ImpactBarsProps {
  /** Rows to plot. */
  items: ImpactBarItem[];
  /** Sort descending by `value` (ranked) before rendering. Default `true`. */
  sort?: boolean;
  /** Scale maximum. Defaults to the largest item value. */
  max?: number;
  /** Accessible name for the list. */
  ariaLabel?: string;
  /** Extra class(es) merged onto the list. */
  className?: string;
}

// Categorical series palette (mirrors --chart-1..8 in tokens.css); rank order is
// the design's default ImpactBars ramp (violet, orange, teal, …).
const PALETTE = [
  "var(--chart-1)",
  "var(--chart-2)",
  "var(--chart-3)",
  "var(--chart-4)",
  "var(--chart-5)",
  "var(--chart-6)",
  "var(--chart-7)",
  "var(--chart-8)",
];

function clampPct(v: number): number {
  return Math.max(0, Math.min(100, v));
}

/**
 * Ranked horizontal bar chart — one bar per item over a neutral track, colored
 * from the categorical palette, with the row label on the left and its value on
 * the right. Presentational and props-driven; colors bind to tokens (never raw
 * hex) and it renders in dark/light. Semantic markup (an ordered list of
 * label + value) rather than `role="img"`, so screen readers read the data
 * directly; the bars are decorative. Responsive by layout. Mirrors the Figma
 * `ImpactBars` master (node 522:4262).
 */
export default function ImpactBars({
  items,
  sort = true,
  max,
  ariaLabel,
  className,
}: ImpactBarsProps) {
  const ordered = sort ? [...items].sort((a, b) => b.value - a.value) : items;
  const scaleMax = max ?? Math.max(0, ...ordered.map((it) => it.value));

  return (
    <ol
      className={["cs-impact", className].filter(Boolean).join(" ")}
      aria-label={ariaLabel}
    >
      {ordered.map((it, i) => {
        const pct = scaleMax > 0 ? clampPct((it.value / scaleMax) * 100) : 0;
        const color = it.color ?? PALETTE[i % PALETTE.length];
        return (
          <li key={i} className="cs-impact__row">
            <span className="cs-impact__label">{it.label}</span>
            <span className="cs-impact__track">
              <span
                className="cs-impact__bar"
                style={
                  { width: `${pct}%`, "--cs-bar-color": color } as CSSProperties
                }
                aria-hidden="true"
              />
            </span>
            <span className="cs-impact__value">
              {it.valueLabel ?? it.value}
            </span>
          </li>
        );
      })}
    </ol>
  );
}
