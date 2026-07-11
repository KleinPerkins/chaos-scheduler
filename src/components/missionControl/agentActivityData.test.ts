import { describe, expect, it } from "vitest";
import type { MissionControlActivityItem } from "../../lib/commands";
import {
  recentFailures,
  runningActivity,
  sampleUpcomingRuns,
} from "../../test/fixtures/data";
import {
  activityCounts,
  failureRows,
  isFailure,
  isRunning,
  runningRows,
  upcomingRows,
} from "./agentActivityData";

const NOW = new Date("2026-07-04T12:00:00.000Z").getTime();
const LIVE = [...runningActivity, ...recentFailures];

describe("isRunning / isFailure", () => {
  it("classifies running", () => {
    expect(isRunning("running")).toBe(true);
    expect(isRunning("failed")).toBe(false);
  });

  it("classifies terminal failure states, excluding cancellation + success", () => {
    expect(isFailure("failed")).toBe(true);
    expect(isFailure("error")).toBe(true);
    expect(isFailure("timed_out")).toBe(true);
    expect(isFailure("poll_exhausted")).toBe(true);
    // A deliberate stop and a success are not failures.
    expect(isFailure("cancelled")).toBe(false);
    expect(isFailure("success")).toBe(false);
    expect(isFailure("succeeded")).toBe(false);
    expect(isFailure("running")).toBe(false);
  });
});

describe("runningRows", () => {
  it("keeps only running items, longest-running (earliest start) first, with elapsed labels", () => {
    const rows = runningRows(LIVE, NOW);
    expect(rows.map((r) => r.name)).toEqual([
      "ML scoring", // 10:42 → 1h 18m
      "ETL rollup", // 11:16 → 44m
      "Nightly sync", // 11:40 → 20m
    ]);
    expect(rows[0].timeLabel).toBe("for 1h 18m");
    expect(rows[1].timeLabel).toBe("for 44m 0s");
    expect(rows[2].timeLabel).toBe("for 20m 0s");
    expect(rows[0].sub).toBe("ml / sandbox");
    expect(rows[0].runId).toBe("run-live-ml");
  });

  it("excludes failures", () => {
    expect(runningRows(recentFailures, NOW)).toHaveLength(0);
  });
});

describe("failureRows", () => {
  it("keeps only failures, most-recent first, with 'ago' labels + status labels", () => {
    const rows = failureRows(LIVE, NOW);
    expect(rows.map((r) => r.name)).toEqual([
      "Data export", // finished 11:50 → 10m ago
      "Search reindex", // finished 11:35 → 25m ago
    ]);
    expect(rows[0].timeLabel).toBe("10m 0s ago");
    expect(rows[1].timeLabel).toBe("25m 0s ago");
    // timed_out gets the friendly label.
    expect(rows[1].statusLabel).toBe("timed out");
    expect(rows[0].sub).toBe("data / production");
  });

  it("falls back to started_at when finished_at is null", () => {
    const item: MissionControlActivityItem = {
      id: "x",
      workflow_id: "wf-x",
      workflow_name: "X",
      environment: "production",
      domain: "ops",
      status: "failed",
      started_at: "2026-07-04T11:30:00.000Z",
      finished_at: null,
      run_id: "run-x",
    };
    expect(failureRows([item], NOW)[0].timeLabel).toBe("30m 0s ago");
  });
});

describe("upcomingRows", () => {
  it("sorts soonest first with ETA labels", () => {
    const rows = upcomingRows(sampleUpcomingRuns, NOW);
    expect(rows.map((r) => r.name)).toEqual(["Nightly sync", "Weekly report"]);
    expect(rows[0].etaLabel).toBe("in 3h 0m");
    expect(rows[1].etaLabel).toBe("in 20h 0m");
    expect(rows[0].sub).toBe("ops / 0 15 * * *");
  });

  it("labels an at/behind trigger 'due now'", () => {
    const rows = upcomingRows(sampleUpcomingRuns, NOW + 4 * 60 * 60 * 1000);
    // Nightly (15:00) is now 1h in the past → due now; weekly still ahead.
    expect(rows[0].etaLabel).toBe("due now");
    expect(rows[1].etaLabel).toContain("in ");
  });
});

describe("activityCounts", () => {
  it("counts running, upcoming, and failures independently", () => {
    expect(activityCounts(LIVE, sampleUpcomingRuns)).toEqual({
      running: 3,
      upcoming: 2,
      failures: 2,
    });
  });

  it("is all-zero for an empty snapshot", () => {
    expect(activityCounts([], [])).toEqual({
      running: 0,
      upcoming: 0,
      failures: 0,
    });
  });
});
