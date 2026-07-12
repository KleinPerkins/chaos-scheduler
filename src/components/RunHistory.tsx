import { useState, useEffect, useMemo, useCallback } from "react";
import {
  getRunHistory,
  getWorkflowHistoryBuckets,
  rerunWorkflow,
  environmentOf,
} from "../lib/commands";
import { openExternalSafe } from "../lib/openExternalSafe";
import {
  queueWorkflowRun,
  formatWorkflowQueueOutcome,
  formatWorkflowQueueError,
} from "../lib/workflowEnqueue";
import RerunModal from "./RerunModal";
import type { Run, Workflow, WorkflowHistoryBucket } from "../lib/commands";
import { formatRunStatusLabel, statusKey } from "../lib/runStatus";
import { formatDurationBetween } from "../lib/duration";
import Button from "./Button";
import PageHeader from "./PageHeader";
import Input from "./Input";
import Select from "./Select";
import StatusBadge from "./StatusBadge";
import Notice from "./ui/Notice";
import { cronToHuman } from "./ScheduleBuilder";
import "./RunHistory.css";

const WORKFLOW_HISTORY_LIMIT = 50;

interface Props {
  workflow: Workflow;
  onBack: () => void;
  onViewLog: (runId: string) => void;
}

function formatDuration(start: string, end: string | null): string {
  if (!end) return "running...";
  return formatDurationBetween(start, end);
}

function formatDate(iso: string): string {
  const d = new Date(iso);
  return (
    d.toLocaleDateString([], { month: "short", day: "numeric" }) +
    " " +
    d.toLocaleTimeString([], {
      hour: "2-digit",
      minute: "2-digit",
      timeZoneName: "short",
    })
  );
}

function formatBucketDay(day: string): string {
  const match = /^\d{4}-\d{2}-(\d{2})$/.exec(day);
  return match ? String(Number(match[1])) : day;
}

export default function RunHistory({ workflow, onBack, onViewLog }: Props) {
  const [runs, setRuns] = useState<Run[]>([]);
  const [buckets, setBuckets] = useState<WorkflowHistoryBucket[]>([]);
  const [loading, setLoading] = useState(true);
  const [statusFilter, setStatusFilter] = useState("all");
  const [search, setSearch] = useState("");
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<{
    text: string;
    type: "success" | "error";
  } | null>(null);
  const [rerunning, setRerunning] = useState<string | null>(null);
  const [rerunTarget, setRerunTarget] = useState<Run | null>(null);
  const [rerunError, setRerunError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refreshRuns = useCallback(() => {
    setLoading(true);
    setError(null);
    return Promise.all([
      getRunHistory(workflow.id, WORKFLOW_HISTORY_LIMIT).then(setRuns),
      getWorkflowHistoryBuckets(workflow.id, 30).then(setBuckets),
    ])
      .catch((e) => {
        setError(String(e));
      })
      .finally(() => setLoading(false));
  }, [workflow.id]);

  // Defer the initial load to a macrotask so refreshRuns' synchronous
  // setLoading(true)/setError(null) do not run inside the effect body
  // (avoids react-hooks/set-state-in-effect). Mirrors useSchedulerStatus.
  useEffect(() => {
    const id = setTimeout(() => void refreshRuns(), 0);
    return () => clearTimeout(id);
  }, [refreshRuns]);

  const handleEnqueue = async () => {
    setBusy(true);
    setNotice(null);
    try {
      const outcome = await queueWorkflowRun(workflow.id);
      setNotice({
        text: formatWorkflowQueueOutcome(workflow.name, outcome),
        type: "success",
      });
      await refreshRuns();
    } catch (e) {
      setNotice({
        text: formatWorkflowQueueError(workflow.name, e),
        type: "error",
      });
    } finally {
      setBusy(false);
    }
  };

  const submitRerun = async (input: string) => {
    if (!rerunTarget) return;
    setRerunning(rerunTarget.id);
    setRerunError(null);
    try {
      await rerunWorkflow(workflow.id, rerunTarget.id, input);
      setRerunTarget(null);
      await refreshRuns();
    } catch (e) {
      setRerunError(String(e));
    } finally {
      setRerunning(null);
    }
  };

  // Status + search both refine the loaded rows only (the latest 50 for this
  // workflow) — no refetch, so the bounded contract stays truthful.
  const query = search.trim().toLowerCase();
  const visibleRuns = useMemo(() => {
    return runs.filter((run) => {
      if (statusFilter !== "all" && statusKey(run.status) !== statusFilter) {
        return false;
      }
      if (!query) return true;
      const haystack =
        `${run.id} ${formatRunStatusLabel(run.status)} ${run.trigger_kind ?? "cron"}`.toLowerCase();
      return haystack.includes(query);
    });
  }, [runs, statusFilter, query]);

  const filtering = statusFilter !== "all" || query.length > 0;
  const captionMeta = filtering
    ? `${visibleRuns.length} of ${runs.length} loaded`
    : `${runs.length} loaded · newest first`;

  const envLabel = (() => {
    const env = environmentOf(workflow);
    return env.charAt(0).toUpperCase() + env.slice(1);
  })();
  const subtitle = `Latest ${WORKFLOW_HISTORY_LIMIT} runs · ${envLabel} · ${cronToHuman(workflow.cron_schedule)} · 30 calendar-day failure buckets`;

  return (
    <section
      className="run-history"
      aria-label={`${workflow.name} run history`}
    >
      <PageHeader
        title={`${workflow.name} run history`}
        subtitle={subtitle}
        actions={
          <div className="rh-header-actions">
            <Button variant="ghost" onClick={onBack}>
              &larr; Workflow details
            </Button>
            <Button
              variant="primary"
              onClick={handleEnqueue}
              disabled={busy}
              title="Queue through scheduler admission control"
            >
              {busy ? "Submitting…" : "Queue run"}
            </Button>
          </div>
        }
      />

      {notice && (
        <Notice variant={notice.type} onDismiss={() => setNotice(null)}>
          {notice.text}
        </Notice>
      )}

      {loading ? (
        <div className="rh-loading">Loading...</div>
      ) : error ? (
        <div className="rh-error">
          <span>Run history failed to load: {error}</span>
          <Button variant="ghost" size="sm" onClick={() => void refreshRuns()}>
            Retry
          </Button>
        </div>
      ) : runs.length === 0 ? (
        <div className="rh-empty">No runs yet for this workflow.</div>
      ) : (
        <>
          <section
            className="rh-heatmap"
            aria-labelledby="workflow-failure-history-title"
          >
            <div className="rh-heatmap-header">
              <h2 id="workflow-failure-history-title">
                30-day failure history
              </h2>
              <span>
                {buckets.reduce((sum, b) => sum + b.failed, 0)} failed runs ·{" "}
                {buckets.length} calendar-day buckets
              </span>
            </div>
            <div
              className="rh-heatmap-grid"
              role="list"
              aria-label="Daily failure buckets, oldest to newest"
            >
              {buckets.map((bucket) => {
                const failureRate = bucket.total
                  ? bucket.failed / bucket.total
                  : 0;
                const level =
                  failureRate === 0 ? "ok" : failureRate < 0.5 ? "warn" : "bad";
                return (
                  <div
                    key={bucket.day}
                    className={`rh-heatmap-cell ${level}`}
                    role="listitem"
                    // Heatmap cells are non-interactive, but keyboard/switch
                    // users must reach each day's failure summary (the
                    // accessible name) without a pointer, so the cells are
                    // deliberately focusable (cf. GitHub's contribution graph).
                    // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- intentional data-viz focus affordance
                    tabIndex={0}
                    title={`${bucket.day}: ${bucket.failed} of ${bucket.total} runs failed`}
                    aria-label={`${bucket.day}: ${bucket.failed} of ${bucket.total} runs failed`}
                  >
                    <span>{formatBucketDay(bucket.day)}</span>
                  </div>
                );
              })}
            </div>
            <div className="rh-heatmap-bounds" aria-hidden="true">
              <span>Oldest</span>
              <span>Today</span>
            </div>
          </section>

          <div
            className="hist-toolbar hist-toolbar--workflow"
            role="group"
            aria-label="Workflow run history filters"
          >
            <label className="hist-field hist-field-search">
              <span className="hist-field-label">Search loaded rows</span>
              <Input
                type="search"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="Run ID, status, or trigger…"
              />
            </label>
            <label className="hist-field">
              <span className="hist-field-label">Status</span>
              <Select
                value={statusFilter}
                onChange={(e) => setStatusFilter(e.target.value)}
              >
                <option value="all">All statuses</option>
                <option value="running">Running</option>
                <option value="success">Success</option>
                <option value="failed">Failed</option>
                <option value="skipped">Skipped</option>
                <option value="poll_exhausted">Poll exhausted</option>
              </Select>
            </label>
            <span
              className="hist-bounded"
              title={`Showing at most the latest ${WORKFLOW_HISTORY_LIMIT} runs`}
            >
              Latest {WORKFLOW_HISTORY_LIMIT}
            </span>
          </div>

          <section
            className="hist-results"
            aria-labelledby="workflow-history-results-title"
          >
            <div className="hist-caption">
              <div className="hist-caption-copy">
                <h2
                  className="hist-caption-title"
                  id="workflow-history-results-title"
                >
                  Latest runs
                </h2>
                <span className="hist-caption-meta">{captionMeta}</span>
              </div>
            </div>

            {visibleRuns.length === 0 ? (
              <div className="rh-empty">No loaded runs match your filters.</div>
            ) : (
              <table className="rh-table">
                <caption className="sr-only">
                  Latest runs for {workflow.name}
                </caption>
                <thead>
                  <tr>
                    <th>Status</th>
                    <th>Started</th>
                    <th>Duration</th>
                    <th>Exit Code</th>
                    <th>Trigger</th>
                    <th>Result</th>
                    <th>
                      <span className="sr-only">Actions</span>
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {visibleRuns.map((run) => (
                    <tr key={run.id}>
                      <td>
                        <StatusBadge status={run.status}>
                          {formatRunStatusLabel(run.status)}
                        </StatusBadge>
                      </td>
                      <td>{formatDate(run.started_at)}</td>
                      <td>{formatDuration(run.started_at, run.finished_at)}</td>
                      <td className="rh-exit">
                        {run.exit_code !== null ? run.exit_code : "—"}
                      </td>
                      <td>
                        <span className="rh-trigger-kind">
                          {run.trigger_kind ?? "cron"}
                        </span>
                      </td>
                      <td>
                        {run.result_url ? (
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => {
                              void openExternalSafe(run.result_url!);
                            }}
                          >
                            Open
                          </Button>
                        ) : (
                          "—"
                        )}
                      </td>
                      <td>
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => onViewLog(run.id)}
                          aria-label={`View details for ${run.status} run started ${formatDate(run.started_at)}`}
                        >
                          Details
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          disabled={
                            rerunning === run.id || run.status === "running"
                          }
                          onClick={() => {
                            setRerunError(null);
                            setRerunTarget(run);
                          }}
                          aria-label={`Rerun ${run.status} run started ${formatDate(run.started_at)}`}
                        >
                          {rerunning === run.id ? "Rerunning..." : "Rerun"}
                        </Button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}

            <p className="hist-footnote">
              Search and status filter the loaded rows only — the latest{" "}
              {WORKFLOW_HISTORY_LIMIT} runs for this workflow, newest first.
            </p>
          </section>
        </>
      )}
      {rerunTarget && (
        <RerunModal
          workflowName={workflow.name}
          initialJson={rerunTarget.input_json ?? "{}"}
          busy={rerunning === rerunTarget.id}
          error={rerunError}
          onCancel={() => {
            if (rerunning !== rerunTarget.id) {
              setRerunTarget(null);
              setRerunError(null);
            }
          }}
          onSubmit={(input) => void submitRerun(input)}
        />
      )}
    </section>
  );
}
