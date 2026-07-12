import { useCallback, useEffect, useRef, useState } from "react";
import {
  getDashboardKpiSummary,
  getDashboardSuccessFailTrend,
  getDashboardWaitRuntimeTrend,
  type DashboardKpiSummary,
  type DashboardTrendSeries,
  type DashboardWaitRuntimeTrend,
} from "../../lib/commands";

/** The three real bindings the Operational Health surface composes. */
export interface OperationalHealthData {
  kpiSummary: DashboardKpiSummary;
  successFailTrend: DashboardTrendSeries;
  waitRuntimeTrend: DashboardWaitRuntimeTrend;
}

export interface OperationalHealthState {
  data: OperationalHealthData | null;
  loading: boolean;
  error: string | null;
  reload: () => void;
}

/**
 * Fetch the Operational Health bindings for one `(environment, lookback)`
 * selection: the aggregate KPI rollup, the success/failure trend, and the
 * wait + runtime duration trends. A request-id guard drops out-of-order
 * responses when the filters change mid-flight; the fetch is deferred to a
 * macrotask so the effect body performs no synchronous state updates (mirrors
 * `useDashboardOverview`). The caller supplies the already-serialized
 * `lookbackParam` (via `lookbackToParam`).
 */
export function useOperationalHealth(
  environmentFilter: string,
  lookbackParam: string,
): OperationalHealthState {
  const [data, setData] = useState<OperationalHealthData | null>(null);
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
      const [kpiSummary, successFailTrend, waitRuntimeTrend] =
        await Promise.all([
          getDashboardKpiSummary(envArg, lookbackParam),
          getDashboardSuccessFailTrend(envArg, lookbackParam),
          getDashboardWaitRuntimeTrend(envArg, lookbackParam),
        ]);
      if (requestId !== latestRequestId.current) return;
      setData({ kpiSummary, successFailTrend, waitRuntimeTrend });
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
