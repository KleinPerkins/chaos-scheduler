import { test, expect } from "@playwright/test";
import { expectNoAxeViolations } from "./support/axe";
import { gotoDashboard, openSidebar } from "./support/nav";

test.describe("Accessibility — views without dedicated feature specs", () => {
  test("Environments view passes axe", async ({ page }) => {
    await gotoDashboard(page);
    await openSidebar(page, "Environments");
    await expect(
      page.getByRole("heading", { name: "Environments", level: 1 }),
    ).toBeVisible();
    await expectNoAxeViolations(page, "environments");
  });

  test("Queues view passes axe", async ({ page }) => {
    await gotoDashboard(page);
    await openSidebar(page, "Queues");
    await expect(page.getByRole("heading", { name: "Queues" })).toBeVisible();
    await expectNoAxeViolations(page, "queues");
  });
});

test.describe("Accessibility — light theme", () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() =>
      window.localStorage.setItem("chaos-theme", "light"),
    );
  });

  test("Home (Mission Control) passes axe in light theme", async ({ page }) => {
    await gotoDashboard(page);
    await expect(
      page.getByRole("heading", {
        name: "Scheduler operations by environment and owner",
      }),
    ).toBeVisible();
    await expectNoAxeViolations(page, "home/light");
  });

  test("Workflows passes axe in light theme", async ({ page }) => {
    await gotoDashboard(page);
    await openSidebar(page, "Workflows");
    await expect(
      page.getByRole("heading", { name: "Workflows" }),
    ).toBeVisible();
    await expectNoAxeViolations(page, "workflows/light");
  });

  test("Settings passes axe in light theme", async ({ page }) => {
    await gotoDashboard(page);
    await openSidebar(page, "Settings");
    await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();
    await expectNoAxeViolations(page, "settings/light");
  });
});
