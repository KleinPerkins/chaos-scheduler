import { test, expect, type Page } from "@playwright/test";
import {
  gotoDashboard,
  openRunDetail,
  openSidebar,
  openWorkflowRunHistory,
} from "../support/nav";

/**
 * Visual baselines for currently-shipped, stable surfaces at their native
 * window sizes. The Mission Control ("Home") Overview has its own baselines in
 * mission-control-overview.spec.ts; its not-yet-redesigned drill-down tabs stay
 * out of the visual harness until their gated follow-ups.
 *
 * Determinism: the clock is pinned to the fixture "now" so any relative
 * timestamps render identically across runs; fixture data is the fixed mockIPC
 * registry (VITE_PLAYWRIGHT); animations/transitions are disabled via config.
 */

// Matches the fixture `NOW` in src/test/fixtures/data.ts so displayed times are
// stable regardless of when the suite runs.
const FIXTURE_NOW = new Date("2026-07-04T12:00:00.000Z");

async function waitForFonts(page: Page): Promise<void> {
  await page.evaluate(async () => {
    await document.fonts.ready;
  });
}

async function gotoSurface(page: Page, label: string): Promise<void> {
  await gotoDashboard(page);
  await openSidebar(page, label);
  // `aria-current="page"` on the clicked entry confirms the view switched
  // before we screenshot, so the capture never races the transition.
  await expect(
    page
      .getByRole("navigation", { name: "Primary navigation" })
      .getByRole("button", { name: label }),
  ).toHaveAttribute("aria-current", "page");
  await waitForFonts(page);
}

async function installRunDetailVisualFixture(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const startedAt = "2026-07-04T11:42:00.000Z";
    const finishedAt = "2026-07-04T12:00:00.000Z";
    window.__CHAOS_IPC_OVERRIDES__ = {
      ...(window.__CHAOS_IPC_OVERRIDES__ ?? {}),
      get_run_log: () => ({
        id: "run-demo-1",
        workflow_id: "wf-demo-1",
        workflow_name: "Nightly sync",
        started_at: startedAt,
        finished_at: finishedAt,
        exit_code: 1,
        stdout: "Extracted 41 records before the publish step failed.",
        stderr: "Connection refused: analytics.internal:443",
        result_url: "https://example.com/results/run-demo-1",
        status: "failed",
        trigger_kind: "manual",
        error_analysis: {
          diagnosis: "The publish task could not reach the analytics service.",
          likely_cause: "The upstream service was unavailable.",
          recommended_steps: [
            "Check analytics service health.",
            "Retry the workflow after recovery.",
          ],
        },
        summary: {
          title: "Partial synchronization",
          description: "Workflow-emitted results available before the failure.",
          sections: [
            {
              title: "Records",
              type: "stats",
              data: { extracted: 41, published: 0 },
            },
          ],
        },
      }),
      get_run_tasks: () => [
        {
          id: "task-extract",
          run_id: "run-demo-1",
          task_id: "extract",
          status: "succeeded",
          started_at: startedAt,
          finished_at: "2026-07-04T11:48:00.000Z",
          attempt_number: 1,
        },
        {
          id: "task-publish",
          run_id: "run-demo-1",
          task_id: "publish",
          status: "failed",
          started_at: "2026-07-04T11:48:00.000Z",
          finished_at: finishedAt,
          attempt_number: 2,
        },
      ],
      get_run_attempts: () => [
        {
          id: "attempt-extract-1",
          run_id: "run-demo-1",
          task_id: "extract",
          attempt_number: 1,
          status: "succeeded",
          started_at: startedAt,
          finished_at: "2026-07-04T11:48:00.000Z",
        },
        {
          id: "attempt-publish-2",
          run_id: "run-demo-1",
          task_id: "publish",
          attempt_number: 2,
          status: "failed",
          started_at: "2026-07-04T11:48:00.000Z",
          finished_at: finishedAt,
          error_type: "ConnectionError",
          error_message: "Service unavailable",
        },
      ],
      get_run_metrics: () => [
        {
          id: "metric-records",
          run_id: "run-demo-1",
          task_id: "extract",
          metric_name: "records_extracted",
          metric_value: 41,
          metric_unit: "records",
          emitted_at: "2026-07-04T11:48:00.000Z",
        },
      ],
      get_run_relationships: () => [
        {
          id: "relationship-index",
          parent_run_id: "run-demo-1",
          child_run_id: "run-child-1",
          child_workflow_id: "wf-index-1",
          child_workflow_name: "Index refresh",
          relationship: "child",
          task_id: "publish",
          wait: true,
          status: "succeeded",
          created_at: startedAt,
          updated_at: finishedAt,
        },
      ],
    };
  });
}

test.describe("stable surfaces — main window (960x680)", () => {
  test.use({ viewport: { width: 960, height: 680 } });

  test.beforeEach(async ({ page }) => {
    await page.clock.setFixedTime(FIXTURE_NOW);
  });

  // [nav label, baseline slug]
  const surfaces: ReadonlyArray<readonly [string, string]> = [
    ["Workflows", "workflows"],
    ["Queues", "queues"],
    ["Environments", "environments"],
    ["Integrations", "integrations"],
    ["Settings", "settings"],
  ];

  for (const [label, slug] of surfaces) {
    test(slug, async ({ page }) => {
      await gotoSurface(page, label);
      await expect(page).toHaveScreenshot(`${slug}.png`);
    });
  }

  test("global-history dark and light", async ({ page }) => {
    await gotoSurface(page, "History");
    await expect(
      page.getByRole("heading", { name: "Global History" }),
    ).toBeVisible();
    await waitForFonts(page);
    await expect(page).toHaveScreenshot("global-history.png");

    await page.getByRole("button", { name: "Light theme" }).click();
    await expect(page.locator('html[data-theme="light"]')).toHaveCount(1);
    await expect(page).toHaveScreenshot("global-history-light.png");
  });

  test("workflow-history dark and light", async ({ page }) => {
    await page.addInitScript(() => {
      window.__CHAOS_IPC_OVERRIDES__ = {
        ...(window.__CHAOS_IPC_OVERRIDES__ ?? {}),
        get_workflow_history_buckets: () =>
          Array.from({ length: 30 }, (_, index) => {
            const failed = index === 6 || index === 18 ? 1 : 0;
            return {
              day: `2026-06-${String(index + 1).padStart(2, "0")}`,
              total: 1,
              failed,
              succeeded: 1 - failed,
            };
          }),
      };
    });
    await openWorkflowRunHistory(page);
    await waitForFonts(page);
    await expect(page).toHaveScreenshot("workflow-history-dark.png");

    await page.getByRole("button", { name: "Light theme" }).click();
    await expect(page.locator('html[data-theme="light"]')).toHaveCount(1);
    await expect(page).toHaveScreenshot("workflow-history-light.png");
  });

  test("run-detail dark and light", async ({ page }) => {
    await installRunDetailVisualFixture(page);
    await openRunDetail(page);
    await waitForFonts(page);
    await expect(page).toHaveScreenshot("run-detail-dark.png");
    await page.setViewportSize({ width: 960, height: 1800 });
    await waitForFonts(page);
    await expect(page).toHaveScreenshot("run-detail-full-dark.png");

    await page.setViewportSize({ width: 960, height: 680 });
    await page.getByRole("button", { name: "Light theme" }).click();
    await expect(page.locator('html[data-theme="light"]')).toHaveCount(1);
    await expect(page).toHaveScreenshot("run-detail-light.png");
    await page.setViewportSize({ width: 960, height: 1800 });
    await waitForFonts(page);
    await expect(page).toHaveScreenshot("run-detail-full-light.png");
  });

  test("workflow-detail dark and light", async ({ page }) => {
    await gotoSurface(page, "Workflows");
    await page
      .getByRole("button", { name: "Nightly sync", exact: true })
      .click();
    await expect(
      page.getByRole("heading", { name: "Latest run" }),
    ).toBeVisible();
    await waitForFonts(page);
    await expect(page).toHaveScreenshot("workflow-detail-dark.png");

    await page.getByRole("button", { name: "Light theme" }).click();
    await expect(page.locator('html[data-theme="light"]')).toHaveCount(1);
    await expect(page).toHaveScreenshot("workflow-detail-light.png");
  });

  test("workflow-editor dark and light", async ({ page }) => {
    await gotoSurface(page, "Workflows");
    await page
      .getByRole("button", { name: "Nightly sync", exact: true })
      .click();
    await page.getByRole("button", { name: "Edit workflow" }).click();
    await expect(
      page.getByRole("heading", { name: "Edit workflow" }),
    ).toBeVisible();
    await waitForFonts(page);
    await expect(page).toHaveScreenshot("workflow-editor-dark.png");

    await page.getByRole("button", { name: "Light theme" }).click();
    await expect(page.locator('html[data-theme="light"]')).toHaveCount(1);
    await expect(page).toHaveScreenshot("workflow-editor-light.png");
  });
});

test.describe("stable surfaces — menu bar popup (384x590)", () => {
  test.use({ viewport: { width: 384, height: 590 } });

  test.beforeEach(async ({ page }) => {
    await page.clock.setFixedTime(FIXTURE_NOW);
  });

  test("popup", async ({ page }) => {
    await page.addInitScript(() => {
      window.__CHAOS_IPC_OVERRIDES__ = {
        ...(window.__CHAOS_IPC_OVERRIDES__ ?? {}),
        // The mini-dashboard sources its glance from the Mission Control
        // snapshot (running/failed tallies, live activity, upcoming, recent)
        // plus the live queue depth from list_queued_runs.
        get_mission_control_snapshot: () => ({
          preferences: {
            default_landing: "mission_control",
            environment_filter: "all",
            domain_filter: "all",
          },
          domains: [],
          header: {
            active_workflows: 4,
            running_count: 2,
            queued_count: 3,
            recent_failures: 1,
          },
          sla: {
            violations_count: 0,
            success_rate_24h: 1,
            median_wait_seconds: 0,
            max_wait_seconds: 0,
            long_running_count: 0,
            blocked_count: 0,
          },
          needs_attention: [],
          needs_attention_total: 0,
          needs_attention_truncated: false,
          live_activity: [
            {
              id: "act-nightly",
              workflow_id: "wf-nightly-sync",
              workflow_name: "Nightly sync",
              environment: "production",
              domain: "ops",
              status: "running",
              started_at: "2026-07-04T11:40:00.000Z",
              finished_at: null,
              run_id: "run-live-nightly",
            },
            {
              id: "act-etl",
              workflow_id: "wf-etl-rollup",
              workflow_name: "ETL rollup",
              environment: "production",
              domain: "data",
              status: "running",
              started_at: "2026-07-04T11:16:00.000Z",
              finished_at: null,
              run_id: "run-live-etl",
            },
          ],
          upcoming_runs: [
            {
              workflow_id: "wf-nightly-sync",
              workflow_name: "Nightly sync",
              environment: "production",
              domain: "ops",
              trigger_kind: "cron",
              trigger_label: "0 15 * * *",
              next_time: "2026-07-04T15:00:00.000Z",
            },
            {
              workflow_id: "wf-weekly-report",
              workflow_name: "Weekly report",
              environment: "production",
              domain: "ops",
              trigger_kind: "cron",
              trigger_label: "0 8 * * 1",
              next_time: "2026-07-05T08:00:00.000Z",
            },
          ],
          freshness_ledger: [],
          recent_runs: [
            {
              id: "run-demo-1",
              workflow_id: "wf-nightly-sync",
              workflow_name: "Nightly sync",
              started_at: "2026-07-04T11:30:00.000Z",
              finished_at: "2026-07-04T11:38:00.000Z",
              exit_code: 0,
              stdout: "ok",
              stderr: null,
              result_url: null,
              status: "succeeded",
              trigger_kind: "manual",
            },
            {
              id: "run-demo-2",
              workflow_id: "wf-data-export",
              workflow_name: "Data export",
              started_at: "2026-07-04T11:05:00.000Z",
              finished_at: "2026-07-04T11:12:00.000Z",
              exit_code: 1,
              stdout: null,
              stderr: "boom",
              result_url: null,
              status: "failed",
              trigger_kind: "manual",
            },
          ],
          workflow_telemetry: [],
          availability: [],
        }),
        list_queued_runs: () => [
          {
            id: "q1",
            workflow_id: "wf-ingest",
            workflow_name: "Ingest fan-out",
            queue_name: "default",
            environment: "production",
            priority: 0,
            status: "queued",
            queued_at: "2026-07-04T11:58:00.000Z",
          },
          {
            id: "q2",
            workflow_id: "wf-search-index",
            workflow_name: "Search reindex",
            queue_name: "ml",
            environment: "sandbox",
            priority: 0,
            status: "queued",
            queued_at: "2026-07-04T11:59:00.000Z",
          },
          {
            id: "q3",
            workflow_id: "wf-weekly-report",
            workflow_name: "Weekly report",
            queue_name: "default",
            environment: "production",
            priority: 0,
            status: "queued",
            queued_at: "2026-07-04T11:59:30.000Z",
          },
        ],
        get_app_update_status: () => ({
          updater_available: true,
          phase: "available",
          current_version: "1.1.0",
          latest_version: "1.2.0",
          notes: "Bug fixes and improvements.",
          last_checked_at: "2026-07-04T12:00:00.000Z",
          last_error: null,
          progress: null,
          background_check_enabled: true,
          skipped_version: null,
        }),
      };
    });
    await page.goto("/?view=popup");
    await expect(page.getByText("Chaos Scheduler")).toBeVisible();
    await waitForFonts(page);
    await expect(page).toHaveScreenshot("popup.png");
  });
});
