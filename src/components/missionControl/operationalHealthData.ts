/**
 * Pure, React-free transforms that turn the three "Operational Health" bindings
 * (src/lib/commands.ts) into the exact view-models the Operational Health
 * drill-down renders — the aggregate KPI rollup, the success/failure trend, and
 * the wait + runtime dual-axis duration trends (avg + max, with a per-bucket
 * 30-day baseline series). Kept side-effect-free and free of React (no
 * `Date.now()`, no fetching) so every mapping is unit-tested against fixed
 * inputs (determinism gate R10). Chart values are exposed in seconds; the
 * component converts to minutes for the plotted axes and uses `formatDuration`
 * for the accessible-table fallback. Colors bind to tokens (never raw hex).
 */
import type {
  DashboardKpiSummary,
  DashboardMetricBucket,
  DashboardWaitRuntimeTrend,
} from "../../lib/commands";
import { formatDuration } from "../../lib/duration";
import type { GroupTone } from "./groupCard";

// --- shared formatters -----------------------------------------------------

export function formatPercent1(rate: number | null): string {
  if (rate == null || !Number.isFinite(rate)) return "—";
  return `${(rate * 100).toFixed(1)}%`;
}

export function formatPerHour(value: number | null): string {
  if (value == null || !Number.isFinite(value)) return "—";
  return `${value.toFixed(1)}/hr`;
}

export function formatSeconds(value: number | null): string {
  if (value == null || !Number.isFinite(value)) return "—";
  return formatDuration(value * 1000);
}

// --- aggregate KPI rollup --------------------------------------------------

export interface OpHealthStat {
  key: string;
  label: string;
  value: string;
}

/**
 * The aggregate operational KPIs for the drill-down header grid, straight from
 * `getDashboardKpiSummary` (windowed). Durations are formatted with the shared
 * duration utility; rates/throughput with their unit suffixes.
 */
export function aggregateStats(kpi: DashboardKpiSummary): OpHealthStat[] {
  return [
    {
      key: "runs",
      label: "Total runs",
      value: kpi.total_runs.toLocaleString(),
    },
    {
      key: "success",
      label: "Success rate",
      value: formatPercent1(kpi.success_rate),
    },
    {
      key: "throughput",
      label: "Throughput",
      value: formatPerHour(kpi.throughput_per_hour),
    },
    {
      key: "avg-runtime",
      label: "Avg runtime",
      value: formatSeconds(kpi.avg_runtime_seconds),
    },
    {
      key: "median-wait",
      label: "Median wait",
      value: formatSeconds(kpi.median_wait_seconds),
    },
    {
      key: "max-wait",
      label: "Max wait",
      value: formatSeconds(kpi.max_wait_seconds),
    },
  ];
}

// --- wait / runtime duration trend ----------------------------------------

/** Deterministic, locale-free bucket label: `HH:MM` (hour grain) or `MM-DD`. */
function bucketLabel(iso: string, grain: "hour" | "day"): string {
  const d = new Date(iso);
  if (!Number.isFinite(d.getTime())) return iso;
  const s = d.toISOString();
  return grain === "hour" ? s.slice(11, 16) : s.slice(5, 10);
}

export interface MetricTrendChart {
  categories: string[];
  /** Raw seconds (nullable) for the accessible-table fallback. */
  avgSeconds: (number | null)[];
  maxSeconds: (number | null)[];
  baselineSeconds: (number | null)[];
  /** True when at least one bucket carries a sample. */
  hasData: boolean;
}

/**
 * Turn one metric-bucket series (wait or runtime) into aligned arrays for the
 * dual-axis chart and its accessible table. Null (no-sample) buckets are kept
 * null for the table; the component plots them at 0 (the primitive draws no
 * gaps) — the fixed fixtures carry no nulls, so baselines stay clean.
 */
export function metricTrendToChart(
  buckets: DashboardMetricBucket[],
  grain: "hour" | "day",
): MetricTrendChart {
  const categories: string[] = [];
  const avgSeconds: (number | null)[] = [];
  const maxSeconds: (number | null)[] = [];
  const baselineSeconds: (number | null)[] = [];
  let hasData = false;
  for (const b of buckets) {
    categories.push(bucketLabel(b.bucket, grain));
    avgSeconds.push(b.avg_seconds);
    maxSeconds.push(b.max_seconds);
    baselineSeconds.push(b.baseline_avg_seconds);
    if (b.count > 0) hasData = true;
  }
  return { categories, avgSeconds, maxSeconds, baselineSeconds, hasData };
}

/** Chart-ready minutes (null → 0, since the polyline primitive draws no gaps). */
export function toMinutes(seconds: (number | null)[]): number[] {
  return seconds.map((s) => (s == null ? 0 : s / 60));
}

// --- at-a-glance summary ---------------------------------------------------

export interface OperationalHealthSummaryModel {
  successRate: number | null;
  throughputPerHour: number | null;
  totalRuns: number;
  avgRuntimeSeconds: number | null;
  medianWaitSeconds: number | null;
  /** Latest runtime avg vs its 30-day baseline: `up` = regressed (slower). */
  runtimeTrend: "up" | "down" | "flat" | null;
  tone: GroupTone;
  /** One-line summary for the collapsed card + the drill-down header. */
  headline: string;
}

const DELTA_EPSILON = 1e-9;
/** A regression margin: latest avg beyond baseline * this bumps the tone. */
const RUNTIME_REGRESSION_FACTOR = 1.2;

/** Latest non-null value of a metric series (the most recent populated bucket). */
function latestNonNull(values: (number | null)[]): number | null {
  for (let i = values.length - 1; i >= 0; i--) {
    const v = values[i];
    if (v != null && Number.isFinite(v)) return v;
  }
  return null;
}

/**
 * Derive the at-a-glance Operational Health summary from the aggregate KPIs and
 * the runtime trend. Tone escalates to `critical` when the success rate is poor
 * (<90%), `warn` when it is merely soft (<98%) or the latest runtime has
 * regressed materially past its 30-day baseline, and `clear` otherwise.
 */
export function operationalHealthSummary(
  kpi: DashboardKpiSummary,
  waitRuntime: DashboardWaitRuntimeTrend,
): OperationalHealthSummaryModel {
  const latestRuntimeAvg = latestNonNull(
    waitRuntime.runtime.map((b) => b.avg_seconds),
  );
  const latestRuntimeBaseline = latestNonNull(
    waitRuntime.runtime.map((b) => b.baseline_avg_seconds),
  );

  let runtimeTrend: "up" | "down" | "flat" | null = null;
  if (latestRuntimeAvg != null && latestRuntimeBaseline != null) {
    const diff = latestRuntimeAvg - latestRuntimeBaseline;
    runtimeTrend =
      diff > DELTA_EPSILON ? "up" : diff < -DELTA_EPSILON ? "down" : "flat";
  }
  const runtimeRegressed =
    latestRuntimeAvg != null &&
    latestRuntimeBaseline != null &&
    latestRuntimeAvg > latestRuntimeBaseline * RUNTIME_REGRESSION_FACTOR;

  const sr = kpi.success_rate;
  const tone: GroupTone =
    sr != null && sr < 0.9
      ? "critical"
      : (sr != null && sr < 0.98) || runtimeRegressed
        ? "warn"
        : "clear";

  const parts = [
    `${formatPercent1(sr)} success`,
    `${formatPerHour(kpi.throughput_per_hour)} throughput`,
    `avg runtime ${formatSeconds(kpi.avg_runtime_seconds)}`,
  ];
  if (runtimeRegressed) parts.push("runtime above baseline");

  return {
    successRate: sr,
    throughputPerHour: kpi.throughput_per_hour,
    totalRuns: kpi.total_runs,
    avgRuntimeSeconds: kpi.avg_runtime_seconds,
    medianWaitSeconds: kpi.median_wait_seconds,
    runtimeTrend,
    tone,
    headline: parts.join(" · "),
  };
}
