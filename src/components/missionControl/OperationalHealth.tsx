import { useId, useState } from "react";
import DualAxisLine from "../charts/DualAxisLine";
import InfoTip from "../InfoTip";
import { GroupMetric, ToneChip } from "./groupCard";
import { trendToChart } from "../overview/overviewData";
import {
  aggregateStats,
  formatPerHour,
  formatPercent1,
  formatSeconds,
  metricTrendToChart,
  operationalHealthSummary,
  toMinutes,
  type MetricTrendChart,
} from "./operationalHealthData";
import type { OperationalHealthState } from "./useOperationalHealth";
import "./surfaces.css";

const GROUP_TITLE = "Operational Health";
const GROUP_INFO_DEF =
  "Success rate, throughput, and wait/runtime duration trends over the selected window.";

/** A small solid/dashed swatch legend row for a trend chart. */
function ChartLegend({
  items,
}: {
  items: { label: string; color: string; dashed?: boolean }[];
}) {
  return (
    <ul className="mc-chart-legend" aria-hidden="true">
      {items.map((it) => (
        <li key={it.label} className="mc-chart-legend__item">
          <span
            className={`mc-chart-legend__swatch${it.dashed ? " mc-chart-legend__swatch--dashed" : ""}`}
            style={{
              background: it.dashed ? "transparent" : it.color,
              borderColor: it.color,
            }}
          />
          <span className="mc-chart-legend__label">{it.label}</span>
        </li>
      ))}
    </ul>
  );
}

/**
 * One wait/runtime dual-axis duration card: the average (left scale, solid) and
 * a dashed 30-day baseline on the same left scale, with the max on an
 * independent right scale — plotted in minutes for readable axes, with an
 * sr-only table carrying the exact durations. Colors bind to tokens.
 */
function DurationTrendCard({
  id,
  title,
  infoDef,
  chart,
  avgColor,
  maxColor,
}: {
  id: string;
  title: string;
  infoDef: string;
  chart: MetricTrendChart;
  avgColor: string;
  maxColor: string;
}) {
  const baselineColor = "var(--text-muted)";
  return (
    <section className="mc-drill-card" aria-labelledby={id}>
      <div className="mc-drill-card__head">
        <h2 id={id}>{title}</h2>
        <InfoTip title={title} def={infoDef} />
      </div>
      {chart.hasData ? (
        <>
          <ChartLegend
            items={[
              { label: "Avg", color: avgColor },
              { label: "30d baseline", color: baselineColor, dashed: true },
              { label: "Max", color: maxColor },
            ]}
          />
          <DualAxisLine
            categories={chart.categories}
            leftSeries={[
              {
                label: "Avg (min)",
                data: toMinutes(chart.avgSeconds),
                color: avgColor,
              },
              {
                label: "30d baseline (min)",
                data: toMinutes(chart.baselineSeconds),
                color: baselineColor,
                dashed: true,
              },
            ]}
            rightSeries={[
              {
                label: "Max (min)",
                data: toMinutes(chart.maxSeconds),
                color: maxColor,
              },
            ]}
            showAxes
            height={150}
            ariaLabel={`${title}: average and max in minutes across ${chart.categories.length} intervals (left axis average, right axis max)`}
          />
        </>
      ) : (
        <p className="mc-drill__empty">No samples in the selected window.</p>
      )}
      <table className="sr-only">
        <caption>{title} per interval</caption>
        <thead>
          <tr>
            <th scope="col">Interval</th>
            <th scope="col">Average</th>
            <th scope="col">Max</th>
            <th scope="col">30-day baseline</th>
          </tr>
        </thead>
        <tbody>
          {chart.categories.length === 0 ? (
            <tr>
              <td colSpan={4}>No data</td>
            </tr>
          ) : (
            chart.categories.map((c, i) => (
              <tr key={c}>
                <th scope="row">{c}</th>
                <td>{formatSeconds(chart.avgSeconds[i])}</td>
                <td>{formatSeconds(chart.maxSeconds[i])}</td>
                <td>{formatSeconds(chart.baselineSeconds[i])}</td>
              </tr>
            ))
          )}
        </tbody>
      </table>
    </section>
  );
}

/** Success/failure trend card (succeeded + failed counts on one scale). */
function SuccessFailCard({
  chart,
}: {
  chart: ReturnType<typeof trendToChart>;
}) {
  const hasData = chart.categories.length > 0;
  return (
    <section
      className="mc-drill-card mc-drill-card--wide"
      aria-labelledby="mc-oh-sf"
    >
      <div className="mc-drill-card__head">
        <h2 id="mc-oh-sf">Success / failure trend</h2>
        <InfoTip
          title="Success / failure trend"
          def="Succeeded vs failed runs per interval across the selected window."
        />
        <ChartLegend
          items={[
            { label: "Succeeded", color: "var(--success)" },
            { label: "Failed", color: "var(--error)" },
          ]}
        />
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
        <p className="mc-drill__empty">No runs in the selected window.</p>
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
 * At-a-glance "Operational Health" group card for the two-group IA. Summarizes
 * the success rate, throughput and runtime, offers an in-place medium-detail
 * expansion (the success/failure trend), and a "View details" affordance that
 * opens the full drill-down subpage (return state preserved by Mission Control).
 */
export function OperationalHealthSummary({
  state,
  onViewDetails,
}: {
  state: OperationalHealthState;
  onViewDetails: () => void;
}) {
  const { data, loading, error } = state;
  const [expanded, setExpanded] = useState(false);
  const expandId = useId();

  const summary = data
    ? operationalHealthSummary(data.kpiSummary, data.waitRuntimeTrend)
    : null;
  const trend = data ? trendToChart(data.successFailTrend) : null;

  return (
    <section className="mc-grp" aria-labelledby={`${expandId}-title`}>
      <div className="mc-grp__head">
        <h2 className="mc-grp__title" id={`${expandId}-title`}>
          {GROUP_TITLE}
        </h2>
        <InfoTip title="Operational Health" def={GROUP_INFO_DEF} />
        {summary ? <ToneChip tone={summary.tone} /> : null}
      </div>

      {loading && !summary ? (
        <p className="mc-grp__headline" role="status">
          Loading operational health…
        </p>
      ) : error && !summary ? (
        <p className="mc-grp__headline" role="alert">
          Failed to load: {error}
        </p>
      ) : summary ? (
        <>
          <p className="mc-grp__headline">{summary.headline}</p>
          <div className="mc-grp__metrics">
            <GroupMetric
              value={formatPercent1(summary.successRate)}
              label="Success rate"
            />
            <GroupMetric
              value={formatPerHour(summary.throughputPerHour)}
              label="Throughput"
            />
            <GroupMetric
              value={formatSeconds(summary.avgRuntimeSeconds)}
              label="Avg runtime"
            />
          </div>

          <button
            type="button"
            className="mc-grp__btn"
            aria-expanded={expanded}
            aria-controls={expandId}
            onClick={() => setExpanded((v) => !v)}
            style={{ alignSelf: "flex-start" }}
          >
            {expanded ? "Hide trend" : "Show trend"}
          </button>

          {expanded && trend ? (
            <div className="mc-grp__expand" id={expandId}>
              <span className="mc-grp__expand-title">
                Success / failure trend
              </span>
              {trend.categories.length > 0 ? (
                <DualAxisLine
                  categories={trend.categories}
                  leftSeries={[
                    {
                      label: "Succeeded",
                      data: trend.succeeded,
                      color: "var(--success)",
                    },
                    {
                      label: "Failed",
                      data: trend.failed,
                      color: "var(--error)",
                    },
                  ]}
                  showAxes
                  height={130}
                  ariaLabel={`Succeeded and failed runs across ${trend.categories.length} intervals`}
                />
              ) : (
                <p className="mc-drill__empty">
                  No runs in the selected window.
                </p>
              )}
            </div>
          ) : null}

          <div className="mc-grp__actions">
            <button
              type="button"
              className="mc-grp__btn mc-grp__btn--primary"
              aria-label="View Operational Health details"
              onClick={onViewDetails}
            >
              View details →
            </button>
          </div>
        </>
      ) : null}
    </section>
  );
}

/**
 * Full-detail Operational Health drill-down subpage (F03). Composes the
 * aggregate KPI rollup, the success/failure trend, and the wait + runtime
 * dual-axis duration trends (avg + max with a 30-day baseline) — each from a
 * real binding, with loading/empty/error states and a keyboard-operable back
 * affordance. Colors bind to tokens; renders dark + light.
 */
export function OperationalHealthPage({
  state,
  onBack,
}: {
  state: OperationalHealthState;
  onBack: () => void;
}) {
  const { data, loading, error } = state;

  const summary = data
    ? operationalHealthSummary(data.kpiSummary, data.waitRuntimeTrend)
    : null;
  const stats = data ? aggregateStats(data.kpiSummary) : [];
  const sfTrend = data ? trendToChart(data.successFailTrend) : null;
  const waitChart = data
    ? metricTrendToChart(
        data.waitRuntimeTrend.wait,
        data.waitRuntimeTrend.grain,
      )
    : null;
  const runtimeChart = data
    ? metricTrendToChart(
        data.waitRuntimeTrend.runtime,
        data.waitRuntimeTrend.grain,
      )
    : null;

  return (
    <div
      className="mc-drill"
      data-drill="operational-health"
      data-drill-ready={data ? "true" : undefined}
    >
      <button type="button" className="mc-drill__back" onClick={onBack}>
        ← Back to overview
      </button>
      <header className="mc-drill__header">
        <div className="mc-drill__title-row">
          {/* The drill-down replaces the overview landing, so its title is the
              page's h1; the card sections below are h2. */}
          <h1 className="mc-drill__title">Operational Health</h1>
          {summary ? <ToneChip tone={summary.tone} /> : null}
        </div>
        {summary ? (
          <p className="mc-drill__headline">{summary.headline}</p>
        ) : null}
      </header>

      {loading && !data ? (
        <div className="mc-drill__status" role="status">
          Loading operational health detail…
        </div>
      ) : error && !data ? (
        <div className="mc-drill__status mc-drill__status--error" role="alert">
          Failed to load: {error}
        </div>
      ) : data && summary && sfTrend && waitChart && runtimeChart ? (
        <>
          {error ? (
            <div
              className="mc-drill__status mc-drill__status--error"
              role="alert"
            >
              Data may be stale: {error}
            </div>
          ) : null}
          <div className="mc-drill__grid">
            {/* Aggregate KPIs */}
            <section
              className="mc-drill-card mc-drill-card--wide"
              aria-labelledby="mc-oh-kpis"
            >
              <div className="mc-drill-card__head">
                <h2 id="mc-oh-kpis">Aggregate KPIs</h2>
                <InfoTip
                  title="Aggregate KPIs"
                  def="Windowed rollup of runs, success rate, throughput, runtime and admission wait."
                />
              </div>
              <div className="mc-grp__metrics">
                {stats.map((s) => (
                  <GroupMetric key={s.key} value={s.value} label={s.label} />
                ))}
              </div>
            </section>

            {/* Success / failure trend */}
            <SuccessFailCard chart={sfTrend} />

            {/* Wait duration trend */}
            <DurationTrendCard
              id="mc-oh-wait"
              title="Wait"
              infoDef="Admission wait per interval in minutes: average (left) and max (right), with the 30-day trailing-average baseline."
              chart={waitChart}
              avgColor="var(--chart-2)"
              maxColor="var(--warning)"
            />

            {/* Runtime duration trend */}
            <DurationTrendCard
              id="mc-oh-runtime"
              title="Runtime"
              infoDef="End-to-end runtime per interval in minutes: average (left) and max (right), with the 30-day trailing-average baseline."
              chart={runtimeChart}
              avgColor="var(--chart-1)"
              maxColor="var(--warning)"
            />
          </div>
        </>
      ) : null}
    </div>
  );
}
