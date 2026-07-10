import figma from "@figma/code-connect";
import LookbackSelect from "./LookbackSelect";

/**
 * Figma Code Connect mapping for the `LookbackSelect` component set
 * (node 121:585, file twQmWC8dWT4tqeqIigNsRy).
 *
 * The master's `Look` variant (1d / 3d / 7d / 30d) is the *selected* window,
 * which maps to the controlled `value` prop. The `onChange` handler and the
 * `options` / `includeCustom` data seams have no Figma equivalent, so the
 * example supplies a no-op handler and relies on the defaults (which include
 * the trailing `Custom` segment the master always renders). Consumed by the
 * `figma` CLI, not Vite: excluded from tsconfig.app.json and ESLint so it never
 * enters the app build.
 */
figma.connect(
  LookbackSelect,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=121-585",
  {
    props: {
      value: figma.enum("Look", {
        "1d": "1d",
        "3d": "3d",
        "7d": "7d",
        "30d": "30d",
      }),
    },
    example: ({ value }) => (
      <LookbackSelect value={value} onChange={() => {}} />
    ),
  },
);
