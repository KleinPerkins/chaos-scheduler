// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=113-526
// source=src/components/Button.tsx
// component=Button

import figma from "figma";

const variant = figma.selectedInstance.getEnum("Style", {
  Neutral: "neutral",
  Primary: "primary",
  Ghost: "ghost",
});

const disabled = figma.selectedInstance.getEnum("Style", {
  Disabled: true,
});

const loading = figma.selectedInstance.getEnum("Style", {
  Running: true,
});

export default {
  id: "Button",
  imports: [
    "import Button from \"./Button\""
  ],
  example: figma.code`
    <Button ${figma.helpers.react.renderProp("variant", variant)} ${figma.helpers.react.renderProp("disabled", disabled)} ${figma.helpers.react.renderProp("loading", loading)}>
      Button
    </Button>
  `,
  metadata: { nestable: true },
};
