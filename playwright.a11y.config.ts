import { defineConfig, devices } from "@playwright/test";

const port = 5173;

/**
 * Accessibility config, isolated from the functional e2e config
 * (playwright.config.ts, which ignores `e2e/a11y/**`). Runs the strict axe /
 * theme / reduced-motion / font suite. Serial (workers:1) in CI so the required
 * gate is stable; the reduced-motion spec emulates the media feature itself.
 */
export default defineConfig({
  testDir: "./e2e/a11y",
  fullyParallel: !process.env.CI,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: process.env.CI ? "github" : "list",
  use: {
    ...devices["Desktop Chrome"],
    baseURL: `http://127.0.0.1:${port}`,
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
