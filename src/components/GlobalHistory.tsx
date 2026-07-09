import { useEffect, useState } from "react";
import { getGlobalRunHistory } from "../lib/commands";
import type { Run } from "../lib/commands";
import { useEnvironments } from "../hooks/useEnvironments";
import { formatRunStatusLabel } from "../lib/runStatus";
import Button from "./Button";
import Input from "./Input";
import Select from "./Select";
import StatusBadge from "./StatusBadge";
import "./RunHistory.css";
import "./QueueView.css";

interface Props {
  onViewRun: (run: Run) => void;
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

export default function GlobalHistory({ onViewRun }: Props) {
  const { environments } = useEnvironments();
  const [runs, setRuns] = useState<Run[]>([]);
  const [statusFilter, setStatusFilter] = useState("all");
  const [triggerKind, setTriggerKind] = useState("all");
  const [environmentFilter, setEnvironmentFilter] = useState("all");
  const [domainFilter, setDomainFilter] = useState("all");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = () => {
    setLoading(true);
    setError(null);
    getGlobalRunHistory(
      statusFilter,
      triggerKind,
      environmentFilter,
      domainFilter,
      100,
    )
      .then(setRuns)
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  };

  // Defer the initial load to a macrotask so load()'s synchronous
  // setLoading(true)/setError(null) do not run inside the effect body
  // (avoids react-hooks/set-state-in-effect). Mirrors useSchedulerStatus.
  useEffect(() => {
    const id = setTimeout(load, 0);
    return () => clearTimeout(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div>
      <div className="page-header">
        <div>
          <h1 className="page-title">Global History</h1>
          <p className="page-subtitle">
            Filter indexed scheduler.db runs across workflows.
          </p>
        </div>
      </div>

      <div className="rh-heatmap" style={{ marginBottom: 16 }}>
        <div className="rh-heatmap-header">
          <h3>Filters</h3>
          <span>{runs.length} run(s)</span>
        </div>
        <div className="queue-fields">
          <label>
            Status
            <Select
              value={statusFilter}
              onChange={(e) => setStatusFilter(e.target.value)}
            >
              <option value="all">All</option>
              <option value="running">Running</option>
              <option value="success">Success</option>
              <option value="failed">Failed</option>
              <option value="skipped">Skipped</option>
              <option value="poll_exhausted">Poll exhausted</option>
            </Select>
          </label>
          <label>
            Trigger
            <Select
              value={triggerKind}
              onChange={(e) => setTriggerKind(e.target.value)}
            >
              <option value="all">All</option>
              <option value="cron">Cron</option>
              <option value="manual">Manual</option>
              <option value="backfill">Backfill</option>
              <option value="child_workflow">Child workflow</option>
            </Select>
          </label>
          <label>
            Environment
            <Select
              value={environmentFilter}
              onChange={(e) => setEnvironmentFilter(e.target.value)}
            >
              <option value="all">All</option>
              {environments.map((env) => (
                <option key={env.id} value={env.name}>
                  {env.name.charAt(0).toUpperCase() + env.name.slice(1)}
                </option>
              ))}
            </Select>
          </label>
          <label>
            Domain
            <Input
              value={domainFilter}
              onChange={(e) => setDomainFilter(e.target.value || "all")}
            />
          </label>
          <Button variant="primary" size="sm" onClick={load} disabled={loading}>
            {loading ? "Loading..." : "Apply"}
          </Button>
        </div>
      </div>

      {error ? (
        <div className="rh-error">
          <span>Global history failed to load: {error}</span>
          <Button variant="ghost" size="sm" onClick={load} disabled={loading}>
            Retry
          </Button>
        </div>
      ) : runs.length === 0 ? (
        <div className="rh-empty">No runs match these filters.</div>
      ) : (
        <table className="rh-table">
          <thead>
            <tr>
              <th>Status</th>
              <th>Workflow</th>
              <th>Started</th>
              <th>Trigger</th>
              <th>Exit Code</th>
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
                <td>{run.workflow_name ?? run.workflow_id}</td>
                <td>{formatDate(run.started_at)}</td>
                <td>{run.trigger_kind ?? "cron"}</td>
                <td>{run.exit_code ?? "—"}</td>
                <td>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => onViewRun(run)}
                  >
                    Details
                  </Button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
