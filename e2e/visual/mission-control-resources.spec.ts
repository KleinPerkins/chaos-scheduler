import { test, expect, type Page } from "@playwright/test";
import { gotoDashboard } from "../support/nav";

/**
 * Visual baseline for the Mission Control "Resources" drill-down (F05) — the
 * full-detail subpage of the Resources group: the threshold-zone queue-
 * utilization chart, the execution-slot gas gauges (global + per queue), and the
 * per-queue health table — captured in dark + light at the native 960x680
 * window and full-height.
 *
 * Determinism: clock pinned to the fixture NOW, animations disabled via config,
 * charts render from the fixed mockIPC fixtures (VITE_PLAYWRIGHT). Only `-linux`
 * PNGs are committed (see .github/workflows/visual-baselines.yml).
 */

const FIXTURE_NOW = new Date("2026-07-04T12:00:00.000Z");

async function seedTheme(page: Page, theme: "dark" | "light"): Promise<void> {
  await page.addInitScript((value) => {
    window.localStorage.setItem("chaos-theme", value);
  }, theme);
}

async function waitForFonts(page: Page): Promise<void> {
  await page.evaluate(async () => {
    await document.fonts.ready;
  });
}

async function openResources(page: Page): Promise<void> {
  await gotoDashboard(page);
  // The summary card renders below the Overview; opening the drill-down replaces
  // the overview column with the full-detail subpage.
  await page.getByRole("button", { name: "View Resources details" }).click();
  const drill = page.locator('[data-drill="resources"]');
  await expect(drill).toBeVisible();
  await expect(drill).toHaveAttribute("data-drill-ready", "true");
  await expect(
    page.getByRole("heading", { name: "Resources", level: 1 }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Queue utilization" }),
  ).toBeVisible();
  await waitForFonts(page);
}

test.describe("Mission Control — Resources drill-down (960x680)", () => {
  test.use({ viewport: { width: 960, height: 680 } });

  for (const theme of ["dark", "light"] as const) {
    test(`mc-resources-${theme}`, async ({ page }) => {
      await seedTheme(page, theme);
      await page.clock.setFixedTime(FIXTURE_NOW);
      await openResources(page);

      // 1) Native-window parity (what fits in the 960x680 tray window).
      await expect(page).toHaveScreenshot(`mc-resources-${theme}.png`);

      // 2) Full composition: grow the window tall enough that the entire surface
      //    (sidebar + toolbar + the whole drill: header + utilization chart +
      //    gauges + queue-health table) paints in a single frame at scroll-top,
      //    then capture the page. Capturing the page (rather than the taller-
      //    than-window drill element) avoids the element-capture scroll that
      //    re-pins the `position: sticky` toolbar over the drill header.
      const drill = page.locator('[data-drill="resources"]');
      await page.setViewportSize({ width: 960, height: 1700 });
      await expect(drill).toBeVisible();
      await waitForFonts(page);
      await expect(page).toHaveScreenshot(`mc-resources-full-${theme}.png`);
    });
  }
});
