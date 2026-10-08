// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=489-4270
// source=src/components/EditorField.tsx
// component=EditorField

import figma from "figma";

export default {
  id: "EditorField",
  imports: [
    "import EditorField from \"./EditorField\"",
    "import Input from \"./Input\""
  ],
  example: figma.code`
    <EditorField label="Command" hint="Runs in the workflow shell">
      <Input defaultValue="npm run build" />
    </EditorField>
  `,
  metadata: { nestable: true },
};
