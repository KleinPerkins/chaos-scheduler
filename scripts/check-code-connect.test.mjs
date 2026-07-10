import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  sourcePathFromDoc,
  validateCodeConnectDocs,
} from "./check-code-connect.mjs";

const GH = "https://github.com/KleinPerkins/chaos-scheduler/blob/main";
const NODE = "https://www.figma.com/design/abc123/Chaos-Scheduler?node-id=1-2";

function doc(overrides = {}) {
  return {
    figmaNode: NODE,
    component: "Button",
    source: `${GH}/src/components/Button.tsx`,
    template: "<Button />",
    _codeConnectFilePath: "/repo/src/components/Button.figma.tsx",
    ...overrides,
  };
}

// The pure validator resolves _codeConnectFilePath against a root; the tests
// pass already-relative on-disk paths and a root-agnostic fileExists stub.
const opts = (docs, existing = ["src/components/Button.tsx"]) => ({
  docs,
  figmaFilesRel: ["src/components/Button.figma.tsx"],
  fileExistsRel: (p) => existing.includes(p),
});

describe("sourcePathFromDoc", () => {
  it("extracts the path from a GitHub blob URL", () => {
    assert.equal(
      sourcePathFromDoc(`${GH}/src/components/Button.tsx`),
      "src/components/Button.tsx",
    );
  });
  it("passes through a plain relative path", () => {
    assert.equal(sourcePathFromDoc("./src/x.tsx"), "src/x.tsx");
  });
  it("returns null for an unmappable URL / empty", () => {
    assert.equal(sourcePathFromDoc("https://example.com/x"), null);
    assert.equal(sourcePathFromDoc(""), null);
  });
});

describe("validateCodeConnectDocs", () => {
  it("accepts a well-formed mapping", () => {
    // _codeConnectFilePath uses an absolute-ish path; the validator strips the
    // root prefix, but here there's no matching root so it stays as-is — supply
    // a matching figma file to keep the happy path clean.
    const d = doc({ _codeConnectFilePath: "src/components/Button.figma.tsx" });
    assert.deepEqual(validateCodeConnectDocs(opts([d])), []);
  });

  it("flags a mapping whose source file does not exist", () => {
    const d = doc({
      _codeConnectFilePath: "src/components/Button.figma.tsx",
      source: `${GH}/src/components/Ghost.tsx`,
    });
    const errors = validateCodeConnectDocs(opts([d]));
    assert.ok(errors.some((e) => /does not exist/.test(e)));
  });

  it("flags an on-disk mapping that produced no parsed doc", () => {
    const errors = validateCodeConnectDocs(opts([]));
    assert.ok(
      errors.some((e) => /produced no parsed Code Connect doc/.test(e)),
    );
  });

  it("flags an empty rendered template", () => {
    const d = doc({
      _codeConnectFilePath: "src/components/Button.figma.tsx",
      template: "",
    });
    const errors = validateCodeConnectDocs(opts([d]));
    assert.ok(errors.some((e) => /no rendered template/.test(e)));
  });

  it("flags an invalid Figma node URL", () => {
    const d = doc({
      _codeConnectFilePath: "src/components/Button.figma.tsx",
      figmaNode: "https://example.com/not-figma",
    });
    const errors = validateCodeConnectDocs(opts([d]));
    assert.ok(errors.some((e) => /not a Figma design node URL/.test(e)));
  });

  it("flags a parsed doc from an unexpected file path", () => {
    const d = doc({ _codeConnectFilePath: "src/components/Rogue.figma.tsx" });
    const errors = validateCodeConnectDocs(opts([d]));
    assert.ok(errors.some((e) => /unexpected file path/.test(e)));
  });

  it("rejects a non-array parse result", () => {
    const errors = validateCodeConnectDocs({
      docs: null,
      figmaFilesRel: [],
      fileExistsRel: () => true,
    });
    assert.ok(errors.some((e) => /did not return a JSON array/.test(e)));
  });
});
