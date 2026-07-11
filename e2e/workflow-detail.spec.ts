import { test, expect } from "@playwright/test";
import { expectNoAxeViolations } from "./support/axe";
import { gotoDashboard, openSidebar } from "./support/nav";

test.describe("Unified workflow detail", () => {
  test("opens the detail hub from the workflow list and drills into a run", async ({
    page,
  }) => {
    await gotoDashboard(page);
    await openSidebar(page, "Workflows");
    await expect(page.getByText("Nightly sync")).toBeVisible();

    // Open the unified detail from the card title.
    await page
      .getByRole("button", { name: "Nightly sync", exact: true })
      .click();

    const detail = page.locator(".workflow-detail");
    await expect(detail).toBeVisible();
    await expect(
      detail.getByRole("heading", { name: "Configuration" }),
    ).toBeVisible();
    await expect(
      detail.getByRole("heading", { name: "Latest run" }),
    ).toBeVisible();
    await expect(
      detail.getByRole("button", { name: "Edit workflow" }),
    ).toBeVisible();
    await expect(
      detail.getByRole("button", { name: "View latest run" }),
    ).toBeVisible();
    await expect(
      detail.getByRole("heading", { name: "Recent runs" }),
    ).toBeVisible();
    await expectNoAxeViolations(page, "unified workflow detail");
    await page.getByRole("button", { name: "Light theme" }).click();
    await expect(page.locator('html[data-theme="light"]')).toHaveCount(1);
    await expectNoAxeViolations(page, "unified workflow detail light");

    // Drill into a run's detail from the recent-runs table.
    await detail
      .locator(".wd-runs-table")
      .getByRole("button", { name: /View details for run/ })
      .click();
    await expect(page.getByText(/Raw Logs|Run/)).toBeVisible();
  });
});
