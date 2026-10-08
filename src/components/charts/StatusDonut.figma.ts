// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=524-4262
// source=src/components/charts/StatusDonut.tsx
// component=StatusDonut

import figma from "figma";

export default {
  id: "StatusDonut",
  imports: [
    "import StatusDonut from \"./StatusDonut\""
  ],
  example: figma.code`
    <StatusDonut
      centerLabel="runs"
      segments={[
        { label: "Succeeded", value: 1024, color: "var(--success)" },
        { label: "Running", value: 96, color: "var(--running)" },
        { label: "Warning", value: 108, color: "var(--warning)" },
        { label: "Failed", value: 56, color: "var(--error)" },
      ]}
    />
  `,
  metadata: { nestable: true },
};
