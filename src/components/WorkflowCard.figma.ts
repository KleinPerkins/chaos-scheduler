// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=579-4320
// source=src/components/WorkflowCard.tsx
// component=WorkflowCard

import figma from "figma";

const name = figma.selectedInstance.getString("Name");

const environment = figma.selectedInstance.getString("Environment");

const schedule = figma.selectedInstance.getString("Schedule");

const description = figma.selectedInstance.getString("Description");

const enabled = figma.selectedInstance.getEnum("State", {
  Enabled: true,
  Disabled: false,
});

const activity = figma.selectedInstance.getEnum("Activity", {
  None: "none",
  Submitting: "submitting",
  Waiting: "waiting",
});

export default {
  id: "WorkflowCard",
  imports: [
    "import WorkflowCard from \"./WorkflowCard\""
  ],
  example: figma.code`
    <WorkflowCard
      ${figma.helpers.react.renderProp("name", name)}
      ${figma.helpers.react.renderProp("environment", environment)}
      ${figma.helpers.react.renderProp("schedule", schedule)}
      ${figma.helpers.react.renderProp("description", description)}
      ${figma.helpers.react.renderProp("enabled", enabled)}
      ${figma.helpers.react.renderProp("activity", activity)}
      onOpen={() => {}}
      onQueue={() => {}}
      onToggleEnabled={() => {}}
      onHistory={() => {}}
      onEdit={() => {}}
      onDelete={() => {}}
    />
  `,
  metadata: { nestable: true },
};
