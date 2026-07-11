import { describe, expect, it } from "vitest";
import type {
  DashboardKpiSummary,
  DashboardWaitRuntimeTrend,
} from "../../lib/commands";
import {
  sampleDashboardKpiSummary,
  sampleDashboardWaitRuntimeTrend,
} from "../../test/fixtures/data";
import {
  aggregateStats,
  formatPerHour,
  formatPercent1,
  formatSeconds,
  metricTrendToChart,
  operationalHealthSummary,
  toMinutes,
} from "./operationalHealthData";

/** A KPI summary with overridable fields for tone/edge-case coverage. */
function kpi(
  overrides: Partial<DashboardKpiSummary> = {},
): DashboardKpiSummary {
  return { ...sampleDashboardKpiSummary, ...overrides };
}

/** A wait/runtime trend whose latest runtime avg/baseline can be pinned. */
function waitRuntime(
  latestRuntimeAvg: number | null,
  latestRuntimeBaseline: number | null,
): DashboardWaitRuntimeTrend {
  return {
    grain: "hour",
    wait: [
      {
        bucket: "2026-07-04T12:00:00.000Z",
        avg_seconds: 40,
        max_seconds: 200,
        count: 10,
        baseline_avg_seconds: 45,
      },
    ],
    runtime: [
      {
        bucket: "2026-07-04T12:00:00.000Z",
        avg_seconds: latestRuntimeAvg,
        max_seconds: 4000,
        count: 10,
        baseline_avg_seconds: latestRuntimeBaseline,
      },
    ],
  };
}

describe("formatters", () => {
  it("formatPercent1 renders one decimal with an em-dash for null", () => {
    expect(formatPercent1(0.9375)).toBe("93.8%");
    expect(formatPercent1(1)).toBe("100.0%");
    expect(formatPercent1(null)).toBe("—");
    expect(formatPercent1(Number.NaN)).toBe("—");
  });

  it("formatPerHour renders one decimal with a /hr suffix", () => {
    expect(formatPerHour(5.3)).toBe("5.3/hr");
    expect(formatPerHour(0)).toBe("0.0/hr");
    expect(formatPerHour(null)).toBe("—");
  });

  it("formatSeconds delegates to the shared duration ladder", () => {
    expect(formatSeconds(42)).toBe("42s");
    expect(formatSeconds(372)).toBe("6m 12s");
    expect(formatSeconds(318)).toBe("5m 18s");
    expect(formatSeconds(null)).toBe("—");
  });
});

describe("aggregateStats", () => {
  it("maps the six windowed KPIs from the summary in order", () => {
    const stats = aggregateStats(sampleDashboardKpiSummary);
    expect(stats.map((s) => s.key)).toEqual([
      "runs",
      "success",
      "throughput",
      "avg-runtime",
      "median-wait",
      "max-wait",
    ]);
    const byKey = Object.fromEntries(stats.map((s) => [s.key, s.value]));
    expect(byKey.runs).toBe("128");
    expect(byKey.success).toBe("93.8%");
    expect(byKey.throughput).toBe("5.3/hr");
    expect(byKey["avg-runtime"]).toBe("6m 12s");
    expect(byKey["median-wait"]).toBe("42s");
    expect(byKey["max-wait"]).toBe("5m 18s");
  });

  it("renders em-dashes for absent metrics", () => {
    const stats = aggregateStats(
      kpi({
        success_rate: null,
        throughput_per_hour: null,
        avg_runtime_seconds: null,
        median_wait_seconds: null,
        max_wait_seconds: null,
      }),
    );
    const byKey = Object.fromEntries(stats.map((s) => [s.key, s.value]));
    expect(byKey.success).toBe("—");
    expect(byKey.throughput).toBe("—");
    expect(byKey["avg-runtime"]).toBe("—");
  });
});

describe("metricTrendToChart", () => {
  it("aligns categories + seconds arrays and flags data presence", () => {
    const chart = metricTrendToChart(
      sampleDashboardWaitRuntimeTrend.wait,
      sampleDashboardWaitRuntimeTrend.grain,
    );
    expect(chart.categories).toEqual([
      "05:00",
      "06:00",
      "07:00",
      "08:00",
      "09:00",
      "10:00",
      "11:00",
      "12:00",
    ]);
    expect(chart.avgSeconds).toEqual([38, 41, 52, 47, 44, 58, 49, 42]);
    expect(chart.maxSeconds[5]).toBe(312);
    expect(chart.baselineSeconds[0]).toBe(45);
    expect(chart.hasData).toBe(true);
  });

  it("uses MM-DD labels at day grain", () => {
    const chart = metricTrendToChart(
      [
        {
          bucket: "2026-07-04T00:00:00.000Z",
          avg_seconds: 10,
          max_seconds: 20,
          count: 3,
          baseline_avg_seconds: 12,
        },
      ],
      "day",
    );
    expect(chart.categories).toEqual(["07-04"]);
  });

  it("reports no data when every bucket is empty", () => {
    const chart = metricTrendToChart(
      [
        {
          bucket: "2026-07-04T00:00:00.000Z",
          avg_seconds: null,
          max_seconds: null,
          count: 0,
          baseline_avg_seconds: null,
        },
      ],
      "hour",
    );
    expect(chart.hasData).toBe(false);
    expect(chart.avgSeconds).toEqual([null]);
  });
});

describe("toMinutes", () => {
  it("converts seconds to minutes and coerces null to zero", () => {
    expect(toMinutes([60, null, 120])).toEqual([1, 0, 2]);
  });
});

describe("operationalHealthSummary", () => {
  it("summarizes the realistic fixture as warn (soft success rate)", () => {
    const model = operationalHealthSummary(
      sampleDashboardKpiSummary,
      sampleDashboardWaitRuntimeTrend,
    );
    expect(model.tone).toBe("warn");
    expect(model.runtimeTrend).toBe("up");
    expect(model.successRate).toBe(0.9375);
    expect(model.headline).toBe(
      "93.8% success · 5.3/hr throughput · avg runtime 6m 12s",
    );
  });

  it("escalates to critical when the success rate is poor", () => {
    const model = operationalHealthSummary(
      kpi({ success_rate: 0.82 }),
      waitRuntime(300, 300),
    );
    expect(model.tone).toBe("critical");
  });

  it("stays clear when success is high and runtime is on baseline", () => {
    const model = operationalHealthSummary(
      kpi({ success_rate: 0.995 }),
      waitRuntime(300, 300),
    );
    expect(model.tone).toBe("clear");
    expect(model.runtimeTrend).toBe("flat");
    expect(model.headline).not.toContain("runtime above baseline");
  });

  it("warns and annotates when runtime regresses past its baseline", () => {
    const model = operationalHealthSummary(
      kpi({ success_rate: 0.995 }),
      waitRuntime(500, 100),
    );
    expect(model.tone).toBe("warn");
    expect(model.runtimeTrend).toBe("up");
    expect(model.headline).toContain("runtime above baseline");
  });

  it("has no runtime trend when the latest buckets carry no samples", () => {
    const model = operationalHealthSummary(
      kpi({ success_rate: 0.995 }),
      waitRuntime(null, null),
    );
    expect(model.runtimeTrend).toBeNull();
    expect(model.tone).toBe("clear");
  });
});
