/**
 * Pure, React-free transforms that turn the three "Needs Attention" bindings
 * (src/lib/commands.ts) into the exact view-models the Needs Attention drill-down
 * renders — the blocked/waiting reason taxonomy, the heavy-blocker impact bars,
 * the long-running outlier + blast-radius bars, the recent-failure rows, and the
 * at-a-glance summary. Kept side-effect-free and free of React (no `Date.now()`,
 * no data-fetching) so every mapping is unit-tested against fixed inputs
 * (determinism gate R10). Colors bind to tokens (never raw hex).
 */
import type { ImpactBarItem } from "../charts/ImpactBars";
import type {
  DashboardBlastRadius,
  DashboardBlockReasonCount,
  DashboardBlockTaxonomy,
  DashboardHeavyBlocker,
  DashboardWorkflowFailureCount,
} from "../../lib/commands";
import { formatDuration } from "../../lib/duration";

// --- blocked / waiting reason taxonomy ------------------------------------

/** Friendly label for a `reason_category` (mirrors the backend taxonomy). */
export const BLOCK_REASON_LABELS: Record<string, string> = {
  resource: "Resource lock",
  event: "Event wait",
  host: "Host / worker",
  workload: "Workload cap",
  user: "User hold",
  unknown: "Unknown",
};

/** Stable, semantic token color per reason category (rank-independent). */
const BLOCK_REASON_COLORS: Record<string, string> = {
  resource: "var(--chart-1)",
  event: "var(--chart-3)",
  host: "var(--chart-2)",
  workload: "var(--chart-4)",
  user: "var(--chart-6)",
  unknown: "var(--chart-5)",
};

export function blockReasonLabel(category: string): string {
  return BLOCK_REASON_LABELS[category] ?? category;
}

function pluralize(count: number, noun: string): string {
  return `${count} ${noun}${count === 1 ? "" : "s"}`;
}

/**
 * Reason-taxonomy impact bars: one bar per reason category, length ∝ the Σ
 * current admission wait held by that reason, labeled with the held-job count.
 * Empty categories are dropped; colors are semantic per category.
 */
export function blockReasonBars(
  reasons: DashboardBlockReasonCount[],
): ImpactBarItem[] {
  return reasons
    .filter((r) => r.count > 0 || r.current_wait_seconds_total > 0)
    .map((r) => ({
      label: blockReasonLabel(r.reason_category),
      value: Math.max(0, r.current_wait_seconds_total),
      valueLabel: `${pluralize(r.count, "job")} · Σ ${formatDuration(
        Math.max(0, r.current_wait_seconds_total) * 1000,
      )}`,
      color: BLOCK_REASON_COLORS[r.reason_category] ?? "var(--chart-5)",
    }));
}

/**
 * Heavy-blocker impact bars: one bar per workflow that is a heavy source of
 * current blocking, ranked by the Σ admission wait it is holding, labeled with
 * both that Σ wait and the number of jobs it is blocking. (The compound
 * per-held-job stacking of the reference prototype needs a per-job wait
 * breakdown the binding does not expose — see the ledger — so the faithful
 * treatment is a ranked Σ bar, never a fabricated stack.)
 */
export function heavyBlockerBars(
  blockers: DashboardHeavyBlocker[],
): ImpactBarItem[] {
  return blockers
    .filter((b) => b.sigma_wait_seconds > 0 || b.blocked_count > 0)
    .map((b) => ({
      label: b.workflow_name,
      value: Math.max(0, b.sigma_wait_seconds),
      valueLabel: `Σ ${formatDuration(
        Math.max(0, b.sigma_wait_seconds) * 1000,
      )} · ${pluralize(b.blocked_count, "job")}`,
      color: "var(--warning)",
    }));
}

// --- long-running outliers + blast radius ---------------------------------

/**
 * Blast-radius impact bars: one bar per workflow whose in-window runs reach at
 * least one downstream dependent, ranked by the max downstream count, labeled
 * with that count and the longest dependency-chain depth. Workflows with no
 * downstream reach are dropped (they are not outliers).
 */
export function blastRadiusBars(rows: DashboardBlastRadius[]): ImpactBarItem[] {
  return rows
    .filter((r) => r.max_downstream_count > 0)
    .map((r) => ({
      label: r.workflow_name,
      value: r.max_downstream_count,
      valueLabel: `${r.max_downstream_count} downstream · depth ${r.max_depth}`,
      color: "var(--chart-4)",
    }));
}

// --- recent failures -------------------------------------------------------

export interface FailureRow {
  workflowId: string;
  workflowName: string;
  environment: string;
  failureCount: number;
  totalRuns: number;
  /** Failed / total over the window (0..1); null when the window had no runs. */
  failureRate: number | null;
}

/** Recent-failure rows (backend returns worst-first; preserved). */
export function failureRows(
  recurrence: DashboardWorkflowFailureCount[],
): FailureRow[] {
  return recurrence.map((r) => ({
    workflowId: r.workflow_id,
    workflowName: r.workflow_name,
    environment: r.environment,
    failureCount: r.failure_count,
    totalRuns: r.total_runs,
    failureRate: r.total_runs > 0 ? r.failure_count / r.total_runs : null,
  }));
}

export function formatFailureRate(rate: number | null): string {
  if (rate == null) return "—";
  return `${Math.round(rate * 100)}%`;
}

// --- at-a-glance summary ---------------------------------------------------

export type AttentionTone = "clear" | "warn" | "critical";

export interface NeedsAttentionSummaryModel {
  blockedCount: number;
  blockedWaitTotalSeconds: number;
  blockedWaitMaxSeconds: number;
  /** Heaviest single blocker (by Σ wait), or null when nothing is blocking. */
  heaviestBlocker: { name: string; sigmaWaitSeconds: number } | null;
  failingWorkflowCount: number;
  totalFailures: number;
  /** Widest blast radius (by downstream count), or null when none. */
  topBlastRadius: { name: string; downstream: number; depth: number } | null;
  tone: AttentionTone;
  /** One-line summary suitable for the collapsed card + the drill-down header. */
  headline: string;
}

/**
 * Derive the at-a-glance Needs Attention summary from the three bindings. The
 * tone escalates to `critical` when work is failing OR degraded-heavy blocking
 * is present, `warn` when anything is blocked/waiting, and `clear` otherwise.
 */
export function needsAttentionSummary(
  taxonomy: DashboardBlockTaxonomy,
  blastRadius: DashboardBlastRadius[],
  recurrence: DashboardWorkflowFailureCount[],
): NeedsAttentionSummaryModel {
  const heaviest = [...taxonomy.heavy_blockers].sort(
    (a, b) => b.sigma_wait_seconds - a.sigma_wait_seconds,
  )[0];
  const topBlast = [...blastRadius]
    .filter((r) => r.max_downstream_count > 0)
    .sort((a, b) => b.max_downstream_count - a.max_downstream_count)[0];
  const totalFailures = recurrence.reduce((sum, r) => sum + r.failure_count, 0);
  const blockedCount = Math.max(0, taxonomy.current_blocked_count);

  const tone: AttentionTone =
    totalFailures > 0 ? "critical" : blockedCount > 0 ? "warn" : "clear";

  const parts: string[] = [];
  if (blockedCount > 0) parts.push(`${pluralize(blockedCount, "job")} waiting`);
  if (recurrence.length > 0) {
    parts.push(
      `${pluralize(totalFailures, "failure")} across ${pluralize(recurrence.length, "workflow")}`,
    );
  }
  if (topBlast) {
    parts.push(
      `widest blast radius ${pluralize(topBlast.max_downstream_count, "run")}`,
    );
  }
  const headline =
    parts.length > 0 ? parts.join(" · ") : "Nothing needs attention";

  return {
    blockedCount,
    blockedWaitTotalSeconds: Math.max(0, taxonomy.current_wait_seconds_total),
    blockedWaitMaxSeconds: Math.max(0, taxonomy.current_wait_seconds_max),
    heaviestBlocker: heaviest
      ? {
          name: heaviest.workflow_name,
          sigmaWaitSeconds: heaviest.sigma_wait_seconds,
        }
      : null,
    failingWorkflowCount: recurrence.length,
    totalFailures,
    topBlastRadius: topBlast
      ? {
          name: topBlast.workflow_name,
          downstream: topBlast.max_downstream_count,
          depth: topBlast.max_depth,
        }
      : null,
    tone,
    headline,
  };
}
