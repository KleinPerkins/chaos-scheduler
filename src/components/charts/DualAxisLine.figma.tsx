import figma from "@figma/code-connect";
import DualAxisLine from "./DualAxisLine";

/**
 * Figma Code Connect mapping for the `DualAxisLine` chart (node 521:4262, file
 * twQmWC8dWT4tqeqIigNsRy).
 *
 * The master renders a single compact state, so nothing maps to a code prop and
 * `props` is intentionally omitted — the example mirrors the master's two trend
 * lines (left + right scale) over a dashed baseline. Consumed by the `figma`
 * CLI, not Vite: excluded from tsconfig.app.json and ESLint so it never enters
 * the app build.
 */
figma.connect(
  DualAxisLine,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=521-4262",
  {
    example: () => (
      <DualAxisLine
        categories={["Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]}
        leftSeries={[
          {
            label: "Runtime",
            data: [8, 9, 7, 11, 10, 12],
            color: "var(--chart-2)",
          },
        ]}
        rightSeries={[
          {
            label: "Wait",
            data: [40, 55, 45, 70, 60, 80],
            color: "var(--chart-1)",
          },
        ]}
        baselines={[{ value: 4, label: "Target" }]}
      />
    ),
  },
);
