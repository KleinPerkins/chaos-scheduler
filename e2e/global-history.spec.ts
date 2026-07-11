import { test, expect } from "@playwright/test";
import { expectNoAxeViolations } from "./support/axe";
import { gotoDashboard, openSidebar } from "./support/nav";

test.describe("Global History", () => {
  test("load error shows retry affordance", async ({ page }) => {
    await page.addInitScript(() => {
      window.__CHAOS_IPC_OVERRIDES__ = {
        get_global_run_history: () => {
          throw new Error("index unavailable");
        },
      };
    });
    await gotoDashboard(page);
    await openSidebar(page, "History");

    await expect(page.getByRole("heading", { name: "History" })).toBeVisible();
    await expect(page.getByText(/index unavailable/)).toBeVisible();
    await expect(page.getByRole("button", { name: "Retry" })).toBeVisible();
    await expectNoAxeViolations(page, "global history error");
  });

  test("status filter live-scopes the bounded query", async ({ page }) => {
    await gotoDashboard(page);
    await openSidebar(page, "History");

    await expect(page.getByRole("heading", { name: "History" })).toBeVisible();
    await expect(page.getByText("Latest 100")).toBeVisible();

    await page.getByLabel("Status").selectOption("poll_exhausted");

    await expect(page.locator(".status-badge.poll_exhausted")).toBeVisible();
    await expect(page.getByText("1 loaded · newest first")).toBeVisible();
    await expectNoAxeViolations(page, "global history poll_exhausted");
  });
});
