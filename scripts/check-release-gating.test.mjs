import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { describe, it, expect } from "vitest";

import {
  findGatingViolations,
  hasStatusFunction,
  parseWorkflowJobs,
  referencesNeedsResult,
} from "./check-release-gating.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const releaseYml = readFileSync(
  join(root, ".github", "workflows", "release.yml"),
  "utf8",
);

// Fixtures are built line-by-line (plain strings, not template literals) so the
// GitHub `${{ ... }}` expression syntax is preserved verbatim.
//
// The DEFECTIVE shape: `publish-mcp` both `needs:` a skippable sibling AND gates
// on `needs.publish-sdk.result`, but its `if:` has NO status function — so on a
// run where `publish-sdk` is skipped, GitHub's implicit `success()` cascade
// skips `publish-mcp` even though its written `if:` allows `== 'skipped'`. This
// is the exact regression that skipped the desktop build chain.
const CASCADE_BUG_FIXTURE = [
  "name: demo",
  "on:",
  "  workflow_call:",
  "    inputs:",
  "      sdk_tag: { type: string, default: '' }",
  "      mcp_tag: { type: string, default: '' }",
  "jobs:",
  "  publish-sdk:",
  "    if: ${{ inputs.sdk_tag != '' }}",
  "    runs-on: ubuntu-latest",
  "    steps:",
  "      - run: echo sdk",
  "  publish-mcp:",
  "    if: ${{ inputs.mcp_tag != '' && (needs.publish-sdk.result == 'success' || needs.publish-sdk.result == 'skipped') }}",
  "    needs: [publish-sdk]",
  "    runs-on: ubuntu-latest",
  "    steps:",
  "      # a step-level if: must NOT be mistaken for the job gate",
  "      - if: ${{ always() }}",
  "        run: echo mcp",
].join("\n");

// The same workflow, fixed by prefixing the job `if:` with `!cancelled() &&`.
const CASCADE_FIXED_FIXTURE = CASCADE_BUG_FIXTURE.replace(
  "    if: ${{ inputs.mcp_tag != ''",
  "    if: ${{ !cancelled() && inputs.mcp_tag != ''",
);

describe("status-function + needs.*.result detection", () => {
  it("recognizes every GitHub status function (incl. !cancelled())", () => {
    expect(hasStatusFunction("${{ !cancelled() && x }}")).toBe(true);
    expect(hasStatusFunction("${{ cancelled() }}")).toBe(true);
    expect(hasStatusFunction("${{ always() && x }}")).toBe(true);
    expect(hasStatusFunction("${{ success() }}")).toBe(true);
    expect(hasStatusFunction("${{ failure() }}")).toBe(true);
  });

  it("does not treat a plain needs/inputs gate as a status function", () => {
    expect(
      hasStatusFunction(
        "${{ inputs.mcp_tag != '' && needs.publish-sdk.result == 'success' }}",
      ),
    ).toBe(false);
    expect(hasStatusFunction("${{ inputs.desktop_tag != '' }}")).toBe(false);
    expect(hasStatusFunction(null)).toBe(false);
  });

  it("detects a needs.<job>.result reference (hyphens allowed)", () => {
    expect(
      referencesNeedsResult("${{ needs.publish-mcp.result == 'x' }}"),
    ).toBe(true);
    expect(referencesNeedsResult("${{ inputs.mcp_tag != '' }}")).toBe(false);
  });
});

describe("parseWorkflowJobs", () => {
  it("captures job-level if:/needs: but ignores step-level if:", () => {
    const jobs = parseWorkflowJobs(CASCADE_BUG_FIXTURE);
    const byName = new Map(jobs.map((j) => [j.name, j]));

    expect([...byName.keys()]).toEqual(["publish-sdk", "publish-mcp"]);
    expect(byName.get("publish-sdk").hasNeeds).toBe(false);

    const mcp = byName.get("publish-mcp");
    expect(mcp.hasNeeds).toBe(true);
    // The captured if: is the JOB gate, not the step-level `if: ${{ always() }}`.
    expect(mcp.ifExpr).toContain("inputs.mcp_tag");
    expect(mcp.ifExpr).not.toContain("always()");
  });
});

describe("findGatingViolations (the biting guard)", () => {
  it("flags a job that gates on needs.*.result without a status function", () => {
    const violations = findGatingViolations(CASCADE_BUG_FIXTURE);
    expect(violations).toHaveLength(1);
    expect(violations[0]).toContain("publish-mcp");
  });

  it("passes once the job if: carries !cancelled()", () => {
    expect(findGatingViolations(CASCADE_FIXED_FIXTURE)).toEqual([]);
  });
});

describe("release.yml", () => {
  // Non-vacuous guard: prove the parser actually sees the real ordering chain
  // and that the very jobs the fix targets DO gate on a sibling's result — so a
  // silently-empty parse can never make the violation assertion pass by
  // accident.
  it("parses the real ordering-chain jobs, which gate on needs.*.result", () => {
    const byName = new Map(
      parseWorkflowJobs(releaseYml).map((j) => [j.name, j]),
    );

    for (const name of [
      "publish-sdk",
      "publish-mcp",
      "mcp-consumer-smoke",
      "build-macos",
      "guard-latest-for-package-only-release",
    ]) {
      expect(byName.has(name), `expected job "${name}"`).toBe(true);
    }

    for (const name of ["publish-mcp", "mcp-consumer-smoke", "build-macos"]) {
      const job = byName.get(name);
      expect(job.hasNeeds, `${name} should declare needs:`).toBe(true);
      expect(
        referencesNeedsResult(job.ifExpr),
        `${name} should gate on needs.*.result`,
      ).toBe(true);
    }
  });

  // RED before the fix (publish-mcp, mcp-consumer-smoke, build-macos violate),
  // GREEN after.
  it("has no skip-cascade gating violations", () => {
    expect(findGatingViolations(releaseYml)).toEqual([]);
  });
});
