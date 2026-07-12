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

test.describe("stable surfaces — menu bar popup (340x440)", () => {
  test.use({ viewport: { width: 340, height: 440 } });

  test.beforeEach(async ({ page }) => {
    await page.clock.setFixedTime(FIXTURE_NOW);
  });

  test("popup", async ({ page }) => {
    await page.goto("/?view=popup");
    await expect(page.getByText("Chaos Scheduler")).toBeVisible();
    await waitForFonts(page);
    await expect(page).toHaveScreenshot("popup.png");
  });
});
