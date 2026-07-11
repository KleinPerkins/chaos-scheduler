import { useId, useState } from "react";
import Gauge from "../charts/Gauge";
import QueueLine from "../charts/QueueLine";
import InfoTip from "../InfoTip";
import { ChartLegend, GroupMetric, ToneChip } from "./groupCard";
import {
  formatPercentFrac,
  queueHealthRows,
  resourcesSummary,
  slotGauges,
  utilizationChart,
  type SlotGauge,
  type UtilizationChart,
} from "./resourcesData";
import type { ResourcesState } from "./useResources";
import "./surfaces.css";

const GROUP_TITLE = "Resources";
const GROUP_INFO_DEF =
  "Queue-to-worker capacity: slot utilization over time, live execution slots, and per-queue health.";

const UTIL_INFO_DEF =
  "Queue slot occupancy per interval as a % of capacity: average and max, with the warn and degraded utilization thresholds.";

/** The utilization series colors + threshold treatment, shared by the summary
 * sparkline and the full chart. */
const AVG_COLOR = "var(--chart-2)";
const MAX_COLOR = "var(--chart-1)";

/**
 * Threshold-zone queue-utilization chart built on the shared `QueueLine`
 * primitive: average + max occupancy lines, a shaded warn band (≥ warn%) with a
 * dashed warn boundary, and a dashed degraded reference line — plus an sr-only
 * table carrying the exact per-interval percentages. All colors bind to tokens.
 */
function UtilizationTrend({
  chart,
  showAxes,
  height,
}: {
  chart: UtilizationChart;
  showAxes: boolean;
  height: number;
}) {
  const degradedLine = chart.categories.map(() => chart.degradedPct);
  return (
    <QueueLine
      categories={chart.categories}
      series={[
        { label: "Avg", data: chart.avgPct, color: AVG_COLOR },
        { label: "Max", data: chart.maxPct, color: MAX_COLOR },
        {
          label: `Degraded ≥${chart.degradedPct}%`,
          data: degradedLine,
          color: "var(--error)",
          dashed: true,
        },
      ]}
      capacity={chart.warnPct}
      capacityColor="var(--warning)"
      capacityLabel={`Warn ≥${chart.warnPct}%`}
      showAxes={showAxes}
      height={height}
      ariaLabel={`Queue slot utilization — average and max across ${chart.categories.length} intervals, against a ${chart.warnPct}% warn and ${chart.degradedPct}% degraded threshold`}
    />
  );
}

/** The sr-only per-interval table backing the utilization chart. */
function UtilizationTable({ chart }: { chart: UtilizationChart }) {
  return (
    <table className="sr-only">
      <caption>Queue slot utilization per interval</caption>
      <thead>
        <tr>
          <th scope="col">Interval</th>
          <th scope="col">Average</th>
          <th scope="col">Max</th>
        </tr>
      </thead>
      <tbody>
        {chart.categories.length === 0 ? (
          <tr>
            <td colSpan={3}>No data</td>
          </tr>
        ) : (
          chart.categories.map((c, i) => (
            <tr key={c}>
              <th scope="row">{c}</th>
              <td>{formatPercentFrac(chart.avgFrac[i])}</td>
              <td>{formatPercentFrac(chart.maxFrac[i])}</td>
            </tr>
          ))
        )}
      </tbody>
    </table>
  );
}

/** One execution-slot gas gauge with its label + running/capacity caption. */
function SlotGaugeCell({
  gauge,
  warnPct,
  degradedPct,
}: {
  gauge: SlotGauge;
  warnPct: number;
  degradedPct: number;
}) {
  return (
    <div className="mc-gauge">
      <Gauge
        value={gauge.running}
        max={gauge.capacity}
        size={132}
        unit="slots"
        thresholds={{ warning: warnPct, danger: degradedPct }}
        ariaLabel={`${gauge.label}${gauge.sublabel ? ` (${gauge.sublabel})` : ""}: ${gauge.utilizationPct}% utilized — ${gauge.running} of ${gauge.capacity} slots running`}
      />
      <span className="mc-gauge__label">{gauge.label}</span>
      {gauge.sublabel ? (
        <span className="mc-gauge__sub">{gauge.sublabel}</span>
      ) : null}
      <span className="mc-gauge__caption">
        {gauge.running}/{gauge.capacity} running · {gauge.available} free
      </span>
    </div>
  );
}

/**
 * At-a-glance "Resources" group card for the two-group IA. Summarizes global
 * slot utilization and queue health, offers an in-place medium-detail expansion
 * (a compact utilization sparkline), and a "View details" affordance that opens
 * the full drill-down subpage (return state preserved by Mission Control).
 */
export function ResourcesSummary({
  state,
  onViewDetails,
}: {
  state: ResourcesState;
  onViewDetails: () => void;
}) {
  const { data, loading, error } = state;
  const [expanded, setExpanded] = useState(false);
  const expandId = useId();

  const summary = data
    ? resourcesSummary(
        data.executionSlots,
        data.queueHealth,
        data.utilizationHistory,
      )
    : null;
  const chart = data ? utilizationChart(data.utilizationHistory) : null;

  return (
    <section
      className="mc-grp mc-grp--wide"
      aria-labelledby={`${expandId}-title`}
    >
      <div className="mc-grp__head">
        <h2 className="mc-grp__title" id={`${expandId}-title`}>
          {GROUP_TITLE}
        </h2>
        <InfoTip title="Resources" def={GROUP_INFO_DEF} />
        {summary ? <ToneChip tone={summary.tone} /> : null}
      </div>

      {loading && !summary ? (
        <p className="mc-grp__headline" role="status">
          Loading resource health…
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
              value={`${summary.globalUtilizationPct}%`}
              label="Slots used"
            />
            <GroupMetric
              value={`${summary.running}/${summary.capacity}`}
              label="Running / capacity"
            />
            <GroupMetric
              value={summary.degradedQueues.toLocaleString()}
              label="Degraded queues"
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
            {expanded ? "Hide utilization" : "Show utilization"}
          </button>

          {expanded && chart ? (
            <div className="mc-grp__expand" id={expandId}>
              <span className="mc-grp__expand-title">
                Queue utilization over window
              </span>
              {chart.hasData ? (
                <>
                  <ChartLegend
                    items={[
                      { label: "Avg", color: AVG_COLOR },
                      { label: "Max", color: MAX_COLOR },
                      {
                        label: `Degraded ≥${chart.degradedPct}%`,
                        color: "var(--error)",
                        dashed: true,
                      },
                    ]}
                  />
                  <UtilizationTrend
                    chart={chart}
                    showAxes={false}
                    height={90}
                  />
                </>
              ) : (
                <p className="mc-drill__empty">
                  No queue-occupancy samples in the selected window.
                </p>
              )}
            </div>
          ) : null}

          <div className="mc-grp__actions">
            <button
              type="button"
              className="mc-grp__btn mc-grp__btn--primary"
              aria-label="View Resources details"
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
 * Full-detail Resources drill-down subpage (F05). Composes the threshold-zone
 * queue-utilization history chart, the execution-slot gas gauges (global +
 * per-queue), and the per-queue health table — each from a real binding, with
 * loading/empty/error states and a keyboard-operable back affordance. Colors
 * bind to tokens; renders dark + light.
 */
export function ResourcesPage({
  state,
  onBack,
}: {
  state: ResourcesState;
  onBack: () => void;
}) {
  const { data, loading, error } = state;

  const summary = data
    ? resourcesSummary(
        data.executionSlots,
        data.queueHealth,
        data.utilizationHistory,
      )
    : null;
  const chart = data ? utilizationChart(data.utilizationHistory) : null;
  const gauges = data ? slotGauges(data.executionSlots) : null;
  const rows = data ? queueHealthRows(data.queueHealth) : [];

  return (
    <div
      className="mc-drill"
      data-drill="resources"
      data-drill-ready={data ? "true" : undefined}
    >
      <button type="button" className="mc-drill__back" onClick={onBack}>
        ← Back to overview
      </button>
      <header className="mc-drill__header">
        <div className="mc-drill__title-row">
          {/* The drill-down replaces the overview landing, so its title is the
              page's h1; the card sections below are h2. */}
          <h1 className="mc-drill__title">Resources</h1>
          {summary ? <ToneChip tone={summary.tone} /> : null}
        </div>
        {summary ? (
          <p className="mc-drill__headline">{summary.headline}</p>
        ) : null}
      </header>

      {loading && !data ? (
        <div className="mc-drill__status" role="status">
          Loading resource detail…
        </div>
      ) : error && !data ? (
        <div className="mc-drill__status mc-drill__status--error" role="alert">
          Failed to load: {error}
        </div>
      ) : data && summary && chart && gauges ? (
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
            {/* Queue utilization history — threshold-zone chart */}
            <section
              className="mc-drill-card mc-drill-card--wide"
              aria-labelledby="mc-res-util"
            >
              <div className="mc-drill-card__head">
                <h2 id="mc-res-util">Queue utilization</h2>
                <InfoTip title="Queue utilization" def={UTIL_INFO_DEF} />
                <ChartLegend
                  items={[
                    { label: "Avg", color: AVG_COLOR },
                    { label: "Max", color: MAX_COLOR },
                    {
                      label: `Warn ≥${chart.warnPct}%`,
                      color: "var(--warning)",
                      dashed: true,
                    },
                    {
                      label: `Degraded ≥${chart.degradedPct}%`,
                      color: "var(--error)",
                      dashed: true,
                    },
                  ]}
                />
              </div>
              {chart.hasData ? (
                <UtilizationTrend chart={chart} showAxes height={170} />
              ) : (
                <p className="mc-drill__empty">
                  No queue-occupancy samples in the selected window.
                </p>
              )}
              <UtilizationTable chart={chart} />
            </section>

            {/* Execution slots — gas gauges */}
            <section className="mc-drill-card" aria-labelledby="mc-res-slots">
              <div className="mc-drill-card__head">
                <h2 id="mc-res-slots">Execution slots</h2>
                <InfoTip
                  title="Execution slots"
                  def="Live running runs vs configured concurrency capacity, per queue and scheduler-wide."
                />
                <span className="mc-drill-card__sub">
                  {summary.globalUtilizationPct}% used
                </span>
              </div>
              <div className="mc-gauge-grid">
                <SlotGaugeCell
                  gauge={gauges.global}
                  warnPct={chart.warnPct}
                  degradedPct={chart.degradedPct}
                />
                {gauges.queues.map((g) => (
                  <SlotGaugeCell
                    key={g.key}
                    gauge={g}
                    warnPct={chart.warnPct}
                    degradedPct={chart.degradedPct}
                  />
                ))}
              </div>
            </section>

            {/* Queue health table */}
            <section className="mc-drill-card" aria-labelledby="mc-res-health">
              <div className="mc-drill-card__head">
                <h2 id="mc-res-health">Queue health</h2>
                <InfoTip
                  title="Queue health"
                  def="Each queue's live capacity, active + queued counts, and utilization, classified healthy / warn / degraded."
                />
                <span className="mc-drill-card__sub">
                  {summary.degradedQueues} degraded · {summary.warnQueues} warn
                </span>
              </div>
              {rows.length > 0 ? (
                <table className="mc-data-table">
                  <thead>
                    <tr>
                      <th scope="col">Queue</th>
                      <th scope="col">Environment</th>
                      <th scope="col" className="mc-num">
                        Active
                      </th>
                      <th scope="col" className="mc-num">
                        Queued
                      </th>
                      <th scope="col" className="mc-num">
                        Utilization
                      </th>
                      <th scope="col">Status</th>
                    </tr>
                  </thead>
                  <tbody>
                    {rows.map((row) => (
                      <tr key={row.key}>
                        <th scope="row">{row.name}</th>
                        <td>
                          <span className="mc-env-tag">{row.environment}</span>
                        </td>
                        <td className="mc-num">
                          {row.active.toLocaleString()}
                        </td>
                        <td className="mc-num">
                          {row.queued.toLocaleString()}
                        </td>
                        <td className="mc-num">{row.utilizationPct}%</td>
                        <td>
                          <span className={`mc-status mc-status--${row.tone}`}>
                            {row.status}
                          </span>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              ) : (
                <p className="mc-drill__empty">
                  No queues match the selected environment.
                </p>
              )}
            </section>
          </div>
        </>
      ) : null}
    </div>
  );
}
