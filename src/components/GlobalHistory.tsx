import { useEffect, useState } from "react";
import { getGlobalRunHistory } from "../lib/commands";
import type { Run } from "../lib/commands";
import { useEnvironments } from "../hooks/useEnvironments";
import Button from "./Button";
import EnvSelect from "./EnvSelect";
import Input from "./Input";
import RunsTable from "./RunsTable";
import Select from "./Select";
import "./RunHistory.css";
import "./QueueView.css";

interface Props {
  onViewRun: (run: Run) => void;
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
            <EnvSelect
              value={environmentFilter}
              onChange={(e) => setEnvironmentFilter(e.target.value)}
              environments={environments}
              includeAllOption
            />
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
      ) : (
        <RunsTable
          runs={runs}
          emptyLabel="No runs match these filters."
          onViewRun={onViewRun}
        />
      )}
    </div>
  );
}
