import type { ReactNode } from "react";
import Swatch, { type SwatchShape } from "./chartSwatch";
import "./Legend.css";

export interface LegendItem {
  /** Series/segment name. */
  label: ReactNode;
  /** Key color. Bind to a token (e.g. `var(--chart-1)`). */
  color: string;
  /** Marker shape. Defaults to a rounded `square`. */
  shape?: SwatchShape;
}

export interface LegendProps {
  /** Legend entries, in display order. */
  items: LegendItem[];
  /** Lay entries in a row (`horizontal`) or a column (`vertical`). */
  orientation?: "horizontal" | "vertical";
  /** Extra class(es) merged onto the list. */
  className?: string;
}

/**
 * A presentational chart legend — a list of color keys and their labels, meant
 * to sit beside/below a chart (the SVG charts stay plot-only). Renders a real
 * `<ul>`/`<li>` list for semantics; each key's color binds to a token via the
 * shared `Swatch` (never raw hex). Mirrors the Figma `Legend` master
 * (node 518:4262).
 */
export default function Legend({
  items,
  orientation = "horizontal",
  className,
}: LegendProps) {
  const classes = ["cs-legend", `cs-legend--${orientation}`, className]
    .filter(Boolean)
    .join(" ");
  return (
    <ul className={classes}>
      {items.map((item, i) => (
        <li key={i} className="cs-legend__item">
          <Swatch color={item.color} shape={item.shape} />
          <span className="cs-legend__label">{item.label}</span>
        </li>
      ))}
    </ul>
  );
}
