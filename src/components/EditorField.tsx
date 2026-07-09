export interface EditorFieldProps extends React.LabelHTMLAttributes<HTMLLabelElement> {
  /** Label text rendered in the `.editor-label` span. */
  label: React.ReactNode;
  /** Optional hint rendered in a trailing `.editor-hint` span; omitted when falsy. */
  hint?: React.ReactNode;
  /** The form control(s), nested inside the label. */
  children: React.ReactNode;
}

/**
 * Shared editor form-field primitive. A typed extraction of the repeated
 * `<label className="editor-field"><span className="editor-label">…</span>{control}
 * {span.editor-hint?}</label>` shape (control nested in the label) used by the
 * workflow operator/config forms — renders byte-identical DOM. Rest props
 * (e.g. `style`) pass through to the `<label>`. This is the classed-`.editor-label`
 * + optional-`.editor-hint` sibling of `Field` (which handles the class-less-span
 * `.step-field`/`.env-field`/`.intg-field` shape). The DIV-wrapper editor-field
 * shape in WorkflowEditor (separate `htmlFor` label / fieldset / checkbox
 * variants) is different DOM and intentionally NOT handled here.
 */
export default function EditorField({
  label,
  hint,
  className,
  children,
  ...rest
}: EditorFieldProps) {
  const classes = ["editor-field", className].filter(Boolean).join(" ");
  return (
    <label {...rest} className={classes}>
      <span className="editor-label">{label}</span>
      {children}
      {hint ? <span className="editor-hint">{hint}</span> : null}
    </label>
  );
}
