/**
 * Pure, React-free transforms for the Agent Activity view (F16) — the
 * reconciliation of the legacy Live Activity feed + Upcoming Runs + Recent Runs
 * panels into one canonical running / upcoming / failures view inside Mission
 * Control (D04: "merged into Mission Control", no standalone route).
 *
 * All rows are derived from data Mission Control already loads in its snapshot
 * (`live_activity` + `upcoming_runs`) — no new binding, no fabricated data (R02).
 * Time labels are computed from a passed `nowMs` (never `Date.now()` here) so the
 * groupings + labels are deterministic under the frozen test clock (R10). Times
 * use the shared `formatDuration` ladder so every surface reads identically.
 */
import type {
  MissionControlActivityItem,
  MissionControlUpcomingRun,
} from "../../lib/commands";
import { formatDuration } from "../../lib/duration";
import { formatRunStatusLabel, statusKey } from "../../lib/runStatus";

/** Terminal statuses that count as a failure in the Agent Activity view.
 * `cancelled` is a deliberate stop, not a failure, so it is excluded. */
export const FAILURE_STATUSES: ReadonlySet<string> = new Set([
  "failed",
  "error",
  "timed_out",
  "poll_exhausted",
]);

/** True when a run activity item is currently in flight. */
export function isRunning(status: string): boolean {
  return statusKey(status) === "running";
}

/** True when a run activity item ended in a failure state. */
export function isFailure(status: string): boolean {
  return FAILURE_STATUSES.has(statusKey(status));
}

/** A running or failed run row (both are clickable → open the run). */
export interface ActivityRunRow {
  key: string;
  runId: string;
  workflowId: string;
  name: string;
  /** `domain / environment` sub-label. */
  sub: string;
  status: string;
  statusLabel: string;
  /** `for 20m 0s` (running) or `12m 0s ago` (failed). */
  timeLabel: string;
}

/** An upcoming fixed-time trigger row (not yet a run — not clickable). */
export interface UpcomingRow {
  key: string;
  name: string;
  /** `domain / trigger_label` sub-label. */
  sub: string;
  /** `in 3h 0m`, or `due now` when the next time is at/behind `nowMs`. */
  etaLabel: string;
}

export interface ActivityCounts {
  running: number;
  upcoming: number;
  failures: number;
}

function subLabel(item: MissionControlActivityItem): string {
  return `${item.domain} / ${item.environment}`;
}

/** Running rows, longest-running first (earliest `started_at` first). */
export function runningRows(
  items: MissionControlActivityItem[],
  nowMs: number,
): ActivityRunRow[] {
  return items
    .filter((i) => isRunning(i.status))
    .slice()
    .sort(
      (a, b) =>
        new Date(a.started_at).getTime() - new Date(b.started_at).getTime(),
    )
    .map((i) => ({
      key: i.id,
      runId: i.run_id,
      workflowId: i.workflow_id,
      name: i.workflow_name,
      sub: subLabel(i),
      status: i.status,
      statusLabel: formatRunStatusLabel(i.status),
      timeLabel: `for ${formatDuration(nowMs - new Date(i.started_at).getTime())}`,
    }));
}

/** Recent-failure rows, most-recent first (latest `finished_at` first). */
export function failureRows(
  items: MissionControlActivityItem[],
  nowMs: number,
): ActivityRunRow[] {
  const endOf = (i: MissionControlActivityItem) =>
    new Date(i.finished_at ?? i.started_at).getTime();
  return items
    .filter((i) => isFailure(i.status))
    .slice()
    .sort((a, b) => endOf(b) - endOf(a))
    .map((i) => ({
      key: i.id,
      runId: i.run_id,
      workflowId: i.workflow_id,
      name: i.workflow_name,
      sub: subLabel(i),
      status: i.status,
      statusLabel: formatRunStatusLabel(i.status),
      timeLabel: `${formatDuration(nowMs - endOf(i))} ago`,
    }));
}

/** Upcoming rows, soonest first. */
export function upcomingRows(
  items: MissionControlUpcomingRun[],
  nowMs: number,
): UpcomingRow[] {
  return items
    .slice()
    .sort(
      (a, b) =>
        new Date(a.next_time).getTime() - new Date(b.next_time).getTime(),
    )
    .map((i) => {
      const eta = new Date(i.next_time).getTime() - nowMs;
      return {
        key: `${i.workflow_id}-${i.trigger_label}`,
        name: i.workflow_name,
        sub: `${i.domain} / ${i.trigger_label}`,
        etaLabel: eta <= 0 ? "due now" : `in ${formatDuration(eta)}`,
      };
    });
}

/** The three section counts for the header + summary. */
export function activityCounts(
  live: MissionControlActivityItem[],
  upcoming: MissionControlUpcomingRun[],
): ActivityCounts {
  return {
    running: live.filter((i) => isRunning(i.status)).length,
    upcoming: upcoming.length,
    failures: live.filter((i) => isFailure(i.status)).length,
  };
}
