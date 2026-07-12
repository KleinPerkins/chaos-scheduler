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
 * "Agent Activity" view (F16) in BOTH themes. `expectAxeClean` fails on ANY
 * non-allowlisted violation at any impact. Covers the reconciled running /
 * upcoming / failures view on the Activity tab.
 */

const THEMES: readonly ThemeName[] = ["dark", "light"];

// See e2e/a11y/a11y.spec.ts — color-contrast is a design-token decision
// allowlisted suite-wide; every other violation still fails.
const CONTRAST_ALLOW: readonly string[] = ["color-contrast"];

async function openActivity(page: Page): Promise<void> {
  await gotoDashboard(page);
  await page.getByRole("tab", { name: "activity" }).click();
  await expect(page.locator("#mc-panel-activity")).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Agent Activity", level: 1 }),
  ).toBeVisible();
  await waitForFonts(page);
}

for (const theme of THEMES) {
  test.describe(`accessibility — Agent Activity (${theme})`, () => {
    test.beforeEach(async ({ page }) => {
      await seedTheme(page, theme);
    });

    test("activity view passes strict axe", async ({ page }) => {
      await openActivity(page);
      await expect(page.locator(`html[data-theme="${theme}"]`)).toHaveCount(1);
      await expectAxeClean(page, {
        context: `agent-activity/${theme}`,
        allow: CONTRAST_ALLOW,
      });
    });
  });
}

test.describe("Mission Control — Agent Activity keyboard", () => {
  test("run rows are keyboard focusable and section InfoTips Esc-dismiss without losing focus", async ({
    page,
  }) => {
    await openActivity(page);

    // A running-run row is a real focusable button (opens the run on activate).
    const runRow = page
      .locator("#mc-panel-activity")
      .getByRole("button")
      .filter({ hasText: "ML scoring" });
    await runRow.focus();
    await expect(runRow).toBeFocused();

    // The section InfoTip is focusable and Esc-dismissable without moving focus
    // (WAI-ARIA tooltip pattern), matching the hover-only InfoTip convention.
    const infoTip = page.getByRole("button", { name: "Running now" });
    await infoTip.focus();
    await expect(infoTip).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(infoTip).toBeFocused();
  });
});
