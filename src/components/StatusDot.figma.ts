// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=479-4257
// source=src/components/StatusDot.tsx
// component=StatusDot

import figma from "figma";

const variant = figma.selectedInstance.getEnum("Base", {
  "status-dot": "status-dot",
  "mc-dot": "mc-dot",
});

const status = figma.selectedInstance.getEnum("Status", {
  Succeeded: "succeeded",
  Running: "running",
  Failed: "failed",
  Queued: "queued",
  Warning: "poll_exhausted",
});

export default {
  id: "StatusDot",
  imports: [
    "import StatusDot from \"./StatusDot\""
  ],
  example: figma.code`<StatusDot ${figma.helpers.react.renderProp("variant", variant)} ${figma.helpers.react.renderProp("status", status)} />`,
  metadata: { nestable: true },
};
