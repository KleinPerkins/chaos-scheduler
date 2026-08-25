import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join, dirname } from "node:path";
import { describe, expect, it } from "vitest";

const css = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), "Notice.css"),
  "utf8",
);

describe("Notice.css tokenization", () => {
  it("contains no raw RGB triplets in rgba() — all colors bind to cs.* token vars", () => {
    // Matches rgba( with a literal numeric first arg (not var(--…))
    expect(css).not.toMatch(/rgba\(\s*\d+\s*,\s*\d+/);
  });

  it("info variant uses --accent-rgb", () => {
    expect(css).toMatch(/\.ui-notice--info\s*\{[^}]*var\(--accent-rgb\)/s);
  });

  it("success variant uses --success-rgb", () => {
    expect(css).toMatch(/\.ui-notice--success\s*\{[^}]*var\(--success-rgb\)/s);
  });

  it("error variant uses --error-rgb", () => {
    expect(css).toMatch(/\.ui-notice--error\s*\{[^}]*var\(--error-rgb\)/s);
  });

  it("warning variant uses --warning-rgb", () => {
    expect(css).toMatch(/\.ui-notice--warning\s*\{[^}]*var\(--warning-rgb\)/s);
  });
});
