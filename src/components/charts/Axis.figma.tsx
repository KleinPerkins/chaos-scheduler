import figma from "@figma/code-connect";
import Axis from "./Axis";

/**
 * Figma Code Connect mapping for the `Axis` chart primitive
 * (node 519:4262, file twQmWC8dWT4tqeqIigNsRy).
 *
 * The master is a static tick-label strip with no variant properties, so
 * nothing maps to a code prop and `props` is intentionally omitted. The example
 * mirrors the master's `0 / 6h / 12h / 18h / 24h` bottom axis via explicit
 * ticks. Consumed by the `figma` CLI, not Vite: excluded from tsconfig.app.json
 * and ESLint so it never enters the app build.
 */
figma.connect(
  Axis,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=519-4262",
  {
    example: () => (
      <Axis
        orientation="bottom"
        length={320}
        ticks={[
          { offset: 0, label: "0" },
          { offset: 80, label: "6h" },
          { offset: 160, label: "12h" },
          { offset: 240, label: "18h" },
          { offset: 320, label: "24h" },
        ]}
      />
    ),
  },
);
