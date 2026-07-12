import { test, expect } from "@playwright/test";
import { expectNoAxeViolations } from "./support/axe";
import { gotoDashboard, openSidebar } from "./support/nav";

const AVAILABLE_SNAPSHOT = {
  updater_available: true,
  phase: "available",
  current_version: "0.1.0",
  latest_version: "0.2.0",
  notes: "Bug fixes and improvements.",
  last_checked_at: "2026-07-04T12:00:00.000Z",
  last_error: null,
  progress: null,
  background_check_enabled: true,
  skipped_version: null,
};

const IDLE_SNAPSHOT = {
  ...AVAILABLE_SNAPSHOT,
  phase: "idle",
  latest_version: null,
  notes: null,
};

test.describe("Update UX — dashboard banner", () => {
  test("none: no banner when nothing is available", async ({ page }) => {
    await page.addInitScript((snapshot) => {
      window.__CHAOS_IPC_OVERRIDES__ = {
        get_app_update_status: () => snapshot,
      };
    }, IDLE_SNAPSHOT);

    await gotoDashboard(page);

    await expect(page.getByText(/Update available/)).not.toBeVisible();
    await expectNoAxeViolations(page, "dashboard, no update");
  });

  test("available: shows the offered version with install/skip/notes actions", async ({
    page,
  }) => {
    await page.addInitScript((snapshot) => {
      window.__CHAOS_IPC_OVERRIDES__ = {
        get_app_update_status: () => snapshot,
      };
    }, AVAILABLE_SNAPSHOT);

    await gotoDashboard(page);

    await expect(page.getByText("Update available: v0.2.0")).toBeVisible();
    await expect(page.getByText("Bug fixes and improvements.")).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Install and Restart" }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Release notes" }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Skip this version" }),
    ).toBeVisible();
    await expectNoAxeViolations(page, "dashboard, update available");
  });

  test("updater-unavailable: an older-build backend degrades to no banner, no crash", async ({
    page,
  }) => {
    await page.addInitScript(() => {
      window.__CHAOS_IPC_OVERRIDES__ = {
        get_app_update_status: () => {
          throw new Error("unknown command get_app_update_status");
        },
      };
    });

    await gotoDashboard(page);

    await expect(
      page.getByRole("heading", {
        name: "Scheduler operations by environment and owner",
      }),
    ).toBeVisible();
    await expect(page.getByText(/Update available/)).not.toBeVisible();
    await expectNoAxeViolations(page, "dashboard, updater unavailable");
  });

  test("skip: hides the banner immediately, without a fresh check", async ({
    page,
  }) => {
    await page.addInitScript((snapshot) => {
      // Mirrors the real backend: setting `skipped_version` doesn't
      // retroactively clear `phase` — only the next check does that — so
      // this proves the frontend's own instant-hide guard, not the backend.
      const state = { ...snapshot };
      window.__CHAOS_IPC_OVERRIDES__ = {
        get_app_update_status: () => state,
        set_updater_preferences: (args: Record<string, unknown>) => {
          if (typeof args.skippedVersion === "string") {
            state.skipped_version = args.skippedVersion;
          }
          return { ...state };
        },
      };
    }, AVAILABLE_SNAPSHOT);

    await gotoDashboard(page);

    await page.getByRole("button", { name: "Skip this version" }).click();
    await expect(page.getByText(/Update available/)).not.toBeVisible();
    await expectNoAxeViolations(page, "dashboard, just skipped");
  });

  test("error: a failed manual check surfaces an alert", async ({ page }) => {
    await page.addInitScript((snapshot) => {
      window.__CHAOS_IPC_OVERRIDES__ = {
        get_app_update_status: () => ({
          ...snapshot,
          phase: "error",
          latest_version: null,
          last_error: { kind: "network", message: "connection reset" },
        }),
      };
    }, IDLE_SNAPSHOT);

    await gotoDashboard(page);

    await expect(
      page.getByText(/Update check failed \(network\): connection reset/),
    ).toBeVisible();
    await expectNoAxeViolations(page, "dashboard, update check error");
  });
});

test.describe("Update UX — menu bar popup", () => {
  test("available: shows a compact update row with Install and Skip", async ({
    page,
  }) => {
    await page.addInitScript((snapshot) => {
      window.__CHAOS_IPC_OVERRIDES__ = {
        get_app_update_status: () => snapshot,
      };
    }, AVAILABLE_SNAPSHOT);

    await page.goto("/?view=popup");

    await expect(page.getByText("Update available: v0.2.0")).toBeVisible();
    await expect(page.getByRole("button", { name: "Install" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Skip" })).toBeVisible();
    await expectNoAxeViolations(page, "popup, update available");
  });

  test("none: no update row in the popup", async ({ page }) => {
    await page.addInitScript((snapshot) => {
      window.__CHAOS_IPC_OVERRIDES__ = {
        get_app_update_status: () => snapshot,
      };
    }, IDLE_SNAPSHOT);

    await page.goto("/?view=popup");

    await expect(page.getByText("Chaos Scheduler")).toBeVisible();
    await expect(page.getByText(/Update available/)).not.toBeVisible();
    await expectNoAxeViolations(page, "popup, no update");
  });
});

test.describe("Update UX — Settings", () => {
  test("toggle, skip, and clear the background-check preference", async ({
    page,
  }) => {
    await page.addInitScript((snapshot) => {
      const state = { ...snapshot };
      window.__CHAOS_IPC_OVERRIDES__ = {
        get_app_update_status: () => state,
        set_updater_preferences: (args: Record<string, unknown>) => {
          if (typeof args.backgroundCheckEnabled === "boolean") {
            state.background_check_enabled = args.backgroundCheckEnabled;
          }
          if (args.clearSkip === true) {
            state.skipped_version = null;
          } else if (typeof args.skippedVersion === "string") {
            state.skipped_version = args.skippedVersion;
          }
          return { ...state };
        },
      };
    }, AVAILABLE_SNAPSHOT);

    await gotoDashboard(page);
    await openSidebar(page, "Settings");
    await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();

    const toggle = page.getByRole("checkbox", {
      name: "Check for updates automatically",
    });
    await expect(toggle).toBeChecked();
    await toggle.click();
    await expect(toggle).not.toBeChecked();

    const skipBtn = page.getByRole("button", {
      name: "Skip this version (v0.2.0)",
    });
    await expect(skipBtn).toBeVisible();
    await skipBtn.click();

    const clearBtn = page.getByRole("button", { name: "Clear skip" });
    await expect(clearBtn).toBeVisible();
    await expect(skipBtn).not.toBeVisible();
    await expectNoAxeViolations(page, "settings, version skipped");

    await clearBtn.click();
    await expect(skipBtn).toBeVisible();
    await expectNoAxeViolations(page, "settings, skip cleared");
  });
});
