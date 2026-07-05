import type { ActionSpec, StepSpec, WorkflowKind } from "./commands";

export function validateWorkflowSteps(
  kind: WorkflowKind,
  steps: StepSpec[],
): string | null {
  if (kind !== "generic") return null;
  if (steps.length === 0) {
    return "Add at least one step before saving.";
  }
  for (const [index, step] of steps.entries()) {
    const hasCommand = !!step.command?.trim();
    const hasScript = !!step.script?.trim();
    if (!hasCommand && !hasScript) {
      return `Step ${index + 1} needs a command or script path.`;
    }
  }
  return null;
}

export function validateRunWorkflowActions(
  actions: ActionSpec[],
  label: string,
): string | null {
  for (const [index, action] of actions.entries()) {
    if (action.type === "run_workflow" && !action.workflow_id?.trim()) {
      return `${label} action ${index + 1}: select a workflow to run.`;
    }
  }
  return null;
}

/** UI idempotency-key convention for enqueue retries from the desktop app. */
export function buildEnqueueIdempotencyKey(workflowId: string): string {
  return `ui-enqueue:${workflowId}:${crypto.randomUUID()}`;
}
