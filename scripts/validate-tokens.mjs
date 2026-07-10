#!/usr/bin/env node
// Shape / JSON-schema validation for the design-token SOURCES (tokens/*.json).
//
// tokens/*.json is the SOURCE OF TRUTH for the design system; Style Dictionary
// compiles it into the generated artifacts (src/styles/tokens.css|ts,
// figma-tokens.json — freshness of those is guarded separately by
// check-tokens-fresh.mjs). This check does NOT change token values or Style
// Dictionary semantics; it only fails a hand-edit that malforms a source file
// (a leaf missing its `value`, a non-string value, a stray key, an empty
// group, or a whole top-level group renamed/removed from a known file).
//
// The token file format is Style Dictionary "legacy" nesting: every file is a
// tree of GROUPS whose leaves are `{ "value": "<non-empty string>",
// "comment"?: "<string>" }`. New tokens can be added freely — only the shape is
// enforced, never the set of tokens.
import { readFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import Ajv from "ajv";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const tokensDir = join(root, "tokens");

/** Recursive schema: a token file is a group of nodes; a node is a leaf or a
 * nested group. `if/then/else` disambiguates on the presence of `value` so
 * errors point at the real problem instead of a generic `oneOf` failure. */
export const tokenTreeSchema = {
  $id: "https://chaos-scheduler.local/schemas/token-tree.json",
  type: "object",
  minProperties: 1,
  additionalProperties: { $ref: "#/$defs/node" },
  $defs: {
    node: {
      type: "object",
      minProperties: 1,
      if: { type: "object", required: ["value"] },
      then: { $ref: "#/$defs/leaf" },
      else: { $ref: "#/$defs/group" },
    },
    leaf: {
      type: "object",
      required: ["value"],
      additionalProperties: false,
      properties: {
        value: { type: "string", minLength: 1 },
        comment: { type: "string" },
      },
    },
    group: {
      type: "object",
      minProperties: 1,
      additionalProperties: { $ref: "#/$defs/node" },
    },
  },
};

/** Known source files must retain these top-level group paths, so an accidental
 * rename/removal of a whole group is caught (unknown files are shape-only). */
export const REQUIRED_ROOTS = {
  "color.palette.json": [["palette"]],
  "theme.dark.json": [["theme", "dark"]],
  "theme.light.json": [["theme", "light"]],
  "font.json": [["font"]],
  "motion.json": [["duration"], ["ease"]],
  "radius.json": [["radius"]],
  "spacing.json": [["space"]],
  "typography.json": [["font-size"], ["line-height"]],
};

function hasPath(data, path) {
  let cur = data;
  for (const key of path) {
    if (cur == null || typeof cur !== "object" || !(key in cur)) return false;
    cur = cur[key];
  }
  return true;
}

const ajv = new Ajv({ allErrors: true, strict: false });
const validateTree = ajv.compile(tokenTreeSchema);

/**
 * Validate one parsed token file. Pure (no I/O) so it is unit-testable.
 * @returns {string[]} human-readable errors (empty when valid).
 */
export function validateTokenFile(fileName, data) {
  const errors = [];

  if (validateTree(data)) {
    // shape OK
  } else {
    for (const e of validateTree.errors ?? []) {
      const at = e.instancePath || "(root)";
      errors.push(`${fileName}: ${at} ${e.message}`);
    }
  }

  for (const path of REQUIRED_ROOTS[fileName] ?? []) {
    if (!hasPath(data, path)) {
      errors.push(`${fileName}: missing required group "${path.join(".")}"`);
    }
  }

  return errors;
}

function main() {
  const files = readdirSync(tokensDir)
    .filter((f) => f.endsWith(".json"))
    .sort();

  if (files.length === 0) {
    console.error("::error::No tokens/*.json source files found.");
    process.exit(1);
  }

  const allErrors = [];
  for (const file of files) {
    let data;
    try {
      data = JSON.parse(readFileSync(join(tokensDir, file), "utf8"));
    } catch (err) {
      allErrors.push(`${file}: invalid JSON — ${err.message}`);
      continue;
    }
    allErrors.push(...validateTokenFile(file, data));
  }

  if (allErrors.length > 0) {
    console.error(
      "::error::Design-token source validation failed. tokens/*.json is the " +
        "source of truth; fix the malformed source(s) below (do not edit the " +
        "generated tokens.css/tokens.ts/figma-tokens.json):",
    );
    for (const e of allErrors) console.error(`  - ${e}`);
    process.exit(1);
  }

  console.log(
    `OK — ${files.length} token source file(s) are well-formed: ${files.join(", ")}`,
  );
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main();
}
