import type { CSSProperties, ReactNode } from "react";
import { linearAxisTicks } from "./scales";

export type AxisOrientation = "bottom" | "top" | "left" | "right";

/** A single rendered tick: its pixel `offset` from the axis start + a label. */
export interface AxisTick {
  /** Pixel offset from the axis origin (the start passed by the parent). */
  offset: number;
  /** Tick label (usually a formatted value or a category name). */
  label: ReactNode;
}

export interface AxisProps {
  /** Which edge the axis represents; controls tick + label placement. */
  orientation: AxisOrientation;
  /** Axis length in SVG user units (plot width for x, plot height for y). */
  length: number;
  /**
   * Explicit ticks (offset from the axis start). Provide these for categorical
   * axes; omit and pass `domain` to auto-compute evenly-spaced numeric ticks.
   */
  ticks?: AxisTick[];
  /** Numeric domain to auto-compute ticks from when `ticks` is omitted. */
  domain?: readonly [number, number];
  /** Target tick count in `domain` mode. Defaults to `5`. */
  tickCount?: number;
  /** Label formatter in `domain` mode. Defaults to the value as-is. */
  tickFormat?: (value: number) => ReactNode;
  /** Tick mark length. Defaults to `5`. */
  tickSize?: number;
  /** Gap between a tick mark and its label. Defaults to `6`. */
  tickPadding?: number;
  /** Draw the axis domain line. Defaults to `true`. */
  showDomainLine?: boolean;
  /** Extra class(es) merged onto the `<g>`. */
  className?: string;
}

const LINE_STYLE: CSSProperties = { stroke: "var(--border)" };
const LABEL_STYLE: CSSProperties = {
  fill: "var(--text-muted)",
  fontFamily: "var(--font-sans)",
  fontSize: "var(--font-size-xs)",
};

/**
 * A presentational chart axis — the domain line, tick marks, and tick labels for
 * one edge of a plot. Render it inside an `<svg>` and position the `<g>` at the
 * axis origin (the left end for `bottom`/`top`, the top end for `left`/`right`);
 * offsets then grow along the axis. Ticks are either supplied (categorical) or
 * auto-derived from a numeric `domain` via a shared d3 linear scale. Colors/type
 * bind to tokens — never raw hex. Mirrors the Figma `Axis` master (node
 * 519:4262).
 */
export default function Axis({
  orientation,
  length,
  ticks,
  domain,
  tickCount = 5,
  tickFormat,
  tickSize = 5,
  tickPadding = 6,
  showDomainLine = true,
  className,
}: AxisProps) {
  const vertical = orientation === "left" || orientation === "right";
  const resolvedTicks: AxisTick[] =
    ticks ??
    (domain
      ? linearAxisTicks(
          domain,
          vertical ? [length, 0] : [0, length],
          tickCount,
        ).map((t) => ({
          offset: t.offset,
          label: tickFormat ? tickFormat(t.value) : String(t.value),
        }))
      : []);

  const domainLine = vertical
    ? { x1: 0, y1: 0, x2: 0, y2: length }
    : { x1: 0, y1: 0, x2: length, y2: 0 };

  return (
    <g
      className={["cs-axis", `cs-axis--${orientation}`, className]
        .filter(Boolean)
        .join(" ")}
    >
      {showDomainLine ? (
        <line {...domainLine} style={LINE_STYLE} strokeWidth={1} />
      ) : null}
      {resolvedTicks.map((tick, i) => (
        <Tick
          key={i}
          orientation={orientation}
          offset={tick.offset}
          label={tick.label}
          tickSize={tickSize}
          tickPadding={tickPadding}
        />
      ))}
    </g>
  );
}

interface TickProps {
  orientation: AxisOrientation;
  offset: number;
  label: ReactNode;
  tickSize: number;
  tickPadding: number;
}

function Tick({
  orientation,
  offset,
  label,
  tickSize,
  tickPadding,
}: TickProps) {
  let mark: { x1: number; y1: number; x2: number; y2: number };
  let labelX: number;
  let labelY: number;
  let textAnchor: "start" | "middle" | "end";
  let dominantBaseline: "auto" | "hanging" | "central";

  switch (orientation) {
    case "bottom":
      mark = { x1: offset, y1: 0, x2: offset, y2: tickSize };
      labelX = offset;
      labelY = tickSize + tickPadding;
      textAnchor = "middle";
      dominantBaseline = "hanging";
      break;
    case "top":
      mark = { x1: offset, y1: 0, x2: offset, y2: -tickSize };
      labelX = offset;
      labelY = -(tickSize + tickPadding);
      textAnchor = "middle";
      dominantBaseline = "auto";
      break;
    case "left":
      mark = { x1: 0, y1: offset, x2: -tickSize, y2: offset };
      labelX = -(tickSize + tickPadding);
      labelY = offset;
      textAnchor = "end";
      dominantBaseline = "central";
      break;
    case "right":
    default:
      mark = { x1: 0, y1: offset, x2: tickSize, y2: offset };
      labelX = tickSize + tickPadding;
      labelY = offset;
      textAnchor = "start";
      dominantBaseline = "central";
      break;
  }

  return (
    <g className="cs-axis__tick">
      <line {...mark} style={LINE_STYLE} strokeWidth={1} />
      <text
        x={labelX}
        y={labelY}
        textAnchor={textAnchor}
        dominantBaseline={dominantBaseline}
        style={LABEL_STYLE}
      >
        {label}
      </text>
    </g>
  );
}
