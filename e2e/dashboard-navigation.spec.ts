import { test, expect } from "@playwright/test";
import { expectNoAxeViolations } from "./support/axe";

test.describe("Dashboard navigation", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    await expect(
      page.getByRole("navigation", { name: "Primary navigation" }),
    ).toBeVisible();
  });

  test("lands on Mission Control with brand visible", async ({ page }) => {
    await expect(
      page.getByRole("button", { name: "Mission Control" }),
    ).toBeVisible();
    await expect(page.getByText("Chaos Scheduler").first()).toBeVisible();
    await expectNoAxeViolations(page, "mission control landing");
  });

  test("navigates primary sidebar views", async ({ page }) => {
    const nav = page.getByRole("navigation", { name: "Primary navigation" });

    await nav.getByRole("button", { name: "Dashboard" }).click();
    await expect(
      page.getByRole("heading", { name: "Workflows" }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: "+ Add Workflow" }),
    ).toBeVisible();

    await nav.getByRole("button", { name: "History" }).click();
    await expect(
      page.getByRole("heading", { name: "Global History" }),
    ).toBeVisible();

    await nav.getByRole("button", { name: "Queues" }).click();
    await expect(page.getByRole("heading", { name: "Queues" })).toBeVisible();

    await nav.getByRole("button", { name: "Environments" }).click();
    await expect(
      page.getByRole("heading", { name: "Environments", level: 1 }),
    ).toBeVisible();

    await nav.getByRole("button", { name: "Integrations" }).click();
    await expect(
      page.getByRole("heading", { name: "Integrations" }),
    ).toBeVisible();

    await nav.getByRole("button", { name: "Settings" }).click();
    await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();
  });

  test("workflow list shows fixture workflow", async ({ page }) => {
    await page
      .getByRole("navigation", { name: "Primary navigation" })
      .getByRole("button", { name: "Dashboard" })
      .click();

    await expect(page.getByText("Nightly sync")).toBeVisible();
  });
});

test.describe("Settings", () => {
  test("shows updater-unavailable affordance on harness build", async ({
    page,
  }) => {
    await page.goto("/");
    await page
      .getByRole("navigation", { name: "Primary navigation" })
      .getByRole("button", { name: "Settings" })
      .click();

    await page.getByRole("button", { name: "Check for updates" }).click();
    await expect(page.getByText(/latest version|not wired up/i)).toBeVisible();
  });
});
