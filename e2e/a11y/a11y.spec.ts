import { test, expect, type Page } from "@playwright/test";
import { gotoDashboard, openSidebar } from "../support/nav";
import {
  assertInterLoaded,
  assertReducedMotionRespected,
  expectAxeClean,
  seedTheme,
  waitForFonts,
  type ThemeName,
} from "./support";

/**
 * Strict, required accessibility gate across the currently-shipped STABLE
 * surfaces (the same set the visual harness covers) in BOTH themes, plus a
 * theme-toggle interaction, a reduced-motion assertion, and a self-hosted-Inter
 * font-load assertion.
 *
 * The Mission Control ("Home") Overview has its own strict gate in
 * mission-control-overview.spec.ts; only its not-yet-redesigned drill-down tabs
 * remain outside the strict gate (lenient critical/serious axe coverage of Home
 * stays in the functional suite, e2e/accessibility.spec.ts). The whole suite
 * runs under reduced-motion emulation (see playwright.a11y.config.ts).
 *
 * `expectAxeClean` fails on ANY non-allowlisted violation at ANY impact
 * (including moderate/minor). Allowlists below are empty unless a real,
 * pre-existing violation is documented.
 */

// [nav label, surface slug for messages]
const SURFACES: ReadonlyArray<readonly [string, string]> = [
  ["Workflows", "workflows"],
  ["History", "history"],
  ["Queues", "queues"],
  ["Environments", "environments"],
  ["Integrations", "integrations"],
  ["Settings", "settings"],
];

const THEMES: readonly ThemeName[] = ["dark", "light"];

async function gotoSurface(page: Page, label: string): Promise<void> {
  await gotoDashboard(page);
  await openSidebar(page, label);
  await expect(
    page
      .getByRole("navigation", { name: "Primary navigation" })
      .getByRole("button", { name: label }),
  ).toHaveAttribute("aria-current", "page");
  await waitForFonts(page);
}

for (const theme of THEMES) {
  test.describe(`accessibility — stable surfaces (${theme})`, () => {
    test.beforeEach(async ({ page }) => {
      await seedTheme(page, theme);
    });

    for (const [label, slug] of SURFACES) {
      test(`${slug} passes strict axe`, async ({ page }) => {
        await gotoSurface(page, label);
        await expect(page.locator(`html[data-theme="${theme}"]`)).toHaveCount(
          1,
        );
        await expectAxeClean(page, {
          context: `${slug}/${theme}`,
        });
      });
    }

    test(`popup passes strict axe`, async ({ page }) => {
      await page.goto("/?view=popup");
      await expect(page.getByText("Chaos Scheduler")).toBeVisible();
      await waitForFonts(page);
      await expect(page.locator(`html[data-theme="${theme}"]`)).toHaveCount(1);
      await expectAxeClean(page, {
        context: `popup/${theme}`,
      });
    });
  });
}

test.describe("accessibility — interactions & assets", () => {
  test("theme toggle switches the applied theme and stays accessible", async ({
    page,
  }) => {
    await gotoDashboard(page);
    // Default preference is dark (src/lib/theme.ts).
    await expect(page.locator('html[data-theme="dark"]')).toHaveCount(1);

    const themeGroup = page.getByRole("group", { name: "Color theme" }).first();
    await themeGroup.getByRole("button", { name: "Light" }).click();
    await expect(page.locator('html[data-theme="light"]')).toHaveCount(1);
    await waitForFonts(page);
    await expectAxeClean(page, {
      context: "after toggle to light",
    });

    await themeGroup.getByRole("button", { name: "Dark" }).click();
    await expect(page.locator('html[data-theme="dark"]')).toHaveCount(1);
    await waitForFonts(page);
    await expectAxeClean(page, {
      context: "after toggle to dark",
    });
  });

  test("reduced motion is emulated and respected", async ({ page }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    await gotoDashboard(page);
    await assertReducedMotionRespected(page);
  });

  test("self-hosted Inter is loaded and applied", async ({ page }) => {
    await gotoDashboard(page);
    await assertInterLoaded(page);
  });
});
