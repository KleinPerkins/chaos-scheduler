import { test, expect } from "@playwright/test";
import { expectNoAxeViolations } from "./support/axe";
import { gotoDashboard, openSidebar } from "./support/nav";

async function openWorkflowEditor(
  page: Parameters<typeof gotoDashboard>[0],
): Promise<void> {
  await gotoDashboard(page);
  await openSidebar(page, "Workflows");
  await page.getByRole("button", { name: "Nightly sync", exact: true }).click();
  await page.getByRole("button", { name: "Edit workflow" }).click();
}

test.describe("Workflow editor", () => {
  test("preserves the full editor behind the approved scan-first hierarchy", async ({
    page,
  }) => {
    await openWorkflowEditor(page);

    await expect(
      page.getByRole("heading", { name: "Edit workflow" }),
    ).toBeVisible();
    await expect(page.getByRole("region", { name: "General" })).toBeVisible();
    await expect(page.getByRole("region", { name: "Schedule" })).toBeVisible();
    await expect(
      page.getByRole("region", { name: "Runtime and notifications" }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Save changes" }),
    ).toHaveCount(1);
    await expect(page.getByRole("button", { name: "Cancel" })).toHaveCount(1);
    await expectNoAxeViolations(page, "workflow editor dark");

    await page
      .getByText("Execution details · Generic step flow", { exact: true })
      .click();
    await expect(
      page.getByRole("group", { name: "Workflow type" }),
    ).toBeVisible();

    await page.getByRole("button", { name: "Light theme" }).click();
    await expect(page.locator('html[data-theme="light"]')).toHaveCount(1);
    await expectNoAxeViolations(page, "workflow editor light");
  });
});
