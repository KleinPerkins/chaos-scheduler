export interface SettingsCheckProps extends Omit<
  React.InputHTMLAttributes<HTMLInputElement>,
  "type"
> {
  /** Row label rendered after the checkbox, inside the `.settings-check` <label>. */
  label: React.ReactNode;
}

/**
 * Shared settings checkbox-row primitive. A typed extraction of the repeated
 * `<label className="settings-check"><input type="checkbox" …/>TEXT</label>`
 * shape in Settings / EmailProfiles — renders byte-identical DOM. Input props
 * (checked/onChange/disabled/…) pass through to the native checkbox.
 */
export default function SettingsCheck({ label, ...rest }: SettingsCheckProps) {
  return (
    <label className="settings-check">
      <input type="checkbox" {...rest} />
      {label}
    </label>
  );
}
