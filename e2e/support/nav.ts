import type { Page } from "@playwright/test";

export async function gotoDashboard(page: Page): Promise<void> {
  await page.goto("/");
  await page
    .getByRole("navigation", { name: "Primary navigation" })
    .waitFor({ state: "visible" });
}

export async function openSidebar(page: Page, label: string): Promise<void> {
  await page
    .getByRole("navigation", { name: "Primary navigation" })
    .getByRole("button", { name: label })
    .click();
}

export function workflowCard(page: Page, name = "Nightly sync") {
  return page.locator(".wf-card", { hasText: name });
}
