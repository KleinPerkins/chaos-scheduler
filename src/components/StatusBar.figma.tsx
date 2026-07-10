import figma from "@figma/code-connect";
import StatusBar from "./StatusBar";

/**
 * Figma Code Connect mapping for the `StatusBar` component set
 * (node 60:145, file twQmWC8dWT4tqeqIigNsRy).
 *
 * The master is a static composition with no variant properties — its segment
 * widths and legend rows are fixed design content, not component properties —
 * so nothing maps to a code prop and `props` is intentionally omitted. The
 * code-only `segments` data seam is shown directly in the example, mirroring the
 * master's Succeeded / Running / Failed distribution. Consumed by the `figma`
 * CLI, not Vite: excluded from tsconfig.app.json and ESLint so it never enters
 * the app build.
 */
figma.connect(
  StatusBar,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=60-145",
  {
    example: () => (
      <StatusBar
        segments={[
          { status: "succeeded", label: "Succeeded", count: 210 },
          { status: "running", label: "Running", count: 24 },
          { status: "failed", label: "Failed", count: 24 },
        ]}
      />
    ),
  },
);
