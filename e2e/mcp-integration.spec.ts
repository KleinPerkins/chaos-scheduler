import { test, expect, type Page } from "@playwright/test";
import type { McpIntegrationStatus } from "../src/lib/commands";
import { expectNoAxeViolations } from "./support/axe";
import { gotoDashboard, openSidebar } from "./support/nav";

const BASE_STATUS: McpIntegrationStatus = {
  enabled: false,
  install_status: "not_installed",
  node_available: true,
  node_path: "/usr/local/bin/node",
  npm_available: true,
  npm_path: "/usr/local/bin/npm",
  provisioned_version: null,
  pinned_version: "0.5.0",
  registered_in_cursor: false,
  cursor_config_conflict: false,
  api_reachable: true,
  managed_key_id: null,
  matches: false,
  last_error: null,
};

async function mockMcpStatus(page: Page, status: McpIntegrationStatus) {
  await page.addInitScript((s: McpIntegrationStatus) => {
    window.__CHAOS_IPC_OVERRIDES__ = {
      ...window.__CHAOS_IPC_OVERRIDES__,
      get_mcp_integration_status: () => s,
    };
  }, status);
}

async function openManagedCard(page: Page) {
  await gotoDashboard(page);
  await openSidebar(page, "Integrations");
  await expect(page.getByTestId("mcp-managed-card")).toBeVisible();
}

test.describe("Integrations — managed MCP card", () => {
  test("disabled: offers Enable, no version installed", async ({ page }) => {
    await mockMcpStatus(page, BASE_STATUS);
    await openManagedCard(page);

    await expect(page.getByText("Not installed")).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Enable managed integration" }),
    ).toBeVisible();
    await expectNoAxeViolations(page, "mcp card disabled");
  });

  test("provisioned: healthy, matching version, Re-provision + Remove available", async ({
    page,
  }) => {
    await mockMcpStatus(page, {
      ...BASE_STATUS,
      enabled: true,
      install_status: "installed",
      provisioned_version: "0.5.0",
      registered_in_cursor: true,
      managed_key_id: "mcp-key-1",
      matches: true,
    });
    await openManagedCard(page);

    await expect(page.getByText("Installed")).toBeVisible();
    await expect(page.getByText("Healthy")).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Re-provision" }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Remove managed integration" }),
    ).toBeVisible();
    await expectNoAxeViolations(page, "mcp card provisioned");
  });

  test("mismatch: provisioned version is stale vs. pinned", async ({
    page,
  }) => {
    await mockMcpStatus(page, {
      ...BASE_STATUS,
      enabled: true,
      install_status: "stale",
      provisioned_version: "0.4.0",
      registered_in_cursor: true,
      managed_key_id: "mcp-key-1",
      matches: false,
    });
    await openManagedCard(page);

    await expect(page.getByText("Update available")).toBeVisible();
    await expect(page.getByText("Needs attention")).toBeVisible();
    await expect(page.getByText("0.4.0")).toBeVisible();
    await expectNoAxeViolations(page, "mcp card mismatch");
  });

  test("node-missing: integration unavailable with guidance", async ({
    page,
  }) => {
    await mockMcpStatus(page, {
      ...BASE_STATUS,
      enabled: true,
      install_status: "node_unavailable",
      node_available: false,
      node_path: null,
      npm_available: false,
      npm_path: null,
      last_error:
        "Node.js was not found at any known absolute install location.",
    });
    await openManagedCard(page);

    await expect(page.getByText("Node.js not found")).toBeVisible();
    await expect(page.getByText(/install Node/)).toBeVisible();
    await expect(
      page.getByText(/Node\.js was not found at any known/),
    ).toBeVisible();
    await expectNoAxeViolations(page, "mcp card node missing");
  });

  test("cursor-conflict: offers a forced take-over action", async ({
    page,
  }) => {
    await mockMcpStatus(page, {
      ...BASE_STATUS,
      enabled: true,
      install_status: "installed",
      provisioned_version: "0.5.0",
      registered_in_cursor: false,
      cursor_config_conflict: true,
      managed_key_id: "mcp-key-1",
      matches: false,
    });
    await openManagedCard(page);

    await expect(page.getByText(/Conflict/)).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Take over conflicting entry" }),
    ).toBeVisible();
    await expectNoAxeViolations(page, "mcp card cursor conflict");
  });

  test("api-down: reachability reported as No", async ({ page }) => {
    await mockMcpStatus(page, {
      ...BASE_STATUS,
      enabled: true,
      install_status: "installed",
      provisioned_version: "0.5.0",
      registered_in_cursor: true,
      managed_key_id: "mcp-key-1",
      api_reachable: false,
      matches: false,
    });
    await openManagedCard(page);

    await expect(page.getByText("Needs attention")).toBeVisible();
    const apiRow = page.locator("dt", { hasText: "API reachable" });
    await expect(apiRow.locator("xpath=following-sibling::dd[1]")).toHaveText(
      "No",
    );
    await expectNoAxeViolations(page, "mcp card api down");
  });

  test("enabling the integration calls provision and reflects the result", async ({
    page,
  }) => {
    await mockMcpStatus(page, BASE_STATUS);
    await page.addInitScript(() => {
      window.__CHAOS_IPC_OVERRIDES__ = {
        ...window.__CHAOS_IPC_OVERRIDES__,
        provision_mcp_integration: () => ({
          enabled: true,
          install_status: "installed",
          node_available: true,
          node_path: "/usr/local/bin/node",
          npm_available: true,
          npm_path: "/usr/local/bin/npm",
          provisioned_version: "0.5.0",
          pinned_version: "0.5.0",
          registered_in_cursor: true,
          cursor_config_conflict: false,
          api_reachable: true,
          managed_key_id: "mcp-key-1",
          matches: true,
          last_error: null,
        }),
      };
    });
    await openManagedCard(page);

    await page
      .getByRole("button", { name: "Enable managed integration" })
      .click();

    await expect(page.getByText("Installed")).toBeVisible();
    await expect(page.getByText("Healthy")).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Re-provision" }),
    ).toBeVisible();
  });
});
