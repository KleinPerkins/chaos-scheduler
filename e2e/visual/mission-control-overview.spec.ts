import { test, expect, type Page } from "@playwright/test";
import { gotoDashboard } from "../support/nav";

/**
 * Visual baseline for the Mission Control Overview vNext (the "Home" landing
 * surface) at its native 960x680 window, in dark + light. Home was previously
 * excluded from the visual harness while under design approval; this is the
 * approved Overview composition (SLA banner + 6-KPI strip + race hero + status
 * donut + success/fail trend).
 *
 * Determinism: the clock is pinned to the fixture `NOW` (so the race elapsed +
 * any relative time render identically), animations are disabled via config,
 * charts render from the fixed mockIPC fixture data, and the race idle motion is
 * off by default. Baselines are captured in the pinned Linux container (see
 * .github/workflows/visual-baselines.yml); only `-linux` PNGs are committed.
 */

// Matches the fixture `NOW` in src/test/fixtures/data.ts.
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

test.describe("Mission Control Overview vNext (960x680)", () => {
  test.use({ viewport: { width: 960, height: 680 } });

  for (const theme of ["dark", "light"] as const) {
    test(`mc-overview-${theme}`, async ({ page }) => {
      await seedTheme(page, theme);
      await page.clock.setFixedTime(FIXTURE_NOW);
      await gotoDashboard(page);

      // Overview is the default landing tab; wait until its data has loaded and
      // the hero + charts have rendered so the capture never races the fetch.
      const overview = page.locator("[data-overview-ready]");
      await expect(overview).toBeVisible();
      await expect(
        page.getByRole("heading", { name: "Running now" }),
      ).toBeVisible();
      await expect(
        page.getByRole("heading", { name: "Status distribution" }),
      ).toBeVisible();
      await expect(
        page.getByRole("heading", { name: "Success / failure trend" }),
      ).toBeVisible();
      await waitForFonts(page);

      // 1) Native-window parity: exactly what fits in the 960x680 tray window
      //    (sticky filter bar + tabs + the top of the scrolling Overview).
      await expect(page).toHaveScreenshot(`mc-overview-${theme}.png`);

      // 2) Full composition: grow the window tall enough that the whole Overview
      //    column paints in a single frame (avoids beyond-viewport stitching and
      //    sticky-toolbar overlap), then capture just the Overview element — SLA
      //    banner + 6-KPI strip + race hero + donut + trend — so the design
      //    review sees everything below the fold in one deterministic image.
      await page.setViewportSize({ width: 960, height: 1320 });
      await expect(overview).toBeVisible();
      await waitForFonts(page);
      await expect(overview).toHaveScreenshot(`mc-overview-full-${theme}.png`);
    });
  }
});
