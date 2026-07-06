import { test, expect } from "@playwright/test";
import { expectNoAxeViolations } from "./support/axe";
import { gotoDashboard, openSidebar } from "./support/nav";

test.describe("Enqueue workflow", () => {
  test("enqueue shows success notice with queued run id", async ({ page }) => {
    await gotoDashboard(page);
    await openSidebar(page, "Workflows");
    await expect(page.getByText("Nightly sync")).toBeVisible();

    await page.getByRole("button", { name: "Enqueue Nightly sync" }).click();
    await expect(
      page.getByText(/Queued Nightly sync \(queue-fi/),
    ).toBeVisible();
    await expectNoAxeViolations(page, "enqueue success");
  });
});
