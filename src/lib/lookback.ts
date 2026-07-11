/**
 * Shared lookback-window contract for the dashboard's `(environment, lookback)`
 * filters. The UI exposes a fixed set of trailing windows plus a `custom`
 * (explicit date-range) sentinel; `1d` is the default.
 *
 * NOTE: the Rust backend accepts a broader lookback *grammar* over the wire
 * (`1d` / `3d` / `7d` / `30d` / `<n>h` / `all`) and resolves the window
 * server-side. This module models the UI-facing contract and provides a
 * client-side resolver for surfaces that need concrete bounds (e.g. a
 * custom-range picker) — it does not change how existing calls talk to the
 * backend (they still pass the lookback string).
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
