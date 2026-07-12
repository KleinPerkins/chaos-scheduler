import { afterEach, describe, expect, it } from "vitest";
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
import { sampleWorkflow } from "../test/fixtures/data";

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
    window.__CHAOS_IPC_OVERRIDES__ = {
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
    expect(enqueueArgs).toHaveLength(1);
    expect(enqueueArgs[0]?.idempotencyKey).toMatch(/^ui-enqueue:wf-demo-1:/);
  });

  it("renders flat searchable cards and only observed queue activity", async () => {
    installStrictIpcMocks();
    const cacheWarmup = {
      ...sampleWorkflow,
      id: "wf-cache",
      name: "Cache warmup",
      description: "Primes the sandbox cache",
      environment: "sandbox",
    };
    const disabledExport = {
      ...sampleWorkflow,
      id: "wf-export",
      name: "Month-end export",
      description: "Produces the accounting export",
      enabled: false,
    };
    window.__CHAOS_IPC_OVERRIDES__ = {
      list_workflows: () => [sampleWorkflow, cacheWarmup, disabledExport],
      list_queued_runs: () => [
        {
          id: "queued-cache",
          workflow_id: cacheWarmup.id,
          workflow_name: cacheWarmup.name,
          queue_name: "default",
          environment: "sandbox",
          priority: 0,
          status: "queued",
          queued_at: "2026-07-11T12:00:00.000Z",
        },
      ],
    };

    const { container } = render(
      <WorkflowList
        onOpen={() => {}}
        onEdit={() => {}}
        onNew={() => {}}
        onHistory={() => {}}
      />,
    );

    await screen.findByText("Enabled · Waiting to start");
    expect(screen.getByText("3 workflows · flat results")).toBeInTheDocument();
    expect(screen.queryByText(/Running/)).not.toBeInTheDocument();
    expect(container.querySelector(".wf-group")).not.toBeInTheDocument();

    fireEvent.change(
      screen.getByRole("searchbox", { name: "Search workflows" }),
      {
        target: { value: "cache" },
      },
    );
    expect(screen.getByText("1 workflow · flat results")).toBeInTheDocument();
    expect(screen.getByText("Cache warmup")).toBeInTheDocument();
    expect(screen.queryByText("Nightly sync")).not.toBeInTheDocument();

    fireEvent.change(
      screen.getByRole("searchbox", { name: "Search workflows" }),
      {
        target: { value: "" },
      },
    );
    fireEvent.change(screen.getByRole("combobox", { name: "Status" }), {
      target: { value: "disabled" },
    });
    expect(screen.getByText("Month-end export")).toBeInTheDocument();
    expect(screen.queryByText("Cache warmup")).not.toBeInTheDocument();

    fireEvent.change(screen.getByRole("combobox", { name: "Status" }), {
      target: { value: "all" },
    });
    fireEvent.change(screen.getByRole("combobox", { name: "Environment" }), {
      target: { value: "sandbox" },
    });
    expect(screen.getByText("Cache warmup")).toBeInTheDocument();
    expect(screen.queryByText("Nightly sync")).not.toBeInTheDocument();
  });
});
