import { afterEach, describe, expect, it } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import MenuBarPopup from "./MenuBarPopup";
import {
  createDefaultIpcRegistry,
  resolveIpcInvoke,
} from "../test/fixtures/ipc-registry";
import {
  availableUpdateSnapshot,
  idleUpdateSnapshot,
} from "../test/fixtures/data";
import { resetWorkflowQueueRequests } from "../lib/workflowEnqueue";

function installStrictIpcMocks(): void {
  const registry = createDefaultIpcRegistry();
  mockIPC(
    (cmd, args) =>
      resolveIpcInvoke(cmd, (args ?? {}) as Record<string, unknown>, registry),
    { shouldMockEvents: true },
  );
}

describe("MenuBarPopup update row", () => {
  afterEach(async () => {
    cleanup();
    // Let `useAppUpdate`'s async listener-cleanup microtask run before
    // clearMocks() tears down the event-plugin internals out from under it.
    await new Promise((r) => setTimeout(r, 0));
    clearMocks();
    delete window.__CHAOS_IPC_OVERRIDES__;
  });

  it("stays hidden when no update is available", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_app_update_status: () => idleUpdateSnapshot,
    };

    render(<MenuBarPopup />);

    await screen.findByText("Chaos Scheduler");
    expect(screen.queryByText(/Update available/)).not.toBeInTheDocument();
  });

  it("labels its main landmark with the product heading", async () => {
    installStrictIpcMocks();

    render(<MenuBarPopup />);

    await screen.findByRole("heading", {
      level: 1,
      name: "Chaos Scheduler",
    });
    expect(
      screen.getByRole("main", { name: "Chaos Scheduler" }),
    ).toBeInTheDocument();
  });

  it("shows an Update button when an update is available", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_app_update_status: () => availableUpdateSnapshot,
    };

    render(<MenuBarPopup />);

    await screen.findByText("Update available: v0.2.0");
    expect(screen.getByRole("button", { name: "Update" })).toBeEnabled();
  });

  it("clicking Update installs the offered version", async () => {
    installStrictIpcMocks();
    let installedVersion: unknown;
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_app_update_status: () => availableUpdateSnapshot,
      apply_update: (args) => {
        installedVersion = args.expectedVersion;
        return { ...availableUpdateSnapshot, phase: "ready_to_restart" };
      },
    };

    render(<MenuBarPopup />);

    const updateBtn = await screen.findByRole("button", { name: "Update" });
    fireEvent.click(updateBtn);

    await waitFor(() => expect(installedVersion).toBe("0.2.0"));
  });
});

describe("MenuBarPopup queue action", () => {
  afterEach(async () => {
    cleanup();
    await new Promise((r) => setTimeout(r, 0));
    clearMocks();
    resetWorkflowQueueRequests();
    delete window.__CHAOS_IPC_OVERRIDES__;
  });

  it("queues an upcoming run through admission control instead of firing immediately", async () => {
    installStrictIpcMocks();
    let enqueued = 0;
    let triggered = 0;
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_scheduler_status: () => ({
        active_workflows: 1,
        running_count: 0,
        next_runs: [
          {
            workflow_id: "wf-nightly",
            workflow_name: "Nightly sync",
            environment: "production",
            next_time: new Date(Date.now() + 3_600_000).toISOString(),
          },
        ],
        recent_runs: [],
      }),
      enqueue_workflow: () => {
        enqueued += 1;
        return {
          workflow_id: "wf-nightly",
          status: "queued",
          queued_run_id: "queued-abcdef12",
          queue_name: "default",
        };
      },
      trigger_workflow: () => {
        triggered += 1;
        return "run-immediate";
      },
    };

    render(<MenuBarPopup />);

    const queueBtn = await screen.findByRole("button", {
      name: "Queue Nightly sync",
    });
    fireEvent.click(queueBtn);

    await waitFor(() => expect(enqueued).toBe(1));
    expect(triggered).toBe(0);
    expect(await screen.findByText(/Waiting to start/)).toBeInTheDocument();
  });
});
