import { test, expect, type Page } from "@playwright/test";
import { gotoDashboard } from "../support/nav";

/**
 * Visual baseline for the Mission Control "Needs Attention" drill-down (F04) —
 * the full-detail subpage of the Critical/Needs-Attention group: the
 * blocked/waiting reason bars + summary stats, the heavy-blocker impact bars,
 * the long-running outlier + blast-radius bars, and the recent-failure table —
 * captured in dark + light at the native 960x680 window and full-height.
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

async function openNeedsAttention(page: Page): Promise<void> {
  await gotoDashboard(page);
  // The summary card renders below the Overview; opening the drill-down replaces
  // the overview column with the full-detail subpage.
  await page
    .getByRole("button", { name: "View Needs Attention details" })
    .click();
  const drill = page.locator('[data-drill="needs-attention"]');
  await expect(drill).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Needs Attention" }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Blocked & waiting" }),
  ).toBeVisible();
  await waitForFonts(page);
}

test.describe("Mission Control — Needs Attention drill-down (960x680)", () => {
  test.use({ viewport: { width: 960, height: 680 } });

  for (const theme of ["dark", "light"] as const) {
    test(`mc-needs-attention-${theme}`, async ({ page }) => {
      await seedTheme(page, theme);
      await page.clock.setFixedTime(FIXTURE_NOW);
      await openNeedsAttention(page);

      // 1) Native-window parity (what fits in the 960x680 tray window).
      await expect(page).toHaveScreenshot(`mc-needs-attention-${theme}.png`);

      // 2) Full composition: grow the window tall enough that the whole
      //    drill-down paints in one frame, then capture just the drill element.
      const drill = page.locator('[data-drill="needs-attention"]');
      await page.setViewportSize({ width: 960, height: 1320 });
      await expect(drill).toBeVisible();
      await waitForFonts(page);
      await expect(drill).toHaveScreenshot(
        `mc-needs-attention-full-${theme}.png`,
      );
    });
  }
});
