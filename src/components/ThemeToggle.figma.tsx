import figma from "@figma/code-connect";
import ThemeToggle from "./ThemeToggle";

/**
 * Figma Code Connect mapping for the `ThemeToggle` component set
 * (node 90:439, file twQmWC8dWT4tqeqIigNsRy).
 *
 * Consumed by the `figma` CLI, not Vite: excluded from tsconfig.app.json and
 * ESLint so it never enters the app build. `onChange` has no Figma equivalent,
 * hence the no-op handler in the example.
 */
figma.connect(
  ThemeToggle,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=90-439",
  {
    props: {
      preference: figma.enum("Selected", {
        Dark: "dark",
        System: "system",
        Light: "light",
      }),
    },
    example: ({ preference }) => (
      <ThemeToggle preference={preference} onChange={() => {}} />
    ),
  },
);
