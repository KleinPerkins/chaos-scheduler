export interface SettingsFieldProps extends React.HTMLAttributes<HTMLDivElement> {
  /** Text for the `.settings-label` <label>. */
  label: React.ReactNode;
  /** `htmlFor` target associating the label with its control. */
  htmlFor?: string;
  /** Optional caption rendered in a trailing `.settings-hint` <span>; omitted when falsy. */
  hint?: React.ReactNode;
  /** The form control (Input / native control / etc.). */
  children: React.ReactNode;
}

/**
 * Shared settings form-field primitive. A typed extraction of the repeated
 * `.settings-field > (label.settings-label[htmlFor] + control + span.settings-hint?)`
 * shape in the Settings / EmailProfiles surfaces — renders byte-identical DOM.
 * Rest props (e.g. `style`) pass through to the container. The span-label
 * (aria-labelledby) variant and `.settings-check` checkbox rows are NOT handled
 * here (different DOM) and are left for their own follow-up.
 */
export default function SettingsField({
  label,
  htmlFor,
  hint,
  className,
  children,
  ...rest
}: SettingsFieldProps) {
  const classes = ["settings-field", className].filter(Boolean).join(" ");
  return (
    <div {...rest} className={classes}>
      <label className="settings-label" htmlFor={htmlFor}>
        {label}
      </label>
      {children}
      {hint ? <span className="settings-hint">{hint}</span> : null}
    </div>
  );
}
