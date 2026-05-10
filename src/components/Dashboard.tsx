import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import WorkflowList from "./WorkflowList";
import WorkflowEditor from "./WorkflowEditor";
import RunHistory from "./RunHistory";
import RunDetail from "./RunDetail";
import QueueView from "./QueueView";
import Settings from "./Settings";
import { getSchedulerStatus, getSlaViolations, getWorkflow } from "../lib/commands";
import type { SchedulerStatus, SlaViolation, Workflow } from "../lib/commands";
import "./Dashboard.css";

type View = "workflows" | "editor" | "history" | "detail" | "queues" | "settings";

interface NavState {
  view: View;
  workflow?: Workflow;
  runId?: string;
}

export default function Dashboard() {
  const [nav, setNav] = useState<NavState>({ view: "workflows" });
  const [refreshKey, setRefreshKey] = useState(0);
  const [status, setStatus] = useState<SchedulerStatus | null>(null);
  const [slaViolations, setSlaViolations] = useState<SlaViolation[]>([]);

  const triggerRefresh = () => setRefreshKey((k) => k + 1);

  useEffect(() => {
    const unlisten = listen<{ runId: string; workflowId: string }>(
      "navigate-to-run",
      async (event) => {
        try {
          const wf = await getWorkflow(event.payload.workflowId);
          setNav({ view: "detail", workflow: wf, runId: event.payload.runId });
        } catch {
          setNav({ view: "detail", runId: event.payload.runId });
        }
      }
    );
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  useEffect(() => {
    if (nav.view !== "workflows") return;
    getSchedulerStatus().then(setStatus).catch(() => setStatus(null));
    getSlaViolations().then(setSlaViolations).catch(() => setSlaViolations([]));
  }, [nav.view, refreshKey]);

  return (
    <div className="dashboard">
      <aside className="dashboard-sidebar">
        <div className="sidebar-brand">
          <span className="brand-icon">&#9881;</span>
          <span className="brand-text">Chaos Labs</span>
        </div>
        <nav className="sidebar-nav">
          <button
            className={`sidebar-link ${["workflows", "editor", "history", "detail"].includes(nav.view) ? "active" : ""}`}
            onClick={() => setNav({ view: "workflows" })}
          >
            <span className="sidebar-icon">&#9776;</span>
            Workflows
          </button>
          <button
            className={`sidebar-link ${nav.view === "queues" ? "active" : ""}`}
            onClick={() => setNav({ view: "queues" })}
          >
            <span className="sidebar-icon">&#8644;</span>
            Queues
          </button>
          <button
            className={`sidebar-link ${nav.view === "settings" ? "active" : ""}`}
            onClick={() => setNav({ view: "settings" })}
          >
            <span className="sidebar-icon">&#9881;</span>
            Settings
          </button>
        </nav>
        <div className="sidebar-footer">
          <span className="sidebar-version">v0.1.0</span>
        </div>
      </aside>

      <main className="dashboard-main">
        {nav.view === "workflows" && (
          <>
            {status && (
              <div className="dashboard-status-grid">
                <div className="dashboard-status-card">
                  <span className="dashboard-status-value">{status.active_workflows}</span>
                  <span className="dashboard-status-label">Active workflows</span>
                </div>
                <div className="dashboard-status-card">
                  <span className="dashboard-status-value">{status.running_count}</span>
                  <span className="dashboard-status-label">Running now</span>
                </div>
                <div className="dashboard-status-card">
                  <span className="dashboard-status-value">{status.recent_runs.filter((run) => run.status === "failed").length}</span>
                  <span className="dashboard-status-label">Recent failures</span>
                </div>
                <div className={`dashboard-status-card ${slaViolations.length ? "warning" : ""}`}>
                  <span className="dashboard-status-value">{slaViolations.length}</span>
                  <span className="dashboard-status-label">SLA violations</span>
                </div>
              </div>
            )}
            {slaViolations.length > 0 && (
              <div className="dashboard-alert-list">
                {slaViolations.slice(0, 3).map((violation) => (
                  <div key={`${violation.workflow_id}-${violation.violation_type}`} className="dashboard-alert">
                    <strong>{violation.workflow_name}</strong>
                    <span>{violation.message}</span>
                  </div>
                ))}
              </div>
            )}
            <WorkflowList
              key={refreshKey}
              onEdit={(w) => setNav({ view: "editor", workflow: w })}
              onNew={() => setNav({ view: "editor" })}
              onHistory={(w) => setNav({ view: "history", workflow: w })}
            />
          </>
        )}
        {nav.view === "editor" && (
          <WorkflowEditor
            workflow={nav.workflow}
            onSaved={() => {
              triggerRefresh();
              setNav({ view: "workflows" });
            }}
            onCancel={() => setNav({ view: "workflows" })}
          />
        )}
        {nav.view === "history" && nav.workflow && (
          <RunHistory
            workflow={nav.workflow}
            onBack={() => setNav({ view: "workflows" })}
            onViewLog={(runId) =>
              setNav({ view: "detail", workflow: nav.workflow, runId })
            }
          />
        )}
        {nav.view === "detail" && nav.runId && (
          <RunDetail
            runId={nav.runId}
            onBack={() =>
              setNav({ view: "history", workflow: nav.workflow })
            }
          />
        )}
        {nav.view === "queues" && <QueueView />}
        {nav.view === "settings" && <Settings />}
      </main>
    </div>
  );
}
