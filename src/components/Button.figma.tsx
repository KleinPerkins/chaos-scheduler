import figma from "@figma/code-connect";
import Button from "./Button";

/**
 * Figma Code Connect mapping for the `ActionButton` component set
 * (node 113:526, file twQmWC8dWT4tqeqIigNsRy).
 *
 * The Figma "Style" variant property conflates color variants
 * (Neutral/Primary/Ghost) with two states (Disabled/Running); the states map to
 * the `disabled` and `loading` props. Consumed by the `figma` CLI, not Vite:
 * excluded from tsconfig.app.json and ESLint so it never enters the app build.
 */
figma.connect(
  Button,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=113-526",
  {
    props: {
      variant: figma.enum("Style", {
        Neutral: "neutral",
        Primary: "primary",
        Ghost: "ghost",
      }),
      disabled: figma.enum("Style", {
        Disabled: true,
      }),
      loading: figma.enum("Style", {
        Running: true,
      }),
    },
    example: ({ variant, disabled, loading }) => (
      <Button variant={variant} disabled={disabled} loading={loading}>
        Button
      </Button>
    ),
  },
);
