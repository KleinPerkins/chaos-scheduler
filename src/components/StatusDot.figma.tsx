import figma from "@figma/code-connect";
import StatusDot from "./StatusDot";

/**
 * Figma Code Connect mapping for the `StatusDot` component set
 * (node 479:4257, file twQmWC8dWT4tqeqIigNsRy).
 *
 * The self-contained master exposes two variant properties that map 1:1 to the
 * code props: `Base` → `variant` (the `status-dot` / `mc-dot` indicator class)
 * and `Status` → the raw status token appended as the `.<variant>.<status>`
 * modifier class (matching the sibling `StatusBadge`; "Warning" → the canonical
 * `poll_exhausted` warning tier). Consumed by the `figma` CLI, not Vite:
 * excluded from tsconfig.app.json and ESLint so it never enters the app build.
 */
figma.connect(
  StatusDot,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=479-4257",
  {
    props: {
      variant: figma.enum("Base", {
        "status-dot": "status-dot",
        "mc-dot": "mc-dot",
      }),
      status: figma.enum("Status", {
        Succeeded: "succeeded",
        Running: "running",
        Failed: "failed",
        Queued: "queued",
        Warning: "poll_exhausted",
      }),
    },
    example: ({ variant, status }) => (
      <StatusDot variant={variant} status={status} />
    ),
  },
);
