import { afterEach, describe, expect, it } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import UpdateBanner from "./UpdateBanner";
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

describe("UpdateBanner", () => {
  afterEach(async () => {
    cleanup();
    // `useAppUpdate`'s listener cleanup runs `unlisten.then(...)` — a
    // microtask — so give it a tick before `clearMocks()` tears down
    // `window.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener` out from
    // under it.
    await new Promise((r) => setTimeout(r, 0));
    clearMocks();
    delete window.__CHAOS_IPC_OVERRIDES__;
  });

  it("renders nothing when no update is available", async () => {
    installStrictIpcMocks();
    let hydrated = false;
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_app_update_status: () => {
        hydrated = true;
        return idleUpdateSnapshot;
      },
    };

    render(<UpdateBanner />);

    await waitFor(() => expect(hydrated).toBe(true));
    expect(screen.queryByText(/Update available/)).not.toBeInTheDocument();
  });

  it("shows the offered version, notes, and actions when available", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_app_update_status: () => availableUpdateSnapshot,
    };

    render(<UpdateBanner />);

    await screen.findByText(/Update available: v0\.2\.0/);
    expect(screen.getByText("Bug fixes and improvements.")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Install and Restart" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Release notes" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Skip this version" }),
    ).toBeInTheDocument();
  });

  it("shows an alert when the last check failed", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_app_update_status: () => ({
        ...idleUpdateSnapshot,
        phase: "error",
        last_error: { kind: "network", message: "connection reset" },
      }),
    };

    render(<UpdateBanner />);

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(
      /Update check failed \(network\): connection reset/,
    );
  });

  it("hides itself immediately after skipping, without waiting for the next check", async () => {
    installStrictIpcMocks();
    // Mirrors the real backend: setting `skipped_version` does not retroactively
    // clear `phase` — that only happens on the *next* check — so the banner's
    // own `justSkipped` guard is what has to hide it right away.
    const snapshot: UpdateSnapshot = { ...availableUpdateSnapshot };
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_app_update_status: () => snapshot,
      set_updater_preferences: (args) => {
        if (typeof args.skippedVersion === "string") {
          snapshot.skipped_version = args.skippedVersion;
        }
        return { ...snapshot };
      },
    };

    render(<UpdateBanner />);

    const skipBtn = await screen.findByRole("button", {
      name: "Skip this version",
    });
    fireEvent.click(skipBtn);

    await waitFor(() =>
      expect(screen.queryByText(/Update available/)).not.toBeInTheDocument(),
    );
  });

  it("shows download progress while downloading", async () => {
    installStrictIpcMocks();
    const downloading: UpdateSnapshot = {
      ...availableUpdateSnapshot,
      phase: "downloading",
      progress: { percent: 42 },
    };
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_app_update_status: () => downloading,
    };

    render(<UpdateBanner />);

    await screen.findByText(/Downloading update…\s*42%/);
    expect(screen.getByRole("button", { name: "Installing…" })).toBeDisabled();
  });
});
