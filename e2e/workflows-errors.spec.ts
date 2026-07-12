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

  test("keeps a queue request disabled while submission is pending", async ({
    page,
  }) => {
    await page.addInitScript(() => {
      window.__CHAOS_IPC_OVERRIDES__ = {
        enqueue_workflow: () =>
          new Promise(() => {
            /* never resolves — keeps pending state */
          }),
      };
    });
    await gotoDashboard(page);
    await openSidebar(page, "Workflows");
    await expect(page.getByText("Nightly sync")).toBeVisible();

    const queueRun = page.getByRole("button", {
      name: "Queue run for Nightly sync",
    });
    await queueRun.click();
    await expect(
      page.getByRole("button", {
        name: /^(Queue run for Nightly sync|Submitting…)$/,
      }),
    ).toBeDisabled();
    await expect(
      page.getByRole("button", { name: "Run Nightly sync now" }),
    ).toHaveCount(0);
  });

  test("requires two clicks to delete a workflow", async ({ page }) => {
    await page.addInitScript(() => {
      window.__CHAOS_FIXTURE_FLAGS__ = { workflowDeleted: false };
    });
    await gotoDashboard(page);
    await openSidebar(page, "Workflows");
    await expect(page.getByText("Nightly sync")).toBeVisible();

    const card = workflowCard(page);
    const moreActions = card.locator("summary");
    if ((await moreActions.count()) > 0) await moreActions.click();

    const deleteBtn = card.getByRole("button", {
      name: /^(Delete|Delete workflow)$/,
    });
    await deleteBtn.click();
    await expect(
      card.getByRole("button", {
        name: /^(Confirm\?|Confirm delete)$/,
      }),
    ).toBeVisible();
    await card
      .getByRole("button", { name: /^(Confirm\?|Confirm delete)$/ })
      .click();
    await expect(page.getByText("Nightly sync")).not.toBeVisible();
  });
});
