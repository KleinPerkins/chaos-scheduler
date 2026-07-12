/**
 * Pure, React-free transforms that turn the three "Resources" bindings
 * (src/lib/commands.ts) into the exact view-models the Resources drill-down
 * renders — the threshold-zone queue-utilization history chart, the
 * execution-slot gas gauges, and the queue-health table — plus the at-a-glance
 * summary. Kept side-effect-free and free of React (no `Date.now()`, no
 * fetching) so every mapping is unit-tested against fixed inputs (determinism
 * gate R10). Utilizations from the backend are fractions (0–1); these transforms
 * expose whole-percent numbers for the chart/gauges. Colors bind to tokens in
 * the component (never raw hex here).
 */
import type {
  DashboardExecutionSlots,
  DashboardQueueHealth,
  DashboardQueueHealthSummary,
  DashboardQueueUtilizationHistory,
} from "../../lib/commands";
import type { GroupTone } from "./groupCard";

// --- shared formatters -----------------------------------------------------

/** A fraction (0–1) as a whole-percent string (`0.79` → `79%`). */
export function formatPercentFrac(fraction: number | null): string {
  if (fraction == null || !Number.isFinite(fraction)) return "—";
  return `${Math.round(fraction * 100)}%`;
}

/** A count with a fallback dash for null/non-finite. */
export function formatCount(value: number | null): string {
  if (value == null || !Number.isFinite(value)) return "—";
  return value.toLocaleString();
}

/** Deterministic, locale-free bucket label: `HH:MM` (hour grain) or `MM-DD`. */
function bucketLabel(iso: string, grain: "hour" | "day"): string {
  const d = new Date(iso);
  if (!Number.isFinite(d.getTime())) return iso;
  const s = d.toISOString();
  return grain === "hour" ? s.slice(11, 16) : s.slice(5, 10);
}

/** Map a queue-health status to the shared group tone. */
export function statusTone(status: DashboardQueueHealth["status"]): GroupTone {
  return status === "degraded"
    ? "critical"
    : status === "warn"
      ? "warn"
      : "clear";
}

// --- utilization history chart ---------------------------------------------

export interface UtilizationChart {
  categories: string[];
  /** Whole-percent avg utilization per bucket (null → 0 for the polyline). */
  avgPct: number[];
  /** Whole-percent max utilization per bucket. */
  maxPct: number[];
  /** Raw fractional utilizations (nullable) for the accessible table. */
  avgFrac: (number | null)[];
  maxFrac: (number | null)[];
  /** Warn / degraded thresholds as whole percents. */
  warnPct: number;
  degradedPct: number;
  /** True when at least one bucket carries a sample. */
  hasData: boolean;
}

/** Turn the queue-occupancy history into aligned arrays for the threshold-zone
 * chart and its accessible table. Null (no-sample) buckets plot at 0 (the
 * polyline draws no gaps) but stay null in the table. */
export function utilizationChart(
  history: DashboardQueueUtilizationHistory,
): UtilizationChart {
  const categories: string[] = [];
  const avgPct: number[] = [];
  const maxPct: number[] = [];
  const avgFrac: (number | null)[] = [];
  const maxFrac: (number | null)[] = [];
  let hasData = false;
  for (const b of history.buckets) {
    categories.push(bucketLabel(b.bucket, history.grain));
    avgFrac.push(b.avg_utilization);
    maxFrac.push(b.max_utilization);
    avgPct.push(
      b.avg_utilization == null ? 0 : Math.round(b.avg_utilization * 100),
    );
    maxPct.push(
      b.max_utilization == null ? 0 : Math.round(b.max_utilization * 100),
    );
    if (b.sample_count > 0) hasData = true;
  }
  return {
    categories,
    avgPct,
    maxPct,
    avgFrac,
    maxFrac,
    warnPct: Math.round(history.warn_utilization * 100),
    degradedPct: Math.round(history.degraded_utilization * 100),
    hasData,
  };
}

// --- execution-slot gauges -------------------------------------------------

export interface SlotGauge {
  key: string;
  label: string;
  sublabel: string | null;
  running: number;
  capacity: number;
  available: number;
  utilizationPct: number;
}

/** Global + per-queue execution-slot gauges from the live slot snapshot. The
 * global gauge leads; queues follow in the backend's order. */
export function slotGauges(slots: DashboardExecutionSlots): {
  global: SlotGauge;
  queues: SlotGauge[];
} {
  const global: SlotGauge = {
    key: "__global__",
    label: "All queues",
    sublabel: null,
    running: slots.global_running,
    capacity: slots.global_capacity,
    available: slots.global_available,
    utilizationPct: Math.round(slots.global_utilization * 100),
  };
  const queues = slots.queues.map((q) => ({
    key: `${q.environment}:${q.name}`,
    label: q.name,
    sublabel: q.environment,
    running: q.running,
    capacity: q.capacity,
    available: q.available,
    utilizationPct: Math.round(q.utilization * 100),
  }));
  return { global, queues };
}

// --- queue-health table ----------------------------------------------------

export interface QueueHealthRow {
  key: string;
  name: string;
  environment: string;
  capacity: number;
  active: number;
  queued: number;
  utilizationPct: number;
  status: DashboardQueueHealth["status"];
  tone: GroupTone;
}

/** One row per queue for the health table, worst status first. */
export function queueHealthRows(
  summary: DashboardQueueHealthSummary,
): QueueHealthRow[] {
  const order: Record<DashboardQueueHealth["status"], number> = {
    degraded: 0,
    warn: 1,
    healthy: 2,
  };
  return summary.queues
    .map((q) => ({
      key: `${q.environment}:${q.name}`,
      name: q.name,
      environment: q.environment,
      capacity: q.capacity,
      active: q.active_count,
      queued: q.queued_count,
      utilizationPct: Math.round(q.utilization * 100),
      status: q.status,
      tone: statusTone(q.status),
    }))
    .sort((a, b) => order[a.status] - order[b.status]);
}

// --- at-a-glance summary ---------------------------------------------------

export interface ResourcesSummaryModel {
  globalUtilizationPct: number;
  running: number;
  capacity: number;
  available: number;
  healthyQueues: number;
  warnQueues: number;
  degradedQueues: number;
  tone: GroupTone;
  headline: string;
}

/**
 * Derive the at-a-glance Resources summary from the live slot snapshot, the
 * queue-health tallies, and the utilization thresholds. Tone escalates to
 * `critical` when any queue is degraded or global utilization is at/above the
 * degraded threshold, `warn` when any queue is in warn or global utilization is
 * at/above the warn threshold, and `clear` otherwise.
 */
export function resourcesSummary(
  slots: DashboardExecutionSlots,
  health: DashboardQueueHealthSummary,
  history: DashboardQueueUtilizationHistory,
): ResourcesSummaryModel {
  const globalUtil = slots.global_utilization;
  const globalUtilizationPct = Math.round(globalUtil * 100);
  const warn = history.warn_utilization;
  const degraded = history.degraded_utilization;

  const tone: GroupTone =
    health.degraded > 0 || globalUtil >= degraded
      ? "critical"
      : health.warn > 0 || globalUtil >= warn
        ? "warn"
        : "clear";

  const parts = [
    `${globalUtilizationPct}% slots used`,
    `${slots.global_running}/${slots.global_capacity} running`,
  ];
  if (health.degraded > 0) parts.push(`${health.degraded} degraded`);
  if (health.warn > 0) parts.push(`${health.warn} warn`);
  if (health.degraded === 0 && health.warn === 0)
    parts.push("all queues healthy");

  return {
    globalUtilizationPct,
    running: slots.global_running,
    capacity: slots.global_capacity,
    available: slots.global_available,
    healthyQueues: health.healthy,
    warnQueues: health.warn,
    degradedQueues: health.degraded,
    tone,
    headline: parts.join(" · "),
  };
}
