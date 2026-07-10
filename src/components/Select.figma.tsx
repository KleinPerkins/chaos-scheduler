import figma from "@figma/code-connect";
import Select from "./Select";

/**
 * Figma Code Connect mapping for the `Select` component set
 * (node 486:4266, file twQmWC8dWT4tqeqIigNsRy).
 *
 * The master's only property is a presentation `State` variant
 * (Default / Focus / Disabled) — visual states with no code prop (the primitive
 * is a thin, class-less passthrough over native `<select>` attributes that
 * renders its `<option>` children unchanged), so nothing maps to a code prop
 * and `props` is intentionally omitted. Consumed by the `figma` CLI, not Vite:
 * excluded from tsconfig.app.json and ESLint so it never enters the app build.
 */
figma.connect(
  Select,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=486-4266",
  {
    example: () => (
      <Select defaultValue="cron">
        <option value="cron">Cron</option>
        <option value="interval">Interval</option>
      </Select>
    ),
  },
);
