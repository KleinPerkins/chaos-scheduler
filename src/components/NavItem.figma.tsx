import figma from "@figma/code-connect";
import { Gauge } from "lucide-react";
import NavItem from "./NavItem";

/**
 * Figma Code Connect mapping for the `NavItem` component set
 * (node 50:127, file twQmWC8dWT4tqeqIigNsRy).
 *
 * The Figma "State" variant (Default/Active/Hover) collapses to the `active`
 * boolean: `Active` → true, `Default` → false. "Hover" is a pure CSS `:hover`
 * presentation state (`.sidebar-link:hover`) with no code prop, so it is left
 * unmapped. The "Label" text property maps to the `label` prop; the icon slot
 * takes any node (a lucide-react icon at the call sites), shown here with the
 * Home/`Gauge` icon the sidebar uses. Consumed by the `figma` CLI, not Vite:
 * excluded from tsconfig.app.json and ESLint so it never enters the app build.
 */
figma.connect(
  NavItem,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=50-127",
  {
    props: {
      label: figma.string("Label"),
      active: figma.enum("State", {
        Active: true,
        Default: false,
      }),
    },
    example: ({ label, active }) => (
      <NavItem
        active={active}
        icon={<Gauge size={16} strokeWidth={2} />}
        label={label}
      />
    ),
  },
);
