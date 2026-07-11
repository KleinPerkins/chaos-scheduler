/**
 * Bespoke SVG chart primitive library (fixed decision D07: hand-built on
 * d3-scale / d3-shape, no charting library). All components are presentational
 * and props-driven — they take every value via props and do no data-fetching —
 * and bind color/space/type to the design tokens (never raw hex). Consumers
 * compose the plot charts with the `Axis` / `Legend` / `ChartTooltip` primitives.
 */

// Primitives
export { default as ThresholdBand } from "./ThresholdBand";
export type {
  ThresholdBandProps,
  ThresholdBandBoundary,
  ThresholdBandLabelPlacement,
} from "./ThresholdBand";

export { default as Axis } from "./Axis";
export type { AxisProps, AxisTick, AxisOrientation } from "./Axis";

export { default as Legend } from "./Legend";
export type { LegendProps, LegendItem } from "./Legend";

export { default as ChartTooltip } from "./ChartTooltip";
export type { ChartTooltipProps, ChartTooltipRow } from "./ChartTooltip";

export { default as Swatch } from "./chartSwatch";
export type { SwatchProps, SwatchShape } from "./chartSwatch";

// Scale + tick helpers
export { niceLinearDomain, linearAxisTicks, timeTicks } from "./scales";
export type { LinearAxisTick, TimeTick } from "./scales";
