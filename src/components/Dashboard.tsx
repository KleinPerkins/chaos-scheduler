import { useState, useEffect } from "react";
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
import { getWorkflow } from "../lib/commands";
import type { Workflow } from "../lib/commands";
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

// The workflow-management surface and its sub-views (editor / per-workflow
// history / run detail) all keep the "Workflows" nav entry highlighted.
const WORKFLOW_VIEWS: View[] = ["workflows", "editor", "history", "detail"];

export default function Dashboard() {
  const [nav, setNav] = useState<NavState>({ view: "mission" });
  const [refreshKey, setRefreshKey] = useState(0);

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
      setNav({ view: "mission" });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const navItems: { view: View; label: string; icon: string; match: View[] }[] =
    [
      { view: "mission", label: "Home", icon: "\u25C9", match: ["mission"] },
      {
        view: "workflows",
        label: "Workflows",
        icon: "\u2630",
        match: WORKFLOW_VIEWS,
      },
      {
        view: "global_history",
        label: "History",
        icon: "\u21BB",
        match: ["global_history"],
      },
      { view: "queues", label: "Queues", icon: "\u21C4", match: ["queues"] },
      {
        view: "environments",
        label: "Environments",
        icon: "\u25ED",
        match: ["environments"],
      },
      {
        view: "integrations",
        label: "Integrations",
        icon: "\u21F9",
        match: ["integrations"],
      },
      {
        view: "settings",
        label: "Settings",
        icon: "\u2699",
        match: ["settings"],
      },
    ];

  return (
    <div className="dashboard">
      <aside className="dashboard-sidebar">
        <div className="sidebar-brand">
          <span className="brand-icon" aria-hidden="true">
            &#9673;
          </span>
          <span className="brand-text">{PRODUCT_SHORT_NAME}</span>
        </div>
        <nav className="sidebar-nav" aria-label="Primary navigation">
          {navItems.map((item) => {
            const active = item.match.includes(nav.view);
            return (
              <button
                key={item.view}
                className={`sidebar-link ${active ? "active" : ""}`}
                aria-current={active ? "page" : undefined}
                onClick={() => setNav({ view: item.view })}
              >
                <span className="sidebar-icon" aria-hidden="true">
                  {item.icon}
                </span>
                {item.label}
              </button>
            );
          })}
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
          />
        )}
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
