import figma from "@figma/code-connect";
import EnvSelect from "./EnvSelect";

/**
 * Figma Code Connect mapping for the `EnvSelect` component set
 * (node 121:540, file twQmWC8dWT4tqeqIigNsRy).
 *
 * The Figma master's `Env` variant (Production / Sandbox) is the *selected*
 * environment, which maps to the controlled `value` prop — lowercased to match
 * the environment `name`s the app uses. The `environments` data seam and the
 * `onChange` handler have no Figma equivalent, so the example supplies the two
 * environments the master depicts plus a no-op handler. Consumed by the `figma`
 * CLI, not Vite: excluded from tsconfig.app.json and ESLint so it never enters
 * the app build.
 */
figma.connect(
  EnvSelect,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=121-540",
  {
    props: {
      value: figma.enum("Env", {
        Production: "production",
        Sandbox: "sandbox",
      }),
    },
    example: ({ value }) => (
      <EnvSelect
        value={value}
        onChange={() => {}}
        environments={[
          { id: "production", name: "production" },
          { id: "sandbox", name: "sandbox" },
        ]}
      />
    ),
  },
);
