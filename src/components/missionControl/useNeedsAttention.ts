import { useCallback, useEffect, useRef, useState } from "react";
import {
  getDashboardBlastRadius,
  getDashboardBlockTaxonomy,
  getDashboardFailureRecurrence,
  type DashboardBlastRadius,
  type DashboardBlockTaxonomy,
  type DashboardWorkflowFailureCount,
} from "../../lib/commands";

/** The three real bindings the Needs Attention surface composes. */
export interface NeedsAttentionData {
  taxonomy: DashboardBlockTaxonomy;
  blastRadius: DashboardBlastRadius[];
  failureRecurrence: DashboardWorkflowFailureCount[];
}

export interface NeedsAttentionState {
  data: NeedsAttentionData | null;
  loading: boolean;
  error: string | null;
  reload: () => void;
}

/**
 * Fetch the Needs Attention bindings for one `(environment, lookback)`
 * selection: the blocked/waiting reason taxonomy (+ heavy blockers), the
 * downstream blast-radius rollup, and the per-workflow failure recurrence. A
 * request-id guard drops out-of-order responses when the filters change
 * mid-flight; the fetch is deferred to a macrotask so the effect body performs
 * no synchronous state updates (mirrors `useDashboardOverview`). The caller
 * supplies the already-serialized `lookbackParam` (via `lookbackToParam`).
 */
export function useNeedsAttention(
  environmentFilter: string,
  lookbackParam: string,
): NeedsAttentionState {
  const [data, setData] = useState<NeedsAttentionData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const latestRequestId = useRef(0);

  const load = useCallback(async () => {
    const requestId = latestRequestId.current + 1;
    latestRequestId.current = requestId;
    setLoading(true);
    setError(null);
    const envArg = environmentFilter === "all" ? undefined : environmentFilter;
    try {
      const [taxonomy, blastRadius, failureRecurrence] = await Promise.all([
        getDashboardBlockTaxonomy(envArg, lookbackParam),
        getDashboardBlastRadius(envArg, lookbackParam),
        getDashboardFailureRecurrence(envArg, lookbackParam),
      ]);
      if (requestId !== latestRequestId.current) return;
      setData({ taxonomy, blastRadius, failureRecurrence });
    } catch (e) {
      if (requestId !== latestRequestId.current) return;
      setError(String(e));
    } finally {
      if (requestId === latestRequestId.current) setLoading(false);
    }
  }, [environmentFilter, lookbackParam]);

  useEffect(() => {
    const id = setTimeout(() => void load(), 0);
    return () => clearTimeout(id);
  }, [load]);

  return { data, loading, error, reload: load };
}
