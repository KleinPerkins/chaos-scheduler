import { afterEach, describe, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
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

  it("uses the approved detail hierarchy while preserving run drill-downs", async () => {
    installStrictIpcMocks();
    const onEdit = vi.fn();
    const onFullHistory = vi.fn();
    const onViewRun = vi.fn();

    const { container } = render(
      <WorkflowDetail
        workflow={sampleWorkflow}
        onBack={() => {}}
        onEdit={onEdit}
        onFullHistory={onFullHistory}
        onViewRun={onViewRun}
      />,
    );

    await screen.findByRole("heading", { name: "Latest run" });

    const header = container.querySelector(".page-header");
    expect(header).not.toBeNull();
    expect(
      within(header as HTMLElement).getByRole("button", {
        name: "Edit workflow",
      }),
    ).toBeInTheDocument();
    expect(
      within(header as HTMLElement).getByText(/Daily at 9:00 AM · UTC/),
    ).toBeInTheDocument();
    expect(
      within(header as HTMLElement).queryByRole("button", { name: "Refresh" }),
    ).not.toBeInTheDocument();
    expect(
      within(header as HTMLElement).queryByRole("button", {
        name: "Full history",
      }),
    ).not.toBeInTheDocument();

    const latestRun = screen
      .getByRole("heading", { name: "Latest run" })
      .closest("section");
    expect(latestRun).not.toBeNull();
    expect(
      await within(latestRun as HTMLElement).findByText(/^Duration · /),
    ).toBeInTheDocument();
    fireEvent.click(
      within(latestRun as HTMLElement).getByRole("button", {
        name: "View latest run",
      }),
    );
    expect(onViewRun).toHaveBeenCalledWith("run-demo-1");

    fireEvent.click(screen.getByRole("button", { name: "View all" }));
    expect(onFullHistory).toHaveBeenCalledWith(
      expect.objectContaining({ id: sampleWorkflow.id }),
    );
  });

  it("exposes failure-heatmap cells to keyboard users with accessible names", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_workflow_history_buckets: () => [
        { day: "2026-07-10", total: 4, failed: 2, succeeded: 2 },
      ],
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

    // The heatmap detail was previously mouse-only (native `title`). Each cell
    // must now be keyboard-focusable with the failure summary as its
    // accessible name.
    const cell = await screen.findByRole("listitem", {
      name: "2026-07-10: 2 of 4 runs failed",
    });
    expect(cell).toHaveAttribute("tabindex", "0");
    cell.focus();
    expect(cell).toHaveFocus();
  });
});
