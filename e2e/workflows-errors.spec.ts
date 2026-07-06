import { test, expect } from "@playwright/test";
import { expectNoAxeViolations } from "./support/axe";
import { gotoDashboard, openSidebar, workflowCard } from "./support/nav";

test.describe("Workflow list errors", () => {
  test("shows load error with retry", async ({ page }) => {
    await page.addInitScript(() => {
      window.__CHAOS_IPC_OVERRIDES__ = {
        list_workflows: () => {
          throw new Error("database unavailable");
        },
      };
    });
    await gotoDashboard(page);
    await openSidebar(page, "Workflows");

    await expect(page.getByText(/database unavailable/)).toBeVisible();
    await expect(page.getByRole("button", { name: "Retry" })).toBeVisible();
    await expectNoAxeViolations(page, "workflow load error");
  });

  test("disables sibling actions while run is pending", async ({ page }) => {
    await page.addInitScript(() => {
      window.__CHAOS_IPC_OVERRIDES__ = {
        trigger_workflow: () =>
          new Promise(() => {
            /* never resolves — keeps pending state */
          }),
      };
    });
    await gotoDashboard(page);
    await openSidebar(page, "Workflows");
    await expect(page.getByText("Nightly sync")).toBeVisible();

    await page.getByRole("button", { name: "Run Nightly sync now" }).click();
    await expect(
      page.getByRole("button", { name: "Run Nightly sync now" }),
    ).toBeDisabled();
    await expect(
      page.getByRole("button", { name: "Enqueue Nightly sync" }),
    ).toBeDisabled();
    await expect(
      page.getByRole("button", { name: "Delete" }).first(),
    ).toBeDisabled();
  });

  test("requires two clicks to delete a workflow", async ({ page }) => {
    await page.addInitScript(() => {
      window.__CHAOS_FIXTURE_FLAGS__ = { workflowDeleted: false };
    });
    await gotoDashboard(page);
    await openSidebar(page, "Workflows");
    await expect(page.getByText("Nightly sync")).toBeVisible();

    const deleteBtn = workflowCard(page).getByRole("button", {
      name: "Delete",
    });
    await deleteBtn.click();
    await expect(
      workflowCard(page).getByRole("button", { name: "Confirm?" }),
    ).toBeVisible();
    await workflowCard(page).getByRole("button", { name: "Confirm?" }).click();
    await expect(page.getByText("Nightly sync")).not.toBeVisible();
  });
});
