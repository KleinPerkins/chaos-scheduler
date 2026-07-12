import { test, expect } from "@playwright/test";
import { openRunDetail } from "../support/nav";
import {
  expectAxeClean,
  seedTheme,
  waitForFonts,
  type ThemeName,
} from "./support";

/**
 * Strict, required accessibility gate for the Run Detail drill-down surface in
 * BOTH themes. `expectAxeClean` fails on ANY non-allowlisted violation at any
 * impact (including moderate/minor).
 *
 * Run Detail is reached by drilling into a run from a workflow's run history,
 * so it sits outside the sidebar-nav SURFACES loop in a11y.spec.ts and gets its
 * own strict scan here. Raw logs are expanded by default (showLogs initial
 * state), so the full observability panel is present in the scanned DOM — the
 * part that previously exposed a duplicate "Raw logs" region landmark
 * (landmark-unique). This scan is the guardrail for that fix.
 */

const THEMES: readonly ThemeName[] = ["dark", "light"];

for (const theme of THEMES) {
  test.describe(`accessibility — Run Detail (${theme})`, () => {
    test.beforeEach(async ({ page }) => {
      await seedTheme(page, theme);
    });

    test("run detail passes strict axe", async ({ page }) => {
      await openRunDetail(page);
      await expect(page.locator(`html[data-theme="${theme}"]`)).toHaveCount(1);
      await waitForFonts(page);
      await expectAxeClean(page, {
        context: `run-detail/${theme}`,
      });
    });
  });
}
