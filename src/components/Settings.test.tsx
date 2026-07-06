import { afterEach, describe, expect, it } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import Settings from "./Settings";
import {
  createDefaultIpcRegistry,
  resolveIpcInvoke,
} from "../test/fixtures/ipc-registry";
import {
  availableUpdateSnapshot,
  idleUpdateSnapshot,
} from "../test/fixtures/data";
import type { UpdateSnapshot } from "../lib/commands";

function installStrictIpcMocks(): void {
  const registry = createDefaultIpcRegistry();
  mockIPC(
    (cmd, args) =>
      resolveIpcInvoke(cmd, (args ?? {}) as Record<string, unknown>, registry),
    { shouldMockEvents: true },
  );
}

describe("Settings updater controls", () => {
  afterEach(async () => {
    cleanup();
    // Let `useAppUpdate`'s async listener-cleanup microtask run before
    // clearMocks() tears down the event-plugin internals out from under it.
    await new Promise((r) => setTimeout(r, 0));
    clearMocks();
    delete window.__CHAOS_IPC_OVERRIDES__;
  });

  it("reflects and toggles the background-check preference", async () => {
    installStrictIpcMocks();
    const snapshot: UpdateSnapshot = { ...idleUpdateSnapshot };
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_app_update_status: () => snapshot,
      set_updater_preferences: (args) => {
        if (typeof args.backgroundCheckEnabled === "boolean") {
          snapshot.background_check_enabled = args.backgroundCheckEnabled;
        }
        return { ...snapshot };
      },
    };

    render(<Settings />);

    const toggle = await screen.findByRole("checkbox", {
      name: "Check for updates automatically",
    });
    await waitFor(() => expect(toggle).toBeChecked());

    fireEvent.click(toggle);

    await waitFor(() => expect(toggle).not.toBeChecked());
  });

  it("offers Skip this version only while an update is available", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_app_update_status: () => idleUpdateSnapshot,
    };

    render(<Settings />);

    await screen.findByText("Updates");
    expect(
      screen.queryByRole("button", { name: /Skip this version/ }),
    ).not.toBeInTheDocument();
  });

  it("shows Install and Restart sourced from useAppUpdate() without requiring a manual check first", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_app_update_status: () => availableUpdateSnapshot,
    };

    render(<Settings />);

    await screen.findByRole("button", {
      name: `Install and Restart v${availableUpdateSnapshot.latest_version}`,
    });
    // The Skip affordance must coexist with (not contradict) the install
    // affordance — both read from the same `useAppUpdate()` snapshot, so
    // there is exactly one update-status/action surface.
    expect(
      screen.getByRole("button", { name: /Skip this version/ }),
    ).toBeInTheDocument();
    expect(
      screen.getAllByRole("button", { name: /Install and Restart/ }),
    ).toHaveLength(1);
  });

  it("passes the exact offered version as expectedVersion when installing", async () => {
    installStrictIpcMocks();
    let receivedArgs: Record<string, unknown> | undefined;
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_app_update_status: () => availableUpdateSnapshot,
      apply_update: (args) => {
        receivedArgs = args;
        return availableUpdateSnapshot;
      },
    };

    render(<Settings />);

    const installBtn = await screen.findByRole("button", {
      name: /Install and Restart/,
    });
    fireEvent.click(installBtn);

    await waitFor(() =>
      expect(receivedArgs).toEqual({
        expectedVersion: availableUpdateSnapshot.latest_version,
      }),
    );
  });

  it("skip then clear round-trips through Settings", async () => {
    installStrictIpcMocks();
    const snapshot: UpdateSnapshot = { ...availableUpdateSnapshot };
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_app_update_status: () => snapshot,
      set_updater_preferences: (args) => {
        if (args.clearSkip === true) {
          snapshot.skipped_version = null;
        } else if (typeof args.skippedVersion === "string") {
          snapshot.skipped_version = args.skippedVersion;
        }
        return { ...snapshot };
      },
    };

    render(<Settings />);

    const skipBtn = await screen.findByRole("button", {
      name: "Skip this version (v0.2.0)",
    });
    fireEvent.click(skipBtn);

    const clearBtn = await screen.findByRole("button", { name: "Clear skip" });
    expect(
      screen.queryByRole("button", { name: /Skip this version/ }),
    ).not.toBeInTheDocument();

    fireEvent.click(clearBtn);

    // Mutually exclusive with the "Clear skip" row in the same conditional
    // branch — its reappearance is proof the skip was cleared.
    await screen.findByRole("button", { name: "Skip this version (v0.2.0)" });
    expect(
      screen.queryByRole("button", { name: "Clear skip" }),
    ).not.toBeInTheDocument();
  });
});
