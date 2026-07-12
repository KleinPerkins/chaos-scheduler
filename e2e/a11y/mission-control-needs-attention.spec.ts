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
 * "Needs Attention" drill-down (F04) in BOTH themes. `expectAxeClean` fails on
 * ANY non-allowlisted violation at any impact. Covers the at-a-glance summary
 * card (its expand toggle) and the full-detail drill-down subpage.
 */

const THEMES: readonly ThemeName[] = ["dark", "light"];

// See e2e/a11y/a11y.spec.ts — color-contrast is a design-token decision
// allowlisted suite-wide; every other violation still fails.
const CONTRAST_ALLOW: readonly string[] = ["color-contrast"];

async function openNeedsAttention(page: Page): Promise<void> {
  await gotoDashboard(page);
  await page
    .getByRole("button", { name: "View Needs Attention details" })
    .click();
  await expect(page.locator('[data-drill="needs-attention"]')).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Blocked & waiting" }),
  ).toBeVisible();
  await waitForFonts(page);
}

for (const theme of THEMES) {
  test.describe(`accessibility — Needs Attention (${theme})`, () => {
    test.beforeEach(async ({ page }) => {
      await seedTheme(page, theme);
    });

    test("drill-down passes strict axe", async ({ page }) => {
      await openNeedsAttention(page);
      await expect(page.locator(`html[data-theme="${theme}"]`)).toHaveCount(1);
      await expectAxeClean(page, {
        context: `needs-attention/${theme}`,
        allow: CONTRAST_ALLOW,
      });
    });
  });
}

test.describe("Mission Control — Needs Attention keyboard", () => {
  test("summary expand toggle is keyboard operable and drives aria-expanded", async ({
    page,
  }) => {
    await gotoDashboard(page);

    // The summary card ships an in-place medium-detail expansion; the toggle
    // must be operable by keyboard alone and reflect state via aria-expanded.
    const toggle = page.getByRole("button", { name: /breakdown/ });
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
    await openNeedsAttention(page);
    const infoTip = page
      .getByRole("button", { name: /Blocked & waiting/i })
      .first();
    await infoTip.focus();
    await expect(infoTip).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(infoTip).toBeFocused();

    // Back affordance returns to the two-group summary.
    await page.getByRole("button", { name: /Back to overview/ }).click();
    await expect(
      page.getByRole("button", { name: "View Needs Attention details" }),
    ).toBeVisible();
  });
});
