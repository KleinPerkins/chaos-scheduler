import { test, expect } from "@playwright/test";
import { expectNoAxeViolations } from "./support/axe";

test.describe("Menu bar popup", () => {
  test("renders scheduler status in popup view", async ({ page }) => {
    await page.goto("/?view=popup");
    await expect(page.getByText("Chaos Scheduler")).toBeVisible();
    await expectNoAxeViolations(page, "menu bar popup");
  });

  test("announces an initial status-load failure as an alert", async ({
    page,
  }) => {
    await page.addInitScript(() => {
      window.__CHAOS_IPC_OVERRIDES__ = {
        // The mini-dashboard glance is sourced from the Mission Control
        // snapshot, so a failed snapshot fetch is what must be announced.
        get_mission_control_snapshot: () => {
          throw new Error("scheduler offline");
        },
      };
    });

    await page.goto("/?view=popup");

    // The failed async status fetch must be announced, not shown silently.
    await expect(page.getByRole("alert")).toContainText(
      /Status failed to load/,
    );
    await expectNoAxeViolations(page, "menu bar popup load error");
  });

  test("queues upcoming work with semantic regions in both themes", async ({
    page,
  }) => {
    await page.addInitScript(() => {
      window.__CHAOS_IPC_OVERRIDES__ = {
        get_mission_control_snapshot: () => ({
          preferences: {
            default_landing: "mission_control",
            environment_filter: "all",
            domain_filter: "all",
          },
          domains: [],
          header: {
            active_workflows: 1,
            running_count: 0,
            queued_count: 0,
            recent_failures: 0,
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
          live_activity: [],
          upcoming_runs: [
            {
              workflow_id: "wf-popup",
              workflow_name: "Nightly sync",
              environment: "production",
              domain: "ops",
              trigger_kind: "cron",
              trigger_label: "0 8 * * *",
              next_time: "2026-07-12T08:00:00.000Z",
            },
          ],
          freshness_ledger: [],
          recent_runs: [],
          workflow_telemetry: [],
          availability: [],
        }),
        enqueue_workflow: () => ({
          status: "queued",
          queued_run_id: "queued-popup-1",
        }),
      };
    });

    await page.goto("/?view=popup");

    await expect(page.getByRole("region", { name: "Upcoming" })).toBeVisible();
    await expect(page.getByRole("region", { name: "Recent" })).toBeVisible();
    await expect(
      page.getByRole("group", { name: "Run summary" }),
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
