// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=491-4290
// source=src/components/PageHeader.tsx
// component=PageHeader

import figma from "figma";

export default {
  id: "PageHeader",
  imports: [
    "import PageHeader from \"./PageHeader\"",
    "import Button from \"./Button\""
  ],
  example: figma.code`
    <PageHeader
      title="Workflows"
      subtitle="Schedule and monitor recurring jobs"
      actions={<Button>New workflow</Button>}
    />
  `,
  metadata: { nestable: true },
};
