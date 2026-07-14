#!/usr/bin/env node
// Guard the narrow presentation prerequisite for issue #292. This deliberately
// verifies both GitHub's documented default-collapse attribute and the committed
// warning that attributes are not redaction.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const MCP_CONFIG_PATH = ".cursor/mcp.json";
const REQUIRED_ATTRIBUTES = ["binary", "linguist-generated"];
const REQUIRED_LIMITATIONS = [
  "default-collapsed presentation hint",
  "Neither is redaction",
  "patch and compare API endpoints",
  "Git history",
  "no live credential may remain in a tracked blob",
  "stop tracking project-local",
  "~/.cursor/mcp.json",
  "focused credential guard",
  "GitHub Support cleanup/history handling",
  "do not remove existing history",
  "prevent all disclosure",
];

export function validateMcpAttributes(text) {
  const entries = String(text)
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line && !line.startsWith("#"))
    .map((line) => line.split(/\s+/))
    .filter(([path]) => path === MCP_CONFIG_PATH);

  if (entries.length !== 1) {
    return [`${MCP_CONFIG_PATH} must have exactly one .gitattributes entry`];
  }

  const attributes = new Set(entries[0].slice(1));
  return REQUIRED_ATTRIBUTES.filter(
    (attribute) => !attributes.has(attribute),
  ).map(
    (attribute) =>
      `${MCP_CONFIG_PATH} must include the documented ${attribute} attribute`,
  );
}

export function validateSecurityLimitations(text) {
  const document = String(text).replace(/\s+/g, " ");
  return REQUIRED_LIMITATIONS.filter(
    (fragment) => !document.includes(fragment),
  ).map(
    (fragment) =>
      `SECURITY.md must retain the MCP attribute limitation: ${fragment}`,
  );
}

export function checkRepository(repoRoot = root) {
  return [
    ...validateMcpAttributes(
      readFileSync(join(repoRoot, ".gitattributes"), "utf8"),
    ),
    ...validateSecurityLimitations(
      readFileSync(join(repoRoot, "SECURITY.md"), "utf8"),
    ),
  ];
}

function main() {
  const violations = checkRepository();
  if (violations.length > 0) {
    console.error("::error::MCP diff-presentation contract failed:");
    for (const violation of violations) console.error(`  - ${violation}`);
    process.exit(1);
  }
  console.log(
    "OK — MCP config is default-collapsed on GitHub and documented as non-redacting.",
  );
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main();
}
