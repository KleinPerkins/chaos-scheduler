// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=487-4257
// source=src/components/Field.tsx
// component=Field

import figma from "figma";

export default {
  id: "Field",
  imports: [
    "import Field from \"./Field\"",
    "import Input from \"./Input\""
  ],
  example: figma.code`
    <Field label="Workflow name">
      <Input placeholder="nightly-refresh" />
    </Field>
  `,
  metadata: { nestable: true },
};
