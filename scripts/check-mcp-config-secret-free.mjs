#!/usr/bin/env node
// Credential guard (issue #292): fail if a TRACKED Cursor MCP config carries a
// real managed credential. The managed scheduler API key must live in the macOS
// Keychain (see SECURITY.md "MCP config and Git history" + ADR 0010), never in a
// tracked `.cursor/**/mcp*.json` blob. `.gitignore` (which now ignores the
// project-local `.cursor/mcp.json`) and the lefthook pre-commit hook are the
// first line of defense, but BOTH are bypassed by the git-data-API single-commit
// PR flow this repo uses, and lefthook is `--no-verify`-bypassable — so this
// CI-required check is the authoritative gate.
//
// Scope: tracked `.cursor/**/mcp*.json` EXCLUDING `*.example.json` (the example
// files legitimately carry placeholders). Scans `git ls-files` when run with no
// args (CI), or exactly the paths passed as args (the lefthook pre-commit hook
// passes the staged files). Detection is content-aware: a file is a violation if
// it carries a non-empty, non-placeholder `CHAOS_SCHEDULER_API_KEY` or a
// `Bearer`/`Authorization` token — checked both via a parsed-JSON walk AND a raw
// regex fallback (so a mangled/unparseable file can't smuggle a live secret
// past the guard). Never prints the offending value.
import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

// A tracked MCP config we must scan: under `.cursor/`, basename matches
// `mcp*.json`, and NOT an `*.example.json` (examples carry placeholders).
export function isScannableMcpConfig(p) {
  const path = String(p).trim().replace(/\\/g, "/");
  if (!path.startsWith(".cursor/")) return false;
  const base = path.slice(path.lastIndexOf("/") + 1);
  if (/\.example\.json$/.test(base)) return false;
  return /^mcp.*\.json$/.test(base);
}

// Obvious, non-secret placeholders that must NOT be treated as a leaked
// credential (case-insensitive). Keeps the guard about REAL token material
// rather than onboarding placeholders. Empty/whitespace is also safe.
const PLACEHOLDER =
  /^(?:$|replace[_-]?with|replace_with_scoped_api_key|your[_-]|<.*>|\{\{.*\}\}|example|placeholder|changeme|xxx+|\.\.\.|test[_-]?key)/i;

function isPlaceholder(value) {
  return PLACEHOLDER.test(String(value).trim());
}

// The credential-bearing part of an `Authorization` header value: the token
// after a leading `Bearer ` scheme, or the whole value if there is no scheme.
function authTokenPart(value) {
  const v = String(value).trim();
  const bearer = /^Bearer\s+(.+)$/i.exec(v);
  return (bearer ? bearer[1] : v).trim();
}

// Managed keys that are NOT secrets — their values are config, not credentials,
// so a real value in these keys must never be flagged (they'd otherwise trip the
// secret-shaped-key heuristic on `*_URL`/marker-adjacent names). Exact match.
const NON_SECRET_MANAGED_KEYS = new Set([
  "CHAOS_SCHEDULER_URL",
  "CHAOS_SCHEDULER_MCP_PROTECTED_ENVIRONMENTS",
  "CHAOS_SCHEDULER_MANAGED_BY",
  "CHAOS_SCHEDULER_MANAGED_ID",
]);

// A key whose NAME implies it carries credential material (api key / token /
// secret / bearer), under ANY prefix — so an alternate name like
// `SCHEDULER_API_KEY` or `SESSION_TOKEN` is caught, not just the exact managed
// key. `authorization` is handled by its own bearer-aware branch.
const SECRETISH_KEY = /(?:^|_)(?:api[_-]?key|token|secret|bearer)$/i;

function isSecretishKey(key) {
  return SECRETISH_KEY.test(String(key));
}

// Recursively walk parsed JSON, collecting a human reason for each violation.
function walk(node, reasons) {
  if (Array.isArray(node)) {
    for (const item of node) walk(item, reasons);
    return;
  }
  if (node && typeof node === "object") {
    for (const [key, value] of Object.entries(node)) {
      const isString = typeof value === "string";
      if (key === "CHAOS_SCHEDULER_API_KEY" && isString) {
        if (value.trim() !== "" && !isPlaceholder(value)) {
          reasons.push("a non-empty CHAOS_SCHEDULER_API_KEY");
        }
      } else if (key.toLowerCase() === "authorization" && isString) {
        const token = authTokenPart(value);
        if (token !== "" && !isPlaceholder(token)) {
          reasons.push("an Authorization/Bearer token");
        }
      } else if (
        isString &&
        isSecretishKey(key) &&
        !NON_SECRET_MANAGED_KEYS.has(key)
      ) {
        // A live value under a secret-shaped key name (e.g. an alternate env
        // name that isn't the exact managed key). The KEY name is safe to print;
        // the value never is.
        if (value.trim() !== "" && !isPlaceholder(value)) {
          reasons.push(`a non-empty secret-shaped field (${key})`);
        }
      }
      walk(value, reasons);
    }
  }
}

// Raw-text scan: catch a live secret the structural walk can miss — a mangled
// (unparseable) file, a `Bearer …` token buried in a free-text field, or a
// secret-shaped key under a name the walk didn't special-case. Runs on EVERY
// file (parseable or not), so parseable JSON can't smuggle a secret past the
// gate via a free-text field.
function rawScan(text, reasons) {
  let m;
  const apiKey = /"CHAOS_SCHEDULER_API_KEY"\s*:\s*"([^"]*)"/g;
  while ((m = apiKey.exec(text)) !== null) {
    if (m[1].trim() !== "" && !isPlaceholder(m[1])) {
      reasons.push("a non-empty CHAOS_SCHEDULER_API_KEY (raw)");
    }
  }
  // Any secret-shaped JSON key ("...API_KEY"/"...TOKEN"/"...SECRET") with a live
  // string value, even when the surrounding JSON is broken.
  const secretishKv =
    /"([A-Za-z0-9_.\-]*(?:api[_-]?key|token|secret))"\s*:\s*"([^"]*)"/gi;
  while ((m = secretishKv.exec(text)) !== null) {
    const key = m[1];
    if (
      !NON_SECRET_MANAGED_KEYS.has(key) &&
      m[2].trim() !== "" &&
      !isPlaceholder(m[2])
    ) {
      reasons.push("a non-empty secret-shaped field (raw)");
    }
  }
  // A `Bearer <token>` anywhere (headers, or a free-text field).
  const bearer = /Bearer\s+([A-Za-z0-9._\-]+)/g;
  while ((m = bearer.exec(text)) !== null) {
    if (!isPlaceholder(m[1]))
      reasons.push("an Authorization/Bearer token (raw)");
  }
}

// Returns the deduped list of violation reasons for a single file's contents.
// ALWAYS runs the raw scan in addition to the structural walk (Finding 4): a
// parseable file can still hide a live secret under an alternate key name or in
// a free-text field, which the structural walk alone would miss.
export function findSecretsInConfig(text) {
  const reasons = [];
  let parsed;
  try {
    parsed = JSON.parse(text);
  } catch {
    parsed = undefined;
  }
  if (parsed !== undefined) {
    walk(parsed, reasons);
  }
  rawScan(text, reasons);
  return [...new Set(reasons)];
}

// Returns [{ path, reasons }] for every scannable config that carries a secret.
export function findViolations(
  paths,
  read = (p) => readFileSync(join(root, p), "utf8"),
) {
  const hits = [];
  for (const raw of paths) {
    const p = String(raw).trim();
    if (!p || !isScannableMcpConfig(p)) continue;
    let text;
    try {
      text = read(p);
    } catch {
      // A path we can't read (e.g. deleted-but-staged) carries no secret we can
      // see; the tracked-tree CI scan is the authoritative pass.
      continue;
    }
    const reasons = findSecretsInConfig(text);
    if (reasons.length > 0) hits.push({ path: p, reasons });
  }
  return hits;
}

function trackedFiles() {
  const out = execFileSync("git", ["ls-files", "-z"], {
    cwd: root,
    encoding: "utf8",
  });
  return out.split("\0").filter(Boolean);
}

function main() {
  const args = process.argv.slice(2);
  const paths = args.length > 0 ? args : trackedFiles();
  const hits = findViolations(paths);
  const scanned = paths.filter(isScannableMcpConfig).length;
  if (hits.length > 0) {
    console.error(
      "::error::mcp-config-secret-free — a tracked Cursor MCP config carries a managed credential:",
    );
    for (const h of hits)
      console.error(`  ${h.path}  — ${h.reasons.join(", ")}`);
    console.error(
      "\nThe managed scheduler key must live in the macOS Keychain (see ADR 0010 / SECURITY.md), " +
        "never in a tracked blob. Move the value to the Keychain and keep the project-local " +
        "`.cursor/mcp.json` untracked (it is git-ignored); example files carry placeholders only.",
    );
    process.exit(1);
  }
  console.log(
    `mcp-config-secret-free OK — scanned ${scanned} tracked MCP config(s), no managed credentials.`,
  );
}

// Run as a CLI only when invoked directly, so the test can import the helpers.
if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main();
}
