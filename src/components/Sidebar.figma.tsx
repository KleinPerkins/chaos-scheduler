import figma from "@figma/code-connect";
import {
  Gauge,
  Workflow as WorkflowIcon,
  History as HistoryIcon,
  ArrowLeftRight,
  Boxes,
  Plug,
  Settings as SettingsIcon,
} from "lucide-react";
import Sidebar from "./Sidebar";

/**
 * Figma Code Connect mapping for the `Sidebar` component set
 * (node 305:6378, file twQmWC8dWT4tqeqIigNsRy).
 *
 * The `Layout` variant maps to the component's controlled `collapsed` prop.
 * Production may omit that prop to use Sidebar's internal toggle state. The
 * remaining data/theme seams are represented by the example; composed
 * `NavItem`s resolve through their own Code Connect mapping. Consumed by the
 * `figma` CLI, not Vite.
 */
figma.connect(
  Sidebar,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=305-6378",
  {
    props: {
      collapsed: figma.enum("Layout", {
        Expanded: false,
        Collapsed: true,
      }),
    },
    example: ({ collapsed }) => (
      <Sidebar
        collapsed={collapsed}
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
    ),
  },
);
