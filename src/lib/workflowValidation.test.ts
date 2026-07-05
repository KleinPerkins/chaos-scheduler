import { describe, expect, it } from "vitest";
import {
  buildEnqueueIdempotencyKey,
  validateRunWorkflowActions,
  validateWorkflowSteps,
} from "./workflowValidation";
import { defaultAction, emptyStep } from "../components/workflow/specHelpers";

describe("validateWorkflowSteps", () => {
  it("rejects empty generic step lists", () => {
    expect(validateWorkflowSteps("generic", [])).toMatch(/at least one step/i);
  });

  it("rejects steps without command or script", () => {
    expect(validateWorkflowSteps("generic", [emptyStep(0)])).toMatch(/step 1/i);
  });

  it("allows typed workflows without generic steps", () => {
    expect(validateWorkflowSteps("typed", [])).toBeNull();
  });
});

describe("validateRunWorkflowActions", () => {
  it("requires workflow_id for run_workflow actions", () => {
    const action = { ...defaultAction("run_workflow"), workflow_id: "" };
    expect(validateRunWorkflowActions([action], "On-success")).toMatch(
      /select a workflow/i,
    );
  });
});

describe("buildEnqueueIdempotencyKey", () => {
  it("uses ui-enqueue prefix", () => {
    expect(buildEnqueueIdempotencyKey("daily")).toMatch(/^ui-enqueue:daily:/);
  });
});
