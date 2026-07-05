import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import RunDetail from "./RunDetail";
import {
  createDefaultIpcRegistry,
  resolveIpcInvoke,
} from "../test/fixtures/ipc-registry";
import { sampleRun } from "../test/fixtures/data";

function installStrictIpcMocks(): void {
  const registry = createDefaultIpcRegistry();
  mockIPC(
    (cmd, args) =>
      resolveIpcInvoke(cmd, (args ?? {}) as Record<string, unknown>, registry),
    { shouldMockEvents: true },
  );
}

describe("RunDetail", () => {
  afterEach(() => {
    clearMocks();
    delete window.__CHAOS_IPC_OVERRIDES__;
  });

  it("shows error state with back and retry when load fails", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_run_log: () => {
        throw new Error("run missing");
      },
    };

    const onBack = vi.fn();
    render(<RunDetail runId="missing-run" onBack={onBack} />);

    await waitFor(() =>
      expect(screen.getByText(/Failed to load run/)).toBeInTheDocument(),
    );
    expect(screen.getByRole("button", { name: /Back/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Back/i }));
    expect(onBack).toHaveBeenCalled();
  });

  it("shows live indicator for active runs", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_run_log: () => ({
        ...sampleRun,
        status: "running",
        finished_at: null,
      }),
    };

    render(<RunDetail runId={sampleRun.id} onBack={() => {}} />);

    await waitFor(() => expect(screen.getByText("Live")).toBeInTheDocument());
  });
});
