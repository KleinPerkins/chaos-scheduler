import { useCallback, useEffect, useState } from "react";
import { Play, Pencil, History as HistoryIcon, RefreshCw } from "lucide-react";
import {
  getWorkflow,
  getRunHistory,
  getWorkflowHistoryBuckets,
  triggerWorkflow,
  enqueueWorkflow,
  environmentOf,
} from "../lib/commands";
import type { Run, Workflow, WorkflowHistoryBucket } from "../lib/commands";
import { cronToHuman } from "./ScheduleBuilder";
import { formatRunStatusLabel } from "../lib/runStatus";
import EnvironmentBadge from "./EnvironmentBadge";
import Notice from "./ui/Notice";
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
  const secs = Math.floor(
    (new Date(end).getTime() - new Date(start).getTime()) / 1000,
  );
  if (secs < 60) return `${secs}s`;
  return `${Math.floor(secs / 60)}m ${secs % 60}s`;
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
  const [busy, setBusy] = useState<null | "run" | "enqueue">(null);
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

  const handleRun = async () => {
    setBusy("run");
    setNotice(null);
    try {
      await triggerWorkflow(workflow.id);
      setNotice({ text: `Triggered "${workflow.name}".`, type: "success" });
      await refresh();
    } catch (e) {
      setNotice({ text: `Run failed: ${e}`, type: "error" });
    } finally {
      setBusy(null);
    }
  };

  const handleEnqueue = async () => {
    setBusy("enqueue");
    setNotice(null);
    try {
      const outcome = await enqueueWorkflow(workflow.id);
      setNotice({
        text: `Enqueued "${workflow.name}" (run ${outcome.run_id}).`,
        type: "success",
      });
      await refresh();
    } catch (e) {
      setNotice({ text: `Enqueue failed: ${e}`, type: "error" });
    } finally {
      setBusy(null);
    }
  };

  const summaryRows: { label: string; value: React.ReactNode }[] = [
    {
      label: "Schedule",
      value: cronToHuman(workflow.cron_schedule, workflow.timezone),
    },
    { label: "Timezone", value: workflow.timezone },
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
          <button className="btn btn-ghost btn-sm" onClick={onBack}>
            &larr; Back
          </button>
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
            {workflow.description && (
              <p className="page-subtitle">{workflow.description}</p>
            )}
          </div>
        </div>
        <div className="wd-actions">
          <button
            className="btn btn-ghost btn-sm"
            onClick={() => void refresh()}
            aria-label="Refresh"
            title="Refresh"
          >
            <RefreshCw size={14} strokeWidth={2} />
          </button>
          <button
            className="btn btn-ghost btn-sm"
            onClick={handleRun}
            disabled={busy !== null}
          >
            <Play size={14} strokeWidth={2.5} />
            {busy === "run" ? "Running…" : "Run"}
          </button>
          <button
            className="btn btn-ghost btn-sm"
            onClick={handleEnqueue}
            disabled={busy !== null}
          >
            {busy === "enqueue" ? "Enqueueing…" : "Enqueue"}
          </button>
          <button
            className="btn btn-ghost btn-sm"
            onClick={() => onFullHistory(workflow)}
          >
            <HistoryIcon size={14} strokeWidth={2} /> Full history
          </button>
          <button
            className="btn btn-primary btn-sm"
            onClick={() => onEdit(workflow)}
            disabled={isManaged}
            title={
              isManaged ? "Externally managed — read-only" : "Edit workflow"
            }
          >
            <Pencil size={14} strokeWidth={2} /> Edit
          </button>
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
          <h2 className="wd-card-title">Health</h2>
          <div className="wd-health">
            <div className="wd-stat">
              <span className="wd-stat-label">Last run</span>
              <span className="wd-stat-value">
                {lastRun ? (
                  <span className={`status-badge ${lastRun.status}`}>
                    {formatRunStatusLabel(lastRun.status)}
                  </span>
                ) : (
                  "No runs yet"
                )}
              </span>
              {lastRun && (
                <span className="wd-stat-sub">
                  {formatDate(lastRun.started_at)}
                </span>
              )}
            </div>
            <div className="wd-stat">
              <span className="wd-stat-label">Failed (30d)</span>
              <span className="wd-stat-value">
                {failedCount}
                <span className="wd-stat-sub"> / {totalRuns} runs</span>
              </span>
            </div>
          </div>
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
          <button
            className="btn btn-ghost btn-sm"
            onClick={() => onFullHistory(workflow)}
          >
            View all
          </button>
        </div>
        {loading ? (
          <div className="wd-muted">Loading…</div>
        ) : error ? (
          <div className="wd-error">
            <span>Failed to load: {error}</span>
            <button
              className="btn btn-ghost btn-sm"
              onClick={() => void refresh()}
            >
              Retry
            </button>
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
                    <span className={`status-badge ${run.status}`}>
                      {formatRunStatusLabel(run.status)}
                    </span>
                  </td>
                  <td>{formatDate(run.started_at)}</td>
                  <td>{formatDuration(run.started_at, run.finished_at)}</td>
                  <td>
                    <span className="wd-trigger">
                      {run.trigger_kind ?? "cron"}
                    </span>
                  </td>
                  <td>
                    <button
                      className="btn btn-ghost btn-sm"
                      onClick={() => onViewRun(run.id)}
                      aria-label={`View details for run started ${formatDate(run.started_at)}`}
                    >
                      Details
                    </button>
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
