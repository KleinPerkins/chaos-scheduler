// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=121-540
// source=src/components/EnvSelect.tsx
// component=EnvSelect

import figma from "figma";

const value = figma.selectedInstance.getEnum("Env", {
  Production: "production",
  Sandbox: "sandbox",
});

export default {
  id: "EnvSelect",
  imports: [
    "import EnvSelect from \"./EnvSelect\""
  ],
  example: figma.code`
    <EnvSelect
      ${figma.helpers.react.renderProp("value", value)}
      onChange={() => {}}
      environments={[
        { id: "production", name: "production" },
        { id: "sandbox", name: "sandbox" },
      ]}
    />
  `,
  metadata: { nestable: true },
};
