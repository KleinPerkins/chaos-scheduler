// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=519-4262
// source=src/components/charts/Axis.tsx
// component=Axis

import figma from "figma";

export default {
  id: "Axis",
  imports: [
    "import Axis from \"./Axis\""
  ],
  example: figma.code`
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
  `,
  metadata: { nestable: true },
};
