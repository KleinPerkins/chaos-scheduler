import { enqueueWorkflow } from "./commands";
import type { DispatchOutcome } from "./commands";
import { buildEnqueueIdempotencyKey } from "./workflowValidation";

const pendingRequestKeys = new Map<string, string>();

function shortId(outcome: DispatchOutcome): string | null {
  const id = outcome.queued_run_id ?? outcome.run_id;
  return id ? `${id.slice(0, 8)}…` : null;
}

/**
 * Submit one desktop-UI queue request. An error retains the request key because
 * transport failures are ambiguous: retrying must ask about the same logical
 * request instead of creating a duplicate. Any authoritative outcome consumes
 * the key so a later explicit click represents a new request.
 */
export async function queueWorkflowRun(
  workflowId: string,
): Promise<DispatchOutcome> {
  const key =
    pendingRequestKeys.get(workflowId) ??
    buildEnqueueIdempotencyKey(workflowId);
  pendingRequestKeys.set(workflowId, key);

  const outcome = await enqueueWorkflow(workflowId, key);
  pendingRequestKeys.delete(workflowId);
  return outcome;
}

export function formatWorkflowQueueOutcome(
  workflowName: string,
  outcome: DispatchOutcome,
): string {
  const identity = shortId(outcome);
  const suffix = identity ? ` (${identity})` : "";

  switch (outcome.status) {
    case "queued":
      return `Waiting to start: ${workflowName}${suffix}.`;
    case "admitted":
      return `Started: ${workflowName}${suffix}.`;
    case "duplicate":
      return `Already accepted: ${workflowName}${suffix}.`;
    case "skipped":
      return `Not queued: ${workflowName}${outcome.reason ? ` — ${outcome.reason}` : ""}.`;
    default:
      return `Queue request for ${workflowName}: ${outcome.status}${suffix}.`;
  }
}

export function formatWorkflowQueueError(
  workflowName: string,
  error: unknown,
): string {
  return `Could not confirm whether ${workflowName} was queued: ${String(error)}. Retry Queue run to safely check the same request.`;
}

/** Test isolation for the module-level retry-key cache. */
export function resetWorkflowQueueRequests(): void {
  pendingRequestKeys.clear();
}
