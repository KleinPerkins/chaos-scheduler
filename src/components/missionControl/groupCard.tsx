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
