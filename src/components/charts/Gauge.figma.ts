// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=516-4262
// source=src/components/charts/Gauge.tsx
// component=Gauge

import figma from "figma";

export default {
  id: "Gauge",
  imports: [
    "import Gauge from \"./Gauge\""
  ],
  example: figma.code`<Gauge value={5} max={8} unit="slots" />`,
  metadata: { nestable: true },
};
