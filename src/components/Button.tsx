export type ButtonVariant = "neutral" | "primary" | "ghost" | "danger";
export type ButtonSize = "sm" | "md";

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  /** Visual style; maps to the shared `.btn-*` classes in `index.css`. */
  variant?: ButtonVariant;
  /** `sm` applies the compact `.btn-sm` modifier. */
  size?: ButtonSize;
  /** Busy state: disables the button and marks it `aria-busy`. */
  loading?: boolean;
}

const VARIANT_CLASS: Record<ButtonVariant, string> = {
  neutral: "",
  primary: "btn-primary",
  ghost: "btn-ghost",
  danger: "btn-danger",
};

/**
 * Shared button primitive. A thin, typed wrapper over the global `.btn`
 * classes (see `index.css` / DESIGN-SYSTEM.md) — it renders the exact same
 * markup call sites used before, so behavior and styling are unchanged.
 *
 * `type` is intentionally NOT defaulted so the native default (submit inside a
 * form) is preserved for callers that relied on it.
 */
export default function Button({
  variant = "neutral",
  size = "md",
  loading = false,
  disabled,
  className,
  children,
  ...rest
}: ButtonProps) {
  const classes = [
    "btn",
    VARIANT_CLASS[variant],
    size === "sm" ? "btn-sm" : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <button
      {...rest}
      className={classes}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
    >
      {children}
    </button>
  );
}
