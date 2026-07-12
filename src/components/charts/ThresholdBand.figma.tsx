import figma from "@figma/code-connect";
import ThresholdBand from "./ThresholdBand";

/**
 * Figma Code Connect mapping for the `ThresholdBand` chart primitive
 * (node 513:4262, file twQmWC8dWT4tqeqIigNsRy).
 *
 * The master is a static illustration of tinted zone bands with no variant
 * properties, so nothing maps to a code prop and `props` is intentionally
 * omitted. The code primitive is geometry-driven (`x`/`width`/`y1`/`y2` in SVG
 * user units, resolved by the caller's y-scale); the example shows one band.
 * Consumed by the `figma` CLI, not Vite: excluded from tsconfig.app.json and
 * ESLint so it never enters the app build.
 */
figma.connect(
  ThresholdBand,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=513-4262",
  {
    example: () => (
      <ThresholdBand
        x={0}
        width={320}
        y1={0}
        y2={28}
        color="var(--warning)"
        boundary="bottom"
        label="Near capacity"
      />
    ),
  },
);
