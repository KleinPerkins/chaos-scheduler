// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=527-4262
// source=src/components/charts/RaceTrack.tsx
// component=RaceTrack

import figma from "figma";

export default {
  id: "RaceTrack",
  imports: [
    "import RaceTrack from \"./RaceTrack\""
  ],
  example: figma.code`
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
  `,
  metadata: { nestable: true },
};
