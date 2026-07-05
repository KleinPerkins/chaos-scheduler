import { test, expect } from "@playwright/test";
import { expectNoAxeViolations } from "./support/axe";

test.describe("Menu bar popup", () => {
  test("renders scheduler status in popup view", async ({ page }) => {
    await page.goto("/?view=popup");
    await expect(page.getByText("Chaos Scheduler")).toBeVisible();
    await expectNoAxeViolations(page, "menu bar popup");
  });
});
