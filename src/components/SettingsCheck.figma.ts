// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=490-4277
// source=src/components/SettingsCheck.tsx
// component=SettingsCheck

import figma from "figma";

const checked = figma.selectedInstance.getEnum("Checked", {
  Checked: true,
  Unchecked: false,
});

const disabled = figma.selectedInstance.getEnum("Disabled", {
  Yes: true,
  No: false,
});

export default {
  id: "SettingsCheck",
  imports: [
    "import SettingsCheck from \"./SettingsCheck\""
  ],
  example: figma.code`
    <SettingsCheck
      label="Enable notifications"
      ${figma.helpers.react.renderProp("checked", checked)}
      ${figma.helpers.react.renderProp("disabled", disabled)}
      onChange={() => {}}
    />
  `,
  metadata: { nestable: true },
};
