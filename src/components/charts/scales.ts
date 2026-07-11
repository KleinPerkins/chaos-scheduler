/**
 * Shared scale + tick helpers for the bespoke SVG chart primitives.
 *
 * Charts here are hand-built on `d3-scale` / `d3-shape` (+ `d3-array` /
 * `d3-time`) with NO charting library (fixed decision D07). This module holds
 * the small, pure, framework-free scale math the components share so each chart
 * stays presentational. Nothing here touches the DOM, React, or design tokens —
 * colors/spacing are bound at the component layer.
 */
import { extent } from "d3-array";
import { scaleLinear } from "d3-scale";
import { timeDay, timeHour, timeMinute, type TimeInterval } from "d3-time";

/** A computed axis tick: the data value and its pixel offset along the axis. */
export interface LinearAxisTick {
  /** The domain value at this tick. */
  value: number;
  /** Pixel offset from the axis origin (start), within `range`. */
  offset: number;
}

/** A computed time tick: an epoch-ms value and a short display label. */
export interface TimeTick {
  /** Epoch milliseconds at this tick. */
  value: number;
  /** Short, granularity-aware label (e.g. `14:00`, `Jul 4`). */
  label: string;
}

/**
 * A padded, "nice" numeric domain `[min, max]` for a set of values. Anchors to
 * zero by default (the usual baseline for counts / percentages) and rounds the
 * bounds to human-friendly values via a d3 linear scale. Empty input yields
 * `[0, 1]`; a flat series is widened by one unit so the range is never zero.
 */
export function niceLinearDomain(
  values: Iterable<number>,
  options: { zero?: boolean } = {},
): [number, number] {
  const { zero = true } = options;
  const [rawMin, rawMax] = extent(values);
  let min = rawMin ?? 0;
  let max = rawMax ?? 1;
  if (zero) {
    min = Math.min(0, min);
    max = Math.max(0, max);
  }
  if (min === max) max = min + 1;
  const [niceMin, niceMax] = scaleLinear().domain([min, max]).nice().domain();
  return [niceMin, niceMax];
}

/**
 * Evenly-spaced "nice" ticks for a numeric axis, each carrying its pixel
 * `offset` within `range`. The caller owns `domain` (typically from
 * {@link niceLinearDomain}) so the axis and the data plot share one scale — this
 * does NOT re-`nice()` the domain, keeping ticks aligned to the rendered marks.
 */
export function linearAxisTicks(
  domain: readonly [number, number],
  range: readonly [number, number],
  count = 5,
): LinearAxisTick[] {
  const scale = scaleLinear()
    .domain(domain as [number, number])
    .range(range as [number, number]);
  return scale.ticks(count).map((value) => ({ value, offset: scale(value) }));
}

const MINUTE_MS = 60_000;
const HOUR_MS = 60 * MINUTE_MS;
const DAY_MS = 24 * HOUR_MS;

function pad2(n: number): string {
  return String(n).padStart(2, "0");
}

/**
 * Granularity-aware time ticks across `[startMs, endMs]`, built on `d3-time`
 * intervals so tick boundaries land on whole minutes / hours / days rather than
 * arbitrary instants. Returned for consumers wiring a time-based x-axis (the
 * charts are categorical by default); labels are compact and locale-aware for
 * day granularity.
 */
export function timeTicks(
  startMs: number,
  endMs: number,
  targetCount = 6,
): TimeTick[] {
  if (!(endMs > startMs) || targetCount < 1) {
    return [{ value: startMs, label: labelFor(new Date(startMs), HOUR_MS) }];
  }
  const step = (endMs - startMs) / targetCount;
  let interval: TimeInterval;
  if (step >= DAY_MS) {
    interval = timeDay.every(Math.ceil(step / DAY_MS)) ?? timeDay;
  } else if (step >= HOUR_MS) {
    interval = timeHour.every(Math.ceil(step / HOUR_MS)) ?? timeHour;
  } else {
    interval =
      timeMinute.every(Math.max(1, Math.ceil(step / MINUTE_MS))) ?? timeMinute;
  }
  const start = interval.floor(new Date(startMs));
  return interval
    .range(start, new Date(endMs + 1))
    .filter((d) => +d >= startMs)
    .map((d) => ({ value: +d, label: labelFor(d, step) }));
}

function labelFor(d: Date, step: number): string {
  if (step >= DAY_MS) {
    return d.toLocaleDateString(undefined, { month: "short", day: "numeric" });
  }
  if (step >= HOUR_MS) {
    return `${pad2(d.getHours())}:00`;
  }
  return `${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
}
