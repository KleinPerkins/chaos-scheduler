import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  getMissionControlSnapshot,
  setMissionControlPreferences,
  environmentOf,
  type MissionControlActivityItem,
  type MissionControlFreshnessItem,
  type MissionControlPreferences,
  type MissionControlSnapshot,
  type MissionControlWorkflowTelemetry,
  type Run,
} from "../lib/commands";
import { useEnvironments } from "../hooks/useEnvironments";
import { formatRunStatusLabel } from "../lib/runStatus";
import "./MissionControl.css";

export type MissionTab =
  "overview" | "activity" | "freshness" | "telemetry" | "matrix";
export interface MissionControlReturnState {
  tab: MissionTab;
  environmentFilter: MissionControlPreferences["environment_filter"];
  domain: string;
}

interface MissionControlProps {
  initialTab?: MissionTab;
  initialEnvironment?: MissionControlPreferences["environment_filter"];
  initialDomain?: string;
  onOpenRun: (
    runId: string,
    workflowId: string,
    returnState: MissionControlReturnState,
  ) => void;
  onOpenQueues: (returnState: MissionControlReturnState) => void;
  onOpenHistory: (
    workflowId: string,
    returnState: MissionControlReturnState,
  ) => void;
}

interface MissionControlFilters {
  environmentFilter: MissionControlPreferences["environment_filter"];
  domain: string;
}

const MissionControlFiltersContext = createContext<MissionControlFilters>({
  environmentFilter: "all",
  domain: "all",
});

function useMissionControlFilters() {
  return useContext(MissionControlFiltersContext);
}

function formatTime(value?: string | null): string {
  if (!value) return "not recorded";
  return new Date(value).toLocaleString([], {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatTimeUntil(value: string): string {
  const diff = new Date(value).getTime() - Date.now();
  if (diff < 0) return "overdue";
  const minutes = Math.floor(diff / 60000);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ${minutes % 60}m`;
  const days = Math.floor(hours / 24);
  return `${days}d ${hours % 24}h`;
}

function formatPercent(value: number | null): string {
  if (value == null) return "n/a";
  return `${Math.round(value * 100)}%`;
}

function formatBytes(value?: number | null): string {
  if (value == null) return "no samples";
  const mib = value / 1024 / 1024;
  return `${mib.toFixed(mib > 99 ? 0 : 1)} MiB`;
}

function statusClass(status: string) {
  return status === "success" || status === "succeeded" ? "success" : status;
}

function EmptyPanel({ children }: { children: string }) {
  return <div className="mc-empty">{children}</div>;
}

function HeaderStatus({ snapshot }: { snapshot: MissionControlSnapshot }) {
  const { environmentFilter, domain } = useMissionControlFilters();
  const cards = [
    ["Active workflows", snapshot.header.active_workflows],
    ["Running now", snapshot.header.running_count],
    ["Queued / admitted", snapshot.header.queued_count],
    ["24h failures", snapshot.header.recent_failures],
  ];
  return (
    <section className="mc-hero-panel">
      <div>
        <p className="mc-kicker">Mission Control</p>
        <h1>Scheduler operations by environment and owner</h1>
        <p className="mc-hero-copy">
          Durable scheduler.db state only. Filtered by {environmentFilter} /{" "}
          {domain === "__unowned__" ? "Unowned" : domain}.
        </p>
      </div>
      <div className="mc-stat-grid">
        {cards.map(([label, value]) => (
          <div className="mc-stat-card" key={label}>
            <span className="mc-stat-value">{value}</span>
            <span className="mc-stat-label">{label}</span>
          </div>
        ))}
      </div>
    </section>
  );
}

function SlaStrip({
  snapshot,
  onOpenQueues,
}: {
  snapshot: MissionControlSnapshot;
  onOpenQueues: () => void;
}) {
  const items = [
    { label: "SLA risks", value: snapshot.sla.violations_count.toString() },
    {
      label: "24h success",
      value: formatPercent(snapshot.sla.success_rate_24h),
    },
    {
      label: "Median queue wait",
      value:
        snapshot.sla.median_wait_seconds == null
          ? "n/a"
          : `${snapshot.sla.median_wait_seconds}s`,
    },
    {
      label: "Waiting",
      value: snapshot.sla.blocked_count.toString(),
      action: onOpenQueues,
    },
  ];
  return (
    <section className="mc-panel mc-sla-strip">
      {items.map((item) =>
        item.action ? (
          <button
            className="mc-sla-card"
            key={item.label}
            onClick={item.action}
          >
            <span>{item.label}</span>
            <strong>{item.value}</strong>
          </button>
        ) : (
          <div className="mc-sla-card" key={item.label}>
            <span>{item.label}</span>
            <strong>{item.value}</strong>
          </div>
        ),
      )}
    </section>
  );
}

function NeedsAttention({
  snapshot,
  onOpenRun,
  onOpenQueues,
  onOpenHistory,
}: {
  snapshot: MissionControlSnapshot;
  onOpenRun: (runId: string, workflowId: string) => void;
  onOpenQueues: () => void;
  onOpenHistory: (workflowId: string) => void;
}) {
  const attentionAction = (
    item: MissionControlSnapshot["needs_attention"][number],
  ) => {
    if (item.run_id && item.workflow_id)
      return () => onOpenRun(item.run_id!, item.workflow_id!);
    if (item.target === "queues") return onOpenQueues;
    if (item.target === "history" && item.workflow_id)
      return () => onOpenHistory(item.workflow_id!);
    return null;
  };
  return (
    <section className="mc-panel">
      <div className="mc-panel-header">
        <h2>Needs Attention</h2>
        <span>
          {snapshot.needs_attention_truncated
            ? `top ${snapshot.needs_attention.length} of ${snapshot.needs_attention_total} persisted issues`
            : `${snapshot.needs_attention_total} persisted issues`}
        </span>
      </div>
      {snapshot.needs_attention.length === 0 ? (
        <EmptyPanel>No persisted issues need attention.</EmptyPanel>
      ) : (
        <div className="mc-attention-list">
          {snapshot.needs_attention.map((item) => {
            const action = attentionAction(item);
            const content = (
              <>
                <span className="mc-attention-severity">{item.severity}</span>
                <span>
                  <strong>{item.title}</strong>
                  <small>{item.detail}</small>
                </span>
              </>
            );
            return action ? (
              <button
                className={`mc-attention-item ${item.severity}`}
                key={item.id}
                onClick={action}
              >
                {content}
              </button>
            ) : (
              <div
                className={`mc-attention-item ${item.severity}`}
                key={item.id}
              >
                {content}
              </div>
            );
          })}
        </div>
      )}
    </section>
  );
}

function ActivityList({
  title,
  items,
  onOpenRun,
}: {
  title: string;
  items: MissionControlActivityItem[];
  onOpenRun: (runId: string, workflowId: string) => void;
}) {
  return (
    <section className="mc-panel">
      <div className="mc-panel-header">
        <h2>{title}</h2>
        <span>auto-refreshes every 15s</span>
      </div>
      {items.length === 0 ? (
        <EmptyPanel>No run activity for this filter.</EmptyPanel>
      ) : (
        <div className="mc-table">
          {items.map((item) => (
            <button
              className="mc-table-row"
              key={item.id}
              onClick={() => onOpenRun(item.run_id, item.workflow_id)}
            >
              <span className={`mc-dot ${statusClass(item.status)}`} />
              <span>
                <strong>{item.workflow_name}</strong>
                <small>
                  {item.domain} / {environmentOf(item)}
                </small>
              </span>
              <span className={`status-badge ${statusClass(item.status)}`}>
                {formatRunStatusLabel(item.status)}
              </span>
              <time dateTime={item.started_at}>
                {formatTime(item.started_at)}
              </time>
            </button>
          ))}
        </div>
      )}
    </section>
  );
}

function UpcomingRuns({ snapshot }: { snapshot: MissionControlSnapshot }) {
  return (
    <section className="mc-panel">
      <div className="mc-panel-header">
        <h2>Upcoming Runs</h2>
        <span>fixed-time cron triggers</span>
      </div>
      {snapshot.upcoming_runs.length === 0 ? (
        <EmptyPanel>
          No fixed-time cron triggers match this filter. Event-driven workflows
          need durable readiness state before Mission Control can show ETA.
        </EmptyPanel>
      ) : (
        <div className="mc-upcoming-grid">
          {snapshot.upcoming_runs.map((run) => (
            <div
              className="mc-upcoming-card"
              key={`${run.workflow_id}-${run.trigger_label}`}
            >
              <span>{formatTimeUntil(run.next_time)}</span>
              <strong>{run.workflow_name}</strong>
              <small>
                {run.domain} / {run.trigger_label}
              </small>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

function RecentRuns({
  runs,
  onOpenRun,
}: {
  runs: Run[];
  onOpenRun: (runId: string, workflowId: string) => void;
}) {
  return (
    <section className="mc-panel">
      <div className="mc-panel-header">
        <h2>Recent Runs</h2>
        <span>filtered before limit</span>
      </div>
      {runs.length === 0 ? (
        <EmptyPanel>No recent runs for this filter.</EmptyPanel>
      ) : (
        <div className="mc-table">
          {runs.map((run) => (
            <button
              className="mc-table-row"
              key={run.id}
              onClick={() => onOpenRun(run.id, run.workflow_id)}
            >
              <span className={`mc-dot ${statusClass(run.status)}`} />
              <span>
                <strong>{run.workflow_name ?? run.workflow_id}</strong>
                <small>{run.workflow_id.slice(0, 8)}</small>
              </span>
              <span className={`status-badge ${statusClass(run.status)}`}>
                {formatRunStatusLabel(run.status)}
              </span>
              <time dateTime={run.started_at}>
                {formatTime(run.started_at)}
              </time>
            </button>
          ))}
        </div>
      )}
    </section>
  );
}

function FreshnessLedger({ items }: { items: MissionControlFreshnessItem[] }) {
  return (
    <section className="mc-panel mc-panel-wide">
      <div className="mc-panel-header">
        <h2>SLA & Freshness Ledger</h2>
        <span>assets join through last_writer_run_id</span>
      </div>
      {items.length === 0 ? (
        <EmptyPanel>No stale assets for this filter.</EmptyPanel>
      ) : (
        <div className="mc-table">
          {items.map((item) => (
            <div className="mc-table-row" key={item.asset_id}>
              <span className="mc-asset-kind">{item.asset_kind}</span>
              <span>
                <strong>{item.asset_namespace}</strong>
                <small>{item.asset_partition}</small>
              </span>
              <span>{item.domain}</span>
              <small>{item.attribution}</small>
              {item.last_written_at ? (
                <time dateTime={item.last_written_at}>
                  {formatTime(item.last_written_at)}
                </time>
              ) : (
                <span>not recorded</span>
              )}
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

function TelemetryCards({
  items,
}: {
  items: MissionControlWorkflowTelemetry[];
}) {
  return (
    <section className="mc-panel mc-panel-wide">
      <div className="mc-panel-header">
        <h2>Per-workflow Resource & Token Rollup</h2>
        <span>bounded backend batch</span>
      </div>
      {items.length === 0 ? (
        <EmptyPanel>No active workflows match this filter.</EmptyPanel>
      ) : (
        <div className="mc-telemetry-grid">
          {items.map((item) => (
            <div className="mc-telemetry-card" key={item.workflow_id}>
              <div>
                <strong>{item.workflow_name}</strong>
                <small>
                  {item.domain} / {environmentOf(item)}
                </small>
              </div>
              <div className="mc-meter-row">
                <span>CPU</span>
                <b>
                  {item.max_cpu_percent == null
                    ? "no samples"
                    : `${item.max_cpu_percent.toFixed(1)}%`}
                </b>
              </div>
              <div className="mc-meter-row">
                <span>Memory</span>
                <b>{formatBytes(item.max_memory_rss_bytes)}</b>
              </div>
              <div className="mc-meter-row">
                <span>Tokens</span>
                <b>{item.total_tokens.toLocaleString()}</b>
              </div>
              <small>
                {item.sample_count} resource samples, {item.token_call_count}{" "}
                token calls
              </small>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

export default function MissionControl({
  initialTab = "overview",
  initialEnvironment,
  initialDomain,
  onOpenRun,
  onOpenQueues,
  onOpenHistory,
}: MissionControlProps) {
  const { environments } = useEnvironments();
  const [snapshot, setSnapshot] = useState<MissionControlSnapshot | null>(null);
  const [tab, setTab] = useState<MissionTab>(initialTab);
  const [environmentFilter, setEnvironmentFilter] = useState<
    MissionControlPreferences["environment_filter"]
  >(initialEnvironment ?? "all");
  const [domain, setDomain] = useState(initialDomain ?? "all");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const latestRequestId = useRef(0);
  const latestPreferenceWriteId = useRef(0);
  const persistentRequestInFlight = useRef(false);
  const preferenceWriteQueue = useRef<Promise<void>>(Promise.resolve());

  const loadSnapshot = useCallback(
    async (
      nextEnvironment?: MissionControlPreferences["environment_filter"],
      nextDomain?: string,
      persist = false,
    ) => {
      const requestId = latestRequestId.current + 1;
      latestRequestId.current = requestId;
      if (persist) persistentRequestInFlight.current = true;
      setLoading(true);
      setError(null);
      try {
        const next = await getMissionControlSnapshot(
          nextEnvironment,
          nextDomain,
        );
        if (requestId !== latestRequestId.current) return;
        setSnapshot(next);
        setEnvironmentFilter(next.preferences.environment_filter);
        setDomain(next.preferences.domain_filter);
        if (persist) {
          const writeId = latestPreferenceWriteId.current + 1;
          latestPreferenceWriteId.current = writeId;
          preferenceWriteQueue.current = preferenceWriteQueue.current
            .catch(() => undefined)
            .then(async () => {
              if (writeId !== latestPreferenceWriteId.current) return;
              await setMissionControlPreferences(
                next.preferences.default_landing,
                next.preferences.environment_filter,
                next.preferences.domain_filter,
              );
            })
            .catch((e) => {
              if (writeId === latestPreferenceWriteId.current) {
                setError(`Filter updated but not saved: ${String(e)}`);
              }
            });
          void preferenceWriteQueue.current;
        }
      } catch (e) {
        if (requestId !== latestRequestId.current) return;
        setError(String(e));
      } finally {
        if (persist) persistentRequestInFlight.current = false;
        if (requestId === latestRequestId.current) setLoading(false);
      }
    },
    [],
  );

  // Defer the initial snapshot load to a macrotask so loadSnapshot's
  // synchronous state updates do not run inside the effect body (avoids
  // react-hooks/set-state-in-effect). Mirrors useSchedulerStatus.
  useEffect(() => {
    const id = setTimeout(
      () => void loadSnapshot(initialEnvironment, initialDomain, false),
      0,
    );
    return () => clearTimeout(id);
  }, [initialEnvironment, initialDomain, loadSnapshot]);

  useEffect(() => {
    if (!snapshot) return;
    const timer = window.setInterval(() => {
      if (persistentRequestInFlight.current) return;
      void loadSnapshot(environmentFilter, domain, false);
    }, 15000);
    return () => window.clearInterval(timer);
  }, [loadSnapshot, environmentFilter, domain, snapshot]);

  const filters = useMemo(
    () => ({ environmentFilter, domain }),
    [environmentFilter, domain],
  );
  const returnState = useMemo(
    () => ({ tab, environmentFilter, domain }),
    [tab, environmentFilter, domain],
  );

  // Environment filter options are sourced from the environments backend and
  // unioned with the currently-selected value (so an out-of-list selection is
  // still shown). Unknown values normalize to "all" server-side (graceful).
  const envFilterOptions = useMemo(() => {
    const names = new Set<string>(["all"]);
    for (const env of environments) names.add(env.name);
    if (environmentFilter) names.add(environmentFilter);
    return Array.from(names);
  }, [environments, environmentFilter]);

  if (loading && !snapshot) {
    return <div className="mc-loading">Loading Mission Control...</div>;
  }

  if (error && !snapshot) {
    return (
      <div className="mc-error">Mission Control failed to load: {error}</div>
    );
  }

  if (!snapshot) return null;

  return (
    <MissionControlFiltersContext.Provider value={filters}>
      <div className="mission-control">
        <div className="mc-toolbar">
          <div
            className="mc-tabs"
            role="tablist"
            aria-label="Mission Control tabs"
          >
            {(
              [
                "overview",
                "activity",
                "freshness",
                "telemetry",
                "matrix",
              ] as const
            ).map((item) => (
              <button
                key={item}
                id={`mc-tab-${item}`}
                className={tab === item ? "active" : ""}
                onClick={() => setTab(item)}
                role="tab"
                aria-selected={tab === item}
                aria-controls={`mc-panel-${item}`}
                tabIndex={tab === item ? 0 : -1}
              >
                {item}
              </button>
            ))}
          </div>
          <div className="mc-filters">
            <div
              className="mc-segmented"
              role="group"
              aria-label="Environment filter"
            >
              {envFilterOptions.map((item) => (
                <button
                  key={item}
                  className={environmentFilter === item ? "active" : ""}
                  onClick={() => {
                    void loadSnapshot(item, domain, true);
                  }}
                  aria-pressed={environmentFilter === item}
                >
                  {item}
                </button>
              ))}
            </div>
            <select
              value={domain}
              onChange={(event) => {
                void loadSnapshot(environmentFilter, event.target.value, true);
              }}
              aria-label="Domain filter"
            >
              {snapshot.domains.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label} ({option.workflow_count})
                </option>
              ))}
            </select>
          </div>
        </div>

        {error && <div className="mc-inline-error">{error}</div>}

        {tab === "overview" && (
          <div
            id="mc-panel-overview"
            role="tabpanel"
            aria-labelledby="mc-tab-overview"
            tabIndex={0}
            className="mc-grid"
          >
            <HeaderStatus snapshot={snapshot} />
            <SlaStrip
              snapshot={snapshot}
              onOpenQueues={() => onOpenQueues(returnState)}
            />
            <NeedsAttention
              snapshot={snapshot}
              onOpenRun={(runId, workflowId) =>
                onOpenRun(runId, workflowId, returnState)
              }
              onOpenQueues={() => onOpenQueues(returnState)}
              onOpenHistory={(workflowId) =>
                onOpenHistory(workflowId, returnState)
              }
            />
            <UpcomingRuns snapshot={snapshot} />
            <RecentRuns
              runs={snapshot.recent_runs}
              onOpenRun={(runId, workflowId) =>
                onOpenRun(runId, workflowId, returnState)
              }
            />
          </div>
        )}

        {tab === "activity" && (
          <div
            id="mc-panel-activity"
            role="tabpanel"
            aria-labelledby="mc-tab-activity"
            tabIndex={0}
          >
            <ActivityList
              title="Live Activity"
              items={snapshot.live_activity}
              onOpenRun={(runId, workflowId) =>
                onOpenRun(runId, workflowId, returnState)
              }
            />
          </div>
        )}

        {tab === "freshness" && (
          <div
            id="mc-panel-freshness"
            role="tabpanel"
            aria-labelledby="mc-tab-freshness"
            tabIndex={0}
          >
            <FreshnessLedger items={snapshot.freshness_ledger} />
          </div>
        )}

        {tab === "telemetry" && (
          <div
            id="mc-panel-telemetry"
            role="tabpanel"
            aria-labelledby="mc-tab-telemetry"
            tabIndex={0}
          >
            <TelemetryCards items={snapshot.workflow_telemetry} />
          </div>
        )}

        {tab === "matrix" && (
          <div
            id="mc-panel-matrix"
            role="tabpanel"
            aria-labelledby="mc-tab-matrix"
            tabIndex={0}
          >
            <section className="mc-panel mc-panel-wide">
              <div className="mc-panel-header">
                <h2>Panel Availability Matrix</h2>
                <span>v0 contract</span>
              </div>
              <table className="mc-matrix">
                <thead>
                  <tr>
                    <th>Panel</th>
                    <th>Source Tables</th>
                    <th>Filter Behavior</th>
                    <th>Empty State</th>
                    <th>Degraded State</th>
                  </tr>
                </thead>
                <tbody>
                  {snapshot.availability.map((item) => (
                    <tr key={item.panel}>
                      <th scope="row">{item.panel}</th>
                      <td>{item.source_tables.join(", ")}</td>
                      <td>{item.filter_behavior}</td>
                      <td>{item.empty_state}</td>
                      <td>{item.degraded_state}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </section>
          </div>
        )}
      </div>
    </MissionControlFiltersContext.Provider>
  );
}
