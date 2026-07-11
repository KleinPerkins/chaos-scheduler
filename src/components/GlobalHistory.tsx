import { useEffect, useMemo, useState } from "react";
import { getGlobalRunHistory } from "../lib/commands";
import type { Run } from "../lib/commands";
import { useEnvironments } from "../hooks/useEnvironments";
import Button from "./Button";
import PageHeader from "./PageHeader";
import EnvSelect from "./EnvSelect";
import Input from "./Input";
import RunsTable from "./RunsTable";
import Select from "./Select";
import "./RunHistory.css";

const GLOBAL_HISTORY_LIMIT = 100;

interface Props {
  onViewRun: (run: Run) => void;
}

export default function GlobalHistory({ onViewRun }: Props) {
  const { environments } = useEnvironments();
  const [runs, setRuns] = useState<Run[]>([]);
  const [statusFilter, setStatusFilter] = useState("all");
  const [triggerKind, setTriggerKind] = useState("all");
  const [environmentFilter, setEnvironmentFilter] = useState("all");
  const [search, setSearch] = useState("");
  const [reloadToken, setReloadToken] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Backend-scoped filters (status / trigger / environment) re-query the indexed
  // scheduler.db, bounded to the latest 100 runs. The initial fetch is deferred
  // to a macrotask so the synchronous setLoading/setError don't run inside the
  // effect body (avoids react-hooks/set-state-in-effect; mirrors other loaders).
  useEffect(() => {
    let cancelled = false;
    const id = setTimeout(() => {
      setLoading(true);
      setError(null);
      getGlobalRunHistory(
        statusFilter,
        triggerKind,
        environmentFilter,
        "all",
        GLOBAL_HISTORY_LIMIT,
      )
        .then((rows) => {
          if (!cancelled) setRuns(rows);
        })
        .catch((e) => {
          if (!cancelled) setError(String(e));
        })
        .finally(() => {
          if (!cancelled) setLoading(false);
        });
    }, 0);
    return () => {
      cancelled = true;
      clearTimeout(id);
    };
  }, [statusFilter, triggerKind, environmentFilter, reloadToken]);

  // Search is intentionally a client-side refinement over the loaded rows only —
  // it never fetches beyond the bounded window, so the contract stays truthful.
  const query = search.trim().toLowerCase();
  const visibleRuns = useMemo(() => {
    if (!query) return runs;
    return runs.filter(
      (run) =>
        (run.workflow_name ?? run.workflow_id).toLowerCase().includes(query) ||
        run.id.toLowerCase().includes(query),
    );
  }, [runs, query]);

  const captionMeta = query
    ? `${visibleRuns.length} of ${runs.length} loaded`
    : loading
      ? "Loading…"
      : `${runs.length} loaded · newest first`;

  return (
    <div>
      <PageHeader
        title="History"
        subtitle="Search and filter every run across all workflows — drill in to per-run logs."
      />

      <div
        className="hist-toolbar"
        role="group"
        aria-label="Run history filters"
      >
        <label className="hist-field">
          <span className="hist-field-label">Environment</span>
          <EnvSelect
            value={environmentFilter}
            onChange={(e) => setEnvironmentFilter(e.target.value)}
            environments={environments}
            includeAllOption
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
        <label className="hist-field">
          <span className="hist-field-label">Trigger</span>
          <Select
            value={triggerKind}
            onChange={(e) => setTriggerKind(e.target.value)}
          >
            <option value="all">All triggers</option>
            <option value="cron">Cron</option>
            <option value="manual">Manual</option>
            <option value="backfill">Backfill</option>
            <option value="child_workflow">Child workflow</option>
          </Select>
        </label>
        <label className="hist-field hist-field-search">
          <span className="sr-only">Search loaded runs</span>
          <Input
            type="search"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search loaded runs — workflow or run ID…"
          />
        </label>
        <span
          className="hist-bounded"
          title={`Showing at most the latest ${GLOBAL_HISTORY_LIMIT} runs`}
        >
          Latest {GLOBAL_HISTORY_LIMIT}
        </span>
      </div>

      {error ? (
        <div className="rh-error">
          <span>History failed to load: {error}</span>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setReloadToken((n) => n + 1)}
            disabled={loading}
          >
            Retry
          </Button>
        </div>
      ) : (
        <>
          <div className="hist-caption">
            <span className="hist-caption-title">Run history</span>
            <span className="hist-caption-meta">{captionMeta}</span>
          </div>
          <RunsTable
            runs={visibleRuns}
            emptyLabel={
              query
                ? "No loaded runs match your search."
                : "No runs match these filters."
            }
            onViewRun={onViewRun}
          />
          <p className="hist-footnote">
            Search filters loaded rows only — older runs beyond the latest{" "}
            {GLOBAL_HISTORY_LIMIT} aren’t fetched.
          </p>
        </>
      )}
    </div>
  );
}
