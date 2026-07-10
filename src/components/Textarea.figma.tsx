import figma from "@figma/code-connect";
import Textarea from "./Textarea";

/**
 * Figma Code Connect mapping for the `Textarea` component set
 * (node 485:4265, file twQmWC8dWT4tqeqIigNsRy).
 *
 * The master's only property is a presentation `State` variant
 * (Default / Focus / Error / Disabled) — visual states with no code prop (the
 * primitive is a thin, class-less passthrough over native `<textarea>`
 * attributes; content is `value` / `defaultValue`, never children), so nothing
 * maps to a code prop and `props` is intentionally omitted. Consumed by the
 * `figma` CLI, not Vite: excluded from tsconfig.app.json and ESLint so it never
 * enters the app build.
 */
figma.connect(
  Textarea,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=485-4265",
  {
    example: () => <Textarea rows={4} placeholder="Notes for this run" />,
  },
);
