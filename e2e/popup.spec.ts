import { test, expect } from "@playwright/test";
import { expectNoAxeViolations } from "./support/axe";

test.describe("Menu bar popup", () => {
  test("renders scheduler status in popup view", async ({ page }) => {
    await page.goto("/?view=popup");
    await expect(page.getByText("Chaos Scheduler")).toBeVisible();
    await expectNoAxeViolations(page, "menu bar popup");
  });

  test("queues upcoming work with semantic regions in both themes", async ({
    page,
  }) => {
    await page.addInitScript(() => {
      window.__CHAOS_IPC_OVERRIDES__ = {
        get_scheduler_status: () => ({
          active_workflows: 1,
          running_count: 0,
          next_runs: [
            {
              workflow_id: "wf-popup",
              workflow_name: "Nightly sync",
              environment: "production",
              next_time: "2026-07-12T08:00:00.000Z",
            },
          ],
          recent_runs: [],
        }),
        enqueue_workflow: () => ({
          status: "queued",
          queued_run_id: "queued-popup-1",
        }),
      };
    });

    await page.goto("/?view=popup");

    await expect(page.getByRole("region", { name: "Next Runs" })).toBeVisible();
    await expect(
      page.getByRole("region", { name: "Recent Results" }),
    ).toBeVisible();
    await page.getByRole("button", { name: "Queue run Nightly sync" }).click();
    await expect(
      page.getByRole("status").filter({
        hasText: "Waiting to start: Nightly sync",
      }),
    ).toBeVisible();
    await expectNoAxeViolations(page, "menu bar popup dark");

    await page.evaluate(() => {
      localStorage.setItem("chaos-theme", "light");
      window.dispatchEvent(new StorageEvent("storage", { key: "chaos-theme" }));
    });
    await expect(page.locator('html[data-theme="light"]')).toHaveCount(1);
    await expectNoAxeViolations(page, "menu bar popup light");
  });
});
