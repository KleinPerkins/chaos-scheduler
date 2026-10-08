// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=559-4262
// source=src/components/charts/Vehicle.tsx
// component=Vehicle

import figma from "figma";

const style = figma.selectedInstance.getEnum("Style", {
  Sedan: "sedan",
  Coupe: "coupe",
  Racer: "racer",
  Truck: "truck",
});

const color = figma.selectedInstance.getEnum("Color", {
  Blue: "blue",
  Teal: "teal",
  Amber: "amber",
});

export default {
  id: "Vehicle",
  imports: [
    "import Vehicle from \"./Vehicle\""
  ],
  example: figma.code`<Vehicle ${figma.helpers.react.renderProp("style", style)} ${figma.helpers.react.renderProp("color", color)} />`,
  metadata: { nestable: true },
};
