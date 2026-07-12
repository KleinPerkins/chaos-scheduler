#!/usr/bin/env node
// Static guard: the release-please "Release PR" must stay OUT of the app
// auto-merge bot, so cutting a release is a deliberate human merge.
//
// `app-auto-merge.yml` auto-approves + squash-auto-merges every non-draft,
// same-repo PR via the chaos-scheduler-automerge GitHub App. release-please
// maintains one standing "chore: release main" PR (head branch
// `release-please--branches--main`) whose merge creates the tag(s) + GitHub
// Release(s) and kicks off the signed build/sign/publish `release.yml`. That
// must NOT happen automatically — so the `auto-merge` job's `if:` excludes any
// PR whose head branch starts with `release-please--` (robust to the
// target-branch suffix, so future targets are covered too).
//
// This check enforces that invariant, compile-free — a targeted structural
// parse rather than a YAML dependency, mirroring scripts/check-release-gating.mjs
// and scripts/check-tauri-versions.mjs — so the guard can never silently
// regress if someone tidies the workflow condition.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

// The negated head-branch-prefix guard that keeps the release train out of the
// auto-merge bot. Tolerant of whitespace and quote style so a reformat (e.g.
// prettier folding the scalar differently) does not defeat it.
const RELEASE_TRAIN_GUARD_RE =
  /!\s*startsWith\(\s*github\.event\.pull_request\.head\.ref\s*,\s*['"]release-please--['"]\s*\)/;

/** Collapse every run of whitespace so a folded (`>-`) scalar compares cleanly. */
function normalize(expr) {
  return String(expr).replace(/\s+/g, " ").trim();
}

/**
 * Return the `if:` expression of a named job as one normalized string,
 * transparently unfolding a block scalar (`>-`, `>`, `|`, `|-`). Returns null
 * when the job or its `if:` is absent. Pure (string in, string|null out) so it
 * is unit-testable without touching disk.
 *
 * @param {string} yamlText raw workflow YAML
 * @param {string} jobName the top-level job id to read
 * @returns {string | null}
 */
export function extractJobIf(yamlText, jobName) {
  const lines = String(yamlText).split(/\r?\n/);

  // Anchor to the top-level `jobs:` key so nested maps under `on:` /
  // `permissions:` are never mistaken for a job.
  let i = 0;
  for (; i < lines.length; i++) {
    if (/^jobs:\s*(#.*)?$/.test(lines[i])) break;
  }

  // Walk to the requested job header (exactly two-space indent).
  const header = new RegExp(`^ {2}${jobName}:\\s*(#.*)?$`);
  for (i += 1; i < lines.length; i++) {
    if (/^\S/.test(lines[i]) && lines[i].trim() !== "") return null; // jobs block ended
    if (header.test(lines[i])) break;
  }
  if (i >= lines.length) return null;

  // Scan the job body for its four-space-indent `if:` key.
  for (i += 1; i < lines.length; i++) {
    const line = lines[i];
    if (line.trim() === "") continue;
    if (/^\S/.test(line)) return null; // left the jobs block, no if:
    if (/^ {2}[A-Za-z0-9_-]+:/.test(line)) return null; // next job, no if:

    const m = /^ {4}if:\s*(.*)$/.exec(line);
    if (!m) continue;

    const inline = m[1].trim();
    // Inline `if:` (not a block-scalar indicator like `>-`).
    if (inline !== "" && !/^[|>][+-]?$/.test(inline)) {
      return normalize(inline);
    }
    // Block scalar: gather the deeper-indented body (>= six spaces).
    const body = [];
    for (let j = i + 1; j < lines.length; j++) {
      if (lines[j].trim() === "") continue;
      if (!/^ {6,}\S/.test(lines[j])) break;
      body.push(lines[j].trim());
    }
    return normalize(body.join(" "));
  }
  return null;
}

/** True when an `if:` expression excludes the release-please head branch. */
export function excludesReleaseTrain(ifExpr) {
  return typeof ifExpr === "string" && RELEASE_TRAIN_GUARD_RE.test(ifExpr);
}

/**
 * Return a human-readable violation when the `auto-merge` job does not exclude
 * the release-please train. Empty array = the guard is in place.
 *
 * @param {string} yamlText raw app-auto-merge.yml
 * @returns {string[]}
 */
export function findAutoMergeGuardViolations(yamlText) {
  const ifExpr = extractJobIf(yamlText, "auto-merge");
  if (ifExpr === null) {
    return [
      "could not find an `auto-merge` job with an `if:` condition in " +
        "app-auto-merge.yml",
    ];
  }
  if (!excludesReleaseTrain(ifExpr)) {
    return [
      "the `auto-merge` job's if: does not exclude the release-please train — " +
        "append `&& !startsWith(github.event.pull_request.head.ref, " +
        "'release-please--')` so the Release PR stays a deliberate human merge. " +
        `Current if: ${ifExpr}`,
    ];
  }
  return [];
}

function main() {
  const workflow = join(root, ".github", "workflows", "app-auto-merge.yml");
  const text = readFileSync(workflow, "utf8");
  const violations = findAutoMergeGuardViolations(text);

  if (violations.length > 0) {
    console.error(
      "::error::app-auto-merge.yml release-train guard failed. The release-please " +
        "Release PR must be a deliberate human merge, not auto-merged:",
    );
    for (const v of violations) console.error(`  - ${v}`);
    process.exit(1);
  }

  console.log(
    "OK — the auto-merge bot excludes the release-please train " +
      "(release-please--*), so cutting a release stays a manual merge.",
  );
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main();
}
