import figma from "@figma/code-connect";
import ScheduleBuilder from "./ScheduleBuilder";

/**
 * Figma Code Connect mapping for the `ScheduleBuilder` component set
 * (node 581:4321, file twQmWC8dWT4tqeqIigNsRy).
 *
 * `Value` and `Timezone` correspond to the controlled code props. Figma's
 * Mode and State variants document internal interaction/validation states;
 * they are intentionally omitted because the code component exposes neither
 * as a prop. `onChange` has no Figma equivalent, so the example uses a no-op.
 * Consumed by the Figma CLI, not Vite.
 */
figma.connect(
  ScheduleBuilder,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=581-4321",
  {
    props: {
      value: figma.string("Value"),
      timezone: figma.string("Timezone"),
    },
    example: ({ value, timezone }) => (
      <ScheduleBuilder value={value} timezone={timezone} onChange={() => {}} />
    ),
  },
);
