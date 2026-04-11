import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import WorkflowList from "./WorkflowList";
import WorkflowEditor from "./WorkflowEditor";
import RunHistory from "./RunHistory";
import RunDetail from "./RunDetail";
import Settings from "./Settings";
import { getWorkflow } from "../lib/commands";
import type { Workflow } from "../lib/commands";
import "./Dashboard.css";

type View = "workflows" | "editor" | "history" | "detail" | "settings";

interface NavState {
  view: View;
  workflow?: Workflow;
  runId?: string;
}

export default function Dashboard() {
  const [nav, setNav] = useState<NavState>({ view: "workflows" });
  const [refreshKey, setRefreshKey] = useState(0);

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
          <WorkflowList
            key={refreshKey}
            onEdit={(w) => setNav({ view: "editor", workflow: w })}
            onNew={() => setNav({ view: "editor" })}
            onHistory={(w) => setNav({ view: "history", workflow: w })}
          />
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
        {nav.view === "settings" && <Settings />}
      </main>
    </div>
  );
}
