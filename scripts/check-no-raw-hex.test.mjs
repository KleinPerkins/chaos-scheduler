// check-no-raw-hex.test.mjs — fails-first regression tests for the hex lint gate.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { writeFileSync, mkdtempSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { checkFiles } from "./check-no-raw-hex.mjs";

describe("check-no-raw-hex", () => {
  it("fails on a CSS file containing a raw hex color", () => {
    const dir = mkdtempSync(join(tmpdir(), "hex-test-"));
    const bad = join(dir, "bad.css");
    writeFileSync(bad, ".foo { color: #ff0000; }\n");
    try {
      assert.throws(() => checkFiles([bad]), /raw hex/i);
    } finally {
      rmSync(dir, { recursive: true });
    }
  });

  it("fails on a TSX file containing a raw hex color", () => {
    const dir = mkdtempSync(join(tmpdir(), "hex-test-"));
    const bad = join(dir, "bad.tsx");
    writeFileSync(bad, 'const color = "#abc123";\n');
    try {
      assert.throws(() => checkFiles([bad]), /raw hex/i);
    } finally {
      rmSync(dir, { recursive: true });
    }
  });

  it("passes on a file using only token vars", () => {
    const dir = mkdtempSync(join(tmpdir(), "hex-test-"));
    const good = join(dir, "good.css");
    writeFileSync(
      good,
      ".foo { color: var(--accent); background: rgba(var(--scrim-rgb), 0.5); }\n",
    );
    try {
      assert.doesNotThrow(() => checkFiles([good]));
    } finally {
      rmSync(dir, { recursive: true });
    }
  });

  it("ignores hex in a block comment", () => {
    const dir = mkdtempSync(join(tmpdir(), "hex-test-"));
    const f = join(dir, "commented.css");
    writeFileSync(
      f,
      "/* color was #ff0000 before tokenization */\n.foo { color: var(--error); }\n",
    );
    try {
      assert.doesNotThrow(() => checkFiles([f]));
    } finally {
      rmSync(dir, { recursive: true });
    }
  });

  it("ignores hex in a // comment", () => {
    const dir = mkdtempSync(join(tmpdir(), "hex-test-"));
    const f = join(dir, "commented.ts");
    writeFileSync(f, "// const old = '#ff0000';\nconst c = 'var(--accent)';\n");
    try {
      assert.doesNotThrow(() => checkFiles([f]));
    } finally {
      rmSync(dir, { recursive: true });
    }
  });

  it("skips non-targeted file extensions (.json, .md)", () => {
    const dir = mkdtempSync(join(tmpdir(), "hex-test-"));
    const f = join(dir, "tokens.json");
    writeFileSync(f, '{ "value": "#ff0000" }\n');
    try {
      assert.doesNotThrow(() => checkFiles([f]));
    } finally {
      rmSync(dir, { recursive: true });
    }
  });

  it("does not flag HTML numeric entities (&#123; is not a hex color)", () => {
    const dir = mkdtempSync(join(tmpdir(), "hex-test-"));
    const f = join(dir, "entities.tsx");
    writeFileSync(f, "<code>POST /workflows/&#123;id&#125;/dispatch</code>\n");
    try {
      assert.doesNotThrow(() => checkFiles([f]));
    } finally {
      rmSync(dir, { recursive: true });
    }
  });
});
