import type { CSSProperties } from "react";
import "./chartSwatch.css";

/** Marker style for a series/segment key in a {@link Legend} or ChartTooltip. */
export type SwatchShape = "square" | "dot" | "line" | "dashed";

export interface SwatchProps {
  /** Marker color. Bind to a token, e.g. `var(--chart-1)` / `var(--success)`. */
  color: string;
  /** Marker geometry. Defaults to a rounded `square`. */
  shape?: SwatchShape;
  /** Extra class(es) merged onto the swatch. */
  className?: string;
}

/**
 * Tiny presentational color key shared by `Legend` and `ChartTooltip`. Renders a
 * decorative (`aria-hidden`) marker whose color is passed through a CSS custom
 * property so the shape styling stays in `chartSwatch.css` and only the token
 * color is inline — never a raw hex.
 */
export default function Swatch({
  color,
  shape = "square",
  className,
}: SwatchProps) {
  const classes = ["cs-swatch", `cs-swatch--${shape}`, className]
    .filter(Boolean)
    .join(" ");
  return (
    <span
      className={classes}
      style={{ "--cs-swatch-color": color } as CSSProperties}
      aria-hidden="true"
    />
  );
}
