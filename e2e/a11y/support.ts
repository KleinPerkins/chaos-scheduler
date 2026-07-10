import AxeBuilder from "@axe-core/playwright";
import { expect, type Page } from "@playwright/test";

export type ThemeName = "dark" | "light";

/**
 * Seed the color-theme preference BEFORE the app boots (initTheme reads
 * `chaos-theme` from localStorage synchronously in main.tsx and sets
 * `data-theme` on <html>). Must be called before the first navigation.
 */
export async function seedTheme(page: Page, theme: ThemeName): Promise<void> {
  await page.addInitScript((value) => {
    window.localStorage.setItem("chaos-theme", value);
  }, theme);
}

export async function waitForFonts(page: Page): Promise<void> {
  await page.evaluate(async () => {
    await document.fonts.ready;
  });
}

export interface AxeCleanOptions {
  /** Human-readable label used in the failure message. */
  context?: string;
  /**
   * Axe rule IDs that are KNOWN, pre-existing, and accepted for this surface.
   * Every entry must be justified at the call site. Anything NOT listed here —
   * at ANY impact, including moderate/minor — fails the check.
   */
  allow?: readonly string[];
}

/**
 * Strict axe gate: fails on ANY violation whose rule ID is not explicitly
 * allowlisted (unlike the lenient `e2e/support/axe.ts` helper used by the
 * functional suite, which only fails on critical/serious). This is the
 * required accessibility bar for the harness.
 */
export async function expectAxeClean(
  page: Page,
  { context, allow = [] }: AxeCleanOptions = {},
): Promise<void> {
  const { violations } = await new AxeBuilder({ page }).analyze();
  const blocking = violations.filter((v) => !allow.includes(v.id));

  if (blocking.length > 0) {
    const summary = blocking
      .map((v) => {
        const nodes = v.nodes
          .map((n) => `        - ${n.target.join(" ")}`)
          .join("\n");
        return `    [${v.impact ?? "n/a"}] ${v.id}: ${v.help} (${v.nodes.length} node(s))\n${nodes}`;
      })
      .join("\n");
    const ignored = violations
      .filter((v) => allow.includes(v.id))
      .map((v) => v.id);
    throw new Error(
      `axe found non-allowlisted violations${context ? ` (${context})` : ""}:\n${summary}` +
        (ignored.length
          ? `\n    (allowlisted, ignored: ${ignored.join(", ")})`
          : ""),
    );
  }
}

/**
 * Assert the self-hosted Inter face (@fontsource/inter, bundled offline — see
 * src/styles/fonts.css) is actually loaded and applied, so text renders in the
 * intended typeface rather than a fallback.
 */
export async function assertInterLoaded(page: Page): Promise<void> {
  const info = await page.evaluate(async () => {
    await document.fonts.ready;
    const faces = Array.from(document.fonts);
    return {
      interLoaded: faces
        .filter((f) => /inter/i.test(f.family) && f.status === "loaded")
        .map((f) => `${f.family}:${f.weight}:${f.style}`),
      bodyFontFamily: getComputedStyle(document.body).fontFamily,
    };
  });
  expect(
    info.interLoaded.length,
    `expected >=1 loaded self-hosted Inter @font-face, saw: ${JSON.stringify(info)}`,
  ).toBeGreaterThan(0);
  expect(
    info.bodyFontFamily.toLowerCase(),
    "body should resolve to Inter (the --font-sans token leads with Inter)",
  ).toContain("inter");
}

/**
 * Confirm the app both emulates reduced motion AND ships a
 * `prefers-reduced-motion: reduce` stylesheet rule that clamps animation /
 * transition durations (src/index.css).
 */
export async function assertReducedMotionRespected(page: Page): Promise<void> {
  const info = await page.evaluate(() => {
    const mediaMatches = window.matchMedia(
      "(prefers-reduced-motion: reduce)",
    ).matches;
    let hasReducedMotionRule = false;
    for (const sheet of Array.from(document.styleSheets)) {
      let rules: CSSRuleList | undefined;
      try {
        rules = sheet.cssRules;
      } catch {
        // Cross-origin sheet — skip.
        continue;
      }
      for (const rule of Array.from(rules ?? [])) {
        if (
          rule instanceof CSSMediaRule &&
          rule.conditionText.includes("prefers-reduced-motion")
        ) {
          hasReducedMotionRule = true;
        }
      }
    }
    return { mediaMatches, hasReducedMotionRule };
  });
  expect(info.mediaMatches, "reduced-motion emulation should be active").toBe(
    true,
  );
  expect(
    info.hasReducedMotionRule,
    "app should ship a prefers-reduced-motion: reduce rule",
  ).toBe(true);
}
