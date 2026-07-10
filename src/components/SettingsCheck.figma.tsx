import figma from "@figma/code-connect";
import SettingsCheck from "./SettingsCheck";

/**
 * Figma Code Connect mapping for the `SettingsCheck` component set
 * (node 490:4277, file twQmWC8dWT4tqeqIigNsRy).
 *
 * The master's `Checked` (Unchecked / Checked) and `Disabled` (No / Yes)
 * variants map to the native checkbox `checked` / `disabled` props; the `label`
 * text is a descendant layer with no component property, so it is shown
 * directly in the example. Consumed by the `figma` CLI, not Vite: excluded from
 * tsconfig.app.json and ESLint so it never enters the app build.
 */
figma.connect(
  SettingsCheck,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=490-4277",
  {
    props: {
      checked: figma.enum("Checked", {
        Checked: true,
        Unchecked: false,
      }),
      disabled: figma.enum("Disabled", {
        Yes: true,
        No: false,
      }),
    },
    example: ({ checked, disabled }) => (
      <SettingsCheck
        label="Enable notifications"
        checked={checked}
        disabled={disabled}
        onChange={() => {}}
      />
    ),
  },
);
