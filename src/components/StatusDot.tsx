export type StatusDotVariant = "status-dot" | "mc-dot";

export interface StatusDotProps extends React.HTMLAttributes<HTMLSpanElement> {
  /** Raw run/task status token; maps to the `.<variant>.<status>` classes in the CSS. */
  status: string;
  /**
   * Base class for the indicator: `status-dot` (default, used in Run Detail /
   * `index.css`) or `mc-dot` (Mission Control). Both are shape-/color-coded by
   * the same status modifier classes.
   */
  variant?: StatusDotVariant;
}

/**
 * Shared status-indicator primitive. A thin, typed wrapper over the global
 * `.status-dot` / `.mc-dot` classes (see `index.css`, `RunDetail.css`,
 * `MissionControl.css` / DESIGN-SYSTEM.md) — it renders the exact same markup
 * call sites used before, so styling is unchanged. The status token is appended
 * as a modifier class. Callers that need the `succeeded`→`success` alias
 * collapse pass a status pre-normalized via `statusKey` (see `lib/runStatus`),
 * matching the sibling `StatusBadge` usage.
 */
export default function StatusDot({
  status,
  variant = "status-dot",
  className,
  ...rest
}: StatusDotProps) {
  const classes = [variant, status, className].filter(Boolean).join(" ");

  return <span {...rest} className={classes} />;
}
