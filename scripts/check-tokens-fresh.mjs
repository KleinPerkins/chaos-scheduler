#!/usr/bin/env node
// Freshness gate for the GENERATED design-token artifacts.
//
// tokens/*.json (source of truth) is compiled by Style Dictionary via
// `npm run tokens` into three tracked, do-not-hand-edit artifacts:
//   - src/styles/tokens.css
//   - src/styles/tokens.ts
//   - figma-tokens.json
// This check regenerates them from source and fails if any drift from what is
// committed — i.e. a token source changed but the generated output was not
// re-committed, or a generated file was hand-edited. It never changes token
// values or Style Dictionary semantics; it only asserts the outputs are fresh.
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const npm = process.platform === "win32" ? "npm.cmd" : "npm";

const GENERATED = [
  "src/styles/tokens.css",
  "src/styles/tokens.ts",
  "figma-tokens.json",
];

try {
  console.log("Regenerating design tokens from tokens/*.json ...");
  execFileSync(npm, ["run", "tokens"], { cwd: root, stdio: "inherit" });
} catch {
  console.error("::error::`npm run tokens` failed — token build is broken.");
  process.exit(1);
}

try {
  execFileSync("git", ["diff", "--exit-code", "--", ...GENERATED], {
    cwd: root,
    stdio: "inherit",
  });
} catch {
  console.error(
    "\n::error::Generated design tokens are STALE. A token source under " +
      "tokens/*.json changed but the generated artifacts were not re-committed " +
      "(or a generated file was hand-edited). Run `npm run tokens` and commit " +
      `the updated: ${GENERATED.join(", ")}.`,
  );
  process.exit(1);
}

console.log(
  `\nOK — generated design tokens are committed-fresh (${GENERATED.join(", ")}).`,
);
