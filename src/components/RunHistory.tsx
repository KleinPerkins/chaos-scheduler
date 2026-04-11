import { useState, useEffect } from "react";
import { getRunHistory, openUrl } from "../lib/commands";
import type { Run, Workflow } from "../lib/commands";
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
  return d.toLocaleDateString([], { month: "short", day: "numeric" }) +
    " " +
    d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", timeZoneName: "short" });
}

export default function RunHistory({ workflow, onBack, onViewLog }: Props) {
  const [runs, setRuns] = useState<Run[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    getRunHistory(workflow.id, 50)
      .then(setRuns)
      .finally(() => setLoading(false));
  }, [workflow.id]);

  return (
    <div>
      <div className="page-header">
        <div>
          <h1 className="page-title">{workflow.name}</h1>
          <p className="page-subtitle">Run History</p>
        </div>
        <button className="btn btn-ghost" onClick={onBack}>
          &larr; Back
        </button>
      </div>

      {loading ? (
        <div className="rh-loading">Loading...</div>
      ) : runs.length === 0 ? (
        <div className="rh-empty">No runs yet for this workflow.</div>
      ) : (
        <table className="rh-table">
          <thead>
            <tr>
              <th>Status</th>
              <th>Started</th>
              <th>Duration</th>
              <th>Exit Code</th>
              <th>Result</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {runs.map((run) => (
              <tr key={run.id} className="rh-row-clickable" onClick={() => onViewLog(run.id)}>
                <td>
                  <span className={`status-badge ${run.status}`}>
                    {run.status}
                  </span>
                </td>
                <td>{formatDate(run.started_at)}</td>
                <td>{formatDuration(run.started_at, run.finished_at)}</td>
                <td className="rh-exit">
                  {run.exit_code !== null ? run.exit_code : "—"}
                </td>
                <td>
                  {run.result_url ? (
                    <button
                      className="btn btn-ghost btn-sm"
                      onClick={(e) => {
                        e.stopPropagation();
                        openUrl(run.result_url!);
                      }}
                    >
                      Open
                    </button>
                  ) : (
                    "—"
                  )}
                </td>
                <td>
                  <span className="rh-detail-arrow">→</span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
