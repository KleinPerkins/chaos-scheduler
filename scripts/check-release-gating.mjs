#!/usr/bin/env node
// Static guard against a GitHub Actions "skip-cascade" in the release workflow.
//
// GitHub rule: a job that declares `needs:` is itself SKIPPED whenever any of
// its needed jobs is skipped — UNLESS the job's `if:` contains a status
// function (`always()`, `!cancelled()`/`cancelled()`, `success()`, or
// `failure()`). Without one, GitHub prepends an implicit `success()`, and
// `success()` is false the moment an upstream `needs` job is skipped. So a job
// whose WRITTEN `if:` would happily accept a skipped dependency
// (`needs.<job>.result == 'skipped'`) still gets skipped anyway — and that skip
// cascades into every downstream `needs:` job.
//
// That is exactly what broke the last several DESKTOP-ONLY releases: on a
// desktop-only run `publish-sdk`/`publish-mcp` are legitimately skipped, so
// `mcp-consumer-smoke` (gated on `needs.publish-mcp.result`) was cascade-skipped
// even though its `if:` explicitly allows `== 'skipped'`, which then
// cascade-skipped `build-macos` — skipping the entire build/publish/upload
// chain. release.yml already applies the correct pattern in
// `guard-latest-for-package-only-release` (`if: ${{ always() && ... }}`); it was
// simply never applied to the ordering-chain jobs.
//
// This check enforces the invariant, compile-free, on every PR: EVERY job that
// (a) declares `needs:` AND (b) references `needs.*.result` in its `if:` MUST
// also contain a status function in that `if:`. Use `!cancelled()` (not
// `always()`) on the ordering chain so a genuine upstream FAILURE still halts
// the chain while a benign SKIP does not.
//
// Zero external deps on purpose (mirrors scripts/check-tauri-versions.mjs): it
// does targeted structural parsing of the workflow text rather than pulling a
// YAML library, so the guard can never break because a transitive dependency
// moved.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

// The four status functions GitHub recognizes as "suppress the implicit
// success()". `!cancelled()` is just a negated `cancelled()`, so matching the
// bare function name covers it too.
const STATUS_FUNCTION_RE = /\b(?:success|always|cancelled|failure)\s*\(\s*\)/;

// A reference to a sibling job's completion status inside an `if:` expression,
// e.g. `needs.publish-mcp.result`. Job ids may contain hyphens.
const NEEDS_RESULT_RE = /needs\.[A-Za-z0-9_-]+\.result/;

/** True when an `if:` expression contains a GitHub status-check function. */
export function hasStatusFunction(ifExpr) {
  return typeof ifExpr === "string" && STATUS_FUNCTION_RE.test(ifExpr);
}

/** True when an `if:` expression gates on a `needs.<job>.result` value. */
export function referencesNeedsResult(ifExpr) {
  return typeof ifExpr === "string" && NEEDS_RESULT_RE.test(ifExpr);
}

/**
 * Parse a GitHub Actions workflow's `jobs:` map into a flat list of
 * `{ name, ifExpr, hasNeeds }`. Pure (string in, data out) so it is
 * unit-testable without touching disk.
 *
 * Only JOB-LEVEL `if:`/`needs:` (exactly four-space indent) are captured — a
 * step-level `if:` lives deeper (>=8 spaces) and is intentionally ignored.
 *
 * @param {string} yamlText raw workflow YAML
 * @returns {{ name: string, ifExpr: string | null, hasNeeds: boolean }[]}
 */
export function parseWorkflowJobs(yamlText) {
  const lines = String(yamlText).split(/\r?\n/);

  // Anchor to the top-level `jobs:` key so nested maps under `on:` /
  // `permissions:` (e.g. `workflow_call:`) are never mistaken for jobs.
  let i = 0;
  for (; i < lines.length; i++) {
    if (/^jobs:\s*(#.*)?$/.test(lines[i])) break;
  }

  const jobs = [];
  let current = null;
  const flush = () => {
    if (current) jobs.push(current);
  };

  for (i += 1; i < lines.length; i++) {
    const line = lines[i];
    if (line.trim() === "") continue;
    // A non-indented line is a new top-level section — the jobs block is over.
    if (/^\S/.test(line)) break;

    // Job header: exactly two-space indent, an id, a colon, nothing else
    // (a trailing comment is allowed). `  # comment` is not a header.
    const header = /^ {2}([A-Za-z0-9_-]+):\s*(#.*)?$/.exec(line);
    if (header) {
      flush();
      current = { name: header[1], ifExpr: null, hasNeeds: false };
      continue;
    }
    if (!current) continue;

    const ifMatch = /^ {4}if:\s*(.*)$/.exec(line);
    if (ifMatch) {
      current.ifExpr = ifMatch[1].trim();
      continue;
    }
    if (/^ {4}needs:/.test(line)) {
      current.hasNeeds = true;
      continue;
    }
  }
  flush();
  return jobs;
}

/**
 * Return a human-readable violation string for every job that would be
 * silently cascade-skipped: it `needs:` a sibling AND gates on
 * `needs.*.result` but has no status function to suppress the implicit
 * `success()`.
 *
 * @param {string} yamlText raw workflow YAML
 * @returns {string[]} empty when the workflow is safe
 */
export function findGatingViolations(yamlText) {
  const violations = [];
  for (const job of parseWorkflowJobs(yamlText)) {
    if (
      job.hasNeeds &&
      referencesNeedsResult(job.ifExpr) &&
      !hasStatusFunction(job.ifExpr)
    ) {
      violations.push(
        `job "${job.name}" gates on needs.*.result without a status function — ` +
          `it will be cascade-skipped when an upstream needs job is skipped. ` +
          `Prefix its if: with \`!cancelled() &&\`. Current if: ${job.ifExpr}`,
      );
    }
  }
  return violations;
}

function main() {
  const workflow = join(root, ".github", "workflows", "release.yml");
  const text = readFileSync(workflow, "utf8");
  const violations = findGatingViolations(text);

  if (violations.length > 0) {
    console.error(
      "::error::release.yml skip-cascade guard failed. GitHub skips a job whose " +
        "`needs:` dependency was skipped unless the job's `if:` contains a status " +
        "function. Add `!cancelled() &&` to the front of each `if:` below (keep " +
        "the existing `== 'success' || == 'skipped'` guards so a real failure " +
        "still halts the chain):",
    );
    for (const v of violations) console.error(`  - ${v}`);
    process.exit(1);
  }

  console.log(
    "OK — every release.yml job that gates on needs.*.result carries a status " +
      "function, so a skipped upstream job cannot cascade-skip the build chain.",
  );
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main();
}
