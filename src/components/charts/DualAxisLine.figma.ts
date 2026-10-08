// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=521-4262
// source=src/components/charts/DualAxisLine.tsx
// component=DualAxisLine

import figma from "figma";

export default {
  id: "DualAxisLine",
  imports: [
    "import DualAxisLine from \"./DualAxisLine\""
  ],
  example: figma.code`
    <DualAxisLine
      categories={["Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]}
      leftSeries={[
        {
          label: "Runtime",
          data: [8, 9, 7, 11, 10, 12],
          color: "var(--chart-2)",
        },
      ]}
      rightSeries={[
        {
          label: "Wait",
          data: [40, 55, 45, 70, 60, 80],
          color: "var(--chart-1)",
        },
      ]}
      baselines={[{ value: 4, label: "Target" }]}
    />
  `,
  metadata: { nestable: true },
};
