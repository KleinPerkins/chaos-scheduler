/**
 * Shared lookback-window contract for the dashboard's `(environment, lookback)`
 * filters. The UI exposes a fixed set of trailing windows plus a `custom`
 * (explicit date-range) sentinel; `1d` is the default.
 *
 * NOTE: the Rust backend accepts a broader lookback *grammar* over the wire
 * (`1d` / `3d` / `7d` / `30d` / `<n>h` / `all`) and resolves the window
 * server-side. This module models the UI-facing contract, provides a
 * client-side resolver for surfaces that need concrete bounds (e.g. a
 * custom-range picker), and serializes a selection back to that existing
 * grammar via {@link lookbackToParam} — it does not invent a parallel wire
 * format (the presets already *are* valid grammar, and a `custom` range
 * serializes to `<n>h`).
 */

/** A lookback window: one of the trailing presets or the `custom` sentinel. */
export type Lookback = "1d" | "3d" | "7d" | "30d" | "custom";

/** The default lookback window (the documented filter-bar default). */
export const DEFAULT_LOOKBACK: Lookback = "1d";

/** Trailing preset windows, in display order (the `custom` sentinel excluded). */
export const LOOKBACK_PRESETS: readonly Lookback[] = ["1d", "3d", "7d", "30d"];

/** Trailing length, in days, of each preset window. */
const PRESET_DAYS: Record<Exclude<Lookback, "custom">, number> = {
  "1d": 1,
  "3d": 3,
  "7d": 7,
  "30d": 30,
};

const DAY_MS = 24 * 60 * 60 * 1000;
const HOUR_MS = 60 * 60 * 1000;

/** A resolved, concrete date range (`start` inclusive, `end` the window edge). */
export interface DateRange {
  start: Date;
  end: Date;
}

/** Options for {@link resolveLookbackRange}. */
export interface ResolveLookbackOptions {
  /** Reference "now" for trailing presets. Defaults to the current time. */
  now?: Date;
  /** Start bound for a `custom` window (required when lookback is `custom`). */
  customStart?: Date;
  /** End bound for a `custom` window (required when lookback is `custom`). */
  customEnd?: Date;
}

/**
 * Resolve a {@link Lookback} to a concrete {@link DateRange}. Trailing presets
 * end at `now` (default: the current time) and start `N` days earlier. A
 * `custom` window returns the supplied `customStart` / `customEnd` bounds and
 * throws if either is missing.
 */
export function resolveLookbackRange(
  lookback: Lookback,
  options: ResolveLookbackOptions = {},
): DateRange {
  if (lookback === "custom") {
    const { customStart, customEnd } = options;
    if (!customStart || !customEnd) {
      throw new Error(
        'resolveLookbackRange("custom") requires customStart and customEnd',
      );
    }
    return { start: customStart, end: customEnd };
  }
  const now = options.now ?? new Date();
  const end = new Date(now.getTime());
  const start = new Date(now.getTime() - PRESET_DAYS[lookback] * DAY_MS);
  return { start, end };
}

/**
 * Serialize a {@link Lookback} to the backend's existing lookback grammar
 * (the string every dashboard/`get_dashboard_*` command already accepts):
 *
 *   - the trailing presets (`1d` / `3d` / `7d` / `30d`) pass through
 *     unchanged — they are already valid `<n>d` grammar, so no parallel
 *     format is introduced;
 *   - a `custom` window resolves its bounds (via {@link resolveLookbackRange})
 *     and serializes to `<n>h`, rounding the span *up* to whole hours (the
 *     backend treats the trailing window inclusively, so rounding up never
 *     drops the earliest bucket). It throws if the range is missing or
 *     non-positive, mirroring {@link resolveLookbackRange}.
 *
 * (The grammar's `all` all-time sentinel is not produced by the current preset
 * set — 1d/3d/7d/30d/custom — but remains available over the wire for any
 * future all-time affordance.)
 */
export function lookbackToParam(
  lookback: Lookback,
  options: ResolveLookbackOptions = {},
): string {
  if (lookback !== "custom") {
    return lookback;
  }
  const { start, end } = resolveLookbackRange("custom", options);
  const spanMs = end.getTime() - start.getTime();
  if (!Number.isFinite(spanMs) || spanMs <= 0) {
    throw new Error(
      'lookbackToParam("custom") requires customEnd to be after customStart',
    );
  }
  return `${Math.ceil(spanMs / HOUR_MS)}h`;
}
