import { test, expect } from "@playwright/test";
import { expectNoAxeViolations } from "./support/axe";
import { gotoDashboard, openSidebar, workflowCard } from "./support/nav";

test.describe("Run detail smoke", () => {
  test("Dashboard history opens run detail via get_run_log", async ({
    page,
  }) => {
    await gotoDashboard(page);
    await openSidebar(page, "Workflows");
    await workflowCard(page).getByRole("button", { name: "History" }).click();

    await expect(
      page.getByRole("heading", { name: "Nightly sync" }),
    ).toBeVisible();
    await expect(page.getByText("Run History")).toBeVisible();
    await page
      .getByRole("button", {
        name: /View details for .* run started/i,
      })
      .first()
      .click();

    await expect(page.getByText("SUCCEEDED")).toBeVisible();
    await expect(page.getByText(/Started /)).toBeVisible();
    await expectNoAxeViolations(page, "run detail");
  });
});
