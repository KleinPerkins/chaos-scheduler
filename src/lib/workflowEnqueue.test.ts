import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { enqueueWorkflow } from "./commands";
import {
  formatWorkflowQueueError,
  formatWorkflowQueueOutcome,
  queueWorkflowRun,
  resetWorkflowQueueRequests,
} from "./workflowEnqueue";
import type { DispatchOutcome } from "./commands";

vi.mock("./commands", () => ({
  enqueueWorkflow: vi.fn(),
}));

const queuedOutcome: DispatchOutcome = {
  workflow_id: "workflow-1",
  status: "queued",
  queued_run_id: "queued-run-123456",
  queue_name: "default",
};

describe("workflow queue contract", () => {
  beforeEach(() => {
    resetWorkflowQueueRequests();
    vi.mocked(enqueueWorkflow).mockReset();
  });

  afterEach(() => {
    resetWorkflowQueueRequests();
  });

  it("reuses a logical request key after an ambiguous error", async () => {
    vi.mocked(enqueueWorkflow)
      .mockRejectedValueOnce(new Error("connection reset"))
      .mockResolvedValue(queuedOutcome);

    await expect(queueWorkflowRun("workflow-1")).rejects.toThrow(
      "connection reset",
    );
    const firstKey = vi.mocked(enqueueWorkflow).mock.calls[0]?.[1];

    await queueWorkflowRun("workflow-1");
    const retryKey = vi.mocked(enqueueWorkflow).mock.calls[1]?.[1];
    expect(retryKey).toBe(firstKey);

    await queueWorkflowRun("workflow-1");
    const nextRequestKey = vi.mocked(enqueueWorkflow).mock.calls[2]?.[1];
    expect(nextRequestKey).not.toBe(firstKey);
  });

  it("uses queued-run identity and outcome-specific language", () => {
    expect(formatWorkflowQueueOutcome("Nightly sync", queuedOutcome)).toBe(
      "Waiting to start: Nightly sync (queued-r…).",
    );
    expect(
      formatWorkflowQueueOutcome("Nightly sync", {
        ...queuedOutcome,
        status: "admitted",
        queued_run_id: null,
        run_id: "run-admitted-123",
      }),
    ).toBe("Started: Nightly sync (run-admi…).");
    expect(
      formatWorkflowQueueOutcome("Nightly sync", {
        ...queuedOutcome,
        status: "skipped",
        reason: "queue full",
      }),
    ).toBe("Not queued: Nightly sync — queue full.");
  });

  it("explains that transport-error retries are safe", () => {
    expect(
      formatWorkflowQueueError("Nightly sync", new Error("offline")),
    ).toContain("Retry Queue run to safely check the same request.");
  });
});
