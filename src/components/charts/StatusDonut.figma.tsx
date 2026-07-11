import figma from "@figma/code-connect";
import StatusDonut from "./StatusDonut";

/**
 * Figma Code Connect mapping for the `StatusDonut` chart (node 524:4262, file
 * twQmWC8dWT4tqeqIigNsRy).
 *
 * The master renders a single distribution state, so nothing maps to a code prop
 * and `props` is intentionally omitted — the example mirrors the master's four
 * status segments summing to the "1,284 runs" center total. Consumed by the
 * `figma` CLI, not Vite: excluded from tsconfig.app.json and ESLint so it never
 * enters the app build.
 */
figma.connect(
  StatusDonut,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=524-4262",
  {
    example: () => (
      <StatusDonut
        centerLabel="runs"
        segments={[
          { label: "Succeeded", value: 1024, color: "var(--success)" },
          { label: "Running", value: 96, color: "var(--running)" },
          { label: "Warning", value: 108, color: "var(--warning)" },
          { label: "Failed", value: 56, color: "var(--error)" },
        ]}
      />
    ),
  },
);
