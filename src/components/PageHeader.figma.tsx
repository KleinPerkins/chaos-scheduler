import figma from "@figma/code-connect";
import PageHeader from "./PageHeader";
import Button from "./Button";

/**
 * Figma Code Connect mapping for the `PageHeader` component set
 * (node 491:4290, file twQmWC8dWT4tqeqIigNsRy).
 *
 * The master's `Subtitle` / `Actions` variants (With / Without) merely toggle
 * the presence of the optional `subtitle` / `actions` content — they are not
 * code props themselves — so they are not mapped. The code-only `title` /
 * `subtitle` / `actions` props are shown directly in the example (the composed
 * `Button` resolves through its own Code Connect mapping) and `props` is
 * omitted. Consumed by the `figma` CLI, not Vite: excluded from
 * tsconfig.app.json and ESLint so it never enters the app build.
 */
figma.connect(
  PageHeader,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=491-4290",
  {
    example: () => (
      <PageHeader
        title="Workflows"
        subtitle="Schedule and monitor recurring jobs"
        actions={<Button>New workflow</Button>}
      />
    ),
  },
);
