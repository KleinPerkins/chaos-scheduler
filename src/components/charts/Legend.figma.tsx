import figma from "@figma/code-connect";
import Legend from "./Legend";

/**
 * Figma Code Connect mapping for the `Legend` chart primitive
 * (node 518:4262, file twQmWC8dWT4tqeqIigNsRy).
 *
 * The master is a static row of color keys with no variant properties, so
 * nothing maps to a code prop and `props` is intentionally omitted. The example
 * mirrors the master's four categorical keys. Consumed by the `figma` CLI, not
 * Vite: excluded from tsconfig.app.json and ESLint so it never enters the app
 * build.
 */
figma.connect(
  Legend,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=518-4262",
  {
    example: () => (
      <Legend
        items={[
          { label: "Deploys", color: "var(--chart-1)" },
          { label: "ETL", color: "var(--chart-2)" },
          { label: "Syncs", color: "var(--chart-3)" },
          { label: "Reports", color: "var(--chart-4)" },
        ]}
      />
    ),
  },
);
