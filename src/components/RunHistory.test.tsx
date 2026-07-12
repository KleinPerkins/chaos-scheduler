import { afterEach, describe, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
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

  it("queues a run through admission control, never the fire-now trigger", async () => {
    installStrictIpcMocks();
    let enqueued = 0;
    const trigger = vi.fn(() => sampleRun.id);
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_run_history: () => runs,
      trigger_workflow: trigger,
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
    // Manual runs are admission-controlled: the fire-immediately trigger path
    // must never be used.
    expect(trigger).not.toHaveBeenCalled();
  });

  it("reruns a specific past run via rerun_workflow, never enqueue or trigger", async () => {
    installStrictIpcMocks();
    // Rerun routes through admission control (#263): it resolves to a
    // DispatchOutcome, not a bare run-id string.
    const rerun = vi.fn(() => ({
      workflow_id: sampleWorkflow.id,
      status: "admitted",
      run_id: "run-rerun-xyz",
      queue_name: "default",
    }));
    const enqueue = vi.fn(() => ({
      workflow_id: sampleWorkflow.id,
      status: "queued",
      queued_run_id: "queued-unused",
      queue_name: "default",
    }));
    const trigger = vi.fn(() => sampleRun.id);
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_run_history: () => runs,
      rerun_workflow: rerun,
      enqueue_workflow: enqueue,
      trigger_workflow: trigger,
    };

    render(
      <RunHistory
        workflow={sampleWorkflow}
        onBack={() => {}}
        onViewLog={() => {}}
      />,
    );

    // Open the rerun modal for the FAILED row (run-bad-1).
    fireEvent.click(
      await screen.findByRole("button", {
        name: /Rerun failed run started/i,
      }),
    );
    const dialog = await screen.findByRole("dialog", {
      name: /Rerun Nightly sync/i,
    });

    // Submit the default "{}" override.
    fireEvent.click(within(dialog).getByRole("button", { name: "Rerun" }));

    await waitFor(() => expect(rerun).toHaveBeenCalledTimes(1));
    const args = rerun.mock.calls[0][0] as Record<string, unknown>;
    expect(args.workflowId).toBe(sampleWorkflow.id);
    // Rerun re-runs a SPECIFIC past run, identified by its source run id.
    expect(args.sourceRunId).toBe("run-bad-1");
    // It targets a past run — it must not fire-now or enqueue a fresh run.
    expect(trigger).not.toHaveBeenCalled();
    expect(enqueue).not.toHaveBeenCalled();
  });

  it("surfaces the admission outcome when a queued rerun is accepted", async () => {
    installStrictIpcMocks();
    // A dependency-/capacity-gated rerun QUEUES instead of starting now. The UI
    // must consume that DispatchOutcome and show the same feedback the
    // Queue-run path does, or a queued rerun is silently invisible to the user.
    const rerun = vi.fn(() => ({
      workflow_id: sampleWorkflow.id,
      status: "queued",
      queued_run_id: "rerun-queued-9",
      queue_name: "default",
    }));
    const enqueue = vi.fn(() => ({
      workflow_id: sampleWorkflow.id,
      status: "queued",
      queued_run_id: "queued-unused",
      queue_name: "default",
    }));
    const trigger = vi.fn(() => sampleRun.id);
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_run_history: () => runs,
      rerun_workflow: rerun,
      enqueue_workflow: enqueue,
      trigger_workflow: trigger,
    };

    render(
      <RunHistory
        workflow={sampleWorkflow}
        onBack={() => {}}
        onViewLog={() => {}}
      />,
    );

    fireEvent.click(
      await screen.findByRole("button", {
        name: /Rerun failed run started/i,
      }),
    );
    const dialog = await screen.findByRole("dialog", {
      name: /Rerun Nightly sync/i,
    });
    fireEvent.click(within(dialog).getByRole("button", { name: "Rerun" }));

    await waitFor(() => expect(rerun).toHaveBeenCalledTimes(1));

    // The queued outcome is surfaced to the user, carrying the short queued-run
    // identity — mirroring formatWorkflowQueueOutcome used by "Queue run".
    expect(
      await screen.findByText(/Waiting to start: Nightly sync.*rerun-qu/),
    ).toBeInTheDocument();

    // Still a rerun-through-admission — never the dead fire-now trigger nor a
    // fresh enqueue.
    expect(trigger).not.toHaveBeenCalled();
    expect(enqueue).not.toHaveBeenCalled();
  });

  it("drills into a run's detail and exposes a named runs table", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = { get_run_history: () => runs };
    const onViewLog = vi.fn();

    render(
      <RunHistory
        workflow={sampleWorkflow}
        onBack={() => {}}
        onViewLog={onViewLog}
      />,
    );

    // The runs render inside a named region + a table with an accessible name
    // (its sr-only <caption>).
    expect(
      await screen.findByRole("region", { name: "Latest runs" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("table", {
        name: `Latest runs for ${sampleWorkflow.name}`,
      }),
    ).toBeInTheDocument();

    const detailButtons = screen.getAllByRole("button", {
      name: /View details for .* run started/i,
    });
    fireEvent.click(detailButtons[0]);
    expect(onViewLog).toHaveBeenCalledTimes(1);
    // First row is the succeeded run (run-ok-1).
    expect(onViewLog).toHaveBeenCalledWith("run-ok-1");
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

  it("announces a run-history load failure as an alert", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_run_history: () => {
        throw new Error("db locked");
      },
    };

    render(
      <RunHistory
        workflow={sampleWorkflow}
        onBack={() => {}}
        onViewLog={() => {}}
      />,
    );

    // A failed async load must be announced to assistive tech, not shown as a
    // silent div.
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(/Run history failed to load/i);
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
