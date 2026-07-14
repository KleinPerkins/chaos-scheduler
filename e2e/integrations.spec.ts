import { test, expect } from "@playwright/test";
import { expectNoAxeViolations } from "./support/axe";
import { gotoDashboard, openSidebar } from "./support/nav";

test.describe("Integrations", () => {
  test("create key surfaces token without a credential-bearing MCP config", async ({
    page,
  }) => {
    await gotoDashboard(page);
    await openSidebar(page, "Integrations");

    await page.getByPlaceholder("ci-runner").fill("Cursor harness");
    await page.getByRole("button", { name: "Create key" }).click();

    await expect(page.locator("code.intg-token")).toHaveText("cs_test_token");
    await expect(
      page.getByRole("button", { name: "Enable managed integration" }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Open MCP setup guide" }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Add to Cursor" }),
    ).not.toBeVisible();
    await expectNoAxeViolations(page, "integrations create key");
  });
});
