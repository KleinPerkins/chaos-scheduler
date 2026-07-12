import { afterEach, describe, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
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
    cleanup();
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
    expect(
      screen.getByRole("button", { name: /Run history/i }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Run history/i }));
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

  it("prioritizes authoritative observability before the structured summary", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_run_log: () => ({
        ...sampleRun,
        stdout: "build complete",
        result_url: "https://example.com/result",
        summary: {
          title: "Synchronization report",
          description: "Only fields emitted by the workflow.",
          sections: [
            {
              title: "Rows processed",
              type: "stats",
              data: { processed: 42 },
            },
          ],
        },
      }),
      get_run_tasks: () => [
        {
          id: "task-record-1",
          run_id: sampleRun.id,
          task_id: "extract",
          status: "succeeded",
          started_at: sampleRun.started_at,
          finished_at: sampleRun.finished_at,
          attempt_number: 1,
        },
      ],
      get_run_attempts: () => [
        {
          id: "attempt-1",
          run_id: sampleRun.id,
          task_id: "extract",
          attempt_number: 1,
          status: "succeeded",
          started_at: sampleRun.started_at,
          finished_at: sampleRun.finished_at,
        },
      ],
      get_run_metrics: () => [
        {
          id: "metric-1",
          run_id: sampleRun.id,
          task_id: "extract",
          metric_name: "rows",
          metric_value: 42,
          metric_unit: "records",
          emitted_at: sampleRun.finished_at,
        },
      ],
      get_run_relationships: () => [
        {
          id: "relationship-1",
          parent_run_id: sampleRun.id,
          child_run_id: "run-child-1",
          child_workflow_id: "wf-child-1",
          child_workflow_name: "Index refresh",
          relationship: "child",
          task_id: "publish",
          wait: true,
          status: "succeeded",
          created_at: sampleRun.started_at,
          updated_at: sampleRun.finished_at,
        },
      ],
    };

    render(<RunDetail runId={sampleRun.id} onBack={() => {}} />);

    expect(
      await screen.findByRole("region", {
        name: `${sampleRun.workflow_name} run detail`,
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /Run history/i }),
    ).toBeInTheDocument();
    expect(screen.getByText(`Run ${sampleRun.id}`)).toBeInTheDocument();
    expect(
      screen.getByRole("table", {
        name: `Attempts for ${sampleRun.workflow_name}`,
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("table", {
        name: `Run metrics for ${sampleRun.workflow_name}`,
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("table", {
        name: `Workflow lineage for ${sampleRun.workflow_name}`,
      }),
    ).toBeInTheDocument();
    const logs = screen.getByRole("heading", { name: "Raw logs" });
    const summary = screen.getByRole("heading", {
      name: "Synchronization report",
    });
    expect(logs.compareDocumentPosition(summary)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
    expect(screen.getByRole("button", { name: "Raw logs" })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    expect(screen.getByRole("tab", { name: "stdout" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(
      screen.getByRole("button", { name: "Open result" }),
    ).toBeInTheDocument();
  });

  it("exposes a completed task's status to assistive tech in the timeline", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_run_log: () => ({ ...sampleRun }),
      get_run_tasks: () => [
        {
          id: "task-record-1",
          run_id: sampleRun.id,
          task_id: "extract",
          status: "succeeded",
          started_at: sampleRun.started_at,
          finished_at: sampleRun.finished_at,
          attempt_number: 1,
        },
      ],
    };

    render(<RunDetail runId={sampleRun.id} onBack={() => {}} />);

    // A completed task renders its DURATION in the bar, so the status is
    // otherwise conveyed only by the color-coded dot. The dot must therefore
    // expose the status as an accessible name (role=img + label) for AT.
    const statusIndicator = await screen.findByRole("img", {
      name: /succeeded/i,
    });
    expect(statusIndicator).toBeInTheDocument();
    expect(statusIndicator).toHaveClass("status-dot", "succeeded");
  });

  it("keeps failure analysis conditional and actionable", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_run_log: () => ({
        ...sampleRun,
        status: "failed",
        exit_code: 1,
        stderr: "connection refused",
      }),
      analyze_run_error: () => ({
        diagnosis: "The upstream service rejected the connection.",
        likely_cause: "The service was unavailable.",
        recommended_steps: ["Check service health.", "Retry the workflow."],
      }),
    };

    render(<RunDetail runId={sampleRun.id} onBack={() => {}} />);

    expect(
      await screen.findByRole("heading", { name: "Failure analysis" }),
    ).toBeInTheDocument();
    fireEvent.click(
      screen.getByRole("button", { name: "Analyze error with AI" }),
    );
    expect(
      await screen.findByText("The upstream service rejected the connection."),
    ).toBeInTheDocument();
    expect(
      screen.getByText("The service was unavailable."),
    ).toBeInTheDocument();
    expect(screen.getByText("Check service health.")).toBeInTheDocument();
  });

  it("returns to run history via the back control from the loaded view", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_run_log: () => ({ ...sampleRun }),
    };
    const onBack = vi.fn();

    render(<RunDetail runId={sampleRun.id} onBack={onBack} />);

    // Wait for the loaded detail region (not the loading/error state) so we are
    // asserting the normal drill-down return path.
    expect(
      await screen.findByRole("region", {
        name: `${sampleRun.workflow_name} run detail`,
      }),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Run history/i }));
    expect(onBack).toHaveBeenCalledTimes(1);
  });
});
