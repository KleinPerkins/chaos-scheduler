import figma from "@figma/code-connect";
import Vehicle from "./Vehicle";

/**
 * Figma Code Connect mapping for the `Vehicle` component set
 * (node 559:4262, file twQmWC8dWT4tqeqIigNsRy).
 *
 * The set's two variant properties map straight to the code props: `Style`
 * (Sedan/Coupe/Racer/Truck) → `style` and `Color` (Blue/Teal/Amber) → `color`.
 * The `over` red override is a contextual state on the race view (not a variant
 * of this set), so it is intentionally not mapped. Consumed by the `figma` CLI,
 * not Vite: excluded from tsconfig.app.json and ESLint so it never enters the
 * app build.
 */
figma.connect(
  Vehicle,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=559-4262",
  {
    props: {
      style: figma.enum("Style", {
        Sedan: "sedan",
        Coupe: "coupe",
        Racer: "racer",
        Truck: "truck",
      }),
      color: figma.enum("Color", {
        Blue: "blue",
        Teal: "teal",
        Amber: "amber",
      }),
    },
    example: ({ style, color }) => <Vehicle style={style} color={color} />,
  },
);
