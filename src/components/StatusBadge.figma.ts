// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=49-124
// source=src/components/StatusBadge.tsx
// component=StatusBadge

import figma from "figma";

const status = figma.selectedInstance.getEnum("Status", {
  Succeeded: "succeeded",
  Running: "running",
  Failed: "failed",
  Warning: "poll_exhausted",
});

export default {
  id: "StatusBadge",
  imports: [
    "import StatusBadge from \"./StatusBadge\""
  ],
  example: figma.code`<StatusBadge ${figma.helpers.react.renderProp("status", status)}>${figma.helpers.react.renderChildren(status)}</StatusBadge>`,
  metadata: { nestable: true },
};
