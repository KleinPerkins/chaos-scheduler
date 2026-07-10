import "./StatusBar.css";

export interface StatusBarSegment {
  /**
   * Raw run/task status token; drives the segment + legend-dot color via the
   * `.<status>` modifier class (matching the sibling `StatusDot` / `StatusBadge`
   * status tokens — e.g. `succeeded`, `running`, `failed`, `poll_exhausted`).
   */
  status: string;
  /** Human-readable label rendered in the legend. */
  label: string;
  /** Weight used for the segment's proportional width (and the legend order). */
  count: number;
}

export interface StatusBarProps extends React.HTMLAttributes<HTMLDivElement> {
  /** Ordered status segments; the bar fills proportionally to each `count`. */
  segments: StatusBarSegment[];
  /** Render the labelled legend beneath the bar (default `true`). */
  showLegend?: boolean;
}

/**
 * Proportional run-status distribution bar with an optional legend. A
 * self-contained presentational primitive matching the `StatusBar` Figma master
 * (node 60:145) — a rounded track whose segments fill proportionally to their
 * `count`, plus a wrapped legend of colored dots + labels. Segment/dot colors
 * bind to the shared status tokens (`--success` / `--running` / `--error` /
 * `--warning`) via the `.<status>` modifier, so they stay in lockstep with the
 * `StatusDot` / `StatusBadge` palette. Zero-count segments contribute no bar
 * width. The track carries a text summary for assistive tech; the dots are
 * decorative. Purely presentational — not yet wired into any screen.
 */
export default function StatusBar({
  segments,
  showLegend = true,
  className,
  ...rest
}: StatusBarProps) {
  const total = segments.reduce((sum, s) => sum + Math.max(0, s.count), 0);
  const classes = ["status-bar", className].filter(Boolean).join(" ");

  const summary =
    segments
      .filter((s) => s.count > 0)
      .map((s) => `${s.count} ${s.label}`)
      .join(", ") || "No runs";

  return (
    <div {...rest} className={classes}>
      <div className="status-bar-track" role="img" aria-label={summary}>
        {total > 0
          ? segments.map((seg, i) =>
              seg.count > 0 ? (
                <span
                  key={`${seg.status}-${i}`}
                  className={`status-bar-seg ${seg.status}`.trim()}
                  style={{ width: `${(seg.count / total) * 100}%` }}
                />
              ) : null,
            )
          : null}
      </div>
      {showLegend ? (
        <ul className="status-bar-legend">
          {segments.map((seg, i) => (
            <li key={`${seg.status}-${i}`} className="status-bar-legend-item">
              <span
                className={`status-bar-dot ${seg.status}`.trim()}
                aria-hidden="true"
              />
              {seg.label}
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}
