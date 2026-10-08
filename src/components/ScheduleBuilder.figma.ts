// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=581-4321
// source=src/components/ScheduleBuilder.tsx
// component=ScheduleBuilder

import figma from "figma";

const value = figma.selectedInstance.getString("Value");

const timezone = figma.selectedInstance.getString("Timezone");

export default {
  id: "ScheduleBuilder",
  imports: [
    "import ScheduleBuilder from \"./ScheduleBuilder\""
  ],
  example: figma.code`<ScheduleBuilder ${figma.helpers.react.renderProp("value", value)} ${figma.helpers.react.renderProp("timezone", timezone)} onChange={() => {}} />`,
  metadata: { nestable: true },
};
