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
  return page.getByRole("article", { name });
}

export async function openWorkflowRunHistory(
  page: Page,
  name = "Nightly sync",
): Promise<void> {
  await gotoDashboard(page);
  await openSidebar(page, "Workflows");
  await page.getByRole("button", { name, exact: true }).click();
  const viewAll = page.getByRole("button", { name: "View all" });
  await viewAll.waitFor({ state: "visible" });
  await viewAll.click();
  await page
    .getByRole("heading", { name: `${name} run history` })
    .waitFor({ state: "visible" });
}
