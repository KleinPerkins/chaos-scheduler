export interface StatusBadgeProps extends React.HTMLAttributes<HTMLSpanElement> {
  /** Raw run/task status token; maps to the `.status-badge.<status>` classes in `index.css`. */
  status: string;
}

/**
 * Shared status pill primitive. A thin, typed wrapper over the global
 * `.status-badge` classes (see `index.css` / DESIGN-SYSTEM.md) — it renders the
 * exact same markup call sites used before, so styling (including the `::before`
 * status dot) is unchanged. The status token is appended as a modifier class;
 * callers pass the human label as children.
 */
export default function StatusBadge({
  status,
  className,
  children,
  ...rest
}: StatusBadgeProps) {
  const classes = ["status-badge", status, className].filter(Boolean).join(" ");

  return (
    <span {...rest} className={classes}>
      {children}
    </span>
  );
}
