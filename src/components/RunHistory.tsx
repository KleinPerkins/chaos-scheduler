import { useState, useEffect, useCallback } from "react";
import {
  getRunHistory,
  getWorkflowHistoryBuckets,
  rerunWorkflow,
} from "../lib/commands";
import { openExternalSafe } from "../lib/openExternalSafe";
import RerunModal from "./RerunModal";
import type { Run, Workflow, WorkflowHistoryBucket } from "../lib/commands";
import { formatRunStatusLabel } from "../lib/runStatus";
import Button from "./Button";
import PageHeader from "./PageHeader";
import StatusBadge from "./StatusBadge";
import "./RunHistory.css";

interface Props {
  workflow: Workflow;
  onBack: () => void;
  onViewLog: (runId: string) => void;
}

function formatDuration(start: string, end: string | null): string {
  if (!end) return "running...";
  const ms = new Date(end).getTime() - new Date(start).getTime();
  const secs = Math.floor(ms / 1000);
  if (secs < 60) return `${secs}s`;
  const mins = Math.floor(secs / 60);
  return `${mins}m ${secs % 60}s`;
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

export default function RunHistory({ workflow, onBack, onViewLog }: Props) {
  const [runs, setRuns] = useState<Run[]>([]);
  const [buckets, setBuckets] = useState<WorkflowHistoryBucket[]>([]);
  const [loading, setLoading] = useState(true);
  const [rerunning, setRerunning] = useState<string | null>(null);
  const [rerunTarget, setRerunTarget] = useState<Run | null>(null);
  const [rerunError, setRerunError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refreshRuns = useCallback(() => {
    setLoading(true);
    setError(null);
    return Promise.all([
      getRunHistory(workflow.id, 50).then(setRuns),
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

  return (
    <div>
      <PageHeader
        title={workflow.name}
        subtitle="Run History"
        actions={
          <Button variant="ghost" onClick={onBack}>
            &larr; Back
          </Button>
        }
      />

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
          <div className="rh-heatmap">
            <div className="rh-heatmap-header">
              <h3>30-day failure heatmap</h3>
              <span>
                {buckets.reduce((sum, b) => sum + b.failed, 0)} failed runs
              </span>
            </div>
            <div className="rh-heatmap-grid">
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
                    title={`${bucket.day}: ${bucket.failed}/${bucket.total} failed`}
                  >
                    <span>{new Date(bucket.day).getDate()}</span>
                  </div>
                );
              })}
            </div>
          </div>
          <table className="rh-table">
            <thead>
              <tr>
                <th>Status</th>
                <th>Started</th>
                <th>Duration</th>
                <th>Exit Code</th>
                <th>Trigger</th>
                <th>Result</th>
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
    </div>
  );
}
