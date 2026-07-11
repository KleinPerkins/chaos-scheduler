import { test, expect } from "@playwright/test";
import { expectNoAxeViolations } from "./support/axe";
import { openWorkflowRunHistory } from "./support/nav";

test.describe("Workflow run history", () => {
  test("keeps bounded history keyboard-accessible in both themes", async ({
    page,
  }) => {
    await openWorkflowRunHistory(page);

    await expect(
      page.getByRole("heading", { name: "30-day failure history" }),
    ).toBeVisible();
    await expect(page.getByText("Latest 50", { exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Queue run" })).toBeVisible();
    await expect(
      page.getByRole("table", { name: "Latest runs for Nightly sync" }),
    ).toBeVisible();

    const search = page.getByLabel("Search loaded rows");
    const status = page.getByLabel("Status");
    await search.focus();
    await page.keyboard.press("Tab");
    await expect(status).toBeFocused();

    await expectNoAxeViolations(page, "workflow run history dark");
    await page.getByRole("button", { name: "Light theme" }).click();
    await expect(page.locator('html[data-theme="light"]')).toHaveCount(1);
    await expectNoAxeViolations(page, "workflow run history light");
  });
});
