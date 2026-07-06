import { test, expect } from "@playwright/test";
import { expectNoAxeViolations } from "./support/axe";
import { gotoDashboard, openSidebar } from "./support/nav";

test.describe("Email profiles", () => {
  test.beforeEach(async ({ page }) => {
    // Stateful in-memory mock so the list reflects create/delete round-trips.
    await page.addInitScript(() => {
      const store: Record<string, unknown>[] = [];
      window.__CHAOS_IPC_OVERRIDES__ = {
        list_email_profiles: () => store.map((p) => ({ ...p })),
        save_email_profile: (args) => {
          const profile = ((args?.profile as Record<string, unknown>) ??
            {}) as Record<string, unknown>;
          const id = (profile.id as string) || `profile-${store.length + 1}`;
          const saved = { ...profile, id };
          const idx = store.findIndex((p) => p.id === id);
          if (idx >= 0) store[idx] = saved;
          else store.push(saved);
          return { ...saved };
        },
        delete_email_profile: (args) => {
          const id = args?.id as string;
          const idx = store.findIndex((p) => p.id === id);
          if (idx >= 0) store.splice(idx, 1);
          return undefined;
        },
      };
    });
  });

  test("create, list, and delete a profile from Settings", async ({ page }) => {
    await gotoDashboard(page);
    await openSidebar(page, "Settings");

    const section = page.locator(".settings-section", {
      hasText: "Email Profiles",
    });
    await expect(section).toBeVisible();
    await expect(section.getByText("No email profiles yet.")).toBeVisible();
    await expectNoAxeViolations(page, "settings email profiles empty");

    await section.getByRole("button", { name: "New Profile" }).click();
    await page.getByLabel("Profile Name").fill("Production alerts");
    await page.getByLabel("Recipient").fill("prod@example.com");
    await expectNoAxeViolations(page, "settings email profile form");
    await section.getByRole("button", { name: "Save Profile" }).click();

    await expect(
      section.locator(".email-profile-name", { hasText: "Production alerts" }),
    ).toBeVisible();
    await expect(
      section.locator(".email-profile-recipient", {
        hasText: "prod@example.com",
      }),
    ).toBeVisible();

    await section.getByRole("button", { name: "Delete profile" }).click();
    await expect(section.getByText("No email profiles yet.")).toBeVisible();
  });
});
