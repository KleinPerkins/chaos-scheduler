import { describe, expect, it } from "vitest";
import {
  blastRadiusBars,
  blockReasonBars,
  blockReasonLabel,
  failureRows,
  formatFailureRate,
  heavyBlockerBars,
  needsAttentionSummary,
} from "./needsAttentionData";
import {
  sampleDashboardBlastRadius,
  sampleDashboardBlockTaxonomy,
  sampleDashboardFailureRecurrence,
} from "../../test/fixtures/data";
import type {
  DashboardBlastRadius,
  DashboardBlockTaxonomy,
} from "../../lib/commands";

const EMPTY_TAXONOMY: DashboardBlockTaxonomy = {
  by_reason: [],
  current_blocked_count: 0,
  current_wait_seconds_total: 0,
  current_wait_seconds_max: 0,
  trailing_wait_seconds_avg: null,
  trailing_wait_seconds_max: null,
  heavy_blockers: [],
};

describe("blockReasonLabel", () => {
  it("maps known categories to friendly labels and passes unknowns through", () => {
    expect(blockReasonLabel("resource")).toBe("Resource lock");
    expect(blockReasonLabel("event")).toBe("Event wait");
    expect(blockReasonLabel("mystery")).toBe("mystery");
  });
});

describe("blockReasonBars", () => {
  it("builds one ranked bar per non-empty reason with Σ-wait value + job count label", () => {
    const bars = blockReasonBars(sampleDashboardBlockTaxonomy.by_reason);
    expect(bars).toHaveLength(3);
    expect(bars[0]).toMatchObject({
      label: "Resource lock",
      value: 9720,
      valueLabel: "5 jobs · Σ 2h 42m",
    });
    // color binds to a token var, never raw hex
    expect(bars[0].color).toBe("var(--chart-1)");
    expect(bars[2]).toMatchObject({ label: "Host / worker", value: 720 });
  });

  it("drops empty reason categories", () => {
    expect(
      blockReasonBars([
        { reason_category: "user", count: 0, current_wait_seconds_total: 0 },
      ]),
    ).toEqual([]);
  });

  it("singularizes a one-job category", () => {
    const [bar] = blockReasonBars([
      { reason_category: "resource", count: 1, current_wait_seconds_total: 30 },
    ]);
    expect(bar.valueLabel).toBe("1 job · Σ 30s");
  });
});

describe("heavyBlockerBars", () => {
  it("ranks blockers by Σ wait and labels wait held + jobs blocked", () => {
    const bars = heavyBlockerBars(sampleDashboardBlockTaxonomy.heavy_blockers);
    expect(bars).toHaveLength(3);
    expect(bars[0]).toMatchObject({
      label: "ETL rollup",
      value: 8100,
      valueLabel: "Σ 2h 15m · 4 jobs",
      color: "var(--warning)",
    });
  });

  it("returns nothing when there are no heavy blockers", () => {
    expect(heavyBlockerBars([])).toEqual([]);
  });
});

describe("blastRadiusBars", () => {
  it("keeps only workflows that reach a downstream dependent", () => {
    const bars = blastRadiusBars(sampleDashboardBlastRadius);
    // the zero-downstream "Standalone check" row is excluded
    expect(bars.map((b) => b.label)).toEqual([
      "Ingest fan-out",
      "ETL rollup",
      "Nightly sync",
    ]);
    expect(bars[0]).toMatchObject({
      value: 9,
      valueLabel: "9 downstream · depth 4",
      color: "var(--chart-4)",
    });
  });

  it("returns nothing when no workflow has downstream reach", () => {
    const noReach: DashboardBlastRadius[] = [
      {
        workflow_id: "wf-x",
        workflow_name: "X",
        environment: "production",
        runs_considered: 3,
        max_downstream_count: 0,
        avg_downstream_count: 0,
        max_depth: 0,
      },
    ];
    expect(blastRadiusBars(noReach)).toEqual([]);
  });
});

describe("failureRows", () => {
  it("preserves worst-first order and computes the failure rate", () => {
    const rows = failureRows(sampleDashboardFailureRecurrence);
    expect(rows[0]).toMatchObject({
      workflowName: "ETL rollup",
      failureCount: 6,
      totalRuns: 30,
      failureRate: 0.2,
    });
  });

  it("yields a null rate when the window had no runs", () => {
    const [row] = failureRows([
      {
        workflow_id: "wf-x",
        workflow_name: "X",
        environment: "production",
        failure_count: 2,
        total_runs: 0,
      },
    ]);
    expect(row.failureRate).toBeNull();
    expect(formatFailureRate(row.failureRate)).toBe("—");
  });
});

describe("needsAttentionSummary", () => {
  it("summarizes the three bindings with a critical tone when work is failing", () => {
    const summary = needsAttentionSummary(
      sampleDashboardBlockTaxonomy,
      sampleDashboardBlastRadius,
      sampleDashboardFailureRecurrence,
    );
    expect(summary).toMatchObject({
      blockedCount: 9,
      totalFailures: 10,
      failingWorkflowCount: 3,
      tone: "critical",
    });
    expect(summary.heaviestBlocker).toEqual({
      name: "ETL rollup",
      sigmaWaitSeconds: 8100,
    });
    expect(summary.topBlastRadius).toEqual({
      name: "Ingest fan-out",
      downstream: 9,
      depth: 4,
    });
    expect(summary.headline).toBe(
      "9 jobs waiting · 10 failures across 3 workflows · widest blast radius 9 runs",
    );
  });

  it("is warn (not critical) when work is only blocked, and clear when nothing", () => {
    const warnOnly = needsAttentionSummary(
      { ...sampleDashboardBlockTaxonomy, heavy_blockers: [] },
      [],
      [],
    );
    expect(warnOnly.tone).toBe("warn");

    const clear = needsAttentionSummary(EMPTY_TAXONOMY, [], []);
    expect(clear.tone).toBe("clear");
    expect(clear.headline).toBe("Nothing needs attention");
    expect(clear.heaviestBlocker).toBeNull();
    expect(clear.topBlastRadius).toBeNull();
  });
});
