import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { describe, it, expect } from "vitest";

import {
  excludesReleaseTrain,
  extractJobIf,
  findAutoMergeGuardViolations,
} from "./check-auto-merge-guard.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const autoMergeYml = readFileSync(
  join(root, ".github", "workflows", "app-auto-merge.yml"),
  "utf8",
);

// Fixtures are built line-by-line (plain strings) so the folded `>-` scalar and
// the exact `if:` expression are preserved verbatim.
//
// UNGUARDED: the auto-merge job accepts EVERY non-draft same-repo PR — including
// the release-please Release PR — so a release would be cut automatically.
const UNGUARDED_FIXTURE = [
  "name: App auto-merge",
  "on:",
  "  pull_request:",
  "    types: [opened, synchronize, reopened, ready_for_review]",
  "permissions:",
  "  contents: read",
  "jobs:",
  "  auto-merge:",
  "    if: >-",
  "      github.event.pull_request.draft == false &&",
  "      github.repository == 'KleinPerkins/chaos-scheduler' &&",
  "      github.event.pull_request.head.repo.full_name == github.repository",
  "    runs-on: ubuntu-latest",
  "    steps:",
  "      - run: echo merge",
].join("\n");

// GUARDED: the same job with the release-train exclusion appended.
const GUARDED_FIXTURE = UNGUARDED_FIXTURE.replace(
  "      github.event.pull_request.head.repo.full_name == github.repository",
  "      github.event.pull_request.head.repo.full_name == github.repository &&\n" +
    "      !startsWith(github.event.pull_request.head.ref, 'release-please--')",
);

describe("extractJobIf", () => {
  it("unfolds a job's folded (>-) if: into one normalized expression", () => {
    const ifExpr = extractJobIf(UNGUARDED_FIXTURE, "auto-merge");
    expect(ifExpr).toContain("github.event.pull_request.draft == false");
    expect(ifExpr).toContain(
      "github.event.pull_request.head.repo.full_name == github.repository",
    );
    // The fold is collapsed — no newlines survive.
    expect(ifExpr).not.toMatch(/\n/);
  });

  it("returns null for an absent job", () => {
    expect(extractJobIf(UNGUARDED_FIXTURE, "does-not-exist")).toBe(null);
  });
});

describe("excludesReleaseTrain", () => {
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
});

describe("findAutoMergeGuardViolations (the biting guard)", () => {
  it("flags an auto-merge job that does not exclude the release train", () => {
    const violations = findAutoMergeGuardViolations(UNGUARDED_FIXTURE);
    expect(violations).toHaveLength(1);
    expect(violations[0]).toContain("auto-merge");
  });

  it("passes once the release-train exclusion is present", () => {
    expect(findAutoMergeGuardViolations(GUARDED_FIXTURE)).toEqual([]);
  });
});

describe("app-auto-merge.yml", () => {
  // Non-vacuous guard: prove the parser actually sees the real auto-merge job's
  // standard gate, so a silently-empty parse can never make the exclusion
  // assertion below pass by accident.
  it("parses the real auto-merge job's same-repo/non-draft gate", () => {
    const ifExpr = extractJobIf(autoMergeYml, "auto-merge");
    expect(ifExpr).not.toBe(null);
    expect(ifExpr).toContain("github.event.pull_request.draft == false");
    expect(ifExpr).toContain(
      "github.event.pull_request.head.repo.full_name == github.repository",
    );
  });

  // RED before the app-auto-merge.yml guard clause is added, GREEN after.
  it("excludes the release-please Release PR from the auto-merge bot", () => {
    expect(findAutoMergeGuardViolations(autoMergeYml)).toEqual([]);
  });
});
