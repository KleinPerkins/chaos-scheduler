import { afterEach, describe, expect, it, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import { useWorkflows } from "./useWorkflows";
import {
  createDefaultIpcRegistry,
  resolveIpcInvoke,
} from "../test/fixtures/ipc-registry";
import { sampleWorkflow } from "../test/fixtures/data";

function installStrictIpcMocks(): void {
  const registry = createDefaultIpcRegistry();
  mockIPC(
    (cmd, args) =>
      resolveIpcInvoke(cmd, (args ?? {}) as Record<string, unknown>, registry),
    { shouldMockEvents: true },
  );
}

describe("useWorkflows", () => {
  afterEach(() => {
    clearMocks();
  });

  it("loads workflows from IPC", async () => {
    installStrictIpcMocks();
    const { result } = renderHook(() => useWorkflows());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.error).toBeNull();
    expect(result.current.workflows).toEqual([sampleWorkflow]);
  });

  it("surfaces backend errors instead of empty state", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      list_workflows: () => {
        throw new Error("database unavailable");
      },
    };

    const { result } = renderHook(() => useWorkflows());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.workflows).toEqual([]);
    expect(result.current.error).toContain("database unavailable");

    delete window.__CHAOS_IPC_OVERRIDES__;
  });

  it("fails on unhandled invoke commands", async () => {
    const registry = createDefaultIpcRegistry();
    delete (registry as Partial<typeof registry>).list_workflows;
    mockIPC(
      (cmd, args) =>
        resolveIpcInvoke(
          cmd,
          (args ?? {}) as Record<string, unknown>,
          registry,
        ),
      { shouldMockEvents: true },
    );

    const listWorkflows = vi.fn(async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      return invoke("list_workflows");
    });

    await expect(listWorkflows()).rejects.toThrow(/Unhandled IPC invoke/);
  });
});
