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
 * "Resources" drill-down (F05) in BOTH themes. `expectAxeClean` fails on ANY
 * non-allowlisted violation at any impact. Covers the at-a-glance summary card
 * (its utilization expand toggle) and the full-detail drill-down subpage.
 */

const THEMES: readonly ThemeName[] = ["dark", "light"];

async function openResources(page: Page): Promise<void> {
  await gotoDashboard(page);
  await page.getByRole("button", { name: "View Resources details" }).click();
  const drill = page.locator('[data-drill="resources"]');
  await expect(drill).toBeVisible();
  await expect(drill).toHaveAttribute("data-drill-ready", "true");
  await expect(
    page.getByRole("heading", { name: "Queue utilization" }),
  ).toBeVisible();
  await waitForFonts(page);
}

for (const theme of THEMES) {
  test.describe(`accessibility — Resources (${theme})`, () => {
    test.beforeEach(async ({ page }) => {
      await seedTheme(page, theme);
    });

    test("drill-down passes strict axe", async ({ page }) => {
      await openResources(page);
      await expect(page.locator(`html[data-theme="${theme}"]`)).toHaveCount(1);
      await expectAxeClean(page, {
        context: `resources/${theme}`,
      });
    });
  });
}

test.describe("Mission Control — Resources keyboard", () => {
  test("summary utilization toggle is keyboard operable and drives aria-expanded", async ({
    page,
  }) => {
    await gotoDashboard(page);

    // The summary card ships an in-place medium-detail expansion (a utilization
    // sparkline); the toggle must be operable by keyboard alone and reflect
    // state via aria-expanded. The name regex tolerates both toggle states
    // (Show/Hide utilization).
    const toggle = page.getByRole("button", {
      name: /(Show|Hide) utilization/,
    });
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
    await toggle.focus();
    await expect(toggle).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(toggle).toHaveAttribute("aria-expanded", "true");
    await expect(toggle).toBeFocused();
  });

  test("drill-down InfoTips are focusable and Esc-dismissable without losing focus", async ({
    page,
  }) => {
    await openResources(page);
    const infoTip = page
      .getByRole("button", { name: /Queue utilization/i })
      .first();
    await infoTip.focus();
    await expect(infoTip).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(infoTip).toBeFocused();

    // Back affordance returns to the two-group summary.
    await page.getByRole("button", { name: /Back to overview/ }).click();
    await expect(
      page.getByRole("button", { name: "View Resources details" }),
    ).toBeVisible();
  });
});
