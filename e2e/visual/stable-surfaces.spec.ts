import { test, expect, type Page } from "@playwright/test";
import { gotoDashboard, openSidebar } from "../support/nav";

/**
 * Visual baselines for currently-shipped, stable surfaces at their native
 * window sizes. The Mission Control ("Home") Overview has its own baselines in
 * mission-control-overview.spec.ts; its not-yet-redesigned drill-down tabs stay
 * out of the visual harness until their gated follow-ups.
 *
 * Determinism: the clock is pinned to the fixture "now" so any relative
 * timestamps render identically across runs; fixture data is the fixed mockIPC
 * registry (VITE_PLAYWRIGHT); animations/transitions are disabled via config.
 */

// Matches the fixture `NOW` in src/test/fixtures/data.ts so displayed times are
// stable regardless of when the suite runs.
const FIXTURE_NOW = new Date("2026-07-04T12:00:00.000Z");

async function waitForFonts(page: Page): Promise<void> {
  await page.evaluate(async () => {
    await document.fonts.ready;
  });
}

async function gotoSurface(page: Page, label: string): Promise<void> {
  await gotoDashboard(page);
  await openSidebar(page, label);
  // `aria-current="page"` on the clicked entry confirms the view switched
  // before we screenshot, so the capture never races the transition.
  await expect(
    page
      .getByRole("navigation", { name: "Primary navigation" })
      .getByRole("button", { name: label }),
  ).toHaveAttribute("aria-current", "page");
  await waitForFonts(page);
}

test.describe("stable surfaces — main window (960x680)", () => {
  test.use({ viewport: { width: 960, height: 680 } });

  test.beforeEach(async ({ page }) => {
    await page.clock.setFixedTime(FIXTURE_NOW);
  });

  // [nav label, baseline slug]
  const surfaces: ReadonlyArray<readonly [string, string]> = [
    ["Workflows", "workflows"],
    ["History", "global-history"],
    ["Queues", "queues"],
    ["Environments", "environments"],
    ["Integrations", "integrations"],
    ["Settings", "settings"],
  ];

  for (const [label, slug] of surfaces) {
    test(slug, async ({ page }) => {
      await gotoSurface(page, label);
      await expect(page).toHaveScreenshot(`${slug}.png`);
    });
  }

  test("workflow-detail dark and light", async ({ page }) => {
    await gotoSurface(page, "Workflows");
    await page
      .getByRole("button", { name: "Nightly sync", exact: true })
      .click();
    await expect(
      page.getByRole("heading", { name: "Latest run" }),
    ).toBeVisible();
    await waitForFonts(page);
    await expect(page).toHaveScreenshot("workflow-detail-dark.png");

    await page.getByRole("button", { name: "Light theme" }).click();
    await expect(page.locator('html[data-theme="light"]')).toHaveCount(1);
    await expect(page).toHaveScreenshot("workflow-detail-light.png");
  });

  test("workflow-editor dark and light", async ({ page }) => {
    await gotoSurface(page, "Workflows");
    await page
      .getByRole("button", { name: "Nightly sync", exact: true })
      .click();
    await page.getByRole("button", { name: "Edit workflow" }).click();
    await expect(
      page.getByRole("heading", { name: "Edit workflow" }),
    ).toBeVisible();
    await waitForFonts(page);
    await expect(page).toHaveScreenshot("workflow-editor-dark.png");

    await page.getByRole("button", { name: "Light theme" }).click();
    await expect(page.locator('html[data-theme="light"]')).toHaveCount(1);
    await expect(page).toHaveScreenshot("workflow-editor-light.png");
  });
});

test.describe("stable surfaces — menu bar popup (340x440)", () => {
  test.use({ viewport: { width: 340, height: 440 } });

  test.beforeEach(async ({ page }) => {
    await page.clock.setFixedTime(FIXTURE_NOW);
  });

  test("popup", async ({ page }) => {
    await page.goto("/?view=popup");
    await expect(page.getByText("Chaos Scheduler")).toBeVisible();
    await waitForFonts(page);
    await expect(page).toHaveScreenshot("popup.png");
  });
});
