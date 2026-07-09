export type TextareaProps = React.TextareaHTMLAttributes<HTMLTextAreaElement>;

/**
 * Shared multiline text-input primitive. A thin, typed wrapper over the native
 * `<textarea>` element — the Chaos Scheduler form controls are intentionally
 * CLASS-LESS and are styled by the global `input, select, textarea` element
 * selector (see `index.css` / DESIGN-SYSTEM.md) plus contextual parent
 * selectors (e.g. `.editor-field textarea`). To stay byte-identical to the
 * previous raw `<textarea>` call sites this renders NO `class` attribute when
 * no `className` is passed (not even `class=""`), and appends an optional
 * passthrough `className`. Content is provided via `value`/`defaultValue`
 * (native textarea props), never children.
 */
export default function Textarea({ className, ...rest }: TextareaProps) {
  const classes = [className].filter(Boolean).join(" ") || undefined;

  return <textarea {...rest} className={classes} />;
}
