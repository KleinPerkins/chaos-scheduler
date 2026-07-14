import assert from "node:assert/strict";
import { dirname, join } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import {
  checkRepository,
  validateMcpAttributes,
  validateSecurityLimitations,
} from "./check-mcp-diff-attribute.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

const COMPLETE_LIMITATIONS = [
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
].join("\n");

describe("MCP diff attribute", () => {
  it("requires the exact documented default-collapse and local diff attributes", () => {
    assert.deepEqual(
      validateMcpAttributes(".cursor/mcp.json binary linguist-generated\n"),
      [],
    );
  });

  it("fails the previous binary-only rule", () => {
    const violations = validateMcpAttributes(".cursor/mcp.json binary\n");
    assert.equal(violations.length, 1);
    assert.match(violations[0], /linguist-generated/);
  });

  it("does not accept comments or a non-documented assignment as the rule", () => {
    const violations = validateMcpAttributes(
      [
        "# .cursor/mcp.json binary linguist-generated",
        ".cursor/mcp.json binary linguist-generated=true",
      ].join("\n"),
    );
    assert.equal(violations.length, 1);
    assert.match(violations[0], /linguist-generated/);
  });
});

describe("MCP attribute limitations", () => {
  it("requires every non-redaction and authoritative-control caveat", () => {
    assert.deepEqual(validateSecurityLimitations(COMPLETE_LIMITATIONS), []);
  });

  it("fails if the history/API disclosure warning is softened away", () => {
    const violations = validateSecurityLimitations(
      COMPLETE_LIMITATIONS.replace("patch and compare API endpoints", ""),
    );
    assert.equal(violations.length, 1);
    assert.match(violations[0], /patch and compare API endpoints/);
  });

  it("passes against the committed repository contract", () => {
    assert.deepEqual(checkRepository(root), []);
  });
});
