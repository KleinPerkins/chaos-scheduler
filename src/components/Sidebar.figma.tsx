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
 * The Figma master's only variant is a presentation `Layout` (Expanded /
 * Collapsed) together with a collapse-toggle affordance the code sidebar does
 * not implement (it always renders the expanded layout), so nothing maps to a
 * code prop and `props` is intentionally omitted. The primitive is driven by
 * its `navItems` + `currentView` + `onNavigate` data seam and its
 * `themePreference` + `onThemeChange` theme seam; the example mirrors the
 * master's expanded content with Home active, and the composed `NavItem`s
 * resolve through their own Code Connect mapping. Consumed by the `figma` CLI,
 * not Vite: excluded from tsconfig.app.json and ESLint so it never enters the
 * app build.
 */
figma.connect(
  Sidebar,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=305-6378",
  {
    example: () => (
      <Sidebar
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
