import { useCallback, useEffect, useRef, useState } from "react";
import {
  getDashboardExecutionSlots,
  getDashboardKpiSummary,
  getDashboardKpiWow,
  getDashboardQueueHealth,
  getDashboardStatusDistribution,
  getDashboardSuccessFailTrend,
  getDashboardWorkflowBaselines,
  type DashboardExecutionSlots,
  type DashboardKpiDelta,
  type DashboardKpiSummary,
  type DashboardQueueHealthSummary,
  type DashboardStatusCount,
  type DashboardTrendSeries,
  type DashboardWorkflowBaseline,
} from "../../lib/commands";

/** The seven real bindings the Overview vNext composes. */
export interface DashboardOverviewData {
  kpiSummary: DashboardKpiSummary;
  kpiWow: DashboardKpiDelta;
  statusDistribution: DashboardStatusCount[];
  successFailTrend: DashboardTrendSeries;
  queueHealth: DashboardQueueHealthSummary;
  baselines: DashboardWorkflowBaseline[];
  executionSlots: DashboardExecutionSlots;
}

export interface DashboardOverviewState {
  data: DashboardOverviewData | null;
  loading: boolean;
  error: string | null;
  reload: () => void;
}

/**
 * Fetch every `get_dashboard_*` binding the Overview needs for one
 * `(environment, lookback)` selection. The windowed bindings (KPIs, status,
 * trend, baselines, week-over-window) receive the serialized lookback param;
 * the live bindings (queue health, execution slots) take only the environment.
 * A request-id guard drops out-of-order responses when the filters change
 * mid-flight. The caller (Mission Control) supplies the already-serialized
 * `lookbackParam` (via `lookbackToParam`) so no wire-format logic lives here.
 */
export function useDashboardOverview(
  environmentFilter: string,
  lookbackParam: string,
): DashboardOverviewState {
  const [data, setData] = useState<DashboardOverviewData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const latestRequestId = useRef(0);

  const load = useCallback(async () => {
    const requestId = latestRequestId.current + 1;
    latestRequestId.current = requestId;
    setLoading(true);
    setError(null);
    // "all" means no environment filter — let each binding roll up every env.
    const envArg = environmentFilter === "all" ? undefined : environmentFilter;
    try {
      const [
        kpiSummary,
        kpiWow,
        statusDistribution,
        successFailTrend,
        queueHealth,
        baselines,
        executionSlots,
      ] = await Promise.all([
        getDashboardKpiSummary(envArg, lookbackParam),
        getDashboardKpiWow(envArg, lookbackParam),
        getDashboardStatusDistribution(envArg, lookbackParam),
        getDashboardSuccessFailTrend(envArg, lookbackParam),
        getDashboardQueueHealth(envArg),
        getDashboardWorkflowBaselines(envArg),
        getDashboardExecutionSlots(envArg),
      ]);
      if (requestId !== latestRequestId.current) return;
      setData({
        kpiSummary,
        kpiWow,
        statusDistribution,
        successFailTrend,
        queueHealth,
        baselines,
        executionSlots,
      });
    } catch (e) {
      if (requestId !== latestRequestId.current) return;
      setError(String(e));
    } finally {
      if (requestId === latestRequestId.current) setLoading(false);
    }
  }, [environmentFilter, lookbackParam]);

  // Defer the fetch to a macrotask so the effect body performs no synchronous
  // state updates (mirrors MissionControl / useSchedulerStatus).
  useEffect(() => {
    const id = setTimeout(() => void load(), 0);
    return () => clearTimeout(id);
  }, [load]);

  return { data, loading, error, reload: load };
}
