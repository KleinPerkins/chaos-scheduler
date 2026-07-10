import figma from "@figma/code-connect";
import BrandMark from "./BrandMark";

/**
 * Figma Code Connect mapping for the `BrandMark` component
 * (node 186:1241, file twQmWC8dWT4tqeqIigNsRy).
 *
 * The master is a single static glyph with no variant properties, so nothing
 * maps to a code prop and `props` is intentionally omitted. The `size` / `title`
 * code props are ergonomic seams with no Figma equivalent; the example renders
 * the mark at its master size. Consumed by the `figma` CLI, not Vite: excluded
 * from tsconfig.app.json and ESLint so it never enters the app build.
 */
figma.connect(
  BrandMark,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=186-1241",
  {
    example: () => <BrandMark size={30} />,
  },
);
