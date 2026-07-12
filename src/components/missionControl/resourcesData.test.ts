import { describe, expect, it } from "vitest";
import type {
  DashboardExecutionSlots,
  DashboardQueueHealthSummary,
  DashboardQueueUtilizationHistory,
} from "../../lib/commands";
import {
  sampleDashboardExecutionSlots,
  sampleDashboardQueueHealth,
  sampleDashboardQueueUtilizationHistory,
} from "../../test/fixtures/data";
import {
  formatCount,
  formatPercentFrac,
  queueHealthRows,
  resourcesSummary,
  slotGauges,
  statusTone,
  utilizationChart,
} from "./resourcesData";

describe("formatPercentFrac", () => {
  it("renders a fraction as a whole percent", () => {
    expect(formatPercentFrac(0.79)).toBe("79%");
    expect(formatPercentFrac(0)).toBe("0%");
    expect(formatPercentFrac(1)).toBe("100%");
  });

  it("rounds to the nearest whole percent", () => {
    expect(formatPercentFrac(0.666)).toBe("67%");
    expect(formatPercentFrac(0.664)).toBe("66%");
  });

  it("returns a dash for null / non-finite", () => {
    expect(formatPercentFrac(null)).toBe("—");
    expect(formatPercentFrac(Number.NaN)).toBe("—");
    expect(formatPercentFrac(Number.POSITIVE_INFINITY)).toBe("—");
  });
});

describe("formatCount", () => {
  it("formats with locale grouping", () => {
    expect(formatCount(5)).toBe("5");
    expect(formatCount(1234)).toBe("1,234");
  });
  it("returns a dash for null / non-finite", () => {
    expect(formatCount(null)).toBe("—");
    expect(formatCount(Number.NaN)).toBe("—");
  });
});

describe("statusTone", () => {
  it("maps queue-health status to the shared tone", () => {
    expect(statusTone("healthy")).toBe("clear");
    expect(statusTone("warn")).toBe("warn");
    expect(statusTone("degraded")).toBe("critical");
  });
});

describe("utilizationChart", () => {
  it("aligns categories + percent series from the fixture", () => {
    const chart = utilizationChart(sampleDashboardQueueUtilizationHistory);
    expect(chart.categories).toHaveLength(8);
    // Hour grain → HH:MM labels.
    expect(chart.categories[0]).toBe("05:00");
    expect(chart.categories[7]).toBe("12:00");
    // 0.52 → 52%, 0.79 → 79%.
    expect(chart.avgPct[0]).toBe(52);
    expect(chart.avgPct[5]).toBe(79);
    expect(chart.maxPct[5]).toBe(97);
    expect(chart.warnPct).toBe(70);
    expect(chart.degradedPct).toBe(90);
    expect(chart.hasData).toBe(true);
  });

  it("keeps raw fractions for the accessible table", () => {
    const chart = utilizationChart(sampleDashboardQueueUtilizationHistory);
    expect(chart.avgFrac[0]).toBe(0.52);
    expect(chart.maxFrac[5]).toBe(0.97);
  });

  it("uses a day-grain MM-DD label", () => {
    const history: DashboardQueueUtilizationHistory = {
      grain: "day",
      warn_utilization: 0.7,
      degraded_utilization: 0.9,
      buckets: [
        {
          bucket: "2026-07-04T00:00:00.000Z",
          avg_running: 5,
          max_running: 8,
          avg_queued: 2,
          max_queued: 4,
          avg_utilization: 0.5,
          max_utilization: 0.7,
          sample_count: 24,
        },
      ],
    };
    expect(utilizationChart(history).categories[0]).toBe("07-04");
  });

  it("plots null utilization at 0 but keeps it null in the table, and flags no data", () => {
    const history: DashboardQueueUtilizationHistory = {
      grain: "hour",
      warn_utilization: 0.7,
      degraded_utilization: 0.9,
      buckets: [
        {
          bucket: "2026-07-04T05:00:00.000Z",
          avg_running: null,
          max_running: null,
          avg_queued: null,
          max_queued: null,
          avg_utilization: null,
          max_utilization: null,
          sample_count: 0,
        },
      ],
    };
    const chart = utilizationChart(history);
    expect(chart.avgPct[0]).toBe(0);
    expect(chart.avgFrac[0]).toBeNull();
    expect(chart.hasData).toBe(false);
  });
});

describe("slotGauges", () => {
  it("emits a global gauge plus one per queue", () => {
    const { global, queues } = slotGauges(sampleDashboardExecutionSlots);
    expect(global.key).toBe("__global__");
    expect(global.label).toBe("All queues");
    expect(global.running).toBe(3);
    expect(global.capacity).toBe(12);
    expect(global.available).toBe(9);
    expect(global.utilizationPct).toBe(25);
    expect(queues).toHaveLength(2);
    expect(queues[0].label).toBe("default");
    expect(queues[0].sublabel).toBe("production");
    expect(queues[0].utilizationPct).toBe(50);
    expect(queues[0].key).toBe("production:default");
  });

  it("rounds fractional utilization", () => {
    const slots: DashboardExecutionSlots = {
      queues: [
        {
          name: "q",
          environment: "production",
          running: 2,
          capacity: 3,
          available: 1,
          utilization: 2 / 3,
        },
      ],
      global_running: 2,
      global_capacity: 3,
      global_available: 1,
      global_utilization: 2 / 3,
    };
    expect(slotGauges(slots).global.utilizationPct).toBe(67);
    expect(slotGauges(slots).queues[0].utilizationPct).toBe(67);
  });
});

describe("queueHealthRows", () => {
  it("sorts worst status first with the right tone", () => {
    const rows = queueHealthRows(sampleDashboardQueueHealth);
    expect(rows.map((r) => r.status)).toEqual(["degraded", "warn", "healthy"]);
    expect(rows[0].tone).toBe("critical");
    expect(rows[1].tone).toBe("warn");
    expect(rows[2].tone).toBe("clear");
    // "ml" is the degraded queue at 100% utilization.
    expect(rows[0].name).toBe("ml");
    expect(rows[0].utilizationPct).toBe(100);
  });
});

describe("resourcesSummary", () => {
  it("summarizes the live slots + queue health", () => {
    const summary = resourcesSummary(
      sampleDashboardExecutionSlots,
      sampleDashboardQueueHealth,
      sampleDashboardQueueUtilizationHistory,
    );
    expect(summary.globalUtilizationPct).toBe(25);
    expect(summary.running).toBe(3);
    expect(summary.capacity).toBe(12);
    expect(summary.degradedQueues).toBe(1);
    expect(summary.warnQueues).toBe(1);
    // A degraded queue present → critical tone.
    expect(summary.tone).toBe("critical");
    expect(summary.headline).toContain("25% slots used");
    expect(summary.headline).toContain("1 degraded");
  });

  it("is warn when only warn queues exist", () => {
    const health: DashboardQueueHealthSummary = {
      ...sampleDashboardQueueHealth,
      healthy: 2,
      warn: 1,
      degraded: 0,
    };
    const summary = resourcesSummary(
      sampleDashboardExecutionSlots,
      health,
      sampleDashboardQueueUtilizationHistory,
    );
    expect(summary.tone).toBe("warn");
  });

  it("is clear when all queues are healthy and utilization is low", () => {
    const health: DashboardQueueHealthSummary = {
      ...sampleDashboardQueueHealth,
      healthy: 3,
      warn: 0,
      degraded: 0,
    };
    const summary = resourcesSummary(
      sampleDashboardExecutionSlots,
      health,
      sampleDashboardQueueUtilizationHistory,
    );
    expect(summary.tone).toBe("clear");
    expect(summary.headline).toContain("all queues healthy");
  });

  it("escalates on global utilization crossing the degraded threshold even with healthy queues", () => {
    const slots: DashboardExecutionSlots = {
      ...sampleDashboardExecutionSlots,
      global_running: 11,
      global_capacity: 12,
      global_available: 1,
      global_utilization: 0.92,
    };
    const health: DashboardQueueHealthSummary = {
      ...sampleDashboardQueueHealth,
      healthy: 3,
      warn: 0,
      degraded: 0,
    };
    const summary = resourcesSummary(
      slots,
      health,
      sampleDashboardQueueUtilizationHistory,
    );
    expect(summary.tone).toBe("critical");
  });
});
