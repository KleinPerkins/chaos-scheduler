import figma from "@figma/code-connect";
import Field from "./Field";
import Input from "./Input";

/**
 * Figma Code Connect mapping for the `Field` component set
 * (node 487:4257, file twQmWC8dWT4tqeqIigNsRy).
 *
 * The master has no variant properties — its `label` text and control are
 * descendant layers, not component properties — so the code-only `label` /
 * `children` props are represented directly in the example (the composed
 * `Input` resolves through its own Code Connect mapping) and `props` is
 * intentionally omitted. Consumed by the `figma` CLI, not Vite: excluded from
 * tsconfig.app.json and ESLint so it never enters the app build.
 */
figma.connect(
  Field,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=487-4257",
  {
    example: () => (
      <Field label="Workflow name">
        <Input placeholder="nightly-refresh" />
      </Field>
    ),
  },
);
