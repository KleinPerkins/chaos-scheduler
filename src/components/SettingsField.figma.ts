// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=488-4268
// source=src/components/SettingsField.tsx
// component=SettingsField

import figma from "figma";

export default {
  id: "SettingsField",
  imports: [
    "import SettingsField from \"./SettingsField\"",
    "import Input from \"./Input\""
  ],
  example: figma.code`
    <SettingsField label="API base URL" hint="Used for outbound webhooks">
      <Input defaultValue="https://api.example.com" />
    </SettingsField>
  `,
  metadata: { nestable: true },
};
