import { afterEach, describe, expect, it } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import RunHistory from "./RunHistory";
import { sampleRun, sampleWorkflow } from "../test/fixtures/data";
import {
  createDefaultIpcRegistry,
  resolveIpcInvoke,
} from "../test/fixtures/ipc-registry";
import { resetWorkflowQueueRequests } from "../lib/workflowEnqueue";
import type { Run } from "../lib/commands";

function installStrictIpcMocks(): void {
  const registry = createDefaultIpcRegistry();
  mockIPC(
    (cmd, args) =>
      resolveIpcInvoke(cmd, (args ?? {}) as Record<string, unknown>, registry),
    { shouldMockEvents: true },
  );
}

const runs: Run[] = [
  { ...sampleRun, id: "run-ok-1", status: "succeeded", trigger_kind: "cron" },
  { ...sampleRun, id: "run-bad-1", status: "failed", trigger_kind: "manual" },
  {
    ...sampleRun,
    id: "run-live-1",
    status: "running",
    finished_at: null,
    trigger_kind: "cron",
  },
];

describe("RunHistory (workflow-scoped)", () => {
  afterEach(() => {
    cleanup();
    clearMocks();
    resetWorkflowQueueRequests();
    delete window.__CHAOS_IPC_OVERRIDES__;
  });

  it("queues a run through admission control and confirms the outcome", async () => {
    installStrictIpcMocks();
    let enqueued = 0;
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_run_history: () => runs,
      enqueue_workflow: () => {
        enqueued += 1;
        return {
          workflow_id: sampleWorkflow.id,
          status: "queued",
          queued_run_id: "queued-abc12345",
          queue_name: "default",
        };
      },
    };

    render(
      <RunHistory
        workflow={sampleWorkflow}
        onBack={() => {}}
        onViewLog={() => {}}
      />,
    );

    fireEvent.click(await screen.findByRole("button", { name: "Queue run" }));

    await waitFor(() => expect(enqueued).toBe(1));
    expect(await screen.findByText(/Waiting to start/)).toBeInTheDocument();
  });

  it("renders bounded failure history and filters only the loaded rows", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_run_history: () => runs,
      get_workflow_history_buckets: () => [
        { day: "2026-07-10", total: 3, failed: 1, succeeded: 2 },
      ],
    };

    render(
      <RunHistory
        workflow={sampleWorkflow}
        onBack={() => {}}
        onViewLog={() => {}}
      />,
    );

    expect(
      await screen.findByRole("heading", {
        name: `${sampleWorkflow.name} run history`,
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("region", {
        name: `${sampleWorkflow.name} run history`,
      }),
    ).toBeInTheDocument();
    expect(
      await screen.findByRole("heading", { name: "30-day failure history" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("listitem", {
        name: "2026-07-10: 1 of 3 runs failed",
      }),
    ).toBeInTheDocument();
    expect(screen.getByText("10")).toBeInTheDocument();
    expect(screen.getByText("Oldest")).toBeInTheDocument();
    expect(screen.getByText("Today")).toBeInTheDocument();
    expect(screen.getByText("Latest 50", { exact: true })).toBeInTheDocument();
    expect(screen.getByText("succeeded")).toBeInTheDocument();
    expect(screen.getByText("failed")).toBeInTheDocument();
    expect(screen.getByText("running")).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Status"), {
      target: { value: "failed" },
    });

    await waitFor(() =>
      expect(screen.queryByText("succeeded")).not.toBeInTheDocument(),
    );
    expect(screen.getByText("failed")).toBeInTheDocument();
    expect(screen.getByText("1 of 3 loaded")).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Status"), {
      target: { value: "all" },
    });
    fireEvent.change(screen.getByLabelText("Search loaded rows"), {
      target: { value: "live" },
    });

    await waitFor(() =>
      expect(screen.queryByText("failed")).not.toBeInTheDocument(),
    );
    expect(screen.getByText("running")).toBeInTheDocument();
    expect(screen.getByText("1 of 3 loaded")).toBeInTheDocument();
  });

  it("exposes heatmap cells to keyboard users with an accessible name", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_run_history: () => runs,
      get_workflow_history_buckets: () => [
        { day: "2026-07-10", total: 3, failed: 1, succeeded: 2 },
      ],
    };

    render(
      <RunHistory
        workflow={sampleWorkflow}
        onBack={() => {}}
        onViewLog={() => {}}
      />,
    );

    // The per-day detail must be reachable by keyboard, not mouse-only: the
    // cell is focusable (tabindex 0) and carries the failure summary as its
    // accessible name.
    const cell = await screen.findByRole("listitem", {
      name: "2026-07-10: 1 of 3 runs failed",
    });
    expect(cell).toHaveAttribute("tabindex", "0");
    cell.focus();
    expect(cell).toHaveFocus();
  });
});
