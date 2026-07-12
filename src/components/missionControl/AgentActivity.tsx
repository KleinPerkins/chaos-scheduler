import { useEffect, useState, type ReactNode } from "react";
import type {
  MissionControlActivityItem,
  MissionControlUpcomingRun,
} from "../../lib/commands";
import { statusKey } from "../../lib/runStatus";
import InfoTip from "../InfoTip";
import StatusBadge from "../StatusBadge";
import StatusDot from "../StatusDot";
import {
  activityCounts,
  failureRows,
  runningRows,
  upcomingRows,
  type ActivityRunRow,
} from "./agentActivityData";

/**
 * Agent Activity view (F16) — the reconciliation (disposition D04:
 * "merged into Mission Control") of the legacy Live Activity feed + Upcoming
 * Runs + Recent Runs panels into ONE canonical running / upcoming / failures
 * view. It lives as the Mission Control "Activity" tab (no standalone route) and
 * reads only data the snapshot already carries (`live_activity` +
 * `upcoming_runs`) — no new binding, no fabricated data.
 */
export function AgentActivity({
  live,
  upcoming,
  onOpenRun,
}: {
  live: MissionControlActivityItem[];
  upcoming: MissionControlUpcomingRun[];
  onOpenRun: (runId: string, workflowId: string) => void;
}) {
  // Read the clock in a deferred effect (not during render — Date.now() is
  // impure) so labels stay pure/idempotent per the react-hooks rules; the
  // Playwright test clock (page.clock.setFixedTime) has already frozen it by the
  // time the effect fires, keeping the elapsed / ETA / ago labels deterministic
  // in the visual baseline (mirrors Overview's race clock).
  const [clockNow, setClockNow] = useState<number | null>(null);
  useEffect(() => {
    const id = setTimeout(() => setClockNow(Date.now()), 0);
    return () => clearTimeout(id);
  }, [live, upcoming]);
  const nowMs = clockNow ?? 0;
  const running = runningRows(live, nowMs);
  const failures = failureRows(live, nowMs);
  const soon = upcomingRows(upcoming, nowMs);
  const counts = activityCounts(live, upcoming);

  return (
    <div className="mc-activity">
      <header className="mc-activity__header">
        <h1 className="mc-activity__title">Agent Activity</h1>
        <p className="mc-activity__sub">
          Running, upcoming, and recently-failed runs for the current filter.
        </p>
      </header>

      <ActivitySection
        id="mc-act-running"
        title="Running now"
        count={counts.running}
        infoDef="Runs currently executing for this filter, longest-running first."
        hint="live"
        empty="No runs are executing for this filter."
      >
        {running.map((row) => (
          <RunRow key={row.key} row={row} onOpenRun={onOpenRun} />
        ))}
      </ActivitySection>

      <ActivitySection
        id="mc-act-upcoming"
        title="Upcoming"
        count={counts.upcoming}
        infoDef="Next fixed-time cron triggers for this filter, soonest first. Event-driven workflows need durable readiness state before an ETA can show."
        hint="fixed-time cron triggers"
        empty="No fixed-time cron triggers match this filter."
      >
        {soon.length > 0 ? (
          <div className="mc-upcoming-grid">
            {soon.map((row) => (
              <div className="mc-upcoming-card" key={row.key}>
                <span>{row.etaLabel}</span>
                <strong>{row.name}</strong>
                <small>{row.sub}</small>
              </div>
            ))}
          </div>
        ) : null}
      </ActivitySection>

      <ActivitySection
        id="mc-act-failures"
        title="Recent failures"
        count={counts.failures}
        alert={counts.failures > 0}
        infoDef="Runs that ended in a failure state (failed, error, timed out, poll exhausted), most recent first. Cancellations are not counted as failures."
        hint="most recent first"
        empty="No recent failures for this filter."
      >
        {failures.map((row) => (
          <RunRow key={row.key} row={row} onOpenRun={onOpenRun} />
        ))}
      </ActivitySection>
    </div>
  );
}

/** One clickable running/failed run row (opens the run detail). */
function RunRow({
  row,
  onOpenRun,
}: {
  row: ActivityRunRow;
  onOpenRun: (runId: string, workflowId: string) => void;
}) {
  return (
    <button
      className="mc-table-row"
      onClick={() => onOpenRun(row.runId, row.workflowId)}
    >
      <StatusDot variant="mc-dot" status={statusKey(row.status)} />
      <span className="mc-run-name">
        <strong>{row.name}</strong>
        <small>{row.sub}</small>
      </span>
      <StatusBadge status={statusKey(row.status)}>
        {row.statusLabel}
      </StatusBadge>
      <span className="mc-activity__time">{row.timeLabel}</span>
    </button>
  );
}

/** A titled activity section: header (title + count + InfoTip) then body or an
 * empty state. `children` is rendered inside `.mc-table` unless the caller
 * supplies its own wrapper (Upcoming uses the card grid). */
function ActivitySection({
  id,
  title,
  count,
  alert = false,
  infoDef,
  hint,
  empty,
  children,
}: {
  id: string;
  title: string;
  count: number;
  alert?: boolean;
  infoDef: string;
  hint: string;
  empty: string;
  children: ReactNode;
}) {
  const hasChildren = Array.isArray(children)
    ? children.length > 0
    : Boolean(children);
  const isUpcoming = id === "mc-act-upcoming";
  return (
    <section className="mc-panel" aria-labelledby={id}>
      <div className="mc-panel-header">
        <div className="mc-panel-title">
          <h2 id={id}>{title}</h2>
          <span className={`mc-count${alert ? " mc-count--alert" : ""}`}>
            {count}
          </span>
          <InfoTip title={title} def={infoDef} />
        </div>
        <span>{hint}</span>
      </div>
      {count === 0 || !hasChildren ? (
        <div className="mc-empty">{empty}</div>
      ) : isUpcoming ? (
        children
      ) : (
        <div className="mc-table">{children}</div>
      )}
    </section>
  );
}
