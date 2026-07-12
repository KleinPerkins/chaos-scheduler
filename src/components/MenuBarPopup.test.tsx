import { afterEach, describe, expect, it } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import MenuBarPopup from "./MenuBarPopup";
import {
  createDefaultIpcRegistry,
  resolveIpcInvoke,
} from "../test/fixtures/ipc-registry";
import {
  availableUpdateSnapshot,
  emptyMissionControlSnapshot,
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

describe("MenuBarPopup", () => {
  afterEach(async () => {
    cleanup();
    resetWorkflowQueueRequests();
    // Let `useAppUpdate`'s async listener-cleanup microtask run before
    // clearMocks() tears down the event-plugin internals out from under it.
    await new Promise((r) => setTimeout(r, 0));
    clearMocks();
    delete window.__CHAOS_IPC_OVERRIDES__;
  });

  it("announces an initial status-load failure as an alert", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      // The mini-dashboard sources its glance from the Mission Control snapshot,
      // so a failed snapshot fetch is what must be announced.
      get_mission_control_snapshot: () => {
        throw new Error("scheduler offline");
      },
    };

    render(<MenuBarPopup />);

    // The async status fetch failed, so the error must be announced (role=alert)
    // rather than rendered as a silent div a screen reader never voices.
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(/Status failed to load/i);
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

  it("renders running/queued/failed summary chips from the snapshot + queued runs", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_mission_control_snapshot: () => ({
        ...emptyMissionControlSnapshot,
        header: {
          active_workflows: 5,
          running_count: 2,
          // `queued_count` in the header is intentionally different from the
          // live queue depth below, to prove the Queued chip is sourced from
          // `list_queued_runs` (3), not the header (9).
          queued_count: 9,
          recent_failures: 4,
        },
      }),
      list_queued_runs: () => [
        {
          id: "q1",
          workflow_id: "wf-a",
          queue_name: "default",
          environment: "production",
          priority: 0,
          status: "queued",
          queued_at: "2026-07-04T11:59:00.000Z",
        },
        {
          id: "q2",
          workflow_id: "wf-b",
          queue_name: "default",
          environment: "production",
          priority: 0,
          status: "queued",
          queued_at: "2026-07-04T11:59:30.000Z",
        },
        {
          id: "q3",
          workflow_id: "wf-c",
          queue_name: "default",
          environment: "production",
          priority: 0,
          status: "queued",
          queued_at: "2026-07-04T11:59:45.000Z",
        },
      ],
    };

    render(<MenuBarPopup />);

    const summary = await screen.findByRole("group", { name: "Run summary" });
    expect(within(summary).getByText("Running")).toBeInTheDocument();
    expect(within(summary).getByText("Queued")).toBeInTheDocument();
    expect(within(summary).getByText("Failed")).toBeInTheDocument();
    // running ← header.running_count
    expect(within(summary).getByText("2")).toBeInTheDocument();
    // queued ← list_queued_runs length (NOT header.queued_count of 9)
    expect(within(summary).getByText("3")).toBeInTheDocument();
    // failed ← header.recent_failures
    expect(within(summary).getByText("4")).toBeInTheDocument();
    expect(within(summary).queryByText("9")).not.toBeInTheDocument();
  });

  it("has no Pause all control (no backend exists for it)", async () => {
    installStrictIpcMocks();

    render(<MenuBarPopup />);

    await screen.findByText("Chaos Scheduler");
    expect(
      screen.queryByRole("button", { name: /pause all/i }),
    ).not.toBeInTheDocument();
    expect(screen.queryByText(/pause all/i)).not.toBeInTheDocument();
  });

  it("labels the update CTA Install (not Update)", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_app_update_status: () => availableUpdateSnapshot,
    };

    render(<MenuBarPopup />);

    await screen.findByText("Update available: v0.2.0");
    expect(screen.getByRole("button", { name: "Install" })).toBeEnabled();
    expect(
      screen.queryByRole("button", { name: "Update" }),
    ).not.toBeInTheDocument();
  });

  it("clicking Install installs the offered version", async () => {
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

    const installBtn = await screen.findByRole("button", { name: "Install" });
    fireEvent.click(installBtn);

    await waitFor(() => expect(installedVersion).toBe("0.2.0"));
  });

  it("clicking Skip skips the offered version", async () => {
    installStrictIpcMocks();
    let skippedVersion: unknown;
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_app_update_status: () => availableUpdateSnapshot,
      set_updater_preferences: (args) => {
        skippedVersion = args.skippedVersion;
        return {
          ...availableUpdateSnapshot,
          skipped_version: String(args.skippedVersion ?? ""),
        };
      },
    };

    render(<MenuBarPopup />);

    const skipBtn = await screen.findByRole("button", { name: "Skip" });
    fireEvent.click(skipBtn);

    await waitFor(() => expect(skippedVersion).toBe("0.2.0"));
  });

  it("queues upcoming workflows through shared admission control", async () => {
    const registry = createDefaultIpcRegistry();
    const calls: Array<{
      command: string;
      args: Record<string, unknown>;
    }> = [];
    mockIPC(
      (command, args) => {
        const invokeArgs = (args ?? {}) as Record<string, unknown>;
        calls.push({ command, args: invokeArgs });
        return resolveIpcInvoke(command, invokeArgs, registry);
      },
      { shouldMockEvents: true },
    );
    window.__CHAOS_IPC_OVERRIDES__ = {
      // The queue-run affordance is preserved, now sourced from the snapshot's
      // upcoming runs instead of the scheduler-status next_runs.
      get_mission_control_snapshot: () => ({
        ...emptyMissionControlSnapshot,
        header: {
          active_workflows: 1,
          running_count: 0,
          queued_count: 0,
          recent_failures: 0,
        },
        live_activity: [],
        upcoming_runs: [
          {
            workflow_id: "wf-nightly",
            workflow_name: "Nightly sync",
            environment: "production",
            domain: "ops",
            trigger_kind: "cron",
            trigger_label: "0 8 * * *",
            next_time: "2026-07-12T08:00:00.000Z",
          },
        ],
        recent_runs: [],
      }),
      enqueue_workflow: () => ({
        status: "queued",
        queued_run_id: "queued-popup-1",
      }),
    };

    render(<MenuBarPopup />);

    fireEvent.click(
      await screen.findByRole("button", {
        name: "Queue run Nightly sync",
      }),
    );

    await waitFor(() =>
      expect(
        calls.some(
          ({ command, args }) =>
            command === "enqueue_workflow" &&
            args.id === "wf-nightly" &&
            typeof args.idempotencyKey === "string",
        ),
      ).toBe(true),
    );
    expect(calls.some(({ command }) => command === "run_workflow_now")).toBe(
      false,
    );
    expect(
      await screen.findByText(/Waiting to start: Nightly sync/),
    ).toBeInTheDocument();
  });
});
