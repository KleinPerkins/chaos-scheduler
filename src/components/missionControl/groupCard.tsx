import "./surfaces.css";

/** Health verdict shared by the two-group IA summary cards + drill-downs. */
export type GroupTone = "clear" | "warn" | "critical";

const TONE_LABEL: Record<GroupTone, string> = {
  clear: "Clear",
  warn: "Warning",
  critical: "Critical",
};

/** Colored-dot tone chip (Clear / Warning / Critical) bound to status tokens. */
export function ToneChip({ tone }: { tone: GroupTone }) {
  return (
    <span className={`mc-grp__tone mc-grp__tone--${tone}`}>
      {TONE_LABEL[tone]}
    </span>
  );
}

/** One value-over-label metric chip used across the group cards + drill-downs. */
export function GroupMetric({
  value,
  label,
}: {
  value: string;
  label: string;
}) {
  return (
    <div className="mc-grp__metric">
      <span className="mc-grp__metric-value">{value}</span>
      <span className="mc-grp__metric-label">{label}</span>
    </div>
  );
}

/** One entry in a {@link ChartLegend}: a colored (optionally dashed) swatch. */
export interface ChartLegendItem {
  label: string;
  /** A token reference (e.g. `var(--success)`), never a raw hex. */
  color: string;
  dashed?: boolean;
}

/**
 * A small solid/dashed swatch legend for a trend chart. Decorative
 * (`aria-hidden`): every charted series is also exposed in the chart's own
 * `aria-label` and its sr-only data table, so the legend adds no new semantics.
 */
export function ChartLegend({ items }: { items: ChartLegendItem[] }) {
  return (
    <ul className="mc-chart-legend" aria-hidden="true">
      {items.map((it) => (
        <li key={it.label} className="mc-chart-legend__item">
          <span
            className={`mc-chart-legend__swatch${it.dashed ? " mc-chart-legend__swatch--dashed" : ""}`}
            style={{
              background: it.dashed ? "transparent" : it.color,
              borderColor: it.color,
            }}
          />
          <span className="mc-chart-legend__label">{it.label}</span>
        </li>
      ))}
    </ul>
  );
}
