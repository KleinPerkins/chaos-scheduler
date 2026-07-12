import figma from "@figma/code-connect";
import RaceTrack from "./RaceTrack";

/**
 * Figma Code Connect mapping for the `RaceTrack` master (node 527:4262, file
 * twQmWC8dWT4tqeqIigNsRy).
 *
 * The master depicts a fixed four-lane state rather than variant props (the lane
 * data is a code-only seam), so — like the `Gauge` master — nothing maps to a
 * code prop and the example reproduces the depicted lanes (the risk-scoring
 * racer is overrunning, hence red). Consumed by the `figma` CLI, not Vite:
 * excluded from tsconfig.app.json and ESLint so it never enters the app build.
 */
figma.connect(
  RaceTrack,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=527-4262",
  {
    example: () => (
      <RaceTrack
        jobs={[
          {
            job: "ingest-events",
            elapsedSeconds: 660,
            expectedSeconds: 780,
            color: "teal",
          },
          {
            job: "nightly-etl",
            elapsedSeconds: 360,
            expectedSeconds: 720,
            color: "blue",
          },
          { job: "risk-scoring", elapsedSeconds: 2040, expectedSeconds: 1800 },
          {
            job: "ledger-rollup",
            elapsedSeconds: 1320,
            expectedSeconds: 2880,
            color: "amber",
          },
        ]}
      />
    ),
  },
);
