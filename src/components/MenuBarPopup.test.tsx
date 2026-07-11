import { afterEach, describe, expect, it } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import MenuBarPopup from "./MenuBarPopup";
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

describe("MenuBarPopup update row", () => {
  afterEach(async () => {
    cleanup();
    // Let `useAppUpdate`'s async listener-cleanup microtask run before
    // clearMocks() tears down the event-plugin internals out from under it.
    await new Promise((r) => setTimeout(r, 0));
    clearMocks();
    delete window.__CHAOS_IPC_OVERRIDES__;
  });

  it("stays hidden when no update is available", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_app_update_status: () => idleUpdateSnapshot,
    };

    render(<MenuBarPopup />);

    await screen.findByText("Chaos Scheduler");
    expect(screen.queryByText(/Update available/)).not.toBeInTheDocument();
  });

  it("labels its main landmark with the product heading", async () => {
    installStrictIpcMocks();

    render(<MenuBarPopup />);

    await screen.findByRole("heading", {
      level: 1,
      name: "Chaos Scheduler",
    });
    expect(
      screen.getByRole("main", { name: "Chaos Scheduler" }),
    ).toBeInTheDocument();
  });

  it("shows an Update button when an update is available", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_app_update_status: () => availableUpdateSnapshot,
    };

    render(<MenuBarPopup />);

    await screen.findByText("Update available: v0.2.0");
    expect(screen.getByRole("button", { name: "Update" })).toBeEnabled();
  });

  it("clicking Update installs the offered version", async () => {
    installStrictIpcMocks();
    let installedVersion: unknown;
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_app_update_status: () => availableUpdateSnapshot,
      apply_update: (args) => {
        installedVersion = args.expectedVersion;
        return { ...availableUpdateSnapshot, phase: "ready_to_restart" };
      },
    };

    render(<MenuBarPopup />);

    const updateBtn = await screen.findByRole("button", { name: "Update" });
    fireEvent.click(updateBtn);

    await waitFor(() => expect(installedVersion).toBe("0.2.0"));
  });
});
