import { test, expect } from "@playwright/test";
import { expectNoAxeViolations } from "./support/axe";
import { gotoDashboard, openSidebar } from "./support/nav";

test.describe("Queue workflow run", () => {
  test("queue run shows waiting notice with queued-run id", async ({
    page,
  }) => {
    await gotoDashboard(page);
    await openSidebar(page, "Workflows");
    await expect(page.getByText("Nightly sync")).toBeVisible();

    await page
      .getByRole("button", { name: "Queue run for Nightly sync" })
      .click();
    await expect(
      page.getByText(/Waiting to start: Nightly sync \(queue-fi/),
    ).toBeVisible();
    await expectNoAxeViolations(page, "queue run success");
  });
});
