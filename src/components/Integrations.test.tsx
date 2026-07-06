import { afterEach, describe, expect, it } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import Integrations from "./Integrations";
import type { ApiKey } from "../lib/commands";
import {
  createDefaultIpcRegistry,
  resolveIpcInvoke,
} from "../test/fixtures/ipc-registry";

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
