import type { Environment } from "../lib/commands";
import Select, { type SelectProps } from "./Select";

export interface EnvSelectProps extends Omit<SelectProps, "children"> {
  /**
   * The environments to list as `<option>`s. Each renders as a capitalized
   * label (`production` → `Production`) with the raw `name` as its value.
   */
  environments: readonly Environment[];
  /**
   * When true, prepend an `<option value="all">All</option>` sentinel used by
   * filter surfaces (e.g. the Global History environment filter).
   */
  includeAllOption?: boolean;
}

/**
 * Environment selector primitive. Composes the generic class-less `<Select>`
 * (see `Select.tsx` / DESIGN-SYSTEM.md) to render the user-managed environments
 * as capitalized `<option>`s — byte-identical to the previous inline
 * `environments.map(...)` markup, so styling is unchanged (it inherits the
 * global `input, select, textarea` styling plus contextual parent selectors).
 * `includeAllOption` prepends the `All` sentinel used by filter contexts. The
 * surrounding `<label>` / wrapper / hint stay at the call site. Maps to the
 * Figma `EnvSelect` master (node 121:540) via Code Connect.
 */
export default function EnvSelect({
  environments,
  includeAllOption = false,
  ...rest
}: EnvSelectProps) {
  return (
    <Select {...rest}>
      {includeAllOption ? <option value="all">All</option> : null}
      {environments.map((env) => (
        <option key={env.id} value={env.name}>
          {env.name.charAt(0).toUpperCase() + env.name.slice(1)}
        </option>
      ))}
    </Select>
  );
}
