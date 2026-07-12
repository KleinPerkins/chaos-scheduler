import figma from "@figma/code-connect";
import WorkflowCard from "./WorkflowCard";

/**
 * Figma Code Connect mapping for the `WorkflowCard` component set
 * (node 579:4320, file twQmWC8dWT4tqeqIigNsRy).
 *
 * Configuration State and observed Activity remain separate props. Interaction
 * handlers, managed ownership, action-busy state, and guarded deletion have no
 * Figma equivalents and use representative values in the example.
 */
figma.connect(
  WorkflowCard,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=579-4320",
  {
    props: {
      name: figma.string("Name"),
      environment: figma.string("Environment"),
      schedule: figma.string("Schedule"),
      description: figma.string("Description"),
      enabled: figma.enum("State", {
        Enabled: true,
        Disabled: false,
      }),
      activity: figma.enum("Activity", {
        None: "none",
        Submitting: "submitting",
        Waiting: "waiting",
      }),
    },
    example: ({
      name,
      environment,
      schedule,
      description,
      enabled,
      activity,
    }) => (
      <WorkflowCard
        name={name}
        environment={environment}
        schedule={schedule}
        description={description}
        enabled={enabled}
        activity={activity}
        onOpen={() => {}}
        onQueue={() => {}}
        onToggleEnabled={() => {}}
        onHistory={() => {}}
        onEdit={() => {}}
        onDelete={() => {}}
      />
    ),
  },
);
