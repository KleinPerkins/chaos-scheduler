export interface FieldProps {
  /**
   * Label text. Rendered inside a *class-less* `<span>` that is a direct child
   * of the `<label>` — the shared `.step-field > span` / `.env-field > span` /
   * `.intg-field > span` selectors style it.
   */
  label: React.ReactNode;
  /**
   * Class(es) applied to the wrapping `<label>` (e.g. `"step-field"`,
   * `"env-field env-field-grow"`, `"intg-field"`).
   */
  className?: string;
  /** The form control (e.g. an `<Input>` / `<Select>`). */
  children: React.ReactNode;
}

/**
 * Shared form-field WRAPPER primitive. Renders the common Chaos Scheduler field
 * shape — a `<label>` that wraps a class-less `<span>` label followed by its
 * control (implicit label→control association via nesting). It is intentionally
 * minimal so it stays BYTE-IDENTICAL to the hand-written
 * `<label className="…-field"><span>…</span>{control}</label>` call sites it
 * replaces: the `<span>` carries NO class (styled by the `.…-field > span`
 * selectors) and, when no `className` is passed, the `<label>` emits no `class`
 * attribute (not even `class=""`).
 *
 * Only this class-less-span shape is supported. The `editor-field` shape (a
 * classed `.editor-label` span plus an optional `.editor-hint`) and the bare
 * `<label>text{control}</label>` filter shape are deliberately NOT handled here
 * — they are different DOM and are left for a follow-up.
 */
export default function Field({ label, className, children }: FieldProps) {
  const classes = [className].filter(Boolean).join(" ") || undefined;

  return (
    <label className={classes}>
      <span>{label}</span>
      {children}
    </label>
  );
}
