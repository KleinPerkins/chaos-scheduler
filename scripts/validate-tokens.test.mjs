import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { validateTokenFile } from "./validate-tokens.mjs";

describe("validate-tokens", () => {
  it("accepts a well-formed token tree (groups + leaves with value/comment)", () => {
    const data = {
      palette: {
        accent: {
          dark: { value: "#6355e8", comment: "brand accent" },
          "dark-hover": { value: "#5749d4" },
        },
        white: { value: "#ffffff" },
      },
    };
    assert.deepEqual(validateTokenFile("color.palette.json", data), []);
  });

  it("rejects a leaf missing its `value`", () => {
    // A valueless object is interpreted as a group, so its non-object child is
    // what trips the schema — either way the malformed source is rejected.
    const errors = validateTokenFile("radius.json", {
      radius: { default: { comment: "no value here" } },
    });
    assert.ok(errors.length > 0, "expected an error");
    assert.ok(errors.some((e) => /radius|object/i.test(e)));
  });

  it("rejects a non-string value (e.g. a number)", () => {
    const errors = validateTokenFile("spacing.json", {
      space: { 1: { value: 4 } },
    });
    assert.ok(errors.some((e) => /value|string/.test(e)));
  });

  it("rejects an empty string value", () => {
    const errors = validateTokenFile("font.json", {
      font: { sans: { value: "" } },
    });
    assert.ok(errors.length > 0);
  });

  it("rejects a stray key on a leaf", () => {
    const errors = validateTokenFile("radius.json", {
      radius: { default: { value: "8px", unexpected: true } },
    });
    assert.ok(errors.some((e) => /unexpected|additional/i.test(e)));
  });

  it("rejects an empty group", () => {
    const errors = validateTokenFile("motion.json", {
      duration: {},
      ease: { standard: { value: "linear" } },
    });
    assert.ok(errors.length > 0);
  });

  it("flags a known file that dropped its required top-level group", () => {
    // theme.dark.json must expose theme.dark; here it is renamed to theme.night.
    const errors = validateTokenFile("theme.dark.json", {
      theme: { night: { surface: { primary: { value: "#0f1117" } } } },
    });
    assert.ok(errors.some((e) => /required group "theme\.dark"/.test(e)));
  });

  it("shape-only checks an unknown token file (no required roots)", () => {
    assert.deepEqual(
      validateTokenFile("brand-new.json", {
        anything: { goes: { value: "1px" } },
      }),
      [],
    );
  });
});
