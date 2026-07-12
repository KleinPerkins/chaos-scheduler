import { test, expect } from "@playwright/test";
import { expectNoAxeViolations } from "./support/axe";
import { openGlobalHistory } from "./support/nav";

test.describe("Global History", () => {
  test("load error shows retry affordance", async ({ page }) => {
    await page.addInitScript(() => {
      window.__CHAOS_IPC_OVERRIDES__ = {
        get_global_run_history: () => {
          throw new Error("index unavailable");
        },
      };
    });
    await openGlobalHistory(page);

    await expect(
      page.getByRole("heading", { name: "Global History" }),
    ).toBeVisible();
    // The failed async load must be announced as an alert, not a silent div.
    await expect(page.getByRole("alert")).toContainText(
      /History failed to load/,
    );
    await expect(page.getByText(/index unavailable/)).toBeVisible();
    await expect(page.getByRole("button", { name: "Retry" })).toBeVisible();
    await expectNoAxeViolations(page, "global history error");
  });

  test("status filter live-scopes the bounded query", async ({ page }) => {
    await openGlobalHistory(page);

    await expect(
      page.getByRole("heading", { name: "Global History" }),
    ).toBeVisible();
    // The results surface is exposed as a named region (aria-labelledby the
    // "Latest runs" heading); the runs table lives inside it. The table markup
    // itself carries no accessible name today, so we assert the region name.
    await expect(
      page.getByRole("region", { name: "Latest runs" }),
    ).toBeVisible();
    await expect(page.getByText("Latest 100", { exact: true })).toBeVisible();

    await page.getByLabel("Status").selectOption("poll_exhausted");

    await expect(page.locator(".status-badge.poll_exhausted")).toBeVisible();
    await expect(page.getByText("1 loaded · newest first")).toBeVisible();
    await expectNoAxeViolations(page, "global history poll_exhausted");

    await page.getByRole("button", { name: "Light theme" }).click();
    await expect(page.locator('html[data-theme="light"]')).toHaveCount(1);
    await expectNoAxeViolations(page, "global history light");
  });
});
