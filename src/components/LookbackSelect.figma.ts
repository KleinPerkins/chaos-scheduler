// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=121-585
// source=src/components/LookbackSelect.tsx
// component=LookbackSelect

import figma from "figma";

const value = figma.selectedInstance.getEnum("Look", {
  "1d": "1d",
  "3d": "3d",
  "7d": "7d",
  "30d": "30d",
});

export default {
  id: "LookbackSelect",
  imports: [
    "import LookbackSelect from \"./LookbackSelect\""
  ],
  example: figma.code`<LookbackSelect ${figma.helpers.react.renderProp("value", value)} onChange={() => {}} />`,
  metadata: { nestable: true },
};
