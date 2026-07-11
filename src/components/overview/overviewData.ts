/**
 * Pure, presentational-free transforms that turn the raw `get_dashboard_*`
 * bindings (src/lib/commands.ts) into the exact view-models the Overview vNext
 * renders. Kept side-effect-free and free of React so every mapping — KPI
 * formatting + week-over-week delta tone, the race-hero join, the status-donut
 * segments, the success/fail trend series, and the SLA-banner derivation — is
 * unit-tested against fixed inputs (determinism gate R10). No data-fetching, no
 * `Date.now()`: callers pass an explicit `nowMs` so the race is deterministic.
 */
import type { StatusDonutSegment } from "../charts/StatusDonut";
import type { RaceTrackJob } from "../charts/RaceTrack";
import type { VehicleColor } from "../charts/Vehicle";
import {
  environmentOf,
  type DashboardExecutionSlots,
  type DashboardKpiDelta,
  type DashboardKpiSummary,
  type DashboardQueueHealthSummary,
  type DashboardStatusCount,
  type DashboardTrendSeries,
  type DashboardWorkflowBaseline,
  type MissionControlActivityItem,
} from "../../lib/commands";
import { formatDuration } from "../../lib/duration";
import { formatRunStatusLabel, statusKey } from "../../lib/runStatus";

// --- KPI strip ------------------------------------------------------------

export type DeltaDirection = "up" | "down" | "flat";
export type DeltaTone = "positive" | "negative" | "neutral";

/** A rendered week-over-week delta: a visible magnitude + a screen-reader
 * sentence carrying the direction, plus the good/bad tone for coloring. */
export interface KpiDelta {
  direction: DeltaDirection;
  tone: DeltaTone;
  /** Visible magnitude, e.g. `2.1 pp`, `0.4/hr`, `18s`. */
  text: string;
  /** Screen-reader sentence, e.g. `up 2.1 pp vs previous 1d`. */
  srText: string;
}

export interface KpiCard {
  key: string;
  label: string;
  value: string;
  /** InfoTip bold title. */
  infoTitle: string;
  /** InfoTip one-line definition. */
  infoDef: string;
  /** Week-over-window delta, or null for live (no-comparison) KPIs. */
  delta: KpiDelta | null;
}

const DELTA_EPSILON = 1e-9;

function formatPercent1(rate: number | null): string {
  if (rate == null || !Number.isFinite(rate)) return "—";
  return `${(rate * 100).toFixed(1)}%`;
}

function formatPerHour(value: number | null): string {
  if (value == null || !Number.isFinite(value)) return "—";
  return `${value.toFixed(1)}/hr`;
}

function formatSeconds(value: number | null): string {
  if (value == null || !Number.isFinite(value)) return "—";
  return formatDuration(value * 1000);
}

/**
 * Build a {@link KpiDelta} from a raw week-over-window delta. `format` renders
 * the (absolute) magnitude; `betterWhenLower` flips the good/bad tone for
 * "lower is better" metrics (runtime, wait). A null/non-finite delta (an
 * absent prior window) yields no delta.
 */
export function buildKpiDelta(
  delta: number | null,
  format: (absValue: number) => string,
  betterWhenLower: boolean,
  windowLabel: string,
): KpiDelta | null {
  if (delta == null || !Number.isFinite(delta)) return null;
  const direction: DeltaDirection =
    delta > DELTA_EPSILON ? "up" : delta < -DELTA_EPSILON ? "down" : "flat";
  const magnitude = format(Math.abs(delta));
  let tone: DeltaTone = "neutral";
  if (direction !== "flat") {
    const improved = betterWhenLower
      ? direction === "down"
      : direction === "up";
    tone = improved ? "positive" : "negative";
  }
  const srText =
    direction === "flat"
      ? `no change vs previous ${windowLabel}`
      : `${direction} ${magnitude} vs previous ${windowLabel}`;
  return { direction, tone, text: magnitude, srText };
}

/**
 * The 6-KPI strip view-model. The first four KPIs are windowed (with a
 * week-over-window delta from {@link DashboardKpiDelta}); the last two —
 * queue depth and running jobs — are live counts with no comparison.
 */
export function buildKpiCards(
  summary: DashboardKpiSummary,
  wow: DashboardKpiDelta | null,
  queueDepth: number,
  runningNow: number,
  windowLabel: string,
): KpiCard[] {
  return [
    {
      key: "success-rate",
      label: "Success rate",
      value: formatPercent1(summary.success_rate),
      infoTitle: "Success rate",
      infoDef:
        "Share of terminal runs that succeeded over the selected window.",
      delta: buildKpiDelta(
        wow?.success_rate_delta ?? null,
        (v) => `${(v * 100).toFixed(1)} pp`,
        false,
        windowLabel,
      ),
    },
    {
      key: "throughput",
      label: "Throughput",
      value: formatPerHour(summary.throughput_per_hour),
      infoTitle: "Throughput",
      infoDef: "Runs started per hour, averaged over the selected window.",
      delta: buildKpiDelta(
        wow?.throughput_per_hour_delta ?? null,
        (v) => `${v.toFixed(1)}/hr`,
        false,
        windowLabel,
      ),
    },
    {
      key: "avg-runtime",
      label: "Avg runtime",
      value: formatSeconds(summary.avg_runtime_seconds),
      infoTitle: "Average runtime",
      infoDef: "Mean end-to-end run duration over the selected window.",
      delta: buildKpiDelta(
        wow?.avg_runtime_seconds_delta ?? null,
        (v) => formatDuration(v * 1000),
        true,
        windowLabel,
      ),
    },
    {
      key: "max-wait",
      label: "Max wait",
      value: formatSeconds(summary.max_wait_seconds),
      infoTitle: "Max admission wait",
      infoDef:
        "Longest time a run waited in a queue before starting, over the window.",
      delta: buildKpiDelta(
        wow?.max_wait_seconds_delta ?? null,
        (v) => formatDuration(v * 1000),
        true,
        windowLabel,
      ),
    },
    {
      key: "queue-depth",
      label: "Queue depth",
      value: queueDepth.toLocaleString(),
      infoTitle: "Queue depth",
      infoDef: "Runs waiting across all queues right now (live).",
      delta: null,
    },
    {
      key: "running",
      label: "Running now",
      value: runningNow.toLocaleString(),
      infoTitle: "Running now",
      infoDef: "Runs currently executing across all queues (live).",
      delta: null,
    },
  ];
}

/** Sum the live waiting count across every queue (the "queue depth" KPI). */
export function totalQueueDepth(summary: DashboardQueueHealthSummary): number {
  return summary.queues.reduce(
    (sum, q) => sum + Math.max(0, q.queued_count),
    0,
  );
}

/** Live running count for the "running now" KPI (execution-slot occupancy). */
export function runningNow(slots: DashboardExecutionSlots): number {
  return Math.max(0, slots.global_running);
}

// --- Race-track hero ------------------------------------------------------

/** One row of the race-hero accessible table (includes jobs without a
 * baseline, which are excluded from the visual lanes). */
export interface RaceRow {
  job: string;
  agent: string;
  elapsedSeconds: number;
  /** Expected p50 runtime; null when the workflow has no runtime baseline. */
  expectedSeconds: number | null;
}

export interface RaceBuildResult {
  /** Lanes to draw — running jobs that have a real runtime baseline. */
  jobs: RaceTrackJob[];
  /** Every running job, for the accessible-table fallback. */
  rows: RaceRow[];
  /** Running jobs with no runtime baseline (flagged, excluded from lanes). */
  missingBaselineCount: number;
}

const LANE_CYCLE: VehicleColor[] = ["blue", "teal", "amber"];

function elapsedSecondsSince(startedAtIso: string, nowMs: number): number {
  const startedMs = new Date(startedAtIso).getTime();
  if (!Number.isFinite(startedMs)) return 0;
  return Math.max(0, Math.round((nowMs - startedMs) / 1000));
}

/**
 * Join the running jobs Mission Control already has (`live_activity`) with the
 * per-workflow runtime baselines to build the race lanes. Elapsed comes from
 * each run's `started_at` vs the passed `nowMs`; the expected (finish-line)
 * length is the workflow's p50 baseline (falling back to its mean). A running
 * job whose workflow has no baseline is truthfully surfaced in the accessible
 * table but omitted from the visual lanes (and counted) rather than being given
 * a fabricated finish line (R02).
 */
export function buildRaceJobs(
  running: MissionControlActivityItem[],
  baselines: DashboardWorkflowBaseline[],
  nowMs: number,
): RaceBuildResult {
  const baselineByWorkflow = new Map<string, DashboardWorkflowBaseline>();
  for (const b of baselines) baselineByWorkflow.set(b.workflow_id, b);

  const jobs: RaceTrackJob[] = [];
  const rows: RaceRow[] = [];
  let missingBaselineCount = 0;

  running.forEach((item, index) => {
    const elapsedSeconds = elapsedSecondsSince(item.started_at, nowMs);
    const baseline = baselineByWorkflow.get(item.workflow_id);
    const expected =
      baseline?.p50_runtime_seconds ?? baseline?.mean_runtime_seconds ?? null;
    const agent = environmentOf(item);

    rows.push({
      job: item.workflow_name,
      agent,
      elapsedSeconds,
      expectedSeconds: expected != null && expected > 0 ? expected : null,
    });

    if (expected != null && expected > 0) {
      jobs.push({
        job: item.workflow_name,
        agent,
        elapsedSeconds,
        expectedSeconds: expected,
        color: LANE_CYCLE[index % LANE_CYCLE.length],
      });
    } else {
      missingBaselineCount += 1;
    }
  });

  return { jobs, rows, missingBaselineCount };
}

// --- Status-distribution donut -------------------------------------------

/** Token color for a canonical status key (matches the status dots/badges). */
function statusColor(key: string): string {
  switch (key) {
    case "success":
      return "var(--success)";
    case "failed":
    case "timed_out":
    case "poll_exhausted":
      return "var(--error)";
    case "running":
    case "queued":
    case "admitted":
      return "var(--running)";
    case "cancelled":
      return "var(--warning)";
    default:
      return "var(--chart-5)";
  }
}

// Draw order so the donut reads success → failed → running → the rest.
const STATUS_ORDER = ["success", "failed", "running", "cancelled"];

/**
 * Collapse the raw per-status counts into donut segments: alias
 * `succeeded`→`success` (via {@link statusKey}), merge duplicates, drop empty
 * slices, and sort into a stable status order (unknown statuses trail, by
 * count). Segment colors bind to the status tokens.
 */
export function statusDonutSegments(
  distribution: DashboardStatusCount[],
): StatusDonutSegment[] {
  const totals = new Map<string, number>();
  for (const row of distribution) {
    const key = statusKey(row.status);
    totals.set(key, (totals.get(key) ?? 0) + Math.max(0, row.count));
  }
  return [...totals.entries()]
    .filter(([, count]) => count > 0)
    .sort((a, b) => {
      const ia = STATUS_ORDER.indexOf(a[0]);
      const ib = STATUS_ORDER.indexOf(b[0]);
      if (ia !== -1 || ib !== -1) {
        return (ia === -1 ? Infinity : ia) - (ib === -1 ? Infinity : ib);
      }
      return b[1] - a[1];
    })
    .map(([key, count]) => ({
      label: formatRunStatusLabel(key),
      value: count,
      color: statusColor(key),
    }));
}

/** Total runs represented by the donut (the fixed center number). */
export function statusDistributionTotal(
  distribution: DashboardStatusCount[],
): number {
  return distribution.reduce((sum, row) => sum + Math.max(0, row.count), 0);
}

// --- Success / failure trend ---------------------------------------------

export interface TrendChart {
  /** X-axis category labels (deterministic UTC formatting). */
  categories: string[];
  succeeded: number[];
  failed: number[];
  total: number[];
}

/** Deterministic, locale-free bucket label: `HH:MM` (hour grain) or `MM-DD`. */
function bucketLabel(iso: string, grain: "hour" | "day"): string {
  const d = new Date(iso);
  if (!Number.isFinite(d.getTime())) return iso;
  const s = d.toISOString();
  return grain === "hour" ? s.slice(11, 16) : s.slice(5, 10);
}

/** Turn the success/fail trend series into aligned arrays for the line chart
 * and its accessible table. */
export function trendToChart(series: DashboardTrendSeries): TrendChart {
  const categories: string[] = [];
  const succeeded: number[] = [];
  const failed: number[] = [];
  const total: number[] = [];
  for (const bucket of series.buckets) {
    categories.push(bucketLabel(bucket.bucket, series.grain));
    succeeded.push(bucket.succeeded);
    failed.push(bucket.failed);
    total.push(bucket.total);
  }
  return { categories, succeeded, failed, total };
}

// --- SLA alert banner -----------------------------------------------------

export interface SlaWarning {
  /** Highest severity present. */
  level: "warn" | "degraded";
  degradedQueues: string[];
  warnQueues: string[];
  totalQueued: number;
  /** One-line headline summarizing the warning. */
  headline: string;
}

/**
 * Derive the SLA alert banner from live queue health: it appears only when at
 * least one queue is `warn` or `degraded`. Returns null (no banner) otherwise.
 *
 * NOTE (flagged data gap): the plan's banner also lists "unreachable workers"
 * and "SLA-at-risk jobs", but neither is exposed by
 * `get_dashboard_queue_health` (see src/lib/commands.ts —
 * `DashboardQueueHealthSummary` carries only healthy/warn/degraded queue
 * classifications + backlog). The banner is therefore derived truthfully from
 * degraded/warn queues + total backlog only, and gains the extra signals when a
 * real binding provides them.
 */
export function deriveSlaWarning(
  summary: DashboardQueueHealthSummary,
): SlaWarning | null {
  const degradedQueues = summary.queues
    .filter((q) => q.status === "degraded")
    .map((q) => q.name);
  const warnQueues = summary.queues
    .filter((q) => q.status === "warn")
    .map((q) => q.name);
  if (degradedQueues.length === 0 && warnQueues.length === 0) return null;

  const totalQueued = summary.queues.reduce(
    (sum, q) => sum + Math.max(0, q.queued_count),
    0,
  );
  const level = degradedQueues.length > 0 ? "degraded" : "warn";
  const parts: string[] = [];
  if (degradedQueues.length > 0) {
    parts.push(
      `${degradedQueues.length} ${degradedQueues.length === 1 ? "queue" : "queues"} degraded`,
    );
  }
  if (warnQueues.length > 0) {
    parts.push(`${warnQueues.length} warning`);
  }
  parts.push(`${totalQueued} waiting`);
  return {
    level,
    degradedQueues,
    warnQueues,
    totalQueued,
    headline: parts.join(" · "),
  };
}
