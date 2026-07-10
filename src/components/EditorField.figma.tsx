import figma from "@figma/code-connect";
import EditorField from "./EditorField";
import Input from "./Input";

/**
 * Figma Code Connect mapping for the `EditorField` component set
 * (node 489:4270, file twQmWC8dWT4tqeqIigNsRy).
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
  EditorField,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=489-4270",
  {
    example: () => (
      <EditorField label="Command" hint="Runs in the workflow shell">
        <Input defaultValue="npm run build" />
      </EditorField>
    ),
  },
);
