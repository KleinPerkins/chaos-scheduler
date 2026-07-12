import { defineConfig, devices } from "@playwright/test";

const port = 5173;

/**
 * Deterministic visual-regression config, isolated from the functional e2e
 * config (playwright.config.ts, which ignores `e2e/visual/**`) so the functional
 * job never runs screenshot specs and vice versa.
 *
 * Baselines are BOTH generated and compared inside the pinned Playwright
 * container (mcr.microsoft.com/playwright:v<version>-jammy — see the `visual`
 * job in .github/workflows/ci.yml and the visual-baselines.yml regen workflow),
 * so OS-level font rasterization is identical and screenshots are stable. Only
 * the `-linux` baselines are committed; a local `--update-snapshots` run on
 * macOS produces separate `-darwin` files that are gitignored.
 *
 * Determinism levers: fixed viewport at the native window sizes (960x680 main /
 * 384x590 popup), `reducedMotion: "reduce"` (drives the app's reduced-motion
 * CSS), `animations: "disabled"` + `caret: "hide"` on every screenshot, a
 * frozen clock (per spec), and the fixed mockIPC fixture data (VITE_PLAYWRIGHT).
 */
export default defineConfig({
  testDir: "./e2e/visual",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  // Required gate (see the `visual` job in .github/workflows/ci.yml): retry once
  // in CI so a transient sub-pixel diff can't false-block a merge; 0 locally for
  // fast feedback. Mirrors playwright.a11y.config.ts.
  retries: process.env.CI ? 1 : 0,
  workers: 1,
  reporter: process.env.CI ? "github" : "list",
  snapshotPathTemplate: "{testDir}/__screenshots__/{arg}-{platform}{ext}",
  expect: {
    toHaveScreenshot: {
      animations: "disabled",
      caret: "hide",
      // Container rendering is deterministic; a tiny ratio absorbs at most
      // sub-pixel text antialiasing jitter without hiding real regressions.
      maxDiffPixelRatio: 0.01,
    },
  },
  use: {
    ...devices["Desktop Chrome"],
    baseURL: `http://127.0.0.1:${port}`,
    reducedMotion: "reduce",
    viewport: { width: 960, height: 680 },
  },
  webServer: {
    command: "npm run dev -- --host 127.0.0.1 --port 5173",
    url: `http://127.0.0.1:${port}`,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
    env: {
      ...process.env,
      VITE_PLAYWRIGHT: "true",
    },
  },
});
