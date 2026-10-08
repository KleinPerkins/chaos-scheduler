// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=90-439
// source=src/components/ThemeToggle.tsx
// component=ThemeToggle

import figma from "figma";

const preference = figma.selectedInstance.getEnum("Selected", {
  Dark: "dark",
  System: "system",
  Light: "light",
});

export default {
  id: "ThemeToggle",
  imports: [
    "import ThemeToggle from \"./ThemeToggle\""
  ],
  example: figma.code`<ThemeToggle ${figma.helpers.react.renderProp("preference", preference)} onChange={() => {}} />`,
  metadata: { nestable: true },
};
