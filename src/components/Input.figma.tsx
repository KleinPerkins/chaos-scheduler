import figma from "@figma/code-connect";
import Input from "./Input";

/**
 * Figma Code Connect mapping for the `Input` component set
 * (node 481:4257, file twQmWC8dWT4tqeqIigNsRy).
 *
 * The master's only property is a presentation `State` variant
 * (Default / Focus / Error / Disabled) — design-forward visual states with no
 * code prop (the primitive is a thin, class-less passthrough over native
 * `<input>` attributes), so nothing maps to a code prop and `props` is
 * intentionally omitted. The example shows representative native usage.
 * Consumed by the `figma` CLI, not Vite: excluded from tsconfig.app.json and
 * ESLint so it never enters the app build.
 */
figma.connect(
  Input,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=481-4257",
  {
    example: () => <Input placeholder="nightly-refresh" />,
  },
);
