// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=522-4262
// source=src/components/charts/ImpactBars.tsx
// component=ImpactBars

import figma from "figma";

export default {
  id: "ImpactBars",
  imports: [
    "import ImpactBars from \"./ImpactBars\""
  ],
  example: figma.code`
    <ImpactBars
      items={[
        { label: "Resource lock", value: 15120, valueLabel: "4h 12m" },
        { label: "Upstream dep", value: 10080, valueLabel: "2h 48m" },
        { label: "Host pool", value: 5400, valueLabel: "1h 30m" },
      ]}
    />
  `,
  metadata: { nestable: true },
};
