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
import {
  DEFAULT_LOOKBACK,
  lookbackToParam,
  type Lookback,
} from "../lib/lookback";
import { formatRunStatusLabel, statusKey } from "../lib/runStatus";
import FilterBar, { type CustomRange } from "./FilterBar";
import {
  NeedsAttentionPage,
  NeedsAttentionSummary,
} from "./missionControl/NeedsAttention";
import {
  OperationalHealthPage,
  OperationalHealthSummary,
} from "./missionControl/OperationalHealth";
import { ResourcesPage, ResourcesSummary } from "./missionControl/Resources";
import { useNeedsAttention } from "./missionControl/useNeedsAttention";
import { useOperationalHealth } from "./missionControl/useOperationalHealth";
import { useResources } from "./missionControl/useResources";
import Overview from "./overview/Overview";
import Select from "./Select";
import StatusBadge from "./StatusBadge";
import StatusDot from "./StatusDot";
import "./MissionControl.css";

/** Which full-detail Mission Control drill-down is open, if any (G09/G06). */
export type MissionDrilldown =
  "needs-attention" | "operational-health" | "resources" | null;

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

function formatBytes(value?: number | null): string {
  if (value == null) return "no samples";
  const mib = value / 1024 / 1024;
  return `${mib.toFixed(mib > 99 ? 0 : 1)} MiB`;
}

/** Format a `Date` as the `yyyy-mm-dd` value an `<input type="date">` expects. */
function toDateInputValue(date: Date): string {
  return date.toISOString().slice(0, 10);
}

/** Seed a `custom` window with a trailing 7-day range ending today. */
function defaultCustomRange(now: Date = new Date()): CustomRange {
  const start = new Date(now.getTime() - 7 * 24 * 60 * 60 * 1000);
  return { start: toDateInputValue(start), end: toDateInputValue(now) };
}

function EmptyPanel({ children }: { children: string }) {
  return <div className="mc-empty">{children}</div>;
}

function HeaderStatus() {
  const { environmentFilter, domain } = useMissionControlFilters();
  return (
    <section className="mc-hero-panel mc-hero-panel--slim">
      <div>
        <p className="mc-kicker">Mission Control</p>
        <h1>Scheduler operations by environment and owner</h1>
        <p className="mc-hero-copy">
          Durable scheduler.db state only. Filtered by {environmentFilter} /{" "}
          {domain === "__unowned__" ? "Unowned" : domain}.
        </p>
      </div>
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
              <StatusDot variant="mc-dot" status={statusKey(item.status)} />
              <span>
                <strong>{item.workflow_name}</strong>
                <small>
                  {item.domain} / {environmentOf(item)}
                </small>
              </span>
              <StatusBadge status={statusKey(item.status)}>
                {formatRunStatusLabel(item.status)}
              </StatusBadge>
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
              <StatusDot variant="mc-dot" status={statusKey(run.status)} />
              <span>
                <strong>{run.workflow_name ?? run.workflow_id}</strong>
                <small>{run.workflow_id.slice(0, 8)}</small>
              </span>
              <StatusBadge status={statusKey(run.status)}>
                {formatRunStatusLabel(run.status)}
              </StatusBadge>
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
  // Which full-detail two-group drill-down is open on the overview surface.
  // Local, ephemeral UI state layered ON TOP of the overview tab — the shared
  // (environment, lookback, domain) filters live above it and are untouched
  // when drilling in/out, so return state is preserved (G06).
  const [drilldown, setDrilldown] = useState<MissionDrilldown>(null);
  const [environmentFilter, setEnvironmentFilter] = useState<
    MissionControlPreferences["environment_filter"]
  >(initialEnvironment ?? "all");
  const [domain, setDomain] = useState(initialDomain ?? "all");
  // Shared (environment, lookback) filter state. `environment` reuses the
  // server-authoritative `environmentFilter` above; `lookback` is the new
  // standardized window (PR B's Overview queries consume it via
  // `lookbackToParam`). Existing snapshot queries are lookback-agnostic, so
  // changing it is behavior-preserving here.
  const [lookback, setLookback] = useState<Lookback>(DEFAULT_LOOKBACK);
  const [customRange, setCustomRange] = useState<CustomRange | undefined>(
    undefined,
  );
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const latestRequestId = useRef(0);
  const latestPreferenceWriteId = useRef(0);
  const persistentRequestInFlight = useRef(false);
  const preferenceWriteQueue = useRef<Promise<void>>(Promise.resolve());
  const rootRef = useRef<HTMLDivElement>(null);

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

  // Running jobs for the Overview race hero come from the snapshot Mission
  // Control already loads (live_activity), filtered to the running status.
  const runningJobs = useMemo(
    () =>
      snapshot
        ? snapshot.live_activity.filter(
            (item) => statusKey(item.status) === "running",
          )
        : [],
    [snapshot],
  );

  // Serialize the shared (environment, lookback) selection to the backend
  // grammar the Overview's get_dashboard_* queries consume. A `custom` window
  // with no (or invalid) range falls back to the default rather than throwing.
  const lookbackParam = useMemo(() => {
    try {
      if (lookback === "custom") {
        if (!customRange?.start || !customRange?.end) return DEFAULT_LOOKBACK;
        return lookbackToParam("custom", {
          customStart: new Date(customRange.start),
          customEnd: new Date(customRange.end),
        });
      }
      return lookbackToParam(lookback);
    } catch {
      return DEFAULT_LOOKBACK;
    }
  }, [lookback, customRange]);

  // The two-group drill-down surfaces consume the shared (environment,
  // lookback) selection through their own `get_dashboard_*` queries. Fetched
  // once here so the at-a-glance summary card and the full-detail subpage share
  // one request (and one refresh when the filters change).
  const needsAttention = useNeedsAttention(environmentFilter, lookbackParam);
  const operationalHealth = useOperationalHealth(
    environmentFilter,
    lookbackParam,
  );
  const resources = useResources(environmentFilter, lookbackParam);

  // Entering a full-detail drill-down replaces the overview landing, so land at
  // the top of the scroll viewport (its "← Back to overview" affordance),
  // exactly like navigating to a new page — otherwise a drill opened from a
  // scrolled-down group card would render mid-content with its header hidden
  // under the sticky toolbar. Only fires on entry; leaving a drill preserves
  // the overview's own position (G06).
  useEffect(() => {
    if (!drilldown) return;
    rootRef.current?.closest(".dashboard-main")?.scrollTo({ top: 0 });
  }, [drilldown]);

  // Switching to another top-level tab leaves the overview drill-down, so
  // returning to overview lands back on the two-group summary.
  const handleTabChange = useCallback((next: MissionTab) => {
    setTab(next);
    if (next !== "overview") setDrilldown(null);
  }, []);

  const handleEnvironmentChange = useCallback(
    (next: string) => {
      void loadSnapshot(next, domain, true);
    },
    [loadSnapshot, domain],
  );

  // Seed a trailing 7-day range the first time `custom` is chosen so the picker
  // is never a dead affordance (interaction-time only — the default 1d window
  // renders no picker, keeping the initial surface deterministic).
  const handleLookbackChange = useCallback((next: Lookback) => {
    setLookback(next);
    if (next === "custom") {
      setCustomRange((prev) => prev ?? defaultCustomRange());
    }
  }, []);

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
      <div className="mission-control" ref={rootRef}>
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
                onClick={() => handleTabChange(item)}
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
            <FilterBar
              environments={environments}
              environment={environmentFilter}
              onEnvironmentChange={handleEnvironmentChange}
              lookback={lookback}
              onLookbackChange={handleLookbackChange}
              customRange={customRange}
              onCustomRangeChange={setCustomRange}
              extras={
                <label>
                  <span className="filter-bar-label-text">Domain</span>
                  <Select
                    value={domain}
                    onChange={(event) => {
                      void loadSnapshot(
                        environmentFilter,
                        event.target.value,
                        true,
                      );
                    }}
                    aria-label="Domain filter"
                  >
                    {snapshot.domains.map((option) => (
                      <option key={option.value} value={option.value}>
                        {option.label} ({option.workflow_count})
                      </option>
                    ))}
                  </Select>
                </label>
              }
            />
          </div>
        </div>

        {error && <div className="mc-inline-error">{error}</div>}

        {tab === "overview" && (
          <div
            id="mc-panel-overview"
            role="tabpanel"
            aria-labelledby="mc-tab-overview"
            tabIndex={0}
            className="mc-overview-panel"
          >
            {drilldown === "needs-attention" ? (
              <NeedsAttentionPage
                state={needsAttention}
                onBack={() => setDrilldown(null)}
                onOpenQueues={() => onOpenQueues(returnState)}
                onOpenHistory={(workflowId) =>
                  onOpenHistory(workflowId, returnState)
                }
              />
            ) : drilldown === "operational-health" ? (
              <OperationalHealthPage
                state={operationalHealth}
                onBack={() => setDrilldown(null)}
              />
            ) : drilldown === "resources" ? (
              <ResourcesPage
                state={resources}
                onBack={() => setDrilldown(null)}
              />
            ) : (
              <>
                <HeaderStatus />
                <Overview
                  environmentFilter={environmentFilter}
                  lookbackParam={lookbackParam}
                  lookbackLabel={lookback}
                  runningJobs={runningJobs}
                />
                {/* Two-group IA (G09): at-a-glance group summaries →
                    in-place expand → full-detail drill-down subpage. The
                    "Critical / Needs Attention" group supersedes the legacy
                    SLA strip + persisted-issues panel; "Operational Health"
                    is the Operational Health group (F03). */}
                <section
                  className="mc-groups"
                  aria-label="Mission Control groups"
                >
                  <NeedsAttentionSummary
                    state={needsAttention}
                    onViewDetails={() => setDrilldown("needs-attention")}
                  />
                  <OperationalHealthSummary
                    state={operationalHealth}
                    onViewDetails={() => setDrilldown("operational-health")}
                  />
                  <ResourcesSummary
                    state={resources}
                    onViewDetails={() => setDrilldown("resources")}
                  />
                </section>
                {/* Remaining legacy surfaces kept reachable (G06) until their
                    gated drill-downs land (Activity reconciliation, PR D). */}
                <div className="mc-grid">
                  <UpcomingRuns snapshot={snapshot} />
                  <RecentRuns
                    runs={snapshot.recent_runs}
                    onOpenRun={(runId, workflowId) =>
                      onOpenRun(runId, workflowId, returnState)
                    }
                  />
                </div>
              </>
            )}
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
