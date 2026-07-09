import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  Gauge,
  Workflow as WorkflowIcon,
  History as HistoryIcon,
  ArrowLeftRight,
  Boxes,
  Plug,
  Settings as SettingsIcon,
  type LucideIcon,
} from "lucide-react";
import UpdateBanner from "./UpdateBanner";
import WorkflowList from "./WorkflowList";
import Sidebar from "./Sidebar";
import WorkflowEditor from "./WorkflowEditor";
import WorkflowDetail from "./WorkflowDetail";
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
import { useTheme } from "../hooks/useTheme";
import "./Dashboard.css";

type View =
  | "mission"
  | "workflows"
  | "editor"
  | "workflow_detail"
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
  missionEnvironment?: MissionControlReturnState["environmentFilter"];
  missionDomain?: string;
  returnTo?: NavState;
}

// The workflow-management surface and its sub-views (editor / per-workflow
// history / run detail) all keep the "Workflows" nav entry highlighted.
const WORKFLOW_VIEWS: View[] = [
  "workflows",
  "editor",
  "workflow_detail",
  "history",
  "detail",
];

export default function Dashboard() {
  const [nav, setNav] = useState<NavState>({ view: "mission" });
  const [refreshKey, setRefreshKey] = useState(0);
  const theme = useTheme();

  const triggerRefresh = () => setRefreshKey((k) => k + 1);

  const openRunFromMission = async (
    runId: string,
    workflowId: string,
    returnState: MissionControlReturnState,
  ) => {
    const returnTo: NavState = {
      view: "mission",
      missionTab: returnState.tab,
      missionEnvironment: returnState.environmentFilter,
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
      missionEnvironment: returnState.environmentFilter,
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

  const navItems: {
    view: View;
    label: string;
    Icon: LucideIcon;
    match: View[];
  }[] = [
    { view: "mission", label: "Home", Icon: Gauge, match: ["mission"] },
    {
      view: "workflows",
      label: "Workflows",
      Icon: WorkflowIcon,
      match: WORKFLOW_VIEWS,
    },
    {
      view: "global_history",
      label: "History",
      Icon: HistoryIcon,
      match: ["global_history"],
    },
    {
      view: "queues",
      label: "Queues",
      Icon: ArrowLeftRight,
      match: ["queues"],
    },
    {
      view: "environments",
      label: "Environments",
      Icon: Boxes,
      match: ["environments"],
    },
    {
      view: "integrations",
      label: "Integrations",
      Icon: Plug,
      match: ["integrations"],
    },
    {
      view: "settings",
      label: "Settings",
      Icon: SettingsIcon,
      match: ["settings"],
    },
  ];

  return (
    <div className="dashboard">
      <Sidebar
        navItems={navItems}
        currentView={nav.view}
        onNavigate={(view) => setNav({ view })}
        themePreference={theme.preference}
        onThemeChange={theme.setPreference}
      />

      <main className="dashboard-main">
        <UpdateBanner />
        {nav.view === "mission" && (
          <MissionControl
            initialTab={nav.missionTab}
            initialEnvironment={nav.missionEnvironment}
            initialDomain={nav.missionDomain}
            onOpenRun={openRunFromMission}
            onOpenQueues={(returnState) =>
              setNav({
                view: "queues",
                returnTo: {
                  view: "mission",
                  missionTab: returnState.tab,
                  missionEnvironment: returnState.environmentFilter,
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
            onOpen={(w) => setNav({ view: "workflow_detail", workflow: w })}
            onEdit={(w) => setNav({ view: "editor", workflow: w })}
            onNew={() => setNav({ view: "editor" })}
            onHistory={(w) => setNav({ view: "history", workflow: w })}
          />
        )}
        {nav.view === "workflow_detail" && nav.workflow && (
          <WorkflowDetail
            workflow={nav.workflow}
            onBack={() => setNav({ view: "workflows" })}
            onEdit={(w) =>
              setNav({
                view: "editor",
                workflow: w,
                returnTo: { view: "workflow_detail", workflow: w },
              })
            }
            onFullHistory={(w) =>
              setNav({
                view: "history",
                workflow: w,
                returnTo: { view: "workflow_detail", workflow: w },
              })
            }
            onViewRun={(runId) =>
              setNav({
                view: "detail",
                workflow: nav.workflow,
                runId,
                returnTo: { view: "workflow_detail", workflow: nav.workflow },
              })
            }
          />
        )}
        {nav.view === "editor" && (
          <WorkflowEditor
            workflow={nav.workflow}
            onSaved={() => {
              triggerRefresh();
              setNav(nav.returnTo ?? { view: "workflows" });
            }}
            onCancel={() => setNav(nav.returnTo ?? { view: "workflows" })}
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
