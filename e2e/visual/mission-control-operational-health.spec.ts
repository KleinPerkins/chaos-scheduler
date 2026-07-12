import { test, expect, type Page } from "@playwright/test";
import { gotoDashboard } from "../support/nav";

/**
 * Visual baseline for the Mission Control "Operational Health" drill-down (F03) —
 * the full-detail subpage of the Operational Health group: the aggregate KPI
 * rollup, the success/failure trend, and the wait + runtime dual-axis duration
 * trends (avg + max with a 30-day baseline) — captured in dark + light at the
 * native 960x680 window and full-height.
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

async function openOperationalHealth(page: Page): Promise<void> {
  await gotoDashboard(page);
  // The summary card renders below the Overview; opening the drill-down replaces
  // the overview column with the full-detail subpage.
  await page
    .getByRole("button", { name: "View Operational Health details" })
    .click();
  const drill = page.locator('[data-drill="operational-health"]');
  await expect(drill).toBeVisible();
  await expect(drill).toHaveAttribute("data-drill-ready", "true");
  await expect(
    page.getByRole("heading", { name: "Operational Health", level: 1 }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Aggregate KPIs" }),
  ).toBeVisible();
  await waitForFonts(page);
}

test.describe("Mission Control — Operational Health drill-down (960x680)", () => {
  test.use({ viewport: { width: 960, height: 680 } });

  for (const theme of ["dark", "light"] as const) {
    test(`mc-operational-health-${theme}`, async ({ page }) => {
      await seedTheme(page, theme);
      await page.clock.setFixedTime(FIXTURE_NOW);
      await openOperationalHealth(page);

      // 1) Native-window parity (what fits in the 960x680 tray window).
      await expect(page).toHaveScreenshot(`mc-operational-health-${theme}.png`);

      // 2) Full composition: grow the window tall enough that the entire
      //    surface (sidebar + toolbar + the whole drill: header + KPIs + all
      //    three trend cards) paints in a single frame at scroll-top, then
      //    capture the page. Capturing the page (rather than the taller-than-
      //    window drill element) avoids the element-capture scroll that re-pins
      //    the `position: sticky` toolbar over the drill header; at scroll-top
      //    nothing is pinned, so the design review sees the whole drill in one
      //    deterministic image.
      const drill = page.locator('[data-drill="operational-health"]');
      await page.setViewportSize({ width: 960, height: 1700 });
      await expect(drill).toBeVisible();
      await waitForFonts(page);
      await expect(page).toHaveScreenshot(
        `mc-operational-health-full-${theme}.png`,
      );
    });
  }
});
