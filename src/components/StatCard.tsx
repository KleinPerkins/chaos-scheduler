export type StatCardVariant = "mc" | "rd";

interface StatCardVariantConfig {
  card: string;
  value: string;
  label: string;
  /** Element tag used for the value/label; differs between the two surfaces. */
  inner: "span" | "div";
}

const VARIANT_CONFIG: Record<StatCardVariant, StatCardVariantConfig> = {
  mc: {
    card: "mc-stat-card",
    value: "mc-stat-value",
    label: "mc-stat-label",
    inner: "span",
  },
  rd: {
    card: "rd-stat-card",
    value: "rd-stat-value",
    label: "rd-stat-label",
    inner: "div",
  },
};

export interface StatCardProps extends React.HTMLAttributes<HTMLDivElement> {
  /** Metric value rendered in the `.*-stat-value` element. */
  value: React.ReactNode;
  /** Caption rendered in the `.*-stat-label` element. */
  label: React.ReactNode;
  /**
   * Which surface's stat-tile classes to render: `mc` (Mission Control,
   * `<span>` inner elements) or `rd` (Run Detail, `<div>` inner elements).
   */
  variant?: StatCardVariant;
}

/**
 * Shared stat/metric tile primitive. A thin, typed wrapper over the global
 * `.mc-stat-card` / `.rd-stat-card` classes (see `MissionControl.css`,
 * `RunDetail.css` / DESIGN-SYSTEM.md) — it renders the exact same markup call
 * sites used before, so styling is unchanged. The `variant` selects the class
 * family and the inner element tag (`<span>` for Mission Control, `<div>` for
 * Run Detail), keeping each surface byte-identical.
 */
export default function StatCard({
  value,
  label,
  variant = "mc",
  className,
  ...rest
}: StatCardProps) {
  const config = VARIANT_CONFIG[variant];
  const Inner = config.inner;
  const classes = [config.card, className].filter(Boolean).join(" ");

  return (
    <div {...rest} className={classes}>
      <Inner className={config.value}>{value}</Inner>
      <Inner className={config.label}>{label}</Inner>
    </div>
  );
}
