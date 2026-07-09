import figma from "@figma/code-connect";
import StatusBadge from "./StatusBadge";

/**
 * Figma Code Connect mapping for the `StatusPill` component set
 * (node 49:124, file twQmWC8dWT4tqeqIigNsRy).
 *
 * The Figma "Status" variant maps to the raw status token consumed by
 * `StatusBadge` (the `.status-badge.<status>` modifier in index.css). "Warning"
 * maps to `poll_exhausted`, the canonical warning-tier run status. Consumed by
 * the `figma` CLI, not Vite: excluded from tsconfig.app.json and ESLint so it
 * never enters the app build.
 */
figma.connect(
  StatusBadge,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=49-124",
  {
    props: {
      status: figma.enum("Status", {
        Succeeded: "succeeded",
        Running: "running",
        Failed: "failed",
        Warning: "poll_exhausted",
      }),
    },
    example: ({ status }) => (
      <StatusBadge status={status}>{status}</StatusBadge>
    ),
  },
);
