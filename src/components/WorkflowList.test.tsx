import { afterEach, describe, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
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
    cleanup();
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

  it("uses the scheduler queue as its only manual execution path", async () => {
    installStrictIpcMocks();
    const enqueueArgs: Record<string, unknown>[] = [];
    const triggerWorkflow = vi.fn();
    window.__CHAOS_IPC_OVERRIDES__ = {
      trigger_workflow: triggerWorkflow,
      enqueue_workflow: (args) => {
        enqueueArgs.push(args);
        return {
          workflow_id: String(args.id),
          status: "queued",
          queued_run_id: "queue-contract-1",
          queue_name: "default",
        };
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
    expect(
      screen.queryByRole("button", { name: "Run Nightly sync now" }),
    ).not.toBeInTheDocument();

    fireEvent.click(
      screen.getByRole("button", { name: "Queue run for Nightly sync" }),
    );
    await waitFor(() =>
      expect(
        screen.getByText(/Waiting to start: Nightly sync/),
      ).toBeInTheDocument(),
    );
    expect(triggerWorkflow).not.toHaveBeenCalled();
    expect(enqueueArgs).toHaveLength(1);
    expect(enqueueArgs[0]?.idempotencyKey).toMatch(/^ui-enqueue:wf-demo-1:/);
  });
});
