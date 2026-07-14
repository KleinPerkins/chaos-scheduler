#!/usr/bin/env node
// Narrow recurrence guard for issue #292: project-local Cursor MCP configs
// must never carry a scheduler API key. Local integration credentials belong
// in the desktop app-managed, user-level ~/.cursor/mcp.json outside Git.
import { execFileSync } from "node:child_process";
import { lstatSync, readFileSync } from "node:fs";
import { dirname, isAbsolute, join, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const FORBIDDEN_PROPERTY = "CHAOS_SCHEDULER_API_KEY";
const AUTHORIZATION_PROPERTY = "authorization";
const ALLOWED_BEARER_PLACEHOLDER = "Bearer REPLACE_WITH_SCOPED_API_KEY";
const MCP_CONFIG_PATHSPEC = ":(glob).cursor/mcp*.json";
const MAX_CONFIG_BYTES = 1024 * 1024;
const root = join(dirname(fileURLToPath(import.meta.url)), "..");

/**
 * Return true when parsed JSON contains a scheduler API-key property or a
 * non-placeholder Authorization value at any depth. Values are inspected only
 * for exact placeholder allowlisting and are never returned.
 */
export function containsProhibitedCredential(value) {
  if (Array.isArray(value)) {
    return value.some((item) => containsProhibitedCredential(item));
  }
  if (value === null || typeof value !== "object") return false;
  for (const [key, item] of Object.entries(value)) {
    if (key === FORBIDDEN_PROPERTY) return true;
    if (
      key.toLowerCase() === AUTHORIZATION_PROPERTY &&
      item !== ALLOWED_BEARER_PLACEHOLDER
    ) {
      return true;
    }
    if (containsProhibitedCredential(item)) return true;
  }
  return false;
}

/**
 * Validate one candidate without echoing its contents or parse diagnostics.
 */
export function validateMcpConfigText(text, displayPath = "<config>") {
  let parsed;
  try {
    parsed = JSON.parse(text);
  } catch {
    return [`${displayPath}: invalid JSON`];
  }
  return containsProhibitedCredential(parsed)
    ? [`${displayPath}: contains prohibited scheduler credential configuration`]
    : [];
}

/**
 * Enumerate only Git-tracked project MCP JSON files, NUL-delimited so unusual
 * filenames cannot split or inject output.
 */
export function trackedMcpConfigEntries(repoRoot) {
  const raw = execFileSync(
    "git",
    ["ls-files", "--stage", "-z", "--", MCP_CONFIG_PATHSPEC],
    {
      cwd: repoRoot,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    },
  );
  return raw
    .split("\0")
    .filter(Boolean)
    .map((record) => {
      const separator = record.indexOf("\t");
      if (separator < 0) throw new Error("invalid git index record");
      const metadata = record.slice(0, separator).split(" ");
      return { mode: metadata[0], path: record.slice(separator + 1) };
    });
}

/**
 * Fail closed on Git errors, unsafe file types, missing/unreadable files,
 * oversized files, malformed JSON, or the prohibited property. Diagnostics
 * identify paths/reasons only and never include file contents.
 */
export function scanTrackedMcpConfigs(repoRoot) {
  let entries;
  try {
    entries = trackedMcpConfigEntries(repoRoot);
  } catch {
    return ["unable to enumerate tracked MCP configuration"];
  }

  const resolvedRoot = resolve(repoRoot);
  const rootPrefix = `${resolvedRoot}${sep}`;
  const violations = [];

  for (const { mode, path: relativePath } of entries) {
    const absolutePath = resolve(resolvedRoot, relativePath);
    if (
      isAbsolute(relativePath) ||
      (!absolutePath.startsWith(rootPrefix) && absolutePath !== resolvedRoot)
    ) {
      violations.push(`${relativePath}: unsafe tracked path`);
      continue;
    }
    if (mode !== "100644" && mode !== "100755") {
      violations.push(`${relativePath}: tracked path is not a regular file`);
      continue;
    }

    let stat;
    try {
      stat = lstatSync(absolutePath);
    } catch {
      violations.push(`${relativePath}: tracked file is missing or unreadable`);
      continue;
    }
    if (!stat.isFile()) {
      violations.push(`${relativePath}: tracked path is not a regular file`);
      continue;
    }
    if (stat.size > MAX_CONFIG_BYTES) {
      violations.push(`${relativePath}: tracked config exceeds size limit`);
      continue;
    }

    let text;
    try {
      text = readFileSync(absolutePath, "utf8");
    } catch {
      violations.push(`${relativePath}: tracked file is unreadable`);
      continue;
    }
    violations.push(...validateMcpConfigText(text, relativePath));

    // Scan the index independently from the working tree. This closes the
    // staged-secret/clean-working-copy gap in local hooks while preserving the
    // same behavior in CI, where index and checkout normally match.
    let stagedText;
    try {
      stagedText = execFileSync("git", ["show", `:${relativePath}`], {
        cwd: resolvedRoot,
        encoding: "utf8",
        maxBuffer: MAX_CONFIG_BYTES + 1,
        stdio: ["ignore", "pipe", "ignore"],
      });
    } catch {
      violations.push(
        `${relativePath}: staged content is unreadable or exceeds size limit`,
      );
      continue;
    }
    if (stagedText !== text) {
      violations.push(
        ...validateMcpConfigText(stagedText, `${relativePath} (staged)`),
      );
    }
  }

  return [...new Set(violations)];
}

function main() {
  const violations = scanTrackedMcpConfigs(root);
  if (violations.length > 0) {
    console.error(
      "::error::Tracked Cursor MCP configuration must be secret-free:",
    );
    for (const violation of violations) console.error(`  - ${violation}`);
    process.exit(1);
  }
  console.log(
    "OK — tracked Cursor MCP examples contain no scheduler credential material.",
  );
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main();
}
