export type SelectProps = React.SelectHTMLAttributes<HTMLSelectElement>;

/**
 * Shared select primitive. A thin, typed wrapper over the native `<select>`
 * element — the Chaos Scheduler form controls are intentionally CLASS-LESS and
 * are styled by the global `input, select, textarea` element selector (see
 * `index.css` / DESIGN-SYSTEM.md) plus contextual parent selectors (e.g.
 * `.sched-row select`, `.action-row select`). To stay byte-identical to the
 * previous raw `<select>` call sites this renders NO `class` attribute when no
 * `className` is passed (not even `class=""`), appends an optional passthrough
 * `className`, and renders its `<option>` children unchanged.
 */
export default function Select({ className, children, ...rest }: SelectProps) {
  const classes = [className].filter(Boolean).join(" ") || undefined;

  return (
    <select {...rest} className={classes}>
      {children}
    </select>
  );
}
