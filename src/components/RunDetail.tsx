import {
  useState,
  useEffect,
  useCallback,
  useRef,
  type KeyboardEvent as ReactKeyboardEvent,
} from "react";
import {
  getRunAttempts,
  getRunLog,
  getRunMetrics,
  getRunRelationships,
  getRunTasks,
  analyzeRunError,
} from "../lib/commands";
import { openExternalSafe } from "../lib/openExternalSafe";
import { formatDurationBetween } from "../lib/duration";
import Button from "./Button";
import PageHeader from "./PageHeader";
import StatCard from "./StatCard";
import StatusBadge from "./StatusBadge";
import StatusDot from "./StatusDot";
import type {
  ErrorAnalysis,
  Run,
  RunAttempt,
  RunMetric,
  RunRelationship,
  RunTask,
} from "../lib/commands";
import { isActiveRunStatus, nextPollDelayMs } from "../lib/runPolling";
import { formatRunStatusLabel } from "../lib/runStatus";
import "./RunDetail.css";

interface Props {
  runId: string;
  onBack: () => void;
}

interface SummarySection {
  title: string;
  type: "stats" | "items" | "links" | "phases" | "text" | "table";
  data: unknown;
}

interface RunSummary {
  title?: string;
  description?: string;
  sections?: SummarySection[];
  [key: string]: unknown;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function stringifyUnknown(value: unknown): string {
  if (typeof value === "string") return value;
  return JSON.stringify(value, null, 2);
}

function asStatsData(value: unknown): Record<string, number | string> | null {
  if (!isRecord(value)) return null;
  const entries = Object.entries(value).filter(
    ([, v]) => typeof v === "number" || typeof v === "string",
  );
  return Object.fromEntries(entries) as Record<string, number | string>;
}

function asItemList(value: unknown): Array<{
  name: string;
  detail?: string;
  url?: string;
  badge?: string;
}> | null {
  if (!Array.isArray(value)) return null;
  const items = value.filter(isRecord).map((item) => ({
    name: typeof item.name === "string" ? item.name : "Untitled",
    detail: typeof item.detail === "string" ? item.detail : undefined,
    url: typeof item.url === "string" ? item.url : undefined,
    badge: typeof item.badge === "string" ? item.badge : undefined,
  }));
  return items;
}

function asLinkList(
  value: unknown,
): Array<{ label: string; url: string }> | null {
  if (!Array.isArray(value)) return null;
  const links = value
    .filter(isRecord)
    .filter(
      (item) => typeof item.label === "string" && typeof item.url === "string",
    )
    .map((item) => ({ label: item.label as string, url: item.url as string }));
  return links;
}

function asPhaseList(
  value: unknown,
): Array<{ name: string; status: string; duration?: string }> | null {
  if (!Array.isArray(value)) return null;
  return value.filter(isRecord).map((phase) => ({
    name: typeof phase.name === "string" ? phase.name : "Unnamed phase",
    status: typeof phase.status === "string" ? phase.status : "unknown",
    duration: typeof phase.duration === "string" ? phase.duration : undefined,
  }));
}

function asTableData(
  value: unknown,
): { headers: string[]; rows: string[][] } | null {
  if (
    !isRecord(value) ||
    !Array.isArray(value.headers) ||
    !Array.isArray(value.rows)
  )
    return null;
  const headers = value.headers.map((header) => String(header));
  const rows = value.rows
    .filter(Array.isArray)
    .map((row) => row.map((cell) => String(cell)));
  return { headers, rows };
}

function isSummarySection(value: unknown): value is SummarySection {
  return isRecord(value) && typeof value.type === "string";
}

function formatDuration(start: string, end: string | null): string {
  if (!end) return "running...";
  return formatDurationBetween(start, end);
}

function formatDate(iso: string): string {
  return new Date(iso).toLocaleString([], {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    timeZoneName: "short",
  });
}

function taskTimelineWidth(task: RunTask, run: Run): string {
  if (!task.started_at || !task.finished_at || !run.finished_at) return "100%";
  const runDuration = Date.parse(run.finished_at) - Date.parse(run.started_at);
  const taskDuration =
    Date.parse(task.finished_at) - Date.parse(task.started_at);
  if (
    !Number.isFinite(runDuration) ||
    !Number.isFinite(taskDuration) ||
    runDuration <= 0 ||
    taskDuration < 0
  ) {
    return "100%";
  }
  const share = Math.max(12, Math.min(100, (taskDuration / runDuration) * 100));
  return `${Math.round(share)}%`;
}

function StatsGrid({ data }: { data: Record<string, number | string> }) {
  const entries = Object.entries(data);
  return (
    <div className="rd-stats-grid">
      {entries.map(([label, value]) => (
        <StatCard
          key={label}
          variant="rd"
          value={String(value)}
          label={label}
        />
      ))}
    </div>
  );
}

function ItemList({
  data,
}: {
  data: Array<{ name: string; detail?: string; url?: string; badge?: string }>;
}) {
  return (
    <div className="rd-item-list">
      {data.map((item, i) => (
        <div key={i} className="rd-item">
          <div className="rd-item-main">
            <span className="rd-item-name">{item.name}</span>
            {item.badge && <span className="rd-item-badge">{item.badge}</span>}
          </div>
          {item.detail && <div className="rd-item-detail">{item.detail}</div>}
          {item.url && (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => openExternalSafe(item.url!)}
            >
              Open
            </Button>
          )}
        </div>
      ))}
    </div>
  );
}

function LinkList({ data }: { data: Array<{ label: string; url: string }> }) {
  return (
    <div className="rd-link-list">
      {data.map((link, i) => (
        <button
          key={i}
          className="rd-link-button"
          onClick={() => openExternalSafe(link.url)}
        >
          {link.label}
        </button>
      ))}
    </div>
  );
}

function PhaseTimeline({
  data,
}: {
  data: Array<{ name: string; status: string; duration?: string }>;
}) {
  return (
    <div className="rd-phases">
      {data.map((phase, i) => (
        <div key={i} className="rd-phase">
          <span className={`rd-phase-dot ${phase.status}`} />
          <span className="rd-phase-name">{phase.name}</span>
          {phase.duration && (
            <span className="rd-phase-duration">{phase.duration}</span>
          )}
        </div>
      ))}
    </div>
  );
}

function TextBlock({ data }: { data: string }) {
  return <div className="rd-text-block">{data}</div>;
}

function TableView({
  data,
}: {
  data: { headers: string[]; rows: string[][] };
}) {
  return (
    <table className="rd-table">
      <thead>
        <tr>
          {data.headers.map((h, i) => (
            <th key={i}>{h}</th>
          ))}
        </tr>
      </thead>
      <tbody>
        {data.rows.map((row, i) => (
          <tr key={i}>
            {row.map((cell, j) => (
              <td key={j}>{cell}</td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function SectionRenderer({ section }: { section: SummarySection }) {
  switch (section.type) {
    case "stats": {
      const data = asStatsData(section.data);
      return data ? (
        <StatsGrid data={data} />
      ) : (
        <pre className="rd-raw">{stringifyUnknown(section.data)}</pre>
      );
    }
    case "items": {
      const data = asItemList(section.data);
      return data ? (
        <ItemList data={data} />
      ) : (
        <pre className="rd-raw">{stringifyUnknown(section.data)}</pre>
      );
    }
    case "links": {
      const data = asLinkList(section.data);
      return data ? (
        <LinkList data={data} />
      ) : (
        <pre className="rd-raw">{stringifyUnknown(section.data)}</pre>
      );
    }
    case "phases": {
      const data = asPhaseList(section.data);
      return data ? (
        <PhaseTimeline data={data} />
      ) : (
        <pre className="rd-raw">{stringifyUnknown(section.data)}</pre>
      );
    }
    case "text":
      return (
        <TextBlock
          data={
            typeof section.data === "string"
              ? section.data
              : stringifyUnknown(section.data)
          }
        />
      );
    case "table": {
      const data = asTableData(section.data);
      return data ? (
        <TableView data={data} />
      ) : (
        <pre className="rd-raw">{stringifyUnknown(section.data)}</pre>
      );
    }
    default:
      return <pre className="rd-raw">{stringifyUnknown(section.data)}</pre>;
  }
}

export default function RunDetail({ runId, onBack }: Props) {
  const [run, setRun] = useState<Run | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [tasks, setTasks] = useState<RunTask[]>([]);
  const [attempts, setAttempts] = useState<RunAttempt[]>([]);
  const [metrics, setMetrics] = useState<RunMetric[]>([]);
  const [relationships, setRelationships] = useState<RunRelationship[]>([]);
  const [showLogs, setShowLogs] = useState(true);
  const [logTab, setLogTab] = useState<"stdout" | "stderr">("stdout");
  const [analysis, setAnalysis] = useState<ErrorAnalysis | null>(null);
  const [analysisError, setAnalysisError] = useState<string | null>(null);
  const [analyzing, setAnalyzing] = useState(false);
  const pollAttemptRef = useRef(0);

  const loadObservability = useCallback(async () => {
    const [taskRows, attemptRows, metricRows, relationshipRows] =
      await Promise.all([
        getRunTasks(runId).catch(() => [] as RunTask[]),
        getRunAttempts(runId).catch(() => [] as RunAttempt[]),
        getRunMetrics(runId).catch(() => [] as RunMetric[]),
        getRunRelationships(runId).catch(() => [] as RunRelationship[]),
      ]);
    setTasks(taskRows);
    setAttempts(attemptRows);
    setMetrics(metricRows);
    setRelationships(relationshipRows);
  }, [runId]);

  const loadRun = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      const r = await getRunLog(runId);
      setRun(r);
      if (r.status === "failed") {
        setShowLogs(true);
        if (!r.stdout && r.stderr) setLogTab("stderr");
        if (r.error_analysis) setAnalysis(r.error_analysis);
      }
      await loadObservability();
    } catch (e) {
      setRun(null);
      setLoadError(String(e));
    } finally {
      setLoading(false);
    }
  }, [runId, loadObservability]);

  useEffect(() => {
    pollAttemptRef.current = 0;
    const id = window.setTimeout(() => void loadRun(), 0);
    return () => window.clearTimeout(id);
  }, [loadRun]);

  useEffect(() => {
    if (!run || !isActiveRunStatus(run.status)) return;

    const delay = nextPollDelayMs(pollAttemptRef.current);
    const timer = window.setTimeout(() => {
      pollAttemptRef.current += 1;
      void getRunLog(runId)
        .then(async (r) => {
          setRun(r);
          if (r.status === "failed") {
            setShowLogs(true);
            if (!r.stdout && r.stderr) setLogTab("stderr");
            if (r.error_analysis) setAnalysis(r.error_analysis);
          }
          if (!isActiveRunStatus(r.status)) {
            await loadObservability();
          }
        })
        .catch((e) => setLoadError(String(e)));
    }, delay);

    return () => window.clearTimeout(timer);
  }, [run, runId, loadObservability]);

  const handleAnalyze = async () => {
    setAnalyzing(true);
    setAnalysisError(null);
    try {
      const result = await analyzeRunError(runId);
      setAnalysis(result);
    } catch (e) {
      setAnalysisError(String(e));
    } finally {
      setAnalyzing(false);
    }
  };

  const handleLogTabKeyDown = (
    event: ReactKeyboardEvent<HTMLButtonElement>,
  ) => {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    const nextTab = logTab === "stdout" ? "stderr" : "stdout";
    setLogTab(nextTab);
    window.requestAnimationFrame(() => {
      document.getElementById(`run-log-tab-${nextTab}`)?.focus();
    });
  };

  if (loading && !run) {
    return (
      <section className="rd-page" aria-label="Run detail">
        <div className="page-header">
          <h1 className="page-title">Run Details</h1>
          <Button variant="ghost" onClick={onBack}>
            &larr; Run history
          </Button>
        </div>
        <div className="rd-loading" role="status">
          Loading run...
        </div>
      </section>
    );
  }

  if (loadError && !run) {
    return (
      <section className="rd-page" aria-label="Run detail">
        <div className="page-header">
          <h1 className="page-title">Run Details</h1>
          <Button variant="ghost" onClick={onBack}>
            &larr; Run history
          </Button>
        </div>
        <div className="rd-error" role="alert">
          <span>Failed to load run: {loadError}</span>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => void loadRun()}
            disabled={loading}
          >
            Retry
          </Button>
        </div>
      </section>
    );
  }

  if (!run) {
    return null;
  }

  const summary = isRecord(run.summary) ? (run.summary as RunSummary) : null;
  const summaryTitle =
    typeof summary?.title === "string" ? summary.title : null;
  const summaryDescription =
    typeof summary?.description === "string" ? summary.description : null;
  const summarySections = Array.isArray(summary?.sections)
    ? summary.sections.filter(isSummarySection)
    : [];
  const hasSummary = summarySections.length > 0;
  const isFailed = run.status === "failed";
  const hasStderr = !!run.stderr;
  const recommendedSteps = analysis?.recommended_steps ?? [];
  const workflowName = run.workflow_name ?? "Workflow run";
  const attemptsByTask = attempts.reduce<Record<string, RunAttempt[]>>(
    (acc, attempt) => {
      acc[attempt.task_id] = [...(acc[attempt.task_id] ?? []), attempt];
      return acc;
    },
    {},
  );

  return (
    <section className="rd-page" aria-label={`${workflowName} run detail`}>
      <PageHeader
        title={workflowName}
        subtitle={`Run ${run.id}`}
        actions={
          <div className="rd-header-actions">
            <Button variant="ghost" onClick={onBack}>
              &larr; Run history
            </Button>
            {run.result_url && (
              <Button
                variant="primary"
                onClick={() => openExternalSafe(run.result_url!)}
              >
                Open result
              </Button>
            )}
          </div>
        }
      />

      {loadError && (
        <div className="rd-error" role="alert">
          <span>Refresh failed: {loadError}</span>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => void loadRun()}
            disabled={loading}
          >
            Retry
          </Button>
        </div>
      )}

      {/* Run metadata bar */}
      <div className="rd-meta-bar" aria-label="Run metadata">
        <StatusBadge status={run.status}>
          {formatRunStatusLabel(run.status)}
        </StatusBadge>
        {isActiveRunStatus(run.status) && (
          <span className="rd-live-indicator" aria-live="polite">
            Live
          </span>
        )}
        <span className="rd-meta-item">
          Started {formatDate(run.started_at)}
        </span>
        <span className="rd-meta-item">
          Duration: {formatDuration(run.started_at, run.finished_at)}
        </span>
        {run.exit_code !== null && (
          <span className="rd-meta-item rd-meta-mono">
            exit {run.exit_code}
          </span>
        )}
        {run.trigger_kind && (
          <span className="rd-meta-item">
            {run.trigger_kind.replace(/_/g, " ")} trigger
          </span>
        )}
      </div>

      {tasks.length > 0 && (
        <section
          className="rd-observability-card"
          aria-labelledby="run-task-timeline-title"
        >
          <h2 className="rd-section-title" id="run-task-timeline-title">
            Task timeline
          </h2>
          <div className="rd-task-timeline">
            {tasks.map((task) => (
              <div key={task.id} className="rd-task-row">
                <div className="rd-task-label">
                  <StatusDot status={task.status} />
                  <span>{task.task_id}</span>
                  <span className="rd-task-attempt">
                    attempt {task.attempt_number}
                  </span>
                </div>
                <div className="rd-task-bar-wrap">
                  <div
                    className={`rd-task-bar ${task.status}`}
                    style={{ width: taskTimelineWidth(task, run) }}
                  >
                    {task.started_at && task.finished_at
                      ? formatDuration(task.started_at, task.finished_at)
                      : task.status}
                  </div>
                </div>
              </div>
            ))}
          </div>
          <table className="rd-table rd-attempt-table">
            <caption className="sr-only">Attempts for {workflowName}</caption>
            <thead>
              <tr>
                <th>Task</th>
                <th>Attempt</th>
                <th>Status</th>
                <th>Duration</th>
                <th>Error</th>
              </tr>
            </thead>
            <tbody>
              {Object.entries(attemptsByTask).flatMap(([taskId, rows]) =>
                rows.map((attempt) => (
                  <tr key={attempt.id}>
                    <td>{taskId}</td>
                    <td>{attempt.attempt_number}</td>
                    <td>
                      <StatusBadge status={attempt.status}>
                        {formatRunStatusLabel(attempt.status)}
                      </StatusBadge>
                    </td>
                    <td>
                      {formatDuration(
                        attempt.started_at,
                        attempt.finished_at ?? null,
                      )}
                    </td>
                    <td>
                      {attempt.error_message ?? attempt.error_type ?? "—"}
                    </td>
                  </tr>
                )),
              )}
            </tbody>
          </table>
        </section>
      )}

      {metrics.length > 0 && (
        <section
          className="rd-observability-card"
          aria-labelledby="run-metrics-title"
        >
          <h2 className="rd-section-title" id="run-metrics-title">
            Run metrics
          </h2>
          <table className="rd-table">
            <caption className="sr-only">
              Run metrics for {workflowName}
            </caption>
            <thead>
              <tr>
                <th>Metric</th>
                <th>Task</th>
                <th>Value</th>
                <th>Emitted</th>
              </tr>
            </thead>
            <tbody>
              {metrics.map((metric) => (
                <tr key={metric.id}>
                  <td>{metric.metric_name}</td>
                  <td>{metric.task_id ?? "workflow"}</td>
                  <td>
                    {metric.metric_value}
                    {metric.metric_unit ? ` ${metric.metric_unit}` : ""}
                  </td>
                  <td>{formatDate(metric.emitted_at)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      {relationships.length > 0 && (
        <section
          className="rd-observability-card"
          aria-labelledby="run-lineage-title"
        >
          <h2 className="rd-section-title" id="run-lineage-title">
            Child workflow lineage
          </h2>
          <table className="rd-table">
            <caption className="sr-only">
              Workflow lineage for {workflowName}
            </caption>
            <thead>
              <tr>
                <th>Direction</th>
                <th>Workflow</th>
                <th>Task</th>
                <th>Wait</th>
                <th>Status</th>
                <th>Run / Queue</th>
              </tr>
            </thead>
            <tbody>
              {relationships.map((rel) => {
                const isParent = rel.parent_run_id === run.id;
                return (
                  <tr key={rel.id}>
                    <td>{isParent ? "child" : "parent"}</td>
                    <td>{rel.child_workflow_name ?? rel.child_workflow_id}</td>
                    <td>{rel.task_id ?? "workflow"}</td>
                    <td>{rel.wait ? "wait" : "fire-and-forget"}</td>
                    <td>
                      <StatusBadge status={rel.status}>
                        {formatRunStatusLabel(rel.status)}
                      </StatusBadge>
                    </td>
                    <td className="rd-meta-mono">
                      {rel.child_run_id ??
                        rel.queued_run_id ??
                        rel.reason ??
                        "pending"}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </section>
      )}

      <section
        className="rd-observability-card rd-logs-section"
        aria-labelledby="run-logs-title"
      >
        <h2 className="rd-section-title" id="run-logs-title">
          <button
            type="button"
            className="rd-logs-toggle"
            onClick={() => setShowLogs(!showLogs)}
            aria-expanded={showLogs}
            aria-controls="run-log-content"
          >
            Raw logs
            <span aria-hidden="true">{showLogs ? "▾" : "▸"}</span>
          </button>
        </h2>

        {showLogs && (
          <div
            className="rd-logs-content"
            id="run-log-content"
            role="region"
            aria-labelledby="run-logs-title"
          >
            <div className="rd-log-tabs" role="tablist" aria-label="Log stream">
              <button
                type="button"
                className={`log-tab ${logTab === "stdout" ? "active" : ""}`}
                role="tab"
                id="run-log-tab-stdout"
                aria-controls="run-log-panel"
                aria-selected={logTab === "stdout"}
                tabIndex={logTab === "stdout" ? 0 : -1}
                onClick={() => setLogTab("stdout")}
                onKeyDown={handleLogTabKeyDown}
              >
                stdout
              </button>
              <button
                type="button"
                className={`log-tab ${logTab === "stderr" ? "active" : ""}`}
                role="tab"
                id="run-log-tab-stderr"
                aria-controls="run-log-panel"
                aria-selected={logTab === "stderr"}
                tabIndex={logTab === "stderr" ? 0 : -1}
                onClick={() => setLogTab("stderr")}
                onKeyDown={handleLogTabKeyDown}
              >
                stderr
              </button>
            </div>
            <pre
              className="log-output"
              id="run-log-panel"
              role="tabpanel"
              aria-labelledby={`run-log-tab-${logTab}`}
            >
              {(logTab === "stdout" ? run.stdout : run.stderr) || (
                <span className="log-empty">(empty)</span>
              )}
            </pre>
          </div>
        )}
      </section>

      {/* Prominent error output for failed runs */}
      {isFailed && hasStderr && (
        <section className="rd-error-block" aria-labelledby="run-error-title">
          <h2 className="rd-error-title" id="run-error-title">
            Error output
          </h2>
          <pre className="rd-error-output">{run.stderr}</pre>
        </section>
      )}

      {isFailed && !hasStderr && !run.stdout && (
        <section className="rd-error-block" aria-labelledby="run-failed-title">
          <h2 className="rd-error-title" id="run-failed-title">
            Run failed
          </h2>
          <p className="rd-error-hint">
            The workflow exited with code {run.exit_code ?? "unknown"} but
            produced no output. Try running the script manually to diagnose.
          </p>
        </section>
      )}

      {/* AI-powered error analysis */}
      {isFailed && (
        <section
          className="rd-analysis"
          aria-labelledby="run-failure-analysis-title"
        >
          <div className="rd-analysis-header">
            <h2 className="rd-analysis-title" id="run-failure-analysis-title">
              Failure analysis
            </h2>
            {!analysis && (
              <Button
                variant="primary"
                size="sm"
                onClick={handleAnalyze}
                disabled={analyzing}
              >
                {analyzing ? "Analyzing…" : "Analyze error with AI"}
              </Button>
            )}
          </div>

          {analysis ? (
            <>
              <p className="rd-analysis-diagnosis">
                {analysis.diagnosis ??
                  analysis.summary ??
                  "Analysis completed without a narrative diagnosis."}
              </p>
              {analysis.likely_cause && (
                <div className="rd-analysis-cause">
                  <span className="rd-analysis-cause-label">Likely cause:</span>{" "}
                  {analysis.likely_cause}
                </div>
              )}
              {recommendedSteps.length > 0 && (
                <div className="rd-analysis-steps">
                  <span className="rd-analysis-steps-label">
                    Recommended steps:
                  </span>
                  <ol>
                    {recommendedSteps.map((step, i) => (
                      <li key={i}>{step}</li>
                    ))}
                  </ol>
                </div>
              )}
            </>
          ) : (
            <p className="rd-analyze-hint">
              Ask Claude to diagnose this failure and suggest recovery steps.
            </p>
          )}

          {analysisError && (
            <div className="rd-analysis-error" role="alert">
              Analysis failed: {analysisError}
            </div>
          )}
        </section>
      )}

      {(summaryTitle || summaryDescription || hasSummary || !isFailed) && (
        <section className="rd-summary" aria-labelledby="run-summary-title">
          <h2 className="rd-summary-title" id="run-summary-title">
            {summaryTitle ?? "Run summary"}
          </h2>
          {summaryDescription && (
            <p className="rd-description">{summaryDescription}</p>
          )}

          {/* Structured workflow-emitted sections */}
          {hasSummary ? (
            <div className="rd-sections">
              {summarySections.map((section, i) => (
                <div key={i} className="rd-section">
                  <h3 className="rd-section-title">
                    {(typeof section.title === "string" && section.title) ||
                      `Section ${i + 1}`}
                  </h3>
                  <SectionRenderer section={section} />
                </div>
              ))}
            </div>
          ) : (
            <div className="rd-no-summary">
              <p>No structured summary available for this run.</p>
              <p className="rd-no-summary-hint">
                Workflow scripts can emit <code>SUMMARY_JSON:{"{ ... }"}</code>{" "}
                to provide rich run details here.
              </p>
            </div>
          )}
        </section>
      )}
    </section>
  );
}
