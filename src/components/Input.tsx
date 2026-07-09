export type InputProps = React.InputHTMLAttributes<HTMLInputElement>;

/**
 * Shared text-input primitive. A thin, typed wrapper over the native `<input>`
 * element — the Chaos Scheduler form controls are intentionally CLASS-LESS and
 * are styled by the global `input, select, textarea` element selector (see
 * `index.css` / DESIGN-SYSTEM.md) plus contextual parent selectors (e.g.
 * `.queue-fields input`). To stay byte-identical to the previous raw `<input>`
 * call sites this renders NO `class` attribute when no `className` is passed
 * (not even `class=""`), and appends an optional passthrough `className`.
 */
export default function Input({ className, ...rest }: InputProps) {
  const classes = [className].filter(Boolean).join(" ") || undefined;

  return <input {...rest} className={classes} />;
}
