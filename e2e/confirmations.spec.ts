import { test, expect } from "@playwright/test";
import { expectNoAxeViolations } from "./support/axe";
import { gotoDashboard, openSidebar, workflowCard } from "./support/nav";

test.describe("Destructive confirmations", () => {
  test("Integrations revoke requires confirm click", async ({ page }) => {
    await page.addInitScript(() => {
      window.__CHAOS_IPC_OVERRIDES__ = {
        list_api_keys: () => [
          {
            id: "key-e2e-1",
            name: "Harness key",
            scopes: "read",
            created_at: "2026-07-04T12:00:00.000Z",
            last_used_at: null,
          },
        ],
      };
    });
    await gotoDashboard(page);
    await openSidebar(page, "Integrations");

    const revoke = page.getByRole("button", { name: "Revoke API key" });
    await revoke.click();
    await expect(revoke).toHaveText("Confirm revoke?");
    await revoke.click();
    await expect(page.getByText("API key revoked.")).toBeVisible();
    await expectNoAxeViolations(page, "integrations revoke");
  });

  test("RunHistory rerun modal validates JSON and submits", async ({
    page,
  }) => {
    await gotoDashboard(page);
    await openSidebar(page, "Workflows");
    const card = workflowCard(page);
    await card.locator("summary").click();
    await card.getByRole("button", { name: "View history" }).click();

    await expect(
      page.getByRole("heading", { name: "Nightly sync run history" }),
    ).toBeVisible();
    await expect(page.getByText("Latest 50", { exact: true })).toBeVisible();
    await page
      .getByRole("button", { name: /Rerun .* run started/i })
      .first()
      .click();

    await expect(
      page.getByRole("dialog", { name: /Rerun Nightly sync/i }),
    ).toBeVisible();

    const textarea = page.locator("#rerun-input-json");
    await textarea.fill("{not-json");
    await page.getByRole("button", { name: "Rerun", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText(/valid JSON/i);

    await textarea.fill("{}");
    await page.getByRole("button", { name: "Rerun", exact: true }).click();
    await expect(
      page.getByRole("dialog", { name: /Rerun Nightly sync/i }),
    ).not.toBeVisible();
    await expectNoAxeViolations(page, "rerun modal");
  });
});
