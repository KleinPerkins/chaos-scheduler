// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=518-4262
// source=src/components/charts/Legend.tsx
// component=Legend

import figma from "figma";

export default {
  id: "Legend",
  imports: [
    "import Legend from \"./Legend\""
  ],
  example: figma.code`
    <Legend
      items={[
        { label: "Deploys", color: "var(--chart-1)" },
        { label: "ETL", color: "var(--chart-2)" },
        { label: "Syncs", color: "var(--chart-3)" },
        { label: "Reports", color: "var(--chart-4)" },
      ]}
    />
  `,
  metadata: { nestable: true },
};
