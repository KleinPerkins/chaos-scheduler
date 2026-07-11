import { useEffect, useMemo, useState } from "react";
import type { MissionControlActivityItem } from "../../lib/commands";
import { formatDuration } from "../../lib/duration";
import DualAxisLine from "../charts/DualAxisLine";
import RaceTrack from "../charts/RaceTrack";
import StatusDonut from "../charts/StatusDonut";
import InfoTip from "../InfoTip";
import {
  buildKpiCards,
  buildRaceJobs,
  deriveSlaWarning,
  runningNow,
  statusDistributionTotal,
  statusDonutSegments,
  totalQueueDepth,
  trendToChart,
  type DeltaDirection,
  type KpiCard,
  type RaceBuildResult,
  type SlaWarning,
} from "./overviewData";
import { useDashboardOverview } from "./useDashboardOverview";
import "./Overview.css";

export interface OverviewProps {
  /** Active environment filter ("all" or an environment name). */
  environmentFilter: string;
  /** Lookback serialized to the backend grammar (via `lookbackToParam`). */
  lookbackParam: string;
  /** Human lookback label for delta phrasing (e.g. `1d`, `custom`). */
  lookbackLabel: string;
  /** Running jobs Mission Control already loaded (snapshot `live_activity`). */
  runningJobs: MissionControlActivityItem[];
  /** Reference "now" for the race elapsed. Defaults to the current time; the
   * frozen test clock keeps it deterministic. */
  nowMs?: number;
}

const ARROW: Record<DeltaDirection, string> = {
  up: "\u25B2",
  down: "\u25BC",
  flat: "\u2014",
};

function KpiDeltaView({ delta }: { delta: KpiCard["delta"] }) {
  if (!delta) {
    return (
      <span className="mc-ov-kpi__delta mc-ov-kpi__delta--live">live</span>
    );
  }
  return (
    <span className={`mc-ov-kpi__delta mc-ov-kpi__delta--${delta.tone}`}>
      <span aria-hidden="true">
        {ARROW[delta.direction]} {delta.text}
      </span>
      <span className="sr-only">{delta.srText}</span>
    </span>
  );
}

function KpiStrip({ cards }: { cards: KpiCard[] }) {
  return (
    <section className="mc-ov-kpis" aria-labelledby="mc-ov-kpis-title">
      <h2 id="mc-ov-kpis-title" className="sr-only">
        Key metrics
      </h2>
      <div className="mc-ov-kpi-grid">
        {cards.map((card) => (
          <div className="mc-ov-kpi" key={card.key}>
            <span className="mc-ov-kpi__label">
              <span className="mc-ov-kpi__label-text">{card.label}</span>
              <InfoTip title={card.infoTitle} def={card.infoDef} />
            </span>
            <span className="mc-ov-kpi__value">{card.value}</span>
            <KpiDeltaView delta={card.delta} />
          </div>
        ))}
      </div>
    </section>
  );
}

function SlaBanner({ warning }: { warning: SlaWarning }) {
  const queues = [...warning.degradedQueues, ...warning.warnQueues];
  return (
    <div
      className={`mc-ov-sla mc-ov-sla--${warning.level}`}
      data-testid="mc-ov-sla-banner"
    >
      <span className="mc-ov-sla__badge" aria-hidden="true">
        !
      </span>
      <div className="mc-ov-sla__body">
        <strong className="mc-ov-sla__headline">
          {warning.level === "degraded" ? "Queues degraded" : "Queue pressure"}
        </strong>
        <span className="mc-ov-sla__detail">
          {warning.headline}
          {queues.length > 0 ? ` — ${queues.join(", ")}` : ""}
        </span>
      </div>
    </div>
  );
}

function RaceHero({ result }: { result: RaceBuildResult }) {
  return (
    <section
      className="mc-ov-card mc-ov-race"
      aria-labelledby="mc-ov-race-title"
    >
      <div className="mc-ov-card__head">
        <h2 id="mc-ov-race-title">Running now</h2>
        <InfoTip
          title="Running now"
          def="Live runs racing toward their expected (p50) runtime; past the flag is overtime."
        />
      </div>
      <RaceTrack jobs={result.jobs} title={null} />
      {result.missingBaselineCount > 0 ? (
        <p className="mc-ov-race__note">
          {result.missingBaselineCount} running{" "}
          {result.missingBaselineCount === 1 ? "job" : "jobs"} without a runtime
          baseline (listed below, excluded from the race).
        </p>
      ) : null}
      <table className="sr-only">
        <caption>Running jobs versus expected runtime</caption>
        <thead>
          <tr>
            <th scope="col">Job</th>
            <th scope="col">Environment</th>
            <th scope="col">Elapsed</th>
            <th scope="col">Expected</th>
          </tr>
        </thead>
        <tbody>
          {result.rows.length === 0 ? (
            <tr>
              <td colSpan={4}>No running jobs</td>
            </tr>
          ) : (
            result.rows.map((row, i) => (
              <tr key={`${row.job}-${i}`}>
                <th scope="row">{row.job}</th>
                <td>{row.agent}</td>
                <td>{formatDuration(row.elapsedSeconds * 1000)}</td>
                <td>
                  {row.expectedSeconds == null
                    ? "no baseline"
                    : formatDuration(row.expectedSeconds * 1000)}
                </td>
              </tr>
            ))
          )}
        </tbody>
      </table>
    </section>
  );
}

function StatusDistribution({
  segments,
  total,
}: {
  segments: ReturnType<typeof statusDonutSegments>;
  total: number;
}) {
  const [format, setFormat] = useState<"count" | "percent">("count");
  const formatValue = (value: number) =>
    format === "percent" && total > 0
      ? `${Math.round((value / total) * 100)}%`
      : value.toLocaleString();

  return (
    <section
      className="mc-ov-card mc-ov-donut"
      aria-labelledby="mc-ov-donut-title"
    >
      <div className="mc-ov-card__head">
        <h2 id="mc-ov-donut-title">Status distribution</h2>
        <InfoTip
          title="Status distribution"
          def="Share of runs by terminal status over the selected window."
        />
        <div
          className="mc-ov-seg"
          role="group"
          aria-label="Legend value format"
        >
          <button
            type="button"
            aria-pressed={format === "count"}
            onClick={() => setFormat("count")}
          >
            Count
          </button>
          <button
            type="button"
            aria-pressed={format === "percent"}
            onClick={() => setFormat("percent")}
          >
            %
          </button>
        </div>
      </div>
      <div className="mc-ov-donut__body">
        <StatusDonut
          segments={segments}
          size={168}
          centerValue={total.toLocaleString()}
          centerLabel="runs"
        />
        {segments.length > 0 ? (
          <ul className="mc-ov-legend">
            {segments.map((seg) => (
              <li key={seg.label} className="mc-ov-legend__item">
                <span
                  className="mc-ov-legend__swatch"
                  style={{ background: seg.color }}
                  aria-hidden="true"
                />
                <span className="mc-ov-legend__label">{seg.label}</span>
                <span className="mc-ov-legend__value">
                  {formatValue(seg.value)}
                </span>
              </li>
            ))}
          </ul>
        ) : null}
      </div>
      <table className="sr-only">
        <caption>Run status distribution</caption>
        <thead>
          <tr>
            <th scope="col">Status</th>
            <th scope="col">Count</th>
            <th scope="col">Share</th>
          </tr>
        </thead>
        <tbody>
          {segments.length === 0 ? (
            <tr>
              <td colSpan={3}>No data</td>
            </tr>
          ) : (
            segments.map((seg) => (
              <tr key={seg.label}>
                <th scope="row">{seg.label}</th>
                <td>{seg.value.toLocaleString()}</td>
                <td>
                  {total > 0
                    ? `${Math.round((seg.value / total) * 100)}%`
                    : "—"}
                </td>
              </tr>
            ))
          )}
        </tbody>
      </table>
    </section>
  );
}

function SuccessFailTrend({
  chart,
}: {
  chart: ReturnType<typeof trendToChart>;
}) {
  const hasData = chart.categories.length > 0;
  return (
    <section
      className="mc-ov-card mc-ov-trend"
      aria-labelledby="mc-ov-trend-title"
    >
      <div className="mc-ov-card__head">
        <h2 id="mc-ov-trend-title">Success / failure trend</h2>
        <InfoTip
          title="Success / failure trend"
          def="Succeeded vs failed runs per interval across the selected window."
        />
        <ul className="mc-ov-legend mc-ov-legend--inline" aria-hidden="true">
          <li className="mc-ov-legend__item">
            <span
              className="mc-ov-legend__swatch"
              style={{ background: "var(--success)" }}
            />
            <span className="mc-ov-legend__label">Succeeded</span>
          </li>
          <li className="mc-ov-legend__item">
            <span
              className="mc-ov-legend__swatch"
              style={{ background: "var(--error)" }}
            />
            <span className="mc-ov-legend__label">Failed</span>
          </li>
        </ul>
      </div>
      {hasData ? (
        <DualAxisLine
          categories={chart.categories}
          leftSeries={[
            {
              label: "Succeeded",
              data: chart.succeeded,
              color: "var(--success)",
            },
            { label: "Failed", data: chart.failed, color: "var(--error)" },
          ]}
          showAxes
          height={150}
          ariaLabel={`Succeeded and failed runs across ${chart.categories.length} intervals`}
        />
      ) : (
        <p className="mc-ov-empty">No runs in the selected window.</p>
      )}
      <table className="sr-only">
        <caption>Succeeded and failed runs per interval</caption>
        <thead>
          <tr>
            <th scope="col">Interval</th>
            <th scope="col">Succeeded</th>
            <th scope="col">Failed</th>
            <th scope="col">Total</th>
          </tr>
        </thead>
        <tbody>
          {hasData ? (
            chart.categories.map((c, i) => (
              <tr key={c}>
                <th scope="row">{c}</th>
                <td>{chart.succeeded[i]}</td>
                <td>{chart.failed[i]}</td>
                <td>{chart.total[i]}</td>
              </tr>
            ))
          ) : (
            <tr>
              <td colSpan={4}>No data</td>
            </tr>
          )}
        </tbody>
      </table>
    </section>
  );
}

/**
 * Mission Control Overview vNext — the at-a-glance landing surface. Composes
 * the SLA alert banner, the 6-KPI strip (with week-over-window deltas), the
 * race-track hero, the status-distribution donut, and the success/failure
 * trend, each wired to the real `get_dashboard_*` bindings and each carrying a
 * hover/focus InfoTip + a visually-hidden accessible-table fallback. All colors
 * bind to tokens (via the chart primitives) and it renders in dark + light.
 */
export default function Overview({
  environmentFilter,
  lookbackParam,
  lookbackLabel,
  runningJobs,
  nowMs,
}: OverviewProps) {
  const { data, loading, error } = useDashboardOverview(
    environmentFilter,
    lookbackParam,
  );

  // Capture "now" in state — refreshed whenever the running jobs reload — rather
  // than reading the clock during render, so render stays pure (and the frozen
  // test clock keeps the race deterministic). An explicit `nowMs` overrides it.
  // The update is deferred to a macrotask to avoid a synchronous set-in-effect.
  const [clockNow, setClockNow] = useState<number | null>(nowMs ?? null);
  useEffect(() => {
    const id = setTimeout(() => setClockNow(nowMs ?? Date.now()), 0);
    return () => clearTimeout(id);
  }, [nowMs, runningJobs]);
  const effectiveNow = clockNow ?? 0;

  const view = useMemo(() => {
    if (!data) return null;
    const queueDepth = totalQueueDepth(data.queueHealth);
    const running = runningNow(data.executionSlots);
    return {
      cards: buildKpiCards(
        data.kpiSummary,
        data.kpiWow,
        queueDepth,
        running,
        lookbackLabel,
      ),
      race: buildRaceJobs(runningJobs, data.baselines, effectiveNow),
      segments: statusDonutSegments(data.statusDistribution),
      total: statusDistributionTotal(data.statusDistribution),
      trend: trendToChart(data.successFailTrend),
      sla: deriveSlaWarning(data.queueHealth),
    };
  }, [data, runningJobs, effectiveNow, lookbackLabel]);

  if (loading && !view) {
    return (
      <div className="mc-ov-status" role="status">
        Loading overview…
      </div>
    );
  }
  if (error && !view) {
    return (
      <div className="mc-ov-status mc-ov-status--error" role="alert">
        Overview failed to load: {error}
      </div>
    );
  }
  if (!view) return null;

  return (
    <div className="mc-overview" data-overview-ready="true">
      {error ? (
        <div className="mc-ov-status mc-ov-status--error" role="alert">
          Overview data may be stale: {error}
        </div>
      ) : null}
      {view.sla ? <SlaBanner warning={view.sla} /> : null}
      <KpiStrip cards={view.cards} />
      <RaceHero result={view.race} />
      <div className="mc-ov-charts">
        <StatusDistribution segments={view.segments} total={view.total} />
        <SuccessFailTrend chart={view.trend} />
      </div>
    </div>
  );
}
