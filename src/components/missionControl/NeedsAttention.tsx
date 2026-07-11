import { useId, useState } from "react";
import ImpactBars from "../charts/ImpactBars";
import InfoTip from "../InfoTip";
import { formatDuration } from "../../lib/duration";
import {
  blastRadiusBars,
  blockReasonBars,
  failureRows,
  formatFailureRate,
  heavyBlockerBars,
  needsAttentionSummary,
  type AttentionTone,
} from "./needsAttentionData";
import type { NeedsAttentionState } from "./useNeedsAttention";
import "./surfaces.css";

const GROUP_TITLE = "Critical / Needs Attention";
const GROUP_INFO_DEF =
  "Blocked/waiting work, heavy blockers, long-running blast radius, and recent failures over the selected window.";

function ToneChip({ tone }: { tone: AttentionTone }) {
  const label =
    tone === "critical" ? "Critical" : tone === "warn" ? "Warning" : "Clear";
  return <span className={`mc-grp__tone mc-grp__tone--${tone}`}>{label}</span>;
}

function Metric({ value, label }: { value: string; label: string }) {
  return (
    <div className="mc-grp__metric">
      <span className="mc-grp__metric-value">{value}</span>
      <span className="mc-grp__metric-label">{label}</span>
    </div>
  );
}

/**
 * At-a-glance "Critical / Needs Attention" group card for the two-group IA. It
 * summarizes the blocked/waiting, heavy-blocker, blast-radius and failure
 * signals, offers an in-place medium-detail expansion (the reason taxonomy +
 * heaviest blocker), and a "View details" affordance that opens the full
 * drill-down subpage (return state preserved by Mission Control).
 */
export function NeedsAttentionSummary({
  state,
  onViewDetails,
}: {
  state: NeedsAttentionState;
  onViewDetails: () => void;
}) {
  const { data, loading, error } = state;
  const [expanded, setExpanded] = useState(false);
  const expandId = useId();

  const summary = data
    ? needsAttentionSummary(
        data.taxonomy,
        data.blastRadius,
        data.failureRecurrence,
      )
    : null;
  const reasonBars = data ? blockReasonBars(data.taxonomy.by_reason) : [];

  return (
    <section className="mc-grp" aria-labelledby={`${expandId}-title`}>
      <div className="mc-grp__head">
        <h2 className="mc-grp__title" id={`${expandId}-title`}>
          {GROUP_TITLE}
        </h2>
        <InfoTip title="Needs Attention" def={GROUP_INFO_DEF} />
        {summary ? <ToneChip tone={summary.tone} /> : null}
      </div>

      {loading && !summary ? (
        <p className="mc-grp__headline" role="status">
          Loading needs-attention signals…
        </p>
      ) : error && !summary ? (
        <p className="mc-grp__headline" role="alert">
          Failed to load: {error}
        </p>
      ) : summary ? (
        <>
          <p className="mc-grp__headline">{summary.headline}</p>
          <div className="mc-grp__metrics">
            <Metric
              value={summary.blockedCount.toLocaleString()}
              label="Waiting now"
            />
            <Metric
              value={summary.totalFailures.toLocaleString()}
              label={`Failures · ${summary.failingWorkflowCount} wf`}
            />
            <Metric
              value={
                summary.topBlastRadius
                  ? summary.topBlastRadius.downstream.toLocaleString()
                  : "0"
              }
              label="Widest blast radius"
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
            {expanded ? "Hide breakdown" : "Show breakdown"}
          </button>

          {expanded ? (
            <div className="mc-grp__expand" id={expandId}>
              <span className="mc-grp__expand-title">
                Blocked / waiting by reason
              </span>
              {reasonBars.length > 0 ? (
                <ImpactBars
                  items={reasonBars}
                  ariaLabel="Blocked and waiting jobs by reason category"
                />
              ) : (
                <p className="mc-drill__empty">
                  Nothing is blocked or waiting right now.
                </p>
              )}
              {summary.heaviestBlocker ? (
                <p className="mc-grp__headline">
                  Heaviest blocker:{" "}
                  <strong>{summary.heaviestBlocker.name}</strong> holding{" "}
                  {formatDuration(
                    summary.heaviestBlocker.sigmaWaitSeconds * 1000,
                  )}
                  .
                </p>
              ) : null}
            </div>
          ) : null}

          <div className="mc-grp__actions">
            <button
              type="button"
              className="mc-grp__btn mc-grp__btn--primary"
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

function StatCell({ value, label }: { value: string; label: string }) {
  return (
    <div className="mc-grp__metric">
      <span className="mc-grp__metric-value">{value}</span>
      <span className="mc-grp__metric-label">{label}</span>
    </div>
  );
}

/**
 * Full-detail Needs Attention drill-down subpage (F04). Composes the
 * blocked/waiting reason taxonomy + heavy-blocker impact bars, the long-running
 * outlier + blast-radius bars, and the recent-failure table — each from a real
 * binding, with loading/empty/error states and a keyboard-operable back
 * affordance. Colors bind to tokens; renders dark + light.
 */
export function NeedsAttentionPage({
  state,
  onBack,
  onOpenQueues,
  onOpenHistory,
}: {
  state: NeedsAttentionState;
  onBack: () => void;
  onOpenQueues: () => void;
  onOpenHistory: (workflowId: string) => void;
}) {
  const { data, loading, error } = state;

  const summary = data
    ? needsAttentionSummary(
        data.taxonomy,
        data.blastRadius,
        data.failureRecurrence,
      )
    : null;
  const reasonBars = data ? blockReasonBars(data.taxonomy.by_reason) : [];
  const heavyBars = data ? heavyBlockerBars(data.taxonomy.heavy_blockers) : [];
  const blastBars = data ? blastRadiusBars(data.blastRadius) : [];
  const failures = data ? failureRows(data.failureRecurrence) : [];

  return (
    <div
      className="mc-drill"
      data-drill="needs-attention"
      data-drill-ready={data ? "true" : undefined}
    >
      <button type="button" className="mc-drill__back" onClick={onBack}>
        ← Back to overview
      </button>
      <header className="mc-drill__header">
        <div className="mc-drill__title-row">
          {/* The drill-down replaces the overview landing view, so its title is
              the page's h1 (HeaderStatus's h1 is not rendered in this state);
              the card sections below are h2 so the outline increases by one. */}
          <h1 className="mc-drill__title">Needs Attention</h1>
          {summary ? <ToneChip tone={summary.tone} /> : null}
        </div>
        {summary ? (
          <p className="mc-drill__headline">{summary.headline}</p>
        ) : null}
      </header>

      {loading && !data ? (
        <div className="mc-drill__status" role="status">
          Loading needs-attention detail…
        </div>
      ) : error && !data ? (
        <div className="mc-drill__status mc-drill__status--error" role="alert">
          Failed to load: {error}
        </div>
      ) : data && summary ? (
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
            {/* Blocked / waiting */}
            <section
              className="mc-drill-card mc-drill-card--wide"
              aria-labelledby="mc-na-blocked"
            >
              <div className="mc-drill-card__head">
                <h2 id="mc-na-blocked">Blocked &amp; waiting</h2>
                <InfoTip
                  title="Blocked & waiting"
                  def="Currently-queued runs classified by wait reason, with the Σ admission wait each reason is holding."
                />
                <span className="mc-drill-card__sub">
                  {summary.blockedCount.toLocaleString()} waiting now
                </span>
              </div>
              <div className="mc-grp__metrics">
                <StatCell
                  value={summary.blockedCount.toLocaleString()}
                  label="Jobs waiting"
                />
                <StatCell
                  value={formatDuration(summary.blockedWaitTotalSeconds * 1000)}
                  label="Σ current wait"
                />
                <StatCell
                  value={formatDuration(summary.blockedWaitMaxSeconds * 1000)}
                  label="Longest wait"
                />
                <StatCell
                  value={
                    data.taxonomy.trailing_wait_seconds_avg == null
                      ? "—"
                      : formatDuration(
                          data.taxonomy.trailing_wait_seconds_avg * 1000,
                        )
                  }
                  label="Trailing avg wait"
                />
              </div>
              {reasonBars.length > 0 ? (
                <ImpactBars
                  items={reasonBars}
                  ariaLabel="Blocked and waiting jobs by reason category"
                />
              ) : (
                <p className="mc-drill__empty">
                  Nothing is blocked or waiting right now.
                </p>
              )}
              <div className="mc-grp__actions">
                <button
                  type="button"
                  className="mc-grp__btn"
                  onClick={onOpenQueues}
                >
                  View queues →
                </button>
              </div>
            </section>

            {/* Heavy blockers */}
            <section className="mc-drill-card" aria-labelledby="mc-na-heavy">
              <div className="mc-drill-card__head">
                <h2 id="mc-na-heavy">Heavy blockers</h2>
                <InfoTip
                  title="Heavy blockers"
                  def="Workflows holding the most Σ admission wait right now, ranked by total wait held (and jobs blocked)."
                />
              </div>
              {heavyBars.length > 0 ? (
                <ImpactBars
                  items={heavyBars}
                  ariaLabel="Heaviest blocking workflows by total wait held"
                />
              ) : (
                <p className="mc-drill__empty">
                  No heavy blockers in this window.
                </p>
              )}
            </section>

            {/* Long-running outliers + blast radius */}
            <section className="mc-drill-card" aria-labelledby="mc-na-blast">
              <div className="mc-drill-card__head">
                <h2 id="mc-na-blast">Outliers &amp; blast radius</h2>
                <InfoTip
                  title="Blast radius"
                  def="Workflows whose runs reach the most downstream dependents, ranked by downstream count with the longest chain depth."
                />
              </div>
              {blastBars.length > 0 ? (
                <ImpactBars
                  items={blastBars}
                  ariaLabel="Workflows by downstream blast radius"
                />
              ) : (
                <p className="mc-drill__empty">
                  No downstream dependents in this window.
                </p>
              )}
            </section>

            {/* Recent failures */}
            <section
              className="mc-drill-card mc-drill-card--wide"
              aria-labelledby="mc-na-failures"
            >
              <div className="mc-drill-card__head">
                <h2 id="mc-na-failures">Recent failures</h2>
                <InfoTip
                  title="Recent failures"
                  def="Workflows with at least one failed run in the window, worst first. Select a row to open its history."
                />
                <span className="mc-drill-card__sub">
                  {summary.totalFailures.toLocaleString()} failures ·{" "}
                  {summary.failingWorkflowCount} workflows
                </span>
              </div>
              {failures.length > 0 ? (
                <table className="mc-data-table">
                  <thead>
                    <tr>
                      <th scope="col">Workflow</th>
                      <th scope="col">Environment</th>
                      <th scope="col" className="mc-num">
                        Failures
                      </th>
                      <th scope="col" className="mc-num">
                        Runs
                      </th>
                      <th scope="col" className="mc-num">
                        Fail rate
                      </th>
                    </tr>
                  </thead>
                  <tbody>
                    {failures.map((row) => (
                      <tr key={row.workflowId}>
                        <th scope="row">
                          <button
                            type="button"
                            className="mc-linkish"
                            onClick={() => onOpenHistory(row.workflowId)}
                          >
                            {row.workflowName}
                          </button>
                        </th>
                        <td>
                          <span className="mc-env-tag">{row.environment}</span>
                        </td>
                        <td className="mc-num">
                          {row.failureCount.toLocaleString()}
                        </td>
                        <td className="mc-num">
                          {row.totalRuns.toLocaleString()}
                        </td>
                        <td className="mc-num">
                          {formatFailureRate(row.failureRate)}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              ) : (
                <p className="mc-drill__empty">
                  No failures in the selected window.
                </p>
              )}
            </section>
          </div>
        </>
      ) : null}
    </div>
  );
}
