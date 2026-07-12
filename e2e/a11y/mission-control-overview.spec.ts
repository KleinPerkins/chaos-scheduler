import { test, expect, type Page } from "@playwright/test";
import { gotoDashboard } from "../support/nav";
import {
  expectAxeClean,
  seedTheme,
  waitForFonts,
  type ThemeName,
} from "./support";

/**
 * Strict, required accessibility + keyboard gate for the Mission Control
 * Overview vNext (the "Home" landing surface) in BOTH themes. Home was
 * previously EXCLUDED from the strict gate while under redesign; this is the
 * approved Overview composition, so it now carries its own required coverage.
 *
 * `expectAxeClean` fails on ANY non-allowlisted violation at ANY impact. The
 * whole Home page is analyzed (Overview + the reachable legacy panels below it),
 * so this also guards the surfaces PR B preserves for G06.
 */

const THEMES: readonly ThemeName[] = ["dark", "light"];

async function gotoOverview(page: Page): Promise<void> {
  await gotoDashboard(page);
  // Overview is the default landing tab; wait for its data to load so axe sees
  // the fully-rendered hero + charts (not the loading skeleton).
  await expect(page.locator("[data-overview-ready]")).toBeVisible();
  await waitForFonts(page);
}

for (const theme of THEMES) {
  test.describe(`accessibility — Mission Control Overview (${theme})`, () => {
    test.beforeEach(async ({ page }) => {
      await seedTheme(page, theme);
    });

    test("overview passes strict axe", async ({ page }) => {
      await gotoOverview(page);
      await expect(page.locator(`html[data-theme="${theme}"]`)).toHaveCount(1);
      await expectAxeClean(page, {
        context: `overview/${theme}`,
      });
    });
  });
}

test.describe("Mission Control Overview — keyboard", () => {
  test("charts + InfoTips are keyboard operable", async ({ page }) => {
    await gotoOverview(page);

    // The status-distribution donut ships a %/Count legend toggle; it must be
    // operable by keyboard alone (focus + Enter), and reflect state via
    // aria-pressed for AT.
    const formatToggle = page.getByRole("group", {
      name: "Legend value format",
    });
    const percentButton = formatToggle.getByRole("button", { name: "%" });
    const countButton = formatToggle.getByRole("button", { name: "Count" });

    await expect(countButton).toHaveAttribute("aria-pressed", "true");
    await percentButton.focus();
    await expect(percentButton).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(percentButton).toHaveAttribute("aria-pressed", "true");
    await expect(countButton).toHaveAttribute("aria-pressed", "false");

    // Every chart/table carries a hover+focus InfoTip; the trigger must be
    // keyboard-focusable and Esc-dismissable WITHOUT moving focus away.
    const infoTip = page
      .getByRole("button", { name: /Status distribution/i })
      .first();
    await infoTip.focus();
    await expect(infoTip).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(infoTip).toBeFocused();
  });
});
