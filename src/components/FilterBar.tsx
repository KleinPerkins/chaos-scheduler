import { useId, useMemo, type ReactNode } from "react";
import type { Environment } from "../lib/commands";
import type { Lookback } from "../lib/lookback";
import EnvSelect from "./EnvSelect";
import InfoTip from "./InfoTip";
import LookbackSelect from "./LookbackSelect";
import "./FilterBar.css";

/** A `custom`-lookback date range, as `<input type="date">` values (`yyyy-mm-dd`). */
export interface CustomRange {
  start: string;
  end: string;
}

export interface FilterBarProps {
  /** Environments to offer (an `All` sentinel is always prepended). */
  environments: readonly Environment[];
  /** Selected environment (`"all"` or an environment `name`). */
  environment: string;
  /** Called with the chosen environment `name` (or `"all"`). */
  onEnvironmentChange: (value: string) => void;
  /** Selected lookback window. */
  lookback: Lookback;
  /** Called with the chosen lookback window. */
  onLookbackChange: (value: Lookback) => void;
  /** Current custom range (only meaningful when `lookback === "custom"`). */
  customRange?: CustomRange;
  /** Called when either custom date bound changes. */
  onCustomRangeChange?: (range: CustomRange) => void;
  /**
   * Surface-specific extra controls rendered at the trailing edge of the bar
   * (e.g. Mission Control's legacy Domain select). The standardized bar itself
   * only owns the shared `(environment, lookback)` filters.
   */
  extras?: ReactNode;
  /** Extra class(es) merged onto the `.filter-bar` container. */
  className?: string;
}

/**
 * Global dashboard filter bar: the standardized `(environment, lookback)`
 * control every surface shares. Composes the locked primitives — `EnvSelect`
 * (single-select, `All` + environments), `LookbackSelect` (1d/3d/7d/30d +
 * Custom), and hover/focus `InfoTip`s — plus an inline date-range picker that
 * appears only when `custom` is selected. Purely presentational and fully
 * controlled: the owner holds `(environment, lookback, customRange)` state and
 * serializes it for the wire with `lookbackToParam` (see `lib/lookback.ts`).
 *
 * No Figma Code Connect mapping: the design system ships the individual
 * primitive masters (EnvSelect / LookbackSelect / InfoTip), but there is no
 * `FilterBar` master — this is a pure code-side composition, so mapping it
 * would require inventing a node ID.
 */
export default function FilterBar({
  environments,
  environment,
  onEnvironmentChange,
  lookback,
  onLookbackChange,
  customRange,
  onCustomRangeChange,
  extras,
  className,
}: FilterBarProps) {
  const envLabelId = useId();
  const lookbackLabelId = useId();
  const classes = ["filter-bar", className].filter(Boolean).join(" ");

  // Union the selected value in when it is not a known environment (mirrors the
  // previous defensive behavior so an out-of-list selection still shows).
  const envOptions = useMemo<readonly Environment[]>(() => {
    if (
      environment === "all" ||
      environment === "" ||
      environments.some((env) => env.name === environment)
    ) {
      return environments;
    }
    return [...environments, { id: environment, name: environment }];
  }, [environments, environment]);

  return (
    <div className={classes} role="group" aria-label="Dashboard filters">
      <div className="filter-bar-field">
        <span className="filter-bar-label">
          <span id={envLabelId} className="filter-bar-label-text">
            Environment
          </span>
          <InfoTip
            title="Environment"
            def="Scope every KPI, chart, and table on this surface to one environment (or all)."
          />
        </span>
        <EnvSelect
          environments={envOptions}
          value={environment}
          onChange={(event) => onEnvironmentChange(event.target.value)}
          includeAllOption
          aria-labelledby={envLabelId}
        />
      </div>

      <div className="filter-bar-field">
        <span className="filter-bar-label">
          <span id={lookbackLabelId} className="filter-bar-label-text">
            Lookback
          </span>
          <InfoTip
            title="Lookback window"
            def="Trailing time window every KPI, chart, and table on this surface is computed over."
          />
        </span>
        <LookbackSelect value={lookback} onChange={onLookbackChange} />
      </div>

      {lookback === "custom" ? (
        <div
          className="filter-bar-custom"
          role="group"
          aria-label="Custom date range"
        >
          <label className="filter-bar-date">
            <span className="filter-bar-label-text">From</span>
            <input
              type="date"
              value={customRange?.start ?? ""}
              max={customRange?.end || undefined}
              onChange={(event) =>
                onCustomRangeChange?.({
                  start: event.target.value,
                  end: customRange?.end ?? "",
                })
              }
            />
          </label>
          <label className="filter-bar-date">
            <span className="filter-bar-label-text">To</span>
            <input
              type="date"
              value={customRange?.end ?? ""}
              min={customRange?.start || undefined}
              onChange={(event) =>
                onCustomRangeChange?.({
                  start: customRange?.start ?? "",
                  end: event.target.value,
                })
              }
            />
          </label>
        </div>
      ) : null}

      {extras ? <div className="filter-bar-extras">{extras}</div> : null}
    </div>
  );
}
