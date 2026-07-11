import figma from "@figma/code-connect";
import ImpactBars from "./ImpactBars";

/**
 * Figma Code Connect mapping for the `ImpactBars` chart (node 522:4262, file
 * twQmWC8dWT4tqeqIigNsRy).
 *
 * The master renders a single ranked state, so nothing maps to a code prop and
 * `props` is intentionally omitted — the example mirrors the master's three
 * ranked rows with preformatted duration labels. Consumed by the `figma` CLI,
 * not Vite: excluded from tsconfig.app.json and ESLint so it never enters the
 * app build.
 */
figma.connect(
  ImpactBars,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=522-4262",
  {
    example: () => (
      <ImpactBars
        items={[
          { label: "Resource lock", value: 15120, valueLabel: "4h 12m" },
          { label: "Upstream dep", value: 10080, valueLabel: "2h 48m" },
          { label: "Host pool", value: 5400, valueLabel: "1h 30m" },
        ]}
      />
    ),
  },
);
