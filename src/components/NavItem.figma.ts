// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=50-127
// source=src/components/NavItem.tsx
// component=NavItem

import figma from "figma";

const label = figma.selectedInstance.getString("Label");

const active = figma.selectedInstance.getEnum("State", {
  Active: true,
  Default: false,
});

export default {
  id: "NavItem",
  imports: [
    "import { Gauge } from \"lucide-react\"",
    "import NavItem from \"./NavItem\""
  ],
  example: figma.code`
    <NavItem
      ${figma.helpers.react.renderProp("active", active)}
      icon={<Gauge size={16} strokeWidth={2} />}
      ${figma.helpers.react.renderProp("label", label)}
    />
  `,
  metadata: { nestable: true },
};
