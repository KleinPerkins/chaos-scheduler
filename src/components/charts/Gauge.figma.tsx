import figma from "@figma/code-connect";
import Gauge from "./Gauge";

/**
 * Figma Code Connect mapping for the `Gauge` chart (node 516:4262, file
 * twQmWC8dWT4tqeqIigNsRy).
 *
 * The master renders a single 270° arc state, so nothing maps to a code prop and
 * `props` is intentionally omitted — the example mirrors the master's "63% / 5 of
 * 8 slots" state (5/8 = 62.5% → green, below the 70% warning threshold). Consumed
 * by the `figma` CLI, not Vite: excluded from tsconfig.app.json and ESLint so it
 * never enters the app build.
 */
figma.connect(
  Gauge,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=516-4262",
  {
    example: () => <Gauge value={5} max={8} unit="slots" />,
  },
);
