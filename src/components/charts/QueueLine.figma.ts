// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=525-4262
// source=src/components/charts/QueueLine.tsx
// component=QueueLine

import figma from "figma";

export default {
  id: "QueueLine",
  imports: [
    "import QueueLine from \"./QueueLine\""
  ],
  example: figma.code`
    <QueueLine
      categories={["Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]}
      capacity={85}
      series={[
        {
          label: "default",
          data: [20, 45, 60, 70, 85, 92],
          color: "var(--chart-3)",
        },
        {
          label: "batch",
          data: [60, 65, 70, 75, 80, 82],
          color: "var(--chart-2)",
        },
        {
          label: "priority",
          data: [55, 60, 68, 72, 78, 80],
          color: "var(--chart-1)",
        },
      ]}
    />
  `,
  metadata: { nestable: true },
};
