import figma from "@figma/code-connect";
import ChartTooltip from "./ChartTooltip";

/**
 * Figma Code Connect mapping for the `ChartTooltip` chart primitive
 * (node 520:4262, file twQmWC8dWT4tqeqIigNsRy).
 *
 * The master is a static tooltip panel with no variant properties, so nothing
 * maps to a code prop and `props` is intentionally omitted. The example mirrors
 * the master's header + two status rows. Consumed by the `figma` CLI, not Vite:
 * excluded from tsconfig.app.json and ESLint so it never enters the app build.
 */
figma.connect(
  ChartTooltip,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=520-4262",
  {
    example: () => (
      <ChartTooltip
        header="Jul 10 · 14:00"
        rows={[
          { label: "Succeeded", value: 128, color: "var(--success)" },
          { label: "Failed", value: 12, color: "var(--error)" },
        ]}
      />
    ),
  },
);
