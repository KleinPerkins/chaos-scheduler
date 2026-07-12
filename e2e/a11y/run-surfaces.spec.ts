import { test, expect } from "@playwright/test";
import { openRunDetail, openWorkflowRunHistory } from "../support/nav";
import {
  expectAxeClean,
  seedTheme,
  waitForFonts,
  type ThemeName,
} from "./support";

/**
 * Strict accessibility coverage for the run / rerun surfaces that the top-level
 * stable-surface gate (a11y.spec.ts, which only walks the primary nav entries)
 * does not reach: the workflow-scoped run history, the run detail page, and —
 * critically — the rerun confirmation dialog scanned WHILE IT IS OPEN.
 *
 * Every surface is scanned in BOTH themes with `expectAxeClean` (fails on ANY
 * non-allowlisted violation at any impact) plus a real keyboard / focus
 * assertion. Allowlists are empty; a genuine violation must be reported and
 * routed to a code/design lane, never silently suppressed.
 */

const THEMES: readonly ThemeName[] = ["dark", "light"];

for (const theme of THEMES) {
  test.describe(`a11y — run surfaces (${theme})`, () => {
    test.beforeEach(async ({ page }) => {
      await seedTheme(page, theme);
    });

    test("workflow run history passes strict axe + keyboard focus", async ({
      page,
    }) => {
      // A populated failure heatmap so the focusable data-viz cells exist to
      // assert against (default fixtures return no buckets).
      await page.addInitScript(() => {
        window.__CHAOS_IPC_OVERRIDES__ = {
          ...(window.__CHAOS_IPC_OVERRIDES__ ?? {}),
          get_workflow_history_buckets: () => [
            { day: "2026-07-06", total: 4, failed: 2, succeeded: 2 },
            { day: "2026-07-07", total: 3, failed: 0, succeeded: 3 },
          ],
        };
      });

      await openWorkflowRunHistory(page);
      await expect(page.locator(`html[data-theme="${theme}"]`)).toHaveCount(1);
      await waitForFonts(page);

      // Keyboard/switch users must reach each day's failure summary without a
      // pointer: the non-interactive heatmap cell is deliberately focusable and
      // carries the summary as its accessible name.
      const cell = page.getByRole("listitem", {
        name: "2026-07-06: 2 of 4 runs failed",
      });
      await cell.focus();
      await expect(cell).toBeFocused();

      await expectAxeClean(page, { context: `run-history/${theme}` });
    });

    test("run detail passes strict axe + keyboard roving focus", async ({
      page,
    }) => {
      await openRunDetail(page);
      await expect(
        page.getByRole("region", { name: "Nightly sync run detail" }),
      ).toBeVisible();
      await expect(page.locator(`html[data-theme="${theme}"]`)).toHaveCount(1);
      await waitForFonts(page);

      // The stdout/stderr log streams are an ARIA tablist with roving tabindex:
      // arrow keys move selection AND focus between the tabs.
      const stdout = page.getByRole("tab", { name: "stdout" });
      const stderr = page.getByRole("tab", { name: "stderr" });
      await expect(stdout).toHaveAttribute("aria-selected", "true");
      await stdout.focus();
      await page.keyboard.press("ArrowRight");
      await expect(stderr).toBeFocused();
      await expect(stderr).toHaveAttribute("aria-selected", "true");
      await page.keyboard.press("ArrowLeft");
      await expect(stdout).toBeFocused();

      // Full-page strict axe scan, now enabled: the previously-blocking
      // duplicate "Raw logs" region landmark (landmark-unique) was resolved in
      // the component (the redundant inner `role="region"` was dropped), so Run
      // Detail is strict-clean and this is the guardrail against regression.
      await expectAxeClean(page, { context: `run-detail/${theme}` });
    });

    test("rerun modal passes strict axe while open (labelled dialog + focus-in)", async ({
      page,
    }) => {
      await openWorkflowRunHistory(page);
      await page
        .getByRole("button", { name: /Rerun .* run started/i })
        .first()
        .click();

      // A labelled, modal dialog (aria-modal + accessible name from its title).
      const dialog = page.getByRole("dialog", { name: /Rerun Nightly sync/i });
      await expect(dialog).toBeVisible();
      await expect(dialog).toHaveAttribute("aria-modal", "true");

      // Focus is moved INTO the dialog on open (the JSON override textarea
      // autofocuses) — the essential modal focus-management behavior.
      await expect(page.locator("#rerun-input-json")).toBeFocused();

      await expect(page.locator(`html[data-theme="${theme}"]`)).toHaveCount(1);
      await waitForFonts(page);

      // Scan axe WITH THE MODAL OPEN: this covers the dialog, its labelling,
      // and the backdrop scrim together, not just the underlying page.
      await expectAxeClean(page, { context: `rerun-modal/${theme}` });

      // Escape dismisses the dialog (shared Modal shell behavior).
      await page.keyboard.press("Escape");
      await expect(dialog).not.toBeVisible();
    });
  });
}
