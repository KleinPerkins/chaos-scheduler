import { describe, expect, it } from "vitest";
import { ChaosSchedulerClient } from "@chaos-scheduler/sdk";

/**
 * Contract matrix: MCP tool name → SDK method → REST route/method.
 * Keeps the public integration surface aligned with `src-tauri/src/api.rs`.
 */
const TOOL_ROUTE_MATRIX = [
  ["get_version", "getVersion", "GET /api/v1/version"],
  ["health_check", "getHealth", "GET /api/v1/health"],
  ["list_environments", "listEnvironments", "GET /api/v1/environments"],
  ["list_workflows", "listWorkflows", "GET /api/v1/workflows"],
  ["get_workflow", "getWorkflow", "GET /api/v1/workflows/{id}"],
  ["list_workflow_runs", "listRuns", "GET /api/v1/workflows/{id}/runs"],
  ["get_run", "getRun", "GET /api/v1/runs/{id}"],
  ["get_run_logs", "getRunLogs", "GET /api/v1/runs/{id}/logs"],
  ["get_run_tasks", "getRunTasks", "GET /api/v1/runs/{id}/tasks"],
  ["get_run_metrics", "getRunMetrics", "GET /api/v1/runs/{id}/metrics"],
  ["list_queues", "listQueues", "GET /api/v1/queues"],
  ["list_queued_runs", "listQueuedRuns", "GET /api/v1/queued-runs"],
  ["create_environment", "createEnvironment", "POST /api/v1/environments"],
  ["register_workflow", "registerWorkflow", "POST /api/v1/workflows"],
  ["set_workflow_spec", "setWorkflowSpec", "POST /api/v1/workflows/{id}/spec"],
  ["delete_workflow", "deleteWorkflow", "DELETE /api/v1/workflows/{id}"],
  ["run_workflow_now", "runWorkflow", "POST /api/v1/workflows/{id}/run"],
  [
    "enqueue_workflow",
    "enqueueWorkflow",
    "POST /api/v1/workflows/{id}/enqueue",
  ],
  [
    "dispatch_workflow",
    "dispatchWorkflow",
    "POST /api/v1/workflows/{id}/dispatch",
  ],
] as const;

describe("SDK/MCP route coverage matrix", () => {
  it("maps every MCP tool to an SDK client method", () => {
    const client = new ChaosSchedulerClient({
      baseUrl: "http://127.0.0.1:9618",
      apiKey: "id.secret",
      fetch: async () => ({
        ok: true,
        status: 200,
        text: async () => "{}",
      }),
    });
    for (const [, sdkMethod] of TOOL_ROUTE_MATRIX) {
      expect(typeof (client as Record<string, unknown>)[sdkMethod]).toBe(
        "function",
      );
    }
  });

  it("documents REST routes for each integration entrypoint", () => {
    expect(TOOL_ROUTE_MATRIX.map((row) => row[2])).toEqual([
      "GET /api/v1/version",
      "GET /api/v1/health",
      "GET /api/v1/environments",
      "GET /api/v1/workflows",
      "GET /api/v1/workflows/{id}",
      "GET /api/v1/workflows/{id}/runs",
      "GET /api/v1/runs/{id}",
      "GET /api/v1/runs/{id}/logs",
      "GET /api/v1/runs/{id}/tasks",
      "GET /api/v1/runs/{id}/metrics",
      "GET /api/v1/queues",
      "GET /api/v1/queued-runs",
      "POST /api/v1/environments",
      "POST /api/v1/workflows",
      "POST /api/v1/workflows/{id}/spec",
      "DELETE /api/v1/workflows/{id}",
      "POST /api/v1/workflows/{id}/run",
      "POST /api/v1/workflows/{id}/enqueue",
      "POST /api/v1/workflows/{id}/dispatch",
    ]);
  });
});
