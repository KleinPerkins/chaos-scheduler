import { describe, expect, it } from "vitest";
import { defaultAction, defaultTypedSpec } from "./specHelpers";

describe("specHelpers", () => {
  it("default run_workflow action starts with empty workflow_id", () => {
    const action = defaultAction("run_workflow");
    expect(action).toEqual({
      type: "run_workflow",
      workflow_id: "",
      wait: false,
    });
  });

  it("default typed spec uses git_pull operator", () => {
    const spec = defaultTypedSpec();
    expect(spec.operator_type).toBe("git_pull");
    expect(spec.config).toMatchObject({
      repo_url: "",
      branch: "main",
    });
  });
});
