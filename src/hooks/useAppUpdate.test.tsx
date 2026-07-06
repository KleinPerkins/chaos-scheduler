import { afterEach, describe, expect, it } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { useAppUpdate } from "./useAppUpdate";
import {
  createDefaultIpcRegistry,
  resolveIpcInvoke,
} from "../test/fixtures/ipc-registry";
import {
  availableUpdateSnapshot,
  idleUpdateSnapshot,
} from "../test/fixtures/data";

function installStrictIpcMocks(): void {
  const registry = createDefaultIpcRegistry();
  mockIPC(
    (cmd, args) =>
      resolveIpcInvoke(cmd, (args ?? {}) as Record<string, unknown>, registry),
    { shouldMockEvents: true },
  );
}

describe("useAppUpdate", () => {
  afterEach(() => {
    clearMocks();
  });

  it("hydrates from get_app_update_status on mount", async () => {
    installStrictIpcMocks();
    const { result } = renderHook(() => useAppUpdate());

    await waitFor(() => expect(result.current.snapshot).not.toBeNull());
    expect(result.current.snapshot).toEqual(idleUpdateSnapshot);
    expect(result.current.unavailable).toBe(false);
  });

  it("flags CommandUnavailableError instead of throwing", async () => {
    const registry = createDefaultIpcRegistry();
    mockIPC(
      (cmd, args) =>
        resolveIpcInvoke(
          cmd,
          (args ?? {}) as Record<string, unknown>,
          registry,
        ),
      { shouldMockEvents: true },
    );
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_app_update_status: () => {
        throw new Error("unknown command get_app_update_status");
      },
    };

    const { result } = renderHook(() => useAppUpdate());

    await waitFor(() => expect(result.current.unavailable).toBe(true));
    expect(result.current.snapshot).toBeNull();

    delete window.__CHAOS_IPC_OVERRIDES__;
  });

  it("updates the snapshot when an update-status event arrives", async () => {
    installStrictIpcMocks();
    const { result } = renderHook(() => useAppUpdate());

    await waitFor(() => expect(result.current.snapshot).not.toBeNull());

    await act(async () => {
      await emit("update-status", availableUpdateSnapshot);
    });

    await waitFor(() =>
      expect(result.current.snapshot).toEqual(availableUpdateSnapshot),
    );
  });

  it("setBackgroundCheckEnabled persists the preference and updates state", async () => {
    installStrictIpcMocks();
    const { result } = renderHook(() => useAppUpdate());
    await waitFor(() => expect(result.current.snapshot).not.toBeNull());

    await act(async () => {
      await result.current.setBackgroundCheckEnabled(false);
    });

    expect(result.current.snapshot?.background_check_enabled).toBe(false);
  });

  it("skipVersion then clearSkippedVersion round-trips the preference", async () => {
    installStrictIpcMocks();
    const { result } = renderHook(() => useAppUpdate());
    await waitFor(() => expect(result.current.snapshot).not.toBeNull());

    await act(async () => {
      await result.current.skipVersion("0.2.0");
    });
    expect(result.current.snapshot?.skipped_version).toBe("0.2.0");

    await act(async () => {
      await result.current.clearSkippedVersion();
    });
    expect(result.current.snapshot?.skipped_version).toBeNull();
  });

  it("install() delegates to apply_update with the expected version", async () => {
    const registry = createDefaultIpcRegistry();
    let receivedArgs: Record<string, unknown> | undefined;
    mockIPC(
      (cmd, args) => {
        if (cmd === "apply_update") {
          receivedArgs = (args ?? {}) as Record<string, unknown>;
        }
        return resolveIpcInvoke(
          cmd,
          (args ?? {}) as Record<string, unknown>,
          registry,
        );
      },
      { shouldMockEvents: true },
    );

    const { result } = renderHook(() => useAppUpdate());
    await waitFor(() => expect(result.current.snapshot).not.toBeNull());

    await act(async () => {
      await result.current.install("0.2.0");
    });

    expect(receivedArgs).toEqual({ expectedVersion: "0.2.0" });
  });
});
