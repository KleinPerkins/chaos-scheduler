import { ChevronDown } from "lucide-react";
import { LOOKBACK_PRESETS, type Lookback } from "../lib/lookback";
import "./LookbackSelect.css";

export type { Lookback };

export interface LookbackSelectProps {
  /** Currently-selected lookback window. */
  value: Lookback;
  /** Called with the chosen window when a segment is selected. */
  onChange: (value: Lookback) => void;
  /**
   * Preset windows to render, in order. Defaults to the standard filter-bar set
   * (`1d` / `3d` / `7d` / `30d`).
   */
  options?: Lookback[];
  /** Append a trailing `Custom` segment (with a caret affordance). Default `true`. */
  includeCustom?: boolean;
  /** Extra class(es) merged onto the `.lookback-select` container. */
  className?: string;
}

const DEFAULT_OPTIONS: Lookback[] = [...LOOKBACK_PRESETS];

const LABELS: Record<Lookback, string> = {
  "1d": "1d",
  "3d": "3d",
  "7d": "7d",
  "30d": "30d",
  custom: "Custom",
};

/**
 * Segmented lookback-window selector matching the Figma `LookbackSelect` master
 * (node 121:585) — a single-select pill group (`1d` / `3d` / `7d` / `30d`) plus
 * an optional trailing `Custom` segment with a caret. The selected pill fills
 * with the `--accent` token; the rest read as `--text-secondary`. Follows the
 * sibling `ThemeToggle` a11y pattern (a `role="group"` of `aria-pressed`
 * buttons). All colors/type bind to repo tokens — no raw hex. Purely
 * presentational — not yet wired into any screen (the `Custom` window's
 * date-range picker is a later, integration-time concern).
 */
export default function LookbackSelect({
  value,
  onChange,
  options = DEFAULT_OPTIONS,
  includeCustom = true,
  className,
}: LookbackSelectProps) {
  const items: Lookback[] = includeCustom ? [...options, "custom"] : options;
  const classes = ["lookback-select", className].filter(Boolean).join(" ");

  return (
    <div className={classes} role="group" aria-label="Lookback window">
      {items.map((opt) => {
        const active = value === opt;
        return (
          <button
            key={opt}
            type="button"
            className={`lookback-option ${active ? "active" : ""}`.trim()}
            aria-pressed={active}
            onClick={() => onChange(opt)}
          >
            {LABELS[opt]}
            {opt === "custom" ? (
              <ChevronDown size={12} strokeWidth={2} aria-hidden="true" />
            ) : null}
          </button>
        );
      })}
    </div>
  );
}
