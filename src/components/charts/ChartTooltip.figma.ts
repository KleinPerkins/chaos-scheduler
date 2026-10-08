// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=520-4262
// source=src/components/charts/ChartTooltip.tsx
// component=ChartTooltip

import figma from "figma";

export default {
  id: "ChartTooltip",
  imports: [
    "import ChartTooltip from \"./ChartTooltip\""
  ],
  example: figma.code`
    <ChartTooltip
      header="Jul 10 · 14:00"
      rows={[
        { label: "Succeeded", value: 128, color: "var(--success)" },
        { label: "Failed", value: 12, color: "var(--error)" },
      ]}
    />
  `,
  metadata: { nestable: true },
};
