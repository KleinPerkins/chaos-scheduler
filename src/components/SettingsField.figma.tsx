import figma from "@figma/code-connect";
import SettingsField from "./SettingsField";
import Input from "./Input";

/**
 * Figma Code Connect mapping for the `SettingsField` component set
 * (node 488:4268, file twQmWC8dWT4tqeqIigNsRy).
 *
 * The master's only property is a presentation `Hint` variant (With / Without)
 * that merely mirrors whether the optional `hint` is present — not a code prop
 * of its own — so it is intentionally not mapped. The code-only `label` /
 * `hint` / `children` props are shown directly in the example (the composed
 * `Input` resolves through its own Code Connect mapping) and `props` is omitted.
 * Consumed by the `figma` CLI, not Vite: excluded from tsconfig.app.json and
 * ESLint so it never enters the app build.
 */
figma.connect(
  SettingsField,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=488-4268",
  {
    example: () => (
      <SettingsField label="API base URL" hint="Used for outbound webhooks">
        <Input defaultValue="https://api.example.com" />
      </SettingsField>
    ),
  },
);
