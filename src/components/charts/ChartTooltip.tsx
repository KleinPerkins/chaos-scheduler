import type { CSSProperties, ReactNode } from "react";
import Swatch, { type SwatchShape } from "./chartSwatch";
import "./ChartTooltip.css";

export interface ChartTooltipRow {
  /** Series/segment name. */
  label: ReactNode;
  /** Formatted value for this row. */
  value: ReactNode;
  /** Optional key color (token var). When omitted, no swatch is drawn. */
  color?: string;
  /** Marker shape when a `color` is given. Defaults to a `dot`. */
  shape?: SwatchShape;
}

export interface ChartTooltipProps {
  /** Optional bold header (e.g. the hovered bucket/date). */
  header?: ReactNode;
  /** Value rows, in display order. */
  rows: ChartTooltipRow[];
  /** Extra class(es) merged onto the panel. */
  className?: string;
  /**
   * Inline style — the consumer positions the panel (e.g. `position: absolute`
   * with resolved coordinates); the panel itself is `pointer-events: none`.
   */
  style?: CSSProperties;
}

/**
 * A presentational chart tooltip panel — a bold header plus swatch/label/value
 * rows. Purely visual and controlled: the caller decides when it is shown and
 * positions it (hover wiring is an assembly-time concern). Surface/type bind to
 * tokens and key colors flow through the shared `Swatch` — never raw hex.
 * Mirrors the Figma `ChartTooltip` master (node 520:4262).
 */
export default function ChartTooltip({
  header,
  rows,
  className,
  style,
}: ChartTooltipProps) {
  return (
    <div
      className={["cs-chart-tooltip", className].filter(Boolean).join(" ")}
      role="tooltip"
      style={style}
    >
      {header != null ? (
        <div className="cs-chart-tooltip__header">{header}</div>
      ) : null}
      {rows.length > 0 ? (
        <div className="cs-chart-tooltip__rows">
          {rows.map((row, i) => (
            <div key={i} className="cs-chart-tooltip__row">
              <span className="cs-chart-tooltip__key">
                {row.color ? (
                  <Swatch color={row.color} shape={row.shape ?? "dot"} />
                ) : null}
                <span className="cs-chart-tooltip__label">{row.label}</span>
              </span>
              <span className="cs-chart-tooltip__value">{row.value}</span>
            </div>
          ))}
        </div>
      ) : null}
    </div>
  );
}
