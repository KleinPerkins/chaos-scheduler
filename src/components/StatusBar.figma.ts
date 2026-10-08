// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=60-145
// source=src/components/StatusBar.tsx
// component=StatusBar

import figma from "figma";

export default {
  id: "StatusBar",
  imports: [
    "import StatusBar from \"./StatusBar\""
  ],
  example: figma.code`
    <StatusBar
      segments={[
        { status: "succeeded", label: "Succeeded", count: 210 },
        { status: "running", label: "Running", count: 24 },
        { status: "failed", label: "Failed", count: 24 },
      ]}
    />
  `,
  metadata: { nestable: true },
};
