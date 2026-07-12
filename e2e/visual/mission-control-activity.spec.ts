import { test, expect, type Page } from "@playwright/test";
import { gotoDashboard } from "../support/nav";

/**
 * Visual baseline for the Mission Control "Agent Activity" view (F16) — the
 * reconciliation (D04) of the legacy Live Activity + Upcoming Runs + Recent Runs
 * panels into one canonical running / upcoming / failures view on the Activity
 * tab. Captured in dark + light at the native 960x680 window and full-height.
 *
 * Determinism: clock pinned to the fixture NOW (so the elapsed / ETA / ago
 * labels render identically), animations disabled via config, data from the
 * fixed mockIPC snapshot (VITE_PLAYWRIGHT). Only `-linux` PNGs are committed.
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

async function openActivity(page: Page): Promise<void> {
  await gotoDashboard(page);
  await page.getByRole("tab", { name: "activity" }).click();
  const panel = page.locator("#mc-panel-activity");
  await expect(panel).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Agent Activity", level: 1 }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Running now" }),
  ).toBeVisible();
  await waitForFonts(page);
}

test.describe("Mission Control — Agent Activity (960x680)", () => {
  test.use({ viewport: { width: 960, height: 680 } });

  for (const theme of ["dark", "light"] as const) {
    test(`mc-activity-${theme}`, async ({ page }) => {
      await seedTheme(page, theme);
      await page.clock.setFixedTime(FIXTURE_NOW);
      await openActivity(page);

      // 1) Native-window parity (what fits in the 960x680 tray window).
      await expect(page).toHaveScreenshot(`mc-activity-${theme}.png`);

      // 2) Full composition: grow the window tall enough that the whole view
      //    (sidebar + toolbar + the three activity sections) paints in a single
      //    frame at scroll-top, then capture the page — avoiding the element-
      //    capture scroll that re-pins the sticky toolbar over the content.
      await page.setViewportSize({ width: 960, height: 1400 });
      await expect(page.locator("#mc-panel-activity")).toBeVisible();
      await waitForFonts(page);
      await expect(page).toHaveScreenshot(`mc-activity-full-${theme}.png`);
    });
  }
});
