import { describe, expect, it } from "vitest";
import {
  buildKpiCards,
  buildKpiDelta,
  buildRaceJobs,
  deriveSlaWarning,
  runningNow,
  statusDistributionTotal,
  statusDonutSegments,
  totalQueueDepth,
  trendToChart,
} from "./overviewData";
import type {
  DashboardExecutionSlots,
  DashboardKpiDelta,
  DashboardKpiSummary,
  DashboardQueueHealthSummary,
  DashboardTrendSeries,
  DashboardWorkflowBaseline,
  MissionControlActivityItem,
} from "../../lib/commands";

const NOW_MS = new Date("2026-07-04T12:00:00.000Z").getTime();

const kpiSummary: DashboardKpiSummary = {
  total_runs: 128,
  succeeded: 120,
  failed: 8,
  success_rate: 0.9375,
  throughput_per_hour: 5.3,
  avg_runtime_seconds: 372,
  max_runtime_seconds: 5400,
  median_wait_seconds: 42,
  max_wait_seconds: 318,
  window_seconds: 86400,
};

describe("buildKpiDelta", () => {
  it("returns null for an absent (null) delta", () => {
    expect(buildKpiDelta(null, (v) => `${v}`, false, "1d")).toBeNull();
  });

  it("marks an increase positive when higher is better", () => {
    const d = buildKpiDelta(
      0.021,
      (v) => `${(v * 100).toFixed(1)} pp`,
      false,
      "1d",
    );
    expect(d).toEqual({
      direction: "up",
      tone: "positive",
      text: "2.1 pp",
      srText: "up 2.1 pp vs previous 1d",
    });
  });

  it("marks an increase negative when lower is better (runtime/wait)", () => {
    const d = buildKpiDelta(30, (v) => `${v}s`, true, "7d");
    expect(d?.direction).toBe("up");
    expect(d?.tone).toBe("negative");
    expect(d?.srText).toBe("up 30s vs previous 7d");
  });

  it("marks a decrease positive when lower is better", () => {
    const d = buildKpiDelta(-18, (v) => `${v}s`, true, "1d");
    expect(d?.direction).toBe("down");
    expect(d?.tone).toBe("positive");
    expect(d?.text).toBe("18s");
  });

  it("treats a ~zero delta as flat/neutral", () => {
    const d = buildKpiDelta(0, (v) => `${v}`, false, "1d");
    expect(d?.direction).toBe("flat");
    expect(d?.tone).toBe("neutral");
    expect(d?.srText).toBe("no change vs previous 1d");
  });
});

describe("buildKpiCards", () => {
  const wow: DashboardKpiDelta = {
    current: kpiSummary,
    previous: kpiSummary,
    total_runs_delta: 6,
    succeeded_delta: 7,
    failed_delta: -1,
    success_rate_delta: 0.021,
    throughput_per_hour_delta: 0.4,
    avg_runtime_seconds_delta: -18,
    max_wait_seconds_delta: 26,
    max_runtime_seconds_delta: 0,
    median_wait_seconds_delta: -3,
  };

  it("produces the six KPIs in order with formatted values", () => {
    const cards = buildKpiCards(kpiSummary, wow, 14, 3, "1d");
    expect(cards.map((c) => c.key)).toEqual([
      "success-rate",
      "throughput",
      "avg-runtime",
      "max-wait",
      "queue-depth",
      "running",
    ]);
    expect(cards[0].value).toBe("93.8%");
    expect(cards[1].value).toBe("5.3/hr");
    expect(cards[2].value).toBe("6m 12s"); // 372s
    expect(cards[3].value).toBe("5m 18s"); // 318s
    expect(cards[4].value).toBe("14");
    expect(cards[5].value).toBe("3");
  });

  it("attaches WoW deltas to the four windowed KPIs and none to the live ones", () => {
    const cards = buildKpiCards(kpiSummary, wow, 14, 3, "1d");
    expect(cards[0].delta?.tone).toBe("positive"); // success rate up
    expect(cards[2].delta?.tone).toBe("positive"); // avg runtime down = good
    expect(cards[3].delta?.tone).toBe("negative"); // max wait up = bad
    expect(cards[4].delta).toBeNull();
    expect(cards[5].delta).toBeNull();
  });

  it("renders em-dash placeholders and no deltas when data is null", () => {
    const empty: DashboardKpiSummary = {
      ...kpiSummary,
      success_rate: null,
      throughput_per_hour: null,
      avg_runtime_seconds: null,
      max_wait_seconds: null,
    };
    const cards = buildKpiCards(empty, null, 0, 0, "1d");
    expect(cards[0].value).toBe("—");
    expect(cards[1].value).toBe("—");
    expect(cards[2].value).toBe("—");
    expect(cards[3].value).toBe("—");
    expect(
      cards.every((c) =>
        c.key.startsWith("queue") || c.key === "running"
          ? true
          : c.delta === null,
      ),
    ).toBe(true);
  });
});

describe("totalQueueDepth / runningNow", () => {
  it("sums live queued counts across queues", () => {
    const summary: DashboardQueueHealthSummary = {
      queues: [
        {
          name: "a",
          environment: "production",
          capacity: 4,
          max_queued: null,
          active_count: 3,
          queued_count: 5,
          utilization: 0.75,
          status: "warn",
        },
        {
          name: "b",
          environment: "sandbox",
          capacity: 2,
          max_queued: null,
          active_count: 2,
          queued_count: 9,
          utilization: 1,
          status: "degraded",
        },
        {
          name: "c",
          environment: "production",
          capacity: 6,
          max_queued: null,
          active_count: 1,
          queued_count: 0,
          utilization: 0.17,
          status: "healthy",
        },
      ],
      healthy: 1,
      warn: 1,
      degraded: 1,
      warn_utilization: 0.7,
      degraded_backlog: 8,
    };
    expect(totalQueueDepth(summary)).toBe(14);
  });

  it("clamps a negative global_running to zero", () => {
    const slots = { global_running: -2 } as DashboardExecutionSlots;
    expect(runningNow(slots)).toBe(0);
  });
});

describe("buildRaceJobs", () => {
  const baselines: DashboardWorkflowBaseline[] = [
    {
      workflow_id: "wf-a",
      workflow_name: "A",
      environment: "production",
      sample_count: 30,
      p50_runtime_seconds: 1800,
      mean_runtime_seconds: 1920,
    },
    {
      workflow_id: "wf-b",
      workflow_name: "B",
      environment: "production",
      sample_count: 20,
      p50_runtime_seconds: null,
      mean_runtime_seconds: 3600,
    },
    // wf-c has no baseline row at all.
  ];

  function running(
    workflowId: string,
    name: string,
    startedAt: string,
    env = "production",
  ): MissionControlActivityItem {
    return {
      id: `act-${workflowId}`,
      workflow_id: workflowId,
      workflow_name: name,
      environment: env,
      domain: "ops",
      status: "running",
      started_at: startedAt,
      run_id: `run-${workflowId}`,
    };
  }

  it("joins elapsed (now - started_at) with p50 baseline for the lanes", () => {
    const { jobs } = buildRaceJobs(
      [running("wf-a", "A", "2026-07-04T11:40:00.000Z")],
      baselines,
      NOW_MS,
    );
    expect(jobs).toHaveLength(1);
    expect(jobs[0]).toMatchObject({
      job: "A",
      agent: "production",
      elapsedSeconds: 1200, // 20 minutes
      expectedSeconds: 1800,
      color: "blue",
    });
  });

  it("falls back to the mean when p50 is null", () => {
    const { jobs } = buildRaceJobs(
      [running("wf-b", "B", "2026-07-04T11:00:00.000Z")],
      baselines,
      NOW_MS,
    );
    expect(jobs[0].expectedSeconds).toBe(3600);
  });

  it("flags running jobs with no baseline instead of fabricating a finish line", () => {
    const result = buildRaceJobs(
      [
        running("wf-a", "A", "2026-07-04T11:40:00.000Z"),
        running("wf-c", "C", "2026-07-04T11:50:00.000Z"),
      ],
      baselines,
      NOW_MS,
    );
    expect(result.jobs.map((j) => j.job)).toEqual(["A"]);
    expect(result.missingBaselineCount).toBe(1);
    // Every running job appears in the accessible-table rows regardless.
    expect(result.rows.map((r) => r.job)).toEqual(["A", "C"]);
    expect(result.rows[1].expectedSeconds).toBeNull();
  });

  it("never emits a negative elapsed for a future start", () => {
    const { rows } = buildRaceJobs(
      [running("wf-a", "A", "2026-07-04T12:30:00.000Z")],
      baselines,
      NOW_MS,
    );
    expect(rows[0].elapsedSeconds).toBe(0);
  });
});

describe("statusDonutSegments", () => {
  it("aliases succeeded→success, drops empties, and orders segments", () => {
    const segments = statusDonutSegments([
      { status: "cancelled", count: 2 },
      { status: "succeeded", count: 100 },
      { status: "success", count: 20 },
      { status: "failed", count: 8 },
      { status: "running", count: 3 },
      { status: "queued", count: 0 },
    ]);
    expect(segments.map((s) => s.label)).toEqual([
      "success",
      "failed",
      "running",
      "cancelled",
    ]);
    // succeeded (100) + success (20) merge into one 120 slice.
    expect(segments[0]).toEqual({
      label: "success",
      value: 120,
      color: "var(--success)",
    });
    expect(segments[1].color).toBe("var(--error)");
    expect(segments[2].color).toBe("var(--running)");
  });

  it("returns no segments for an all-zero distribution", () => {
    expect(statusDonutSegments([{ status: "succeeded", count: 0 }])).toEqual(
      [],
    );
  });

  it("totals the raw counts", () => {
    expect(
      statusDistributionTotal([
        { status: "succeeded", count: 120 },
        { status: "failed", count: 8 },
      ]),
    ).toBe(128);
  });
});

describe("trendToChart", () => {
  it("aligns succeeded/failed/total arrays and formats hour buckets as HH:MM (UTC)", () => {
    const series: DashboardTrendSeries = {
      grain: "hour",
      buckets: [
        {
          bucket: "2026-07-04T10:00:00.000Z",
          total: 10,
          failed: 1,
          succeeded: 9,
        },
        {
          bucket: "2026-07-04T11:00:00.000Z",
          total: 12,
          failed: 0,
          succeeded: 12,
        },
      ],
    };
    expect(trendToChart(series)).toEqual({
      categories: ["10:00", "11:00"],
      succeeded: [9, 12],
      failed: [1, 0],
      total: [10, 12],
    });
  });

  it("formats day buckets as MM-DD (UTC)", () => {
    const series: DashboardTrendSeries = {
      grain: "day",
      buckets: [
        {
          bucket: "2026-07-01T00:00:00.000Z",
          total: 3,
          failed: 0,
          succeeded: 3,
        },
      ],
    };
    expect(trendToChart(series).categories).toEqual(["07-01"]);
  });
});

describe("deriveSlaWarning", () => {
  function summary(
    queues: DashboardQueueHealthSummary["queues"],
  ): DashboardQueueHealthSummary {
    return {
      queues,
      healthy: queues.filter((q) => q.status === "healthy").length,
      warn: queues.filter((q) => q.status === "warn").length,
      degraded: queues.filter((q) => q.status === "degraded").length,
      warn_utilization: 0.7,
      degraded_backlog: 8,
    };
  }

  it("returns null when every queue is healthy", () => {
    expect(
      deriveSlaWarning(
        summary([
          {
            name: "a",
            environment: "production",
            capacity: 4,
            max_queued: null,
            active_count: 1,
            queued_count: 0,
            utilization: 0.25,
            status: "healthy",
          },
        ]),
      ),
    ).toBeNull();
  });

  it("summarizes degraded + warn queues with the total backlog, highest level wins", () => {
    const warning = deriveSlaWarning(
      summary([
        {
          name: "default",
          environment: "production",
          capacity: 4,
          max_queued: null,
          active_count: 3,
          queued_count: 5,
          utilization: 0.75,
          status: "warn",
        },
        {
          name: "ml",
          environment: "sandbox",
          capacity: 2,
          max_queued: null,
          active_count: 2,
          queued_count: 9,
          utilization: 1,
          status: "degraded",
        },
      ]),
    );
    expect(warning).toEqual({
      level: "degraded",
      degradedQueues: ["ml"],
      warnQueues: ["default"],
      totalQueued: 14,
      headline: "1 queue degraded · 1 warning · 14 waiting",
    });
  });

  it("uses the warn level when nothing is degraded", () => {
    const warning = deriveSlaWarning(
      summary([
        {
          name: "default",
          environment: "production",
          capacity: 4,
          max_queued: null,
          active_count: 3,
          queued_count: 2,
          utilization: 0.75,
          status: "warn",
        },
      ]),
    );
    expect(warning?.level).toBe("warn");
    expect(warning?.headline).toBe("1 warning · 2 waiting");
  });
});
