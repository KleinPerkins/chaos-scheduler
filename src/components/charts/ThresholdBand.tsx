import type { CSSProperties } from "react";

/** Corner of the band the optional label is anchored to. */
export type ThresholdBandLabelPlacement =
  "top-left" | "top-right" | "bottom-left" | "bottom-right";

/** Which horizontal edge(s) of the band get a dashed boundary line. */
export type ThresholdBandBoundary = "none" | "top" | "bottom" | "both";

export interface ThresholdBandProps {
  /** Left edge of the band, in SVG user units. */
  x: number;
  /** Band width, in SVG user units. */
  width: number;
  /** One vertical edge of the band, in SVG user units (order-independent). */
  y1: number;
  /** The other vertical edge, in SVG user units (order-independent). */
  y2: number;
  /** Fill/label/boundary color. Bind to a token (e.g. `var(--warning)`). */
  color?: string;
  /** Opacity of the tinted fill. Defaults to `0.12` for a soft wash. */
  fillOpacity?: number;
  /** Which edge(s) render a dashed boundary line. Defaults to `none`. */
  boundary?: ThresholdBandBoundary;
  /** Dash pattern for boundary lines. Defaults to `"4 3"`. */
  boundaryDasharray?: string;
  /** Opacity of the boundary line(s). Defaults to `0.55`. */
  boundaryOpacity?: number;
  /** Optional short label drawn in a corner of the band. */
  label?: string;
  /** Which corner the label sits in. Defaults to `top-right`. */
  labelPlacement?: ThresholdBandLabelPlacement;
  /** Extra class(es) merged onto the `<g>`. */
  className?: string;
}

const LABEL_INSET = 4;

/**
 * A horizontal threshold/zone band for a chart plot — a tinted rectangle
 * spanning a value range (e.g. a "near-capacity" or "warn" zone) with optional
 * dashed edge line(s) and a corner label. Presentational SVG: it renders a `<g>`
 * of user-space geometry and must live inside an `<svg>`; the caller resolves
 * `y1`/`y2` through its own y-scale. All colors bind to tokens via props — never
 * a raw hex. Mirrors the Figma `ThresholdBand` master (node 513:4262).
 */
export default function ThresholdBand({
  x,
  width,
  y1,
  y2,
  color = "var(--warning)",
  fillOpacity = 0.12,
  boundary = "none",
  boundaryDasharray = "4 3",
  boundaryOpacity = 0.55,
  label,
  labelPlacement = "top-right",
  className,
}: ThresholdBandProps) {
  const top = Math.min(y1, y2);
  const bottom = Math.max(y1, y2);
  const height = bottom - top;
  const drawTop = boundary === "top" || boundary === "both";
  const drawBottom = boundary === "bottom" || boundary === "both";

  const onTop = labelPlacement.startsWith("top");
  const onLeft = labelPlacement.endsWith("left");
  const labelX = onLeft ? x + LABEL_INSET : x + width - LABEL_INSET;
  const labelY = onTop ? top + 11 : bottom - 5;

  return (
    <g className={["cs-threshold-band", className].filter(Boolean).join(" ")}>
      <rect
        x={x}
        y={top}
        width={width}
        height={height}
        style={{ fill: color }}
        fillOpacity={fillOpacity}
      />
      {drawTop ? (
        <line
          x1={x}
          y1={top}
          x2={x + width}
          y2={top}
          style={{ stroke: color }}
          strokeWidth={1}
          strokeDasharray={boundaryDasharray}
          strokeOpacity={boundaryOpacity}
        />
      ) : null}
      {drawBottom ? (
        <line
          x1={x}
          y1={bottom}
          x2={x + width}
          y2={bottom}
          style={{ stroke: color }}
          strokeWidth={1}
          strokeDasharray={boundaryDasharray}
          strokeOpacity={boundaryOpacity}
        />
      ) : null}
      {label ? (
        <text
          x={labelX}
          y={labelY}
          textAnchor={onLeft ? "start" : "end"}
          style={{ ...LABEL_STYLE, fill: color }}
        >
          {label}
        </text>
      ) : null}
    </g>
  );
}

const LABEL_STYLE: CSSProperties = {
  fontFamily: "var(--font-sans)",
  fontSize: "var(--font-size-xs)",
  fontWeight: 600,
};
