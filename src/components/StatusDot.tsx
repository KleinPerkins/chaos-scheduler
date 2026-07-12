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
 * `.status-dot` / `.mc-dot` classes in `index.css`. The status token is
 * appended as a modifier class. Dots are decorative by default because call
 * sites pair them with visible status text; an explicit accessible label or
 * native `aria-hidden` override opts into assistive-technology exposure.
 */
export default function StatusDot({
  status,
  variant = "status-dot",
  className,
  "aria-hidden": ariaHidden,
  "aria-label": ariaLabel,
  "aria-labelledby": ariaLabelledBy,
  role,
  ...rest
}: StatusDotProps) {
  const classes = [variant, status, className].filter(Boolean).join(" ");
  const hasAccessibleLabel =
    ariaLabel !== undefined || ariaLabelledBy !== undefined;
  const resolvedAriaHidden = hasAccessibleLabel
    ? undefined
    : (ariaHidden ?? true);

  return (
    <span
      {...rest}
      aria-hidden={resolvedAriaHidden}
      aria-label={ariaLabel}
      aria-labelledby={ariaLabelledBy}
      role={hasAccessibleLabel ? (role ?? "img") : role}
      className={classes}
    />
  );
}
