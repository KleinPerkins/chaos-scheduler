// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=513-4262
// source=src/components/charts/ThresholdBand.tsx
// component=ThresholdBand

import figma from "figma";

export default {
  id: "ThresholdBand",
  imports: [
    "import ThresholdBand from \"./ThresholdBand\""
  ],
  example: figma.code`
    <ThresholdBand
      x={0}
      width={320}
      y1={0}
      y2={28}
      color="var(--warning)"
      boundary="bottom"
      label="Near capacity"
    />
  `,
  metadata: { nestable: true },
};
