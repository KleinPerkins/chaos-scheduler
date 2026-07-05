import { test, expect } from "@playwright/test";
import { expectNoAxeViolations } from "./support/axe";
import { gotoDashboard, openSidebar } from "./support/nav";

test.describe("Accessibility — views without dedicated feature specs", () => {
  test("Environments view passes axe", async ({ page }) => {
    await gotoDashboard(page);
    await openSidebar(page, "Environments");
    await expect(
      page.getByRole("heading", { name: "Environments", level: 1 }),
    ).toBeVisible();
    await expectNoAxeViolations(page, "environments");
  });

  test("Queues view passes axe", async ({ page }) => {
    await gotoDashboard(page);
    await openSidebar(page, "Queues");
    await expect(page.getByRole("heading", { name: "Queues" })).toBeVisible();
    await expectNoAxeViolations(page, "queues");
  });
});
