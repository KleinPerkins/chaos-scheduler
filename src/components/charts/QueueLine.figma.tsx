import figma from "@figma/code-connect";
import QueueLine from "./QueueLine";

/**
 * Figma Code Connect mapping for the `QueueLine` chart (node 525:4262, file
 * twQmWC8dWT4tqeqIigNsRy).
 *
 * The master renders a single compact state, so nothing maps to a code prop and
 * `props` is intentionally omitted — the example mirrors the master's occupancy
 * lines beneath a near-capacity band + dashed capacity line. Consumed by the
 * `figma` CLI, not Vite: excluded from tsconfig.app.json and ESLint so it never
 * enters the app build.
 */
figma.connect(
  QueueLine,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=525-4262",
  {
    example: () => (
      <QueueLine
        categories={["Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]}
        capacity={85}
        series={[
          {
            label: "default",
            data: [20, 45, 60, 70, 85, 92],
            color: "var(--chart-3)",
          },
          {
            label: "batch",
            data: [60, 65, 70, 75, 80, 82],
            color: "var(--chart-2)",
          },
          {
            label: "priority",
            data: [55, 60, 68, 72, 78, 80],
            color: "var(--chart-1)",
          },
        ]}
      />
    ),
  },
);
