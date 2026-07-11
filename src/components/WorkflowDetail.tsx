import { useCallback, useEffect, useState } from "react";
import { Pencil, RefreshCw } from "lucide-react";
import {
  getWorkflow,
  getRunHistory,
  getWorkflowHistoryBuckets,
  environmentOf,
} from "../lib/commands";
import type { Run, Workflow, WorkflowHistoryBucket } from "../lib/commands";
import { cronToHuman } from "./ScheduleBuilder";
import { formatRunStatusLabel } from "../lib/runStatus";
import { formatDurationBetween } from "../lib/duration";
import EnvironmentBadge from "./EnvironmentBadge";
import Notice from "./ui/Notice";
import Button from "./Button";
import StatusBadge from "./StatusBadge";
import {
  formatWorkflowQueueError,
  formatWorkflowQueueOutcome,
  queueWorkflowRun,
} from "../lib/workflowEnqueue";
import "./WorkflowDetail.css";

interface Props {
  workflow: Workflow;
  onBack: () => void;
  onEdit: (w: Workflow) => void;
  onFullHistory: (w: Workflow) => void;
  onViewRun: (runId: string) => void;
}

const RECENT_LIMIT = 8;

function formatDate(iso: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  return (
    d.toLocaleDateString([], { month: "short", day: "numeric" }) +
    " " +
    d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
  );
}

function formatDuration(start: string, end: string | null): string {
  if (!end) return "running…";
  return formatDurationBetween(start, end);
}

/**
 * Unified per-workflow detail hub. Merges what was previously split across the
 * editor, run history, run detail, and Mission Control telemetry into one
 * "hero" page: configuration summary + health + recent runs, with drill-downs
 * to the full history / run detail / editor.
 */
export default function WorkflowDetail({
  workflow: initial,
  onBack,
  onEdit,
  onFullHistory,
  onViewRun,
}: Props) {
  const [workflow, setWorkflow] = useState<Workflow>(initial);
  const [runs, setRuns] = useState<Run[]>([]);
  const [buckets, setBuckets] = useState<WorkflowHistoryBucket[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<null | "enqueue">(null);
  const [notice, setNotice] = useState<{
    text: string;
    type: "success" | "error";
  } | null>(null);

  const refresh = useCallback(() => {
    setLoading(true);
    setError(null);
    return Promise.all([
      getWorkflow(initial.id)
        .then(setWorkflow)
        .catch(() => undefined),
      getRunHistory(initial.id, RECENT_LIMIT).then(setRuns),
      getWorkflowHistoryBuckets(initial.id, 30).then(setBuckets),
    ])
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, [initial.id]);

  useEffect(() => {
    const id = setTimeout(() => void refresh(), 0);
    return () => clearTimeout(id);
  }, [refresh]);

  const isManaged = workflow.managed_externally;
  const lastRun = runs[0] ?? null;
  const failedCount = buckets.reduce((sum, b) => sum + b.failed, 0);
  const totalRuns = buckets.reduce((sum, b) => sum + b.total, 0);
  const scheduleLabel = `${cronToHuman(workflow.cron_schedule)} · ${workflow.timezone}`;

  const handleEnqueue = async () => {
    setBusy("enqueue");
    setNotice(null);
    try {
      const outcome = await queueWorkflowRun(workflow.id);
      setNotice({
        text: formatWorkflowQueueOutcome(workflow.name, outcome),
        type: "success",
      });
      await refresh();
    } catch (e) {
      setNotice({
        text: formatWorkflowQueueError(workflow.name, e),
        type: "error",
      });
    } finally {
      setBusy(null);
    }
  };

  const summaryRows: { label: string; value: React.ReactNode }[] = [
    {
      label: "Schedule",
      value: scheduleLabel,
    },
    {
      label: workflow.kind === "typed" ? "Operator" : "Script",
      value: <code className="wd-code">{workflow.script_path || "—"}</code>,
    },
    {
      label: "Execution",
      value: workflow.kind === "typed" ? "Typed operator" : "Step flow",
    },
    { label: "Async mode", value: workflow.async_mode ? "Yes" : "No" },
    {
      label: "Email on failure",
      value: workflow.email_on_failure ? "Enabled" : "Off",
    },
  ];

  return (
    <div className="workflow-detail">
      <div className="page-header">
        <div className="wd-heading">
          <Button variant="ghost" size="sm" onClick={onBack}>
            &larr; Back
          </Button>
          <div>
            <div className="wd-title-row">
              <h1 className="page-title">{workflow.name}</h1>
              <EnvironmentBadge
                environment={environmentOf(workflow)}
                managed={isManaged}
                size="sm"
              />
              <span
                className={`wd-state-pill ${workflow.enabled ? "wd-state-on" : "wd-state-off"}`}
              >
                {workflow.enabled ? "Enabled" : "Disabled"}
              </span>
            </div>
            <p className="wd-header-meta">{scheduleLabel}</p>
            {workflow.description && (
              <p className="page-subtitle wd-description">
                {workflow.description}
              </p>
            )}
          </div>
        </div>
        <div className="wd-actions">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => onEdit(workflow)}
            disabled={isManaged}
            title={
              isManaged ? "Externally managed — read-only" : "Edit workflow"
            }
          >
            <Pencil size={14} strokeWidth={2} /> Edit workflow
          </Button>
          <Button
            variant="primary"
            size="sm"
            onClick={handleEnqueue}
            disabled={busy !== null}
            title="Queue through scheduler admission control"
          >
            {busy === "enqueue" ? "Submitting…" : "Queue run"}
          </Button>
        </div>
      </div>

      {notice && (
        <Notice variant={notice.type} onDismiss={() => setNotice(null)}>
          {notice.text}
        </Notice>
      )}

      <div className="wd-grid">
        <section className="wd-card">
          <h2 className="wd-card-title">Configuration</h2>
          <dl className="wd-summary">
            {summaryRows.map((row) => (
              <div key={row.label} className="wd-summary-row">
                <dt>{row.label}</dt>
                <dd>{row.value}</dd>
              </div>
            ))}
          </dl>
        </section>

        <section className="wd-card">
          <div className="wd-card-header">
            <h2 className="wd-card-title">Latest run</h2>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => void refresh()}
              aria-label="Refresh"
              title="Refresh"
            >
              <RefreshCw size={14} strokeWidth={2} />
            </Button>
          </div>
          {loading ? (
            <div className="wd-muted">Loading latest run…</div>
          ) : lastRun ? (
            <div className="wd-latest-run">
              <div className="wd-latest-status">
                <StatusBadge status={lastRun.status}>
                  {formatRunStatusLabel(lastRun.status)}
                </StatusBadge>
                <span>{formatDate(lastRun.started_at)}</span>
              </div>
              <div className="wd-latest-facts">
                <span>
                  Duration ·{" "}
                  {formatDuration(lastRun.started_at, lastRun.finished_at)}
                </span>
                <span>Trigger · {lastRun.trigger_kind ?? "cron"}</span>
                <span>
                  Failed (30d) · {failedCount} / {totalRuns} runs
                </span>
              </div>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => onViewRun(lastRun.id)}
              >
                View latest run
              </Button>
            </div>
          ) : (
            <div className="wd-muted">No runs yet for this workflow.</div>
          )}
          {buckets.length > 0 && (
            <div className="wd-heatmap" aria-label="30-day failure heatmap">
              {buckets.map((b) => {
                const rate = b.total ? b.failed / b.total : 0;
                const level = rate === 0 ? "ok" : rate < 0.5 ? "warn" : "bad";
                return (
                  <span
                    key={b.day}
                    className={`wd-heat-cell ${b.total ? level : "empty"}`}
                    title={`${b.day}: ${b.failed}/${b.total} failed`}
                  />
                );
              })}
            </div>
          )}
        </section>
      </div>

      <section className="wd-card">
        <div className="wd-runs-header">
          <h2 className="wd-card-title">Recent runs</h2>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => onFullHistory(workflow)}
          >
            View all
          </Button>
        </div>
        {loading ? (
          <div className="wd-muted">Loading…</div>
        ) : error ? (
          <div className="wd-error">
            <span>Failed to load: {error}</span>
            <Button variant="ghost" size="sm" onClick={() => void refresh()}>
              Retry
            </Button>
          </div>
        ) : runs.length === 0 ? (
          <div className="wd-muted">No runs yet for this workflow.</div>
        ) : (
          <table className="wd-runs-table">
            <thead>
              <tr>
                <th>Status</th>
                <th>Started</th>
                <th>Duration</th>
                <th>Trigger</th>
                <th></th>
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
                  <td>{formatDate(run.started_at)}</td>
                  <td>{formatDuration(run.started_at, run.finished_at)}</td>
                  <td>
                    <span className="wd-trigger">
                      {run.trigger_kind ?? "cron"}
                    </span>
                  </td>
                  <td>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => onViewRun(run.id)}
                      aria-label={`View details for run started ${formatDate(run.started_at)}`}
                    >
                      Details
                    </Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>
    </div>
  );
}
