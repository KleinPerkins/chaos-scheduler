import { afterEach, describe, expect, it } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import Integrations from "./Integrations";
import type { ApiKey } from "../lib/commands";
import {
  createDefaultIpcRegistry,
  resolveIpcInvoke,
} from "../test/fixtures/ipc-registry";
import { defaultMcpIntegrationStatus } from "../test/fixtures/data";

function installStrictIpcMocks(): void {
  const registry = createDefaultIpcRegistry();
  mockIPC(
    (cmd, args) =>
      resolveIpcInvoke(cmd, (args ?? {}) as Record<string, unknown>, registry),
    { shouldMockEvents: true },
  );
}

const activeKey: ApiKey = {
  id: "key_active",
  name: "ci-runner",
  scopes: "read,write",
  created_at: "2026-01-01T00:00:00Z",
  last_used_at: null,
  revoked: false,
};

const revokedKey: ApiKey = {
  id: "key_revoked",
  name: "old-key",
  scopes: "read",
  created_at: "2025-12-01T00:00:00Z",
  last_used_at: null,
  revoked: true,
};

describe("Integrations API keys", () => {
  afterEach(() => {
    cleanup();
    clearMocks();
    delete window.__CHAOS_IPC_OVERRIDES__;
  });

  it("reflects a key's revoked status instead of showing it as active", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      list_api_keys: () => [activeKey, revokedKey],
    };

    render(<Integrations />);

    // The active key exposes a working Revoke button.
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Revoke API key" }),
      ).toBeInTheDocument(),
    );
    // Exactly one revocable key -> exactly one Revoke button.
    expect(
      screen.getAllByRole("button", { name: "Revoke API key" }),
    ).toHaveLength(1);
    // The revoked key surfaces its status and cannot be revoked again.
    expect(screen.getByText("Revoked")).toBeInTheDocument();
  });

  it("persists a revoke across a reload (durable revoked state)", async () => {
    installStrictIpcMocks();
    // Backend-of-record simulation: revoke flips the persisted `revoked` flag so
    // a subsequent listing (i.e. a reload/restart) returns the key as revoked.
    let persistedRevoked = false;
    window.__CHAOS_IPC_OVERRIDES__ = {
      list_api_keys: () => [{ ...activeKey, revoked: persistedRevoked }],
      revoke_api_key: () => {
        persistedRevoked = true;
      },
    };

    render(<Integrations />);

    const revokeBtn = await screen.findByRole("button", {
      name: "Revoke API key",
    });
    // Two-step confirm.
    fireEvent.click(revokeBtn);
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Confirm revoke API key" }),
      ).toBeInTheDocument(),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Confirm revoke API key" }),
    );

    // After the revoke + reload, the key is shown as revoked and no active
    // Revoke button remains — the revoke visibly took effect and persisted.
    await waitFor(() =>
      expect(screen.getByText("Revoked")).toBeInTheDocument(),
    );
    expect(
      screen.queryByRole("button", { name: /Revoke API key|Confirm revoke/ }),
    ).not.toBeInTheDocument();
  });
});

describe("Integrations managed MCP live refresh", () => {
  afterEach(() => {
    cleanup();
    clearMocks();
    delete window.__CHAOS_IPC_OVERRIDES__;
  });

  // Regression test for the "no live-refresh path after the startup
  // re-provision hook completes" finding: the card must pick up a status
  // change that originates entirely on the Rust side (no button click, no
  // re-fetch triggered by this component) via the `mcp-status-changed`
  // event, exactly like the background startup hook would emit after this
  // component has already mounted and fetched its (now-stale) status.
  it("refreshes the managed MCP status when an mcp-status-changed event arrives, with no user action", async () => {
    installStrictIpcMocks();
    render(<Integrations />);

    // Initial fetch reflects the default (not-installed) status.
    await waitFor(() =>
      expect(screen.getByText("Not installed")).toBeInTheDocument(),
    );

    const healedStatus = {
      ...defaultMcpIntegrationStatus,
      enabled: true,
      install_status: "installed" as const,
      provisioned_version: defaultMcpIntegrationStatus.pinned_version,
      registered_in_cursor: true,
      api_reachable: true,
      managed_key_id: "mcp-key-healed",
      matches: true,
    };

    await act(async () => {
      await emit("mcp-status-changed", healedStatus);
    });

    // No click, no re-fetch call from the test — the event alone must drive
    // the visible status update.
    await waitFor(() =>
      expect(screen.getByText("Installed")).toBeInTheDocument(),
    );
    expect(screen.getByText("Healthy")).toBeInTheDocument();
  });
});
