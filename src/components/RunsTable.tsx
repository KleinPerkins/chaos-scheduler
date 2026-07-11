import type { Run } from "../lib/commands";
import { formatRunStatusLabel } from "../lib/runStatus";
import Button from "./Button";
import StatusBadge from "./StatusBadge";
import "./RunHistory.css";

export interface RunsTableProps {
  /** Rows to render, in display order. */
  runs: Run[];
  /** Row action — fires with the run when its Details button is pressed. */
  onViewRun: (run: Run) => void;
  /** Text shown inside `.rh-empty` when `runs` is empty. */
  emptyLabel?: string;
}

function formatDate(iso: string): string {
  const d = new Date(iso);
  return `${d.toLocaleDateString([], { month: "short", day: "numeric" })} ${d.toLocaleTimeString(
    [],
    {
      hour: "2-digit",
      minute: "2-digit",
      timeZoneName: "short",
    },
  )}`;
}

/**
 * Shared runs table primitive. A typed extraction of the cross-workflow
 * `.rh-table` (see `RunHistory.css` / DESIGN-SYSTEM.md) — it renders the exact
 * same `<table>` markup the Global History call site used before, composing the
 * shared `StatusBadge` and `Button` primitives in the cells, so behavior and
 * styling are unchanged. When `runs` is empty it renders the same `.rh-empty`
 * placeholder the call site rendered, using `emptyLabel`.
 */
export default function RunsTable({
  runs,
  onViewRun,
  emptyLabel = "No runs.",
}: RunsTableProps) {
  if (runs.length === 0) {
    return <div className="rh-empty">{emptyLabel}</div>;
  }

  return (
    <table className="rh-table">
      <thead>
        <tr>
          <th>Status</th>
          <th>Workflow</th>
          <th>Started</th>
          <th>Trigger</th>
          <th>Exit Code</th>
          <th>
            <span className="sr-only">Actions</span>
          </th>
        </tr>
      </thead>
      <tbody>
        {runs.map((run) => (
          <tr key={run.id}>
            <td>
              <StatusBadge status={run.status}>
                {formatRunStatusLabel(run.status)}
              </StatusBadge>
            </td>
            <td>{run.workflow_name ?? run.workflow_id}</td>
            <td>{formatDate(run.started_at)}</td>
            <td>{run.trigger_kind ?? "cron"}</td>
            <td>{run.exit_code ?? "—"}</td>
            <td>
              <Button variant="ghost" size="sm" onClick={() => onViewRun(run)}>
                Details
              </Button>
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
