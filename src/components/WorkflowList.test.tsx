import { afterEach, describe, expect, it } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import WorkflowList from "./WorkflowList";
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

describe("WorkflowList", () => {
  afterEach(() => {
    clearMocks();
    delete window.__CHAOS_IPC_OVERRIDES__;
  });

  it("shows retry when workflow load fails", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      list_workflows: () => {
        throw new Error("database unavailable");
      },
    };

    render(
      <WorkflowList
        onOpen={() => {}}
        onEdit={() => {}}
        onNew={() => {}}
        onHistory={() => {}}
      />,
    );

    await waitFor(() =>
      expect(screen.getByText(/database unavailable/)).toBeInTheDocument(),
    );
    expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
    expect(screen.queryByText("No workflows yet")).not.toBeInTheDocument();
  });

  it("surfaces run trigger errors", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      trigger_workflow: () => {
        throw new Error("scheduler offline");
      },
    };

    render(
      <WorkflowList
        onOpen={() => {}}
        onEdit={() => {}}
        onNew={() => {}}
        onHistory={() => {}}
      />,
    );

    await waitFor(() =>
      expect(screen.getByText("Nightly sync")).toBeInTheDocument(),
    );
    fireEvent.click(screen.getByRole("button", { name: /Run/i }));

    await waitFor(() =>
      expect(screen.getByText(/scheduler offline/)).toBeInTheDocument(),
    );
  });
});
