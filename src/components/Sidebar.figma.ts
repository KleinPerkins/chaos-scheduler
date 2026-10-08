// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=305-6378
// source=src/components/Sidebar.tsx
// component=Sidebar

import figma from "figma";

const collapsed = figma.selectedInstance.getEnum("Layout", {
  Expanded: false,
  Collapsed: true,
});

export default {
  id: "Sidebar",
  imports: [
    "import {\n  Gauge,\n  Workflow as WorkflowIcon,\n  History as HistoryIcon,\n  ArrowLeftRight,\n  Boxes,\n  Plug,\n  Settings as SettingsIcon,\n} from \"lucide-react\"",
    "import Sidebar from \"./Sidebar\""
  ],
  example: figma.code`
    <Sidebar
      ${figma.helpers.react.renderProp("collapsed", collapsed)}
      navItems={[
        { view: "mission", label: "Home", Icon: Gauge, match: ["mission"] },
        {
          view: "workflows",
          label: "Workflows",
          Icon: WorkflowIcon,
          match: ["workflows"],
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
      ]}
      currentView="mission"
      onNavigate={() => {}}
      themePreference="dark"
      onThemeChange={() => {}}
    />
  `,
  metadata: { nestable: true },
};
