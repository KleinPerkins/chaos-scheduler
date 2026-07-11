import { afterEach, describe, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import WorkflowDetail from "./WorkflowDetail";
import { sampleWorkflow } from "../test/fixtures/data";
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

describe("WorkflowDetail manual execution", () => {
  afterEach(() => {
    cleanup();
    clearMocks();
    delete window.__CHAOS_IPC_OVERRIDES__;
  });

  it("queues through admission control and reports the queued-run identity", async () => {
    installStrictIpcMocks();
    const triggerWorkflow = vi.fn();
    let enqueueArgs: Record<string, unknown> | undefined;
    window.__CHAOS_IPC_OVERRIDES__ = {
      trigger_workflow: triggerWorkflow,
      enqueue_workflow: (args) => {
        enqueueArgs = args;
        return {
          workflow_id: sampleWorkflow.id,
          status: "queued",
          queued_run_id: "queued-detail-1",
          queue_name: "default",
        };
      },
    };

    render(
      <WorkflowDetail
        workflow={sampleWorkflow}
        onBack={() => {}}
        onEdit={() => {}}
        onFullHistory={() => {}}
        onViewRun={() => {}}
      />,
    );

    await screen.findByRole("heading", { name: sampleWorkflow.name });
    expect(
      screen.queryByRole("button", { name: "Run" }),
    ).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Queue run" }));

    await waitFor(() =>
      expect(
        screen.getByText(/Waiting to start: Nightly sync.*queued-d/),
      ).toBeInTheDocument(),
    );
    expect(triggerWorkflow).not.toHaveBeenCalled();
    expect(enqueueArgs?.idempotencyKey).toMatch(/^ui-enqueue:wf-demo-1:/);
  });
});
