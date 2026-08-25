#!/usr/bin/env node
// Lint gate: fail if any tracked src/**/*.{css,tsx,ts} file contains a raw hex color.
// Raw hex = a # followed by 3, 4, 6, or 8 hex digits (case-insensitive).
// Exempt: CSS/JS comments, HTML entities (&# prefix), generated token files,
//         and paths listed in .hex-lint-allow.
//
// Invoked from CI (no args — scans git ls-files) or lefthook pre-commit (file paths as args).
// Mirrors the pattern established by check-no-committed-data.mjs.
import { readFileSync, existsSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join, relative } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

// Generated token files are the source of hex values — never hand-edited, always
// committed as output of `npm run tokens`. Exempt by default.
const GENERATED_EXEMPT = new Set([
  "src/styles/tokens.css",
  "src/styles/tokens.ts",
]);

/** Load .hex-lint-allow (repo-relative paths, one per line) if present. */
function loadAllowlist() {
  const f = join(root, ".hex-lint-allow");
  if (!existsSync(f)) return new Set();
  return new Set(
    readFileSync(f, "utf8")
      .split("\n")
      .map((l) => l.trim())
      .filter((l) => l && !l.startsWith("#")),
  );
}

// Matches a raw hex color: # followed by 3/4/6/8 hex digits at a word boundary.
// Negative lookbehind (?<!&) excludes HTML numeric entities such as &#123; / &#125;.
const HEX_RE = /(?<!&)#(?:[0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})\b/;

/** Strip block (/* … *\/) and line (//) comments before scanning. */
function stripComments(src) {
  src = src.replace(/\/\*[\s\S]*?\*\//g, "");
  src = src.replace(/\/\/.*/g, "");
  return src;
}

/** Throw with a descriptive message if any passed file contains a raw hex color. */
export function checkFiles(files) {
  const allow = loadAllowlist();
  const hits = [];
  for (const f of files) {
    if (!existsSync(f)) continue;
    const ext = f.split(".").pop() ?? "";
    if (!["css", "tsx", "ts"].includes(ext)) continue;
    const rel = relative(root, f);
    if (GENERATED_EXEMPT.has(rel) || allow.has(rel)) continue;
    const src = stripComments(readFileSync(f, "utf8"));
    const lines = src.split("\n");
    for (let i = 0; i < lines.length; i++) {
      if (HEX_RE.test(lines[i])) {
        hits.push({ file: rel, line: i + 1, text: lines[i].trim() });
      }
    }
  }
  if (hits.length > 0) {
    const list = hits.map((h) => `  ${h.file}:${h.line}  ${h.text}`).join("\n");
    throw new Error(
      `raw hex color(s) found — use a cs.* token var instead:\n${list}`,
    );
  }
}

// CLI entry point: no args = scan git ls-files; args = scan those paths.
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  let files;
  if (process.argv.length > 2) {
    files = process.argv.slice(2);
  } else {
    const out = execFileSync(
      "git",
      [
        "ls-files",
        "--",
        "src/*.css",
        "src/**/*.css",
        "src/*.tsx",
        "src/**/*.tsx",
        "src/*.ts",
        "src/**/*.ts",
      ],
      { cwd: root, encoding: "utf8" },
    );
    files = out
      .trim()
      .split("\n")
      .filter(Boolean)
      .map((p) => join(root, p));
  }
  try {
    checkFiles(files);
    console.log("check-no-raw-hex: OK — no raw hex colors found in src/");
    process.exit(0);
  } catch (err) {
    console.error(`\n✗ check-no-raw-hex: ${err.message}\n`);
    process.exit(1);
  }
}
