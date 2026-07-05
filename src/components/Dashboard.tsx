import { useState, useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import WorkflowList from "./WorkflowList";
import WorkflowEditor from "./WorkflowEditor";
import RunHistory from "./RunHistory";
import RunDetail from "./RunDetail";
import GlobalHistory from "./GlobalHistory";
import QueueView from "./QueueView";
import Settings from "./Settings";
import Environments from "./Environments";
import Integrations from "./Integrations";
import MissionControl, {
  type MissionControlReturnState,
  type MissionTab,
} from "./MissionControl";
import {
  getMissionControlPreferences,
  getSchedulerStatus,
  getSlaViolations,
  getWorkflow,
} from "../lib/commands";
import type { SchedulerStatus, SlaViolation, Workflow } from "../lib/commands";
import { PRODUCT_SHORT_NAME, APP_VERSION } from "../lib/branding";
import "./Dashboard.css";

type View =
  | "mission"
  | "workflows"
  | "editor"
  | "history"
  | "detail"
  | "global_history"
  | "queues"
  | "environments"
  | "integrations"
  | "settings";

interface NavState {
  view: View;
  workflow?: Workflow;
  runId?: string;
  missionTab?: MissionTab;
  missionCorpus?: MissionControlReturnState["corpus"];
  missionDomain?: string;
  returnTo?: NavState;
}

export default function Dashboard() {
  const [nav, setNav] = useState<NavState>({ view: "mission" });
  const [refreshKey, setRefreshKey] = useState(0);
  const [status, setStatus] = useState<SchedulerStatus | null>(null);
  const [slaViolations, setSlaViolations] = useState<SlaViolation[]>([]);
  const [landingResolved, setLandingResolved] = useState(false);
  const forcedMissionLanding = useRef(false);

  const triggerRefresh = () => setRefreshKey((k) => k + 1);
  const openRunFromMission = async (
    runId: string,
    workflowId: string,
    returnState: MissionControlReturnState,
  ) => {
    const returnTo: NavState = {
      view: "mission",
      missionTab: returnState.tab,
      missionCorpus: returnState.corpus,
      missionDomain: returnState.domain,
    };
    try {
      const workflow = await getWorkflow(workflowId);
      setNav({ view: "detail", workflow, runId, returnTo });
    } catch {
      setNav({ view: "detail", runId, returnTo });
    }
  };

  const openHistoryFromMission = async (
    workflowId: string,
    returnState: MissionControlReturnState,
  ) => {
    const returnTo: NavState = {
      view: "mission",
      missionTab: returnState.tab,
      missionCorpus: returnState.corpus,
      missionDomain: returnState.domain,
    };
    try {
      const workflow = await getWorkflow(workflowId);
      setNav({ view: "history", workflow, returnTo });
    } catch {
      setNav(returnTo);
    }
  };

  useEffect(() => {
    const unlisten = listen<{ runId: string; workflowId: string }>(
      "navigate-to-run",
      async (event) => {
        const returnTo: NavState = { view: "mission", missionTab: "activity" };
        try {
          const wf = await getWorkflow(event.payload.workflowId);
          setNav({
            view: "detail",
            workflow: wf,
            runId: event.payload.runId,
            returnTo,
          });
        } catch {
          setNav({ view: "detail", runId: event.payload.runId, returnTo });
        }
      },
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    const unlisten = listen("navigate-to-mission-control", () => {
      forcedMissionLanding.current = true;
      setLandingResolved(true);
      setNav({ view: "mission" });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    getMissionControlPreferences()
      .then((prefs) => {
        if (
          cancelled ||
          forcedMissionLanding.current ||
          prefs.default_landing !== "dashboard"
        )
          return;
        setNav((current) =>
          current.view === "mission" ? { view: "workflows" } : current,
        );
      })
      .catch(() => {})
      .finally(() => {
        if (!cancelled) setLandingResolved(true);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (nav.view !== "workflows") return;
    getSchedulerStatus()
      .then(setStatus)
      .catch(() => setStatus(null));
    getSlaViolations()
      .then(setSlaViolations)
      .catch(() => setSlaViolations([]));
  }, [nav.view, refreshKey]);

  if (!landingResolved) {
    return <div className="dashboard-loading">Loading Scheduler...</div>;
  }

  return (
    <div className="dashboard">
      <aside className="dashboard-sidebar">
        <div className="sidebar-brand">
          <span className="brand-icon" aria-hidden="true">
            &#9881;
          </span>
          <span className="brand-text">{PRODUCT_SHORT_NAME}</span>
        </div>
        <nav className="sidebar-nav" aria-label="Primary navigation">
          <button
            className={`sidebar-link ${nav.view === "mission" ? "active" : ""}`}
            aria-current={nav.view === "mission" ? "page" : undefined}
            onClick={() => setNav({ view: "mission" })}
          >
            <span className="sidebar-icon">&#9673;</span>
            Mission Control
          </button>
          <button
            className={`sidebar-link ${["workflows", "editor", "history", "detail"].includes(nav.view) ? "active" : ""}`}
            aria-current={
              ["workflows", "editor", "history", "detail"].includes(nav.view)
                ? "page"
                : undefined
            }
            onClick={() => setNav({ view: "workflows" })}
          >
            <span className="sidebar-icon">&#9776;</span>
            Dashboard
          </button>
          <button
            className={`sidebar-link ${nav.view === "global_history" ? "active" : ""}`}
            aria-current={nav.view === "global_history" ? "page" : undefined}
            onClick={() => setNav({ view: "global_history" })}
          >
            <span className="sidebar-icon">&#8635;</span>
            History
          </button>
          <button
            className={`sidebar-link ${nav.view === "queues" ? "active" : ""}`}
            aria-current={nav.view === "queues" ? "page" : undefined}
            onClick={() => setNav({ view: "queues" })}
          >
            <span className="sidebar-icon">&#8644;</span>
            Queues
          </button>
          <button
            className={`sidebar-link ${nav.view === "environments" ? "active" : ""}`}
            aria-current={nav.view === "environments" ? "page" : undefined}
            onClick={() => setNav({ view: "environments" })}
          >
            <span className="sidebar-icon">&#9709;</span>
            Environments
          </button>
          <button
            className={`sidebar-link ${nav.view === "integrations" ? "active" : ""}`}
            aria-current={nav.view === "integrations" ? "page" : undefined}
            onClick={() => setNav({ view: "integrations" })}
          >
            <span className="sidebar-icon">&#128268;</span>
            Integrations
          </button>
          <button
            className={`sidebar-link ${nav.view === "settings" ? "active" : ""}`}
            aria-current={nav.view === "settings" ? "page" : undefined}
            onClick={() => setNav({ view: "settings" })}
          >
            <span className="sidebar-icon">&#9881;</span>
            Settings
          </button>
        </nav>
        <div className="sidebar-footer">
          <span className="sidebar-version">v{APP_VERSION}</span>
        </div>
      </aside>

      <main className="dashboard-main">
        {nav.view === "mission" && (
          <MissionControl
            initialTab={nav.missionTab}
            initialCorpus={nav.missionCorpus}
            initialDomain={nav.missionDomain}
            onOpenRun={openRunFromMission}
            onOpenQueues={(returnState) =>
              setNav({
                view: "queues",
                returnTo: {
                  view: "mission",
                  missionTab: returnState.tab,
                  missionCorpus: returnState.corpus,
                  missionDomain: returnState.domain,
                },
              })
            }
            onOpenHistory={openHistoryFromMission}
            onOpenDashboard={() => setNav({ view: "workflows" })}
          />
        )}
        {nav.view === "workflows" && (
          <>
            {status && (
              <div className="dashboard-status-grid">
                <div className="dashboard-status-card">
                  <span className="dashboard-status-value">
                    {status.active_workflows}
                  </span>
                  <span className="dashboard-status-label">
                    Active workflows
                  </span>
                </div>
                <div className="dashboard-status-card">
                  <span className="dashboard-status-value">
                    {status.running_count}
                  </span>
                  <span className="dashboard-status-label">Running now</span>
                </div>
                <div className="dashboard-status-card">
                  <span className="dashboard-status-value">
                    {
                      status.recent_runs.filter(
                        (run) => run.status === "failed",
                      ).length
                    }
                  </span>
                  <span className="dashboard-status-label">
                    Recent failures
                  </span>
                </div>
                <div
                  className={`dashboard-status-card ${slaViolations.length ? "warning" : ""}`}
                >
                  <span className="dashboard-status-value">
                    {slaViolations.length}
                  </span>
                  <span className="dashboard-status-label">SLA violations</span>
                </div>
              </div>
            )}
            {slaViolations.length > 0 && (
              <div className="dashboard-alert-list">
                {slaViolations.slice(0, 3).map((violation) => (
                  <div
                    key={`${violation.workflow_id}-${violation.violation_type}`}
                    className="dashboard-alert"
                  >
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
            onBack={() => setNav(nav.returnTo ?? { view: "workflows" })}
            onViewLog={(runId) =>
              setNav({
                view: "detail",
                workflow: nav.workflow,
                runId,
                returnTo: {
                  view: "history",
                  workflow: nav.workflow,
                  returnTo: nav.returnTo,
                },
              })
            }
          />
        )}
        {nav.view === "detail" && nav.runId && (
          <RunDetail
            runId={nav.runId}
            onBack={() =>
              setNav(
                nav.returnTo ?? { view: "history", workflow: nav.workflow },
              )
            }
          />
        )}
        {nav.view === "global_history" && (
          <GlobalHistory
            onViewRun={(run) =>
              setNav({
                view: "detail",
                runId: run.id,
                returnTo: { view: "global_history" },
              })
            }
          />
        )}
        {nav.view === "queues" && (
          <QueueView
            onBack={nav.returnTo ? () => setNav(nav.returnTo!) : undefined}
          />
        )}
        {nav.view === "environments" && <Environments />}
        {nav.view === "integrations" && <Integrations />}
        {nav.view === "settings" && <Settings />}
      </main>
    </div>
  );
}
