import type { CSSProperties, ReactNode } from "react";
import "./Vehicle.css";

/** Vehicle silhouette. Mirrors the Figma `Style` variant of set 559:4262. */
export type VehicleStyle = "sedan" | "coupe" | "racer" | "truck";

/** Vehicle body color. Mirrors the Figma `Color` variant of set 559:4262. */
export type VehicleColor = "blue" | "teal" | "amber";

export interface VehicleProps {
  /** Body silhouette. Mirrors the Figma `Style` variant. */
  style: VehicleStyle;
  /** Body color, bound to a token. Mirrors the Figma `Color` variant. Default `blue`. */
  color?: VehicleColor;
  /**
   * Over/error state — replaces the body color with `var(--error)` (the Figma
   * over-state, e.g. the racer for an overrunning job). Default `false`.
   */
  over?: boolean;
  /** Rendered width in px. Defaults to the variant's intrinsic width. */
  width?: number;
  /** Rendered height in px. Defaults to the variant's intrinsic height. */
  height?: number;
  /** Accessible label; auto-generated from `color`/`style` when omitted. */
  ariaLabel?: string;
  /**
   * Render decoratively (`aria-hidden`, no `role`) — used when embedded in a
   * labelled parent such as {@link RaceTrack}. Default `false`.
   */
  decorative?: boolean;
  /** Extra class(es) merged onto the root `<svg>`. */
  className?: string;
}

// Intrinsic viewBox per style, straight from the Figma masters (set 559:4262):
// Sedan/Coupe/Racer share a 36×16 box (the racer is 16.1 tall so its larger
// open wheels aren't clipped); the Truck is a longer 60×18 tractor-trailer.
const DIMS: Record<VehicleStyle, { w: number; h: number }> = {
  sedan: { w: 36, h: 16 },
  coupe: { w: 36, h: 16 },
  racer: { w: 36, h: 16.1 },
  truck: { w: 60, h: 18 },
};

// Body colors bind to the same `cs.*`-mirrored tokens the Figma set uses.
const COLOR_VAR: Record<VehicleColor, string> = {
  blue: "var(--running)",
  teal: "var(--chart-3)",
  amber: "var(--warning)",
};

// Shared part fills (also straight from the Figma set): glass + tires read the
// card background, hubs the secondary-text tone, the headlight the warning amber.
const GLASS: CSSProperties = { fill: "var(--bg-secondary)" };
const TIRE: CSSProperties = { fill: "var(--bg-secondary)" };
const HUB: CSSProperties = { fill: "var(--text-secondary)" };
const HEADLIGHT: CSSProperties = { fill: "var(--warning)" };

// Truck wheel centers — the mid axle is intentionally omitted (locked design).
const TRUCK_WHEELS = [5, 11, 23, 29, 41, 53];

function shapes(style: VehicleStyle, body: CSSProperties): ReactNode {
  switch (style) {
    case "sedan":
      return (
        <>
          <path
            d="M35.5 11V8L32.5 6.5L23.5 6L20.5 1.5H11.5L7.5 6L2.5 7L1 8.5V11H35.5Z"
            style={body}
          />
          <path d="M20 2.6H12.2L9.5 5.4H21.9L20 2.6Z" style={GLASS} />
          <circle cx={33.8} cy={8.4} r={1.2} style={HEADLIGHT} />
          <circle cx={9.5} cy={12} r={4} style={TIRE} />
          <circle cx={9.5} cy={12} r={1.5} style={HUB} />
          <circle cx={27.5} cy={12} r={4} style={TIRE} />
          <circle cx={27.5} cy={12} r={1.5} style={HUB} />
        </>
      );
    case "coupe":
      return (
        <>
          <path
            d="M1 11V9L3 8.5L13 2.5H19L24 5.5L33 6.5L35.5 8V11H1Z"
            style={body}
          />
          <path d="M12.5 3.2H18.5L22.5 5.3H10.5L12.5 3.2Z" style={GLASS} />
          <circle cx={34.1} cy={8.1} r={1.1} style={HEADLIGHT} />
          <circle cx={8.5} cy={11.75} r={4.25} style={TIRE} />
          <circle cx={8.5} cy={11.75} r={1.6} style={HUB} />
          <circle cx={28.5} cy={11.75} r={4.25} style={TIRE} />
          <circle cx={28.5} cy={11.75} r={1.6} style={HUB} />
        </>
      );
    case "racer":
      // Open-wheel F1 car: rear wing (plane + post), narrow body, front wing,
      // a centered cockpit bubble, and two large exposed wheels with centered
      // hubcaps. No number roundel (intentionally removed in the locked design).
      return (
        <>
          <rect x={0} y={2.6} width={6} height={1.6} rx={0.5} style={body} />
          <rect x={1.2} y={3} width={1.6} height={4} style={body} />
          <path
            d="M3 10V7.5L10 7L12.5 4.8H17L19.5 7L31 7.4L35.5 8.6V10H3Z"
            style={body}
          />
          <rect
            x={31.5}
            y={8.8}
            width={4.5}
            height={1.4}
            rx={0.5}
            style={body}
          />
          <ellipse cx={13.2} cy={5.5} rx={1.4} ry={1.1} style={GLASS} />
          <circle cx={8} cy={11.35} r={4.75} style={TIRE} />
          <circle cx={8} cy={11.35} r={1.8} style={HUB} />
          <circle cx={28.5} cy={11.35} r={4.75} style={TIRE} />
          <circle cx={28.5} cy={11.35} r={1.8} style={HUB} />
        </>
      );
    case "truck":
      return (
        <>
          <rect x={0} y={1} width={36} height={11} rx={2} style={body} />
          <rect x={39} y={2} width={9} height={10} rx={1} style={body} />
          <rect x={48} y={6} width={10} height={6} rx={1} style={body} />
          <rect
            x={43}
            y={3.3}
            width={4.5}
            height={3.2}
            rx={0.5}
            style={GLASS}
          />
          {TRUCK_WHEELS.map((cx) => (
            <circle key={`t${cx}`} cx={cx} cy={13} r={3} style={TIRE} />
          ))}
          {TRUCK_WHEELS.map((cx) => (
            <circle key={`h${cx}`} cx={cx} cy={13} r={1} style={HUB} />
          ))}
        </>
      );
  }
}

/**
 * Bespoke presentational vehicle glyph — a flat, right-facing car whose
 * silhouette (`sedan` / `coupe` / `racer` / `truck`) and body color
 * (`blue` / `teal` / `amber`) mirror the 12-variant Figma set. Props-driven and
 * token-bound (never a raw hex); it renders in dark and light. The `over` flag
 * applies the status-red body override. Sized to its intrinsic viewBox by
 * default; renders as a nested `<svg>` so it composes inside another chart's
 * user space (e.g. {@link RaceTrack}). Mirrors the Figma `Vehicle` set
 * (node 559:4262).
 */
export default function Vehicle({
  style,
  color = "blue",
  over = false,
  width,
  height,
  ariaLabel,
  decorative = false,
  className,
}: VehicleProps) {
  const { w, h } = DIMS[style];
  const bodyStyle: CSSProperties = {
    fill: over ? "var(--error)" : COLOR_VAR[color],
  };
  const label = ariaLabel ?? `${over ? "overrunning " : ""}${color} ${style}`;
  const a11y = decorative
    ? ({ "aria-hidden": true } as const)
    : ({ role: "img", "aria-label": label } as const);

  return (
    <svg
      className={["cs-vehicle", className].filter(Boolean).join(" ")}
      viewBox={`0 0 ${w} ${h}`}
      width={width ?? w}
      height={height ?? h}
      overflow="visible"
      data-vehicle-style={style}
      data-vehicle-color={color}
      data-vehicle-over={over ? "" : undefined}
      {...a11y}
    >
      {shapes(style, bodyStyle)}
    </svg>
  );
}
