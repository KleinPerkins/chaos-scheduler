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
 * "Operational Health" drill-down (F03) in BOTH themes. `expectAxeClean` fails
 * on ANY non-allowlisted violation at any impact. Covers the at-a-glance summary
 * card (its trend expand toggle) and the full-detail drill-down subpage.
 */

const THEMES: readonly ThemeName[] = ["dark", "light"];

async function openOperationalHealth(page: Page): Promise<void> {
  await gotoDashboard(page);
  await page
    .getByRole("button", { name: "View Operational Health details" })
    .click();
  const drill = page.locator('[data-drill="operational-health"]');
  await expect(drill).toBeVisible();
  await expect(drill).toHaveAttribute("data-drill-ready", "true");
  await expect(
    page.getByRole("heading", { name: "Aggregate KPIs" }),
  ).toBeVisible();
  await waitForFonts(page);
}

for (const theme of THEMES) {
  test.describe(`accessibility — Operational Health (${theme})`, () => {
    test.beforeEach(async ({ page }) => {
      await seedTheme(page, theme);
    });

    test("drill-down passes strict axe", async ({ page }) => {
      await openOperationalHealth(page);
      await expect(page.locator(`html[data-theme="${theme}"]`)).toHaveCount(1);
      await expectAxeClean(page, {
        context: `operational-health/${theme}`,
      });
    });
  });
}

test.describe("Mission Control — Operational Health keyboard", () => {
  test("summary trend toggle is keyboard operable and drives aria-expanded", async ({
    page,
  }) => {
    await gotoDashboard(page);

    // The summary card ships an in-place medium-detail expansion (the
    // success/failure trend); the toggle must be operable by keyboard alone and
    // reflect state via aria-expanded. The name regex tolerates both toggle
    // states (Show/Hide trend) while excluding the Overview's "Success /
    // failure trend" InfoTip.
    const toggle = page.getByRole("button", { name: /(Show|Hide) trend/ });
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
    await openOperationalHealth(page);
    const infoTip = page
      .getByRole("button", { name: /Aggregate KPIs/i })
      .first();
    await infoTip.focus();
    await expect(infoTip).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(infoTip).toBeFocused();

    // Back affordance returns to the two-group summary.
    await page.getByRole("button", { name: /Back to overview/ }).click();
    await expect(
      page.getByRole("button", { name: "View Operational Health details" }),
    ).toBeVisible();
  });
});
