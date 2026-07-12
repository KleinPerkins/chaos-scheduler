import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { describe, it, expect } from "vitest";

import {
  excludesReleaseTrain,
  excludesReleaseTrainWorkflowRun,
  extractJobIf,
  findAutoMergeGuardViolations,
} from "./check-auto-merge-guard.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const autoMergeYml = readFileSync(
  join(root, ".github", "workflows", "app-auto-merge.yml"),
  "utf8",
);

// Fixtures are built line-by-line (plain strings) so the folded `>-` scalars and
// the exact `if:` expressions are preserved verbatim.
//
// app-auto-merge.yml has TWO auto-merge paths, and the release-please Release PR
// must be excluded from BOTH:
//   - `auto-merge`    (pull_request) → github.event.pull_request.head.ref
//   - `auto-merge-ci` (workflow_run) → github.event.workflow_run.head_branch
//
// UNGUARDED: neither job excludes the release-please Release PR, so a release
// could be cut automatically from either path.
const UNGUARDED_FIXTURE = [
  "name: App auto-merge",
  "on:",
  "  pull_request:",
  "    types: [opened, synchronize, reopened, ready_for_review]",
  "  workflow_run:",
  '    workflows: ["CI"]',
  "    types: [completed]",
  "permissions:",
  "  contents: read",
  "jobs:",
  "  auto-merge:",
  "    if: >-",
  "      github.event_name == 'pull_request' &&",
  "      github.actor != 'dependabot[bot]' &&",
  "      github.event.pull_request.draft == false &&",
  "      github.repository == 'KleinPerkins/chaos-scheduler' &&",
  "      github.event.pull_request.head.repo.full_name == github.repository",
  "    runs-on: ubuntu-latest",
  "    steps:",
  "      - run: echo merge",
  "  auto-merge-ci:",
  "    if: >-",
  "      github.event_name == 'workflow_run' &&",
  "      github.repository == 'KleinPerkins/chaos-scheduler' &&",
  "      github.event.workflow_run.event == 'pull_request' &&",
  "      github.event.workflow_run.conclusion == 'success' &&",
  "      github.event.workflow_run.head_repository.full_name == github.repository &&",
  "      startsWith(github.event.workflow_run.head_branch, 'dependabot/')",
  "    runs-on: ubuntu-latest",
  "    steps:",
  "      - run: echo merge",
].join("\n");

// The last non-guard `if:` line of each job (the anchor we append the exclusion
// after) and the same line WITH its path's release-train exclusion appended.
const PR_ANCHOR =
  "      github.event.pull_request.head.repo.full_name == github.repository";
const PR_ANCHOR_GUARDED =
  PR_ANCHOR +
  " &&\n" +
  "      !startsWith(github.event.pull_request.head.ref, 'release-please--')";
const RUN_ANCHOR =
  "      startsWith(github.event.workflow_run.head_branch, 'dependabot/')";
const RUN_ANCHOR_GUARDED =
  RUN_ANCHOR +
  " &&\n" +
  "      !startsWith(github.event.workflow_run.head_branch, 'release-please--')";

// GUARDED_PR_ONLY: only the pull_request path excludes the release train.
const GUARDED_PR_ONLY_FIXTURE = UNGUARDED_FIXTURE.replace(
  PR_ANCHOR,
  PR_ANCHOR_GUARDED,
);
// GUARDED_RUN_ONLY: only the workflow_run path excludes the release train.
const GUARDED_RUN_ONLY_FIXTURE = UNGUARDED_FIXTURE.replace(
  RUN_ANCHOR,
  RUN_ANCHOR_GUARDED,
);
// GUARDED: both paths exclude the release train (the shipped shape).
const GUARDED_FIXTURE = GUARDED_PR_ONLY_FIXTURE.replace(
  RUN_ANCHOR,
  RUN_ANCHOR_GUARDED,
);

describe("extractJobIf", () => {
  it("unfolds the auto-merge (pull_request) job's folded (>-) if:", () => {
    const ifExpr = extractJobIf(UNGUARDED_FIXTURE, "auto-merge");
    expect(ifExpr).toContain("github.event.pull_request.draft == false");
    expect(ifExpr).toContain(
      "github.event.pull_request.head.repo.full_name == github.repository",
    );
    // The fold is collapsed — no newlines survive.
    expect(ifExpr).not.toMatch(/\n/);
  });

  it("unfolds the auto-merge-ci (workflow_run) job's folded (>-) if:", () => {
    const ifExpr = extractJobIf(UNGUARDED_FIXTURE, "auto-merge-ci");
    expect(ifExpr).toContain("github.event_name == 'workflow_run'");
    expect(ifExpr).toContain(
      "github.event.workflow_run.conclusion == 'success'",
    );
    expect(ifExpr).not.toMatch(/\n/);
  });

  it("returns null for an absent job", () => {
    expect(extractJobIf(UNGUARDED_FIXTURE, "does-not-exist")).toBe(null);
  });
});

describe("excludesReleaseTrain (pull_request path)", () => {
  it("is false without the guard and true with it (whitespace/quote tolerant)", () => {
    expect(
      excludesReleaseTrain(extractJobIf(UNGUARDED_FIXTURE, "auto-merge")),
    ).toBe(false);
    expect(
      excludesReleaseTrain(extractJobIf(GUARDED_FIXTURE, "auto-merge")),
    ).toBe(true);
    // Extra whitespace and double quotes still match.
    expect(
      excludesReleaseTrain(
        '!startsWith( github.event.pull_request.head.ref , "release-please--" )',
      ),
    ).toBe(true);
    expect(excludesReleaseTrain(null)).toBe(false);
  });

  it("does NOT accept the workflow_run head_branch form", () => {
    expect(
      excludesReleaseTrain(
        "!startsWith(github.event.workflow_run.head_branch, 'release-please--')",
      ),
    ).toBe(false);
  });
});

describe("excludesReleaseTrainWorkflowRun (workflow_run path)", () => {
  it("is false without the guard and true with it (whitespace/quote tolerant)", () => {
    expect(
      excludesReleaseTrainWorkflowRun(
        extractJobIf(UNGUARDED_FIXTURE, "auto-merge-ci"),
      ),
    ).toBe(false);
    expect(
      excludesReleaseTrainWorkflowRun(
        extractJobIf(GUARDED_FIXTURE, "auto-merge-ci"),
      ),
    ).toBe(true);
    expect(
      excludesReleaseTrainWorkflowRun(
        '!startsWith( github.event.workflow_run.head_branch , "release-please--" )',
      ),
    ).toBe(true);
    expect(excludesReleaseTrainWorkflowRun(null)).toBe(false);
  });

  it("does NOT accept the pull_request head.ref form", () => {
    expect(
      excludesReleaseTrainWorkflowRun(
        "!startsWith(github.event.pull_request.head.ref, 'release-please--')",
      ),
    ).toBe(false);
  });
});

describe("findAutoMergeGuardViolations (the biting guard — BOTH paths)", () => {
  it("flags both jobs when neither excludes the release train", () => {
    const violations = findAutoMergeGuardViolations(UNGUARDED_FIXTURE);
    expect(violations).toHaveLength(2);
    expect(violations.some((v) => v.includes("(pull_request) job"))).toBe(true);
    expect(violations.some((v) => v.includes("(workflow_run) job"))).toBe(true);
  });

  it("still flags the workflow_run path when only pull_request is guarded", () => {
    const violations = findAutoMergeGuardViolations(GUARDED_PR_ONLY_FIXTURE);
    expect(violations).toHaveLength(1);
    expect(violations[0]).toContain("(workflow_run) job");
  });

  it("still flags the pull_request path when only workflow_run is guarded", () => {
    const violations = findAutoMergeGuardViolations(GUARDED_RUN_ONLY_FIXTURE);
    expect(violations).toHaveLength(1);
    expect(violations[0]).toContain("(pull_request) job");
  });

  it("passes once BOTH paths exclude the release train", () => {
    expect(findAutoMergeGuardViolations(GUARDED_FIXTURE)).toEqual([]);
  });
});

describe("app-auto-merge.yml (the real workflow)", () => {
  // Non-vacuous guard: prove the parser actually sees each real job's standard
  // gate, so a silently-empty parse can never make the exclusion assertion below
  // pass by accident.
  it("parses the real auto-merge (pull_request) job's same-repo/non-draft gate", () => {
    const ifExpr = extractJobIf(autoMergeYml, "auto-merge");
    expect(ifExpr).not.toBe(null);
    expect(ifExpr).toContain("github.event.pull_request.draft == false");
    expect(ifExpr).toContain(
      "github.event.pull_request.head.repo.full_name == github.repository",
    );
  });

  it("parses the real auto-merge-ci (workflow_run) job's CI-success/same-repo gate", () => {
    const ifExpr = extractJobIf(autoMergeYml, "auto-merge-ci");
    expect(ifExpr).not.toBe(null);
    expect(ifExpr).toContain(
      "github.event.workflow_run.conclusion == 'success'",
    );
    expect(ifExpr).toContain(
      "github.event.workflow_run.head_repository.full_name == github.repository",
    );
  });

  // RED if the release-please exclusion is removed from EITHER path, GREEN when
  // both hold.
  it("excludes the release-please Release PR from BOTH auto-merge paths", () => {
    expect(findAutoMergeGuardViolations(autoMergeYml)).toEqual([]);
  });
});
