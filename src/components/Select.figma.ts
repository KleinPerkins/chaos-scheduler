// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=486-4266
// source=src/components/Select.tsx
// component=Select

import figma from "figma";

export default {
  id: "Select",
  imports: [
    "import Select from \"./Select\""
  ],
  example: figma.code`
    <Select defaultValue="cron">
      <option value="cron">Cron</option>
      <option value="interval">Interval</option>
    </Select>
  `,
  metadata: { nestable: true },
};
