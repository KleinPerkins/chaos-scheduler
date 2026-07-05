import { test, expect } from "@playwright/test";
import { expectNoAxeViolations } from "./support/axe";
import { gotoDashboard } from "./support/nav";

const TABS = [
  "overview",
  "activity",
  "freshness",
  "telemetry",
  "matrix",
] as const;

test.describe("Mission Control tabs", () => {
  test.beforeEach(async ({ page }) => {
    await gotoDashboard(page);
    await expect(
      page.getByRole("tablist", { name: "Mission Control tabs" }),
    ).toBeVisible();
  });

  for (const tab of TABS) {
    test(`tab ${tab} renders panel and passes axe`, async ({ page }) => {
      await page.getByRole("tab", { name: tab }).click();
      const panel = page.locator(`#mc-panel-${tab}`);
      await expect(panel).toBeVisible();
      await expectNoAxeViolations(page, `mission control ${tab}`);
    });
  }
});
