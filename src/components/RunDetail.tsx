import { useState, useEffect } from "react";
import { getRunLog, getRunTasks, getRunAttempts, getRunMetrics, openUrl, analyzeRunError } from "../lib/commands";
import type { Run, RunTask, RunAttempt, RunMetric, ErrorAnalysis } from "../lib/commands";
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

function asItemList(value: unknown): Array<{ name: string; detail?: string; url?: string; badge?: string }> | null {
  if (!Array.isArray(value)) return null;
  const items = value.filter(isRecord).map((item) => ({
    name: typeof item.name === "string" ? item.name : "Untitled",
    detail: typeof item.detail === "string" ? item.detail : undefined,
    url: typeof item.url === "string" ? item.url : undefined,
    badge: typeof item.badge === "string" ? item.badge : undefined,
  }));
  return items;
}

function asLinkList(value: unknown): Array<{ label: string; url: string }> | null {
  if (!Array.isArray(value)) return null;
  const links = value
    .filter(isRecord)
    .filter((item) => typeof item.label === "string" && typeof item.url === "string")
    .map((item) => ({ label: item.label as string, url: item.url as string }));
  return links;
}

function asPhaseList(value: unknown): Array<{ name: string; status: string; duration?: string }> | null {
  if (!Array.isArray(value)) return null;
  return value.filter(isRecord).map((phase) => ({
    name: typeof phase.name === "string" ? phase.name : "Unnamed phase",
    status: typeof phase.status === "string" ? phase.status : "unknown",
    duration: typeof phase.duration === "string" ? phase.duration : undefined,
  }));
}

function asTableData(value: unknown): { headers: string[]; rows: string[][] } | null {
  if (!isRecord(value) || !Array.isArray(value.headers) || !Array.isArray(value.rows)) return null;
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
  const ms = new Date(end).getTime() - new Date(start).getTime();
  const secs = Math.floor(ms / 1000);
  if (secs < 60) return `${secs}s`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m ${secs % 60}s`;
  const hrs = Math.floor(mins / 60);
  return `${hrs}h ${mins % 60}m`;
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

function StatsGrid({ data }: { data: Record<string, number | string> }) {
  const entries = Object.entries(data);
  return (
    <div className="rd-stats-grid">
      {entries.map(([label, value]) => (
        <div key={label} className="rd-stat-card">
          <div className="rd-stat-value">{String(value)}</div>
          <div className="rd-stat-label">{label}</div>
        </div>
      ))}
    </div>
  );
}

function ItemList({ data }: { data: Array<{ name: string; detail?: string; url?: string; badge?: string }> }) {
  return (
    <div className="rd-item-list">
      {data.map((item, i) => (
        <div key={i} className="rd-item">
          <div className="rd-item-main">
            <span className="rd-item-name">{item.name}</span>
            {item.badge && (
              <span className="rd-item-badge">{item.badge}</span>
            )}
          </div>
          {item.detail && <div className="rd-item-detail">{item.detail}</div>}
          {item.url && (
            <button
              className="btn btn-ghost btn-sm"
              onClick={() => openUrl(item.url!)}
            >
              Open
            </button>
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
          onClick={() => openUrl(link.url)}
        >
          {link.label}
        </button>
      ))}
    </div>
  );
}

function PhaseTimeline({ data }: { data: Array<{ name: string; status: string; duration?: string }> }) {
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

function TableView({ data }: { data: { headers: string[]; rows: string[][] } }) {
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
      return data ? <StatsGrid data={data} /> : <pre className="rd-raw">{stringifyUnknown(section.data)}</pre>;
    }
    case "items": {
      const data = asItemList(section.data);
      return data ? <ItemList data={data} /> : <pre className="rd-raw">{stringifyUnknown(section.data)}</pre>;
    }
    case "links": {
      const data = asLinkList(section.data);
      return data ? <LinkList data={data} /> : <pre className="rd-raw">{stringifyUnknown(section.data)}</pre>;
    }
    case "phases": {
      const data = asPhaseList(section.data);
      return data ? <PhaseTimeline data={data} /> : <pre className="rd-raw">{stringifyUnknown(section.data)}</pre>;
    }
    case "text":
      return <TextBlock data={typeof section.data === "string" ? section.data : stringifyUnknown(section.data)} />;
    case "table": {
      const data = asTableData(section.data);
      return data ? <TableView data={data} /> : <pre className="rd-raw">{stringifyUnknown(section.data)}</pre>;
    }
    default:
      return <pre className="rd-raw">{stringifyUnknown(section.data)}</pre>;
  }
}

export default function RunDetail({ runId, onBack }: Props) {
  const [run, setRun] = useState<Run | null>(null);
  const [tasks, setTasks] = useState<RunTask[]>([]);
  const [attempts, setAttempts] = useState<RunAttempt[]>([]);
  const [metrics, setMetrics] = useState<RunMetric[]>([]);
  const [showLogs, setShowLogs] = useState(false);
  const [logTab, setLogTab] = useState<"stdout" | "stderr">("stdout");
  const [analysis, setAnalysis] = useState<ErrorAnalysis | null>(null);
  const [analyzing, setAnalyzing] = useState(false);

  useEffect(() => {
    getRunLog(runId).then((r) => {
      setRun(r);
      if (r.status === "failed") {
        setShowLogs(true);
        if (!r.stdout && r.stderr) setLogTab("stderr");
        if (r.error_analysis) setAnalysis(r.error_analysis);
      }
    });
    getRunTasks(runId).then(setTasks).catch(() => setTasks([]));
    getRunAttempts(runId).then(setAttempts).catch(() => setAttempts([]));
    getRunMetrics(runId).then(setMetrics).catch(() => setMetrics([]));
  }, [runId]);

  const handleAnalyze = async () => {
    setAnalyzing(true);
    try {
      const result = await analyzeRunError(runId);
      setAnalysis(result);
    } catch (e) {
      console.error("Analysis failed:", e);
    } finally {
      setAnalyzing(false);
    }
  };

  if (!run) {
    return <div className="rd-loading">Loading...</div>;
  }

  const summary = isRecord(run.summary) ? (run.summary as RunSummary) : null;
  const summaryTitle = typeof summary?.title === "string" ? summary.title : null;
  const summaryDescription = typeof summary?.description === "string" ? summary.description : null;
  const summarySections = Array.isArray(summary?.sections)
    ? summary.sections.filter(isSummarySection)
    : [];
  const hasSummary = summarySections.length > 0;
  const isFailed = run.status === "failed";
  const hasStderr = !!run.stderr;
  const recommendedSteps = analysis?.recommended_steps ?? [];
  const attemptsByTask = attempts.reduce<Record<string, RunAttempt[]>>((acc, attempt) => {
    acc[attempt.task_id] = [...(acc[attempt.task_id] ?? []), attempt];
    return acc;
  }, {});

  return (
    <div className="rd-page">
      <div className="page-header">
        <div>
          <h1 className="page-title">
            {run.workflow_name ?? "Workflow Run"}
          </h1>
          {summaryTitle && (
            <p className="page-subtitle">{summaryTitle}</p>
          )}
        </div>
        <button className="btn btn-ghost" onClick={onBack}>
          &larr; Back
        </button>
      </div>

      {/* Run metadata bar */}
      <div className="rd-meta-bar">
        <span className={`status-badge ${run.status}`}>{run.status}</span>
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
        {run.result_url && (
          <button
            className="btn btn-ghost btn-sm"
            onClick={() => openUrl(run.result_url!)}
          >
            Open Result
          </button>
        )}
      </div>

      {tasks.length > 0 && (
        <div className="rd-observability-card">
          <h3 className="rd-section-title">Task Timeline</h3>
          <div className="rd-task-timeline">
            {tasks.map((task) => (
              <div key={task.id} className="rd-task-row">
                <div className="rd-task-label">
                  <span className={`status-dot ${task.status}`} />
                  <span>{task.task_id}</span>
                  <span className="rd-task-attempt">attempt {task.attempt_number}</span>
                </div>
                <div className="rd-task-bar-wrap">
                  <div className={`rd-task-bar ${task.status}`}>
                    {task.started_at && task.finished_at
                      ? formatDuration(task.started_at, task.finished_at)
                      : task.status}
                  </div>
                </div>
              </div>
            ))}
          </div>
          <table className="rd-table rd-attempt-table">
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
                    <td><span className={`status-badge ${attempt.status}`}>{attempt.status}</span></td>
                    <td>{formatDuration(attempt.started_at, attempt.finished_at ?? null)}</td>
                    <td>{attempt.error_message ?? attempt.error_type ?? "—"}</td>
                  </tr>
                )),
              )}
            </tbody>
          </table>
        </div>
      )}

      {metrics.length > 0 && (
        <div className="rd-observability-card">
          <h3 className="rd-section-title">Run Metrics</h3>
          <table className="rd-table">
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
                  <td>{metric.metric_value}{metric.metric_unit ? ` ${metric.metric_unit}` : ""}</td>
                  <td>{formatDate(metric.emitted_at)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* Prominent error output for failed runs */}
      {isFailed && hasStderr && (
        <div className="rd-error-block">
          <h3 className="rd-error-title">Error Output</h3>
          <pre className="rd-error-output">{run.stderr}</pre>
        </div>
      )}

      {isFailed && !hasStderr && !run.stdout && (
        <div className="rd-error-block">
          <h3 className="rd-error-title">Run Failed</h3>
          <p className="rd-error-hint">
            The workflow exited with code {run.exit_code ?? "unknown"} but
            produced no output. Try running the script manually to diagnose.
          </p>
        </div>
      )}

      {/* AI-powered error analysis */}
      {isFailed && analysis && (
        <div className="rd-analysis">
          <h3 className="rd-analysis-title">AI Diagnosis</h3>
          <p className="rd-analysis-diagnosis">{analysis.diagnosis}</p>
          <div className="rd-analysis-cause">
            <span className="rd-analysis-cause-label">Likely cause:</span>{" "}
            {analysis.likely_cause}
          </div>
          {recommendedSteps.length > 0 && (
            <div className="rd-analysis-steps">
              <span className="rd-analysis-steps-label">Recommended steps:</span>
              <ol>
                {recommendedSteps.map((step, i) => (
                  <li key={i}>{step}</li>
                ))}
              </ol>
            </div>
          )}
        </div>
      )}

      {isFailed && !analysis && (
        <div className="rd-analyze-prompt">
          <button
            className="btn btn-primary btn-sm"
            onClick={handleAnalyze}
            disabled={analyzing}
          >
            {analyzing ? "Analyzing..." : "Analyze Error with AI"}
          </button>
          {!analyzing && (
            <span className="rd-analyze-hint">
              Uses Claude to diagnose the failure and suggest fixes
            </span>
          )}
        </div>
      )}

      {summaryDescription && (
        <p className="rd-description">{summaryDescription}</p>
      )}

      {/* Structured sections */}
      {hasSummary ? (
        <div className="rd-sections">
          {summarySections.map((section, i) => (
            <div key={i} className="rd-section">
              <h3 className="rd-section-title">
                {(typeof section.title === "string" && section.title) || `Section ${i + 1}`}
              </h3>
              <SectionRenderer section={section} />
            </div>
          ))}
        </div>
      ) : !isFailed ? (
        <div className="rd-no-summary">
          <p>No structured summary available for this run.</p>
          <p className="rd-no-summary-hint">
            Workflow scripts can emit <code>SUMMARY_JSON:{'{ ... }'}</code> to
            provide rich run details here.
          </p>
        </div>
      ) : null}

      {/* Collapsible raw logs */}
      <div className="rd-logs-section">
        <button
          className="rd-logs-toggle"
          onClick={() => setShowLogs(!showLogs)}
        >
          {showLogs ? "▾" : "▸"} Raw Logs
        </button>

        {showLogs && (
          <div className="rd-logs-content">
            <div className="rd-log-tabs">
              <button
                className={`log-tab ${logTab === "stdout" ? "active" : ""}`}
                onClick={() => setLogTab("stdout")}
              >
                stdout
              </button>
              <button
                className={`log-tab ${logTab === "stderr" ? "active" : ""}`}
                onClick={() => setLogTab("stderr")}
              >
                stderr
              </button>
            </div>
            <pre className="log-output">
              {(logTab === "stdout" ? run.stdout : run.stderr) || (
                <span className="log-empty">(empty)</span>
              )}
            </pre>
          </div>
        )}
      </div>
    </div>
  );
}
