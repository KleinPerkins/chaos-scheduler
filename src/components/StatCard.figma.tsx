import figma from "@figma/code-connect";
import StatCard from "./StatCard";

/**
 * Figma Code Connect mapping for the `StatCard` component set
 * (node 53:132, file twQmWC8dWT4tqeqIigNsRy).
 *
 * The Figma master's only property is a presentation `State` variant
 * (Resting/Hover/Expanded) — together with a delta pill and an expanded-state
 * sparkline that have no code equivalent — so nothing maps to a code prop and
 * `props` is intentionally omitted. The code primitive is driven by its
 * `value` + `label` props; the example mirrors the master's resting content.
 * Consumed by the `figma` CLI, not Vite: excluded from tsconfig.app.json and
 * ESLint so it never enters the app build.
 */
figma.connect(
  StatCard,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=53-132",
  {
    example: () => <StatCard value="12" label="Active workflows" />,
  },
);
