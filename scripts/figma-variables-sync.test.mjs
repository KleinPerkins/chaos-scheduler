// Unit tests for the pure logic of figma-variables-sync.mjs.
// Run with: node --test scripts/figma-variables-sync.test.mjs
// No network / token required — everything below feeds synthetic state through
// the pure functions. The headline safety property (never emit a DELETE) is
// asserted directly.
import { test } from "node:test";
import assert from "node:assert/strict";

import {
  hexToRgba,
  valuesEqual,
  buildDesiredCollections,
  indexCurrent,
  diff,
  buildPostPayload,
  assertNoDeletes,
  planIsEmpty,
} from "./figma-variables-sync.mjs";

const manifest = {
  fileKeyHint: "TESTKEY",
  collections: [
    {
      name: "cs.color",
      modes: ["Light", "Dark"],
      variables: [
        {
          name: "bg/primary",
          type: "COLOR",
          scopes: ["FRAME_FILL"],
          codeSyntax: { WEB: "var(--bg-primary)" },
          values: {
            Light: { hex: "#ffffff", alpha: 1 },
            Dark: { hex: "#000000", alpha: 1 },
          },
        },
      ],
    },
    {
      name: "cs.space",
      modes: ["Value"],
      variables: [
        {
          name: "1",
          type: "FLOAT",
          scopes: ["GAP"],
          codeSyntax: { WEB: "var(--space-1)" },
          values: { Value: 4 },
        },
      ],
    },
  ],
};

// A GET /variables/local `meta` that already matches the manifest exactly.
function syncedMeta() {
  return {
    variableCollections: {
      C1: {
        id: "C1",
        name: "cs.color",
        remote: false,
        defaultModeId: "m1",
        modes: [
          { modeId: "m1", name: "Light" },
          { modeId: "m2", name: "Dark" },
        ],
        variableIds: ["V1"],
      },
      C2: {
        id: "C2",
        name: "cs.space",
        remote: false,
        defaultModeId: "m3",
        modes: [{ modeId: "m3", name: "Value" }],
        variableIds: ["V2"],
      },
    },
    variables: {
      V1: {
        id: "V1",
        name: "bg/primary",
        variableCollectionId: "C1",
        remote: false,
        resolvedType: "COLOR",
        scopes: ["FRAME_FILL"],
        codeSyntax: { WEB: "var(--bg-primary)" },
        description: "",
        valuesByMode: {
          m1: { r: 1, g: 1, b: 1, a: 1 },
          m2: { r: 0, g: 0, b: 0, a: 1 },
        },
      },
      V2: {
        id: "V2",
        name: "1",
        variableCollectionId: "C2",
        remote: false,
        resolvedType: "FLOAT",
        scopes: ["GAP"],
        codeSyntax: { WEB: "var(--space-1)" },
        description: "",
        valuesByMode: { m3: 4 },
      },
    },
  };
}

test("hexToRgba normalises hex + alpha to 0..1 floats", () => {
  assert.deepEqual(hexToRgba("#ffffff", 1), { r: 1, g: 1, b: 1, a: 1 });
  assert.deepEqual(hexToRgba("#000000", 0.3), { r: 0, g: 0, b: 0, a: 0.3 });
  const mid = hexToRgba("#804020", 1);
  assert.ok(Math.abs(mid.r - 128 / 255) < 1e-9);
  assert.ok(Math.abs(mid.g - 64 / 255) < 1e-9);
  assert.throws(() => hexToRgba("nope"));
});

test("valuesEqual tolerates float noise but detects real steps", () => {
  assert.ok(
    valuesEqual("COLOR", { r: 1, g: 1, b: 1, a: 1 }, hexToRgba("#ffffff")),
  );
  assert.ok(
    !valuesEqual("COLOR", { r: 1, g: 1, b: 1, a: 1 }, hexToRgba("#fefefe")),
  );
  assert.ok(valuesEqual("FLOAT", 4, 4));
  assert.ok(!valuesEqual("FLOAT", 4, 5));
  assert.ok(valuesEqual("STRING", "0.15s", "0.15s"));
});

test("buildDesiredCollections converts colors and keeps primitives", () => {
  const desired = buildDesiredCollections(manifest);
  const color = desired[0].variables[0];
  assert.deepEqual(color.valuesByMode.Light, { r: 1, g: 1, b: 1, a: 1 });
  assert.equal(desired[1].variables[0].valuesByMode.Value, 4);
});

test("diff against empty Figma plans creates for everything, zero deletes", () => {
  const desired = buildDesiredCollections(manifest);
  const idx = indexCurrent({ variableCollections: {}, variables: {} });
  const plan = diff(desired, idx);
  assert.equal(plan.createCollections.length, 2);
  assert.equal(plan.setValues.length, 3); // Light + Dark + Value
  assert.equal(plan.extras.collections.length, 0);
  const payload = buildPostPayload(plan, idx);
  assert.ok(assertNoDeletes(payload));
});

test("diff against a synced file is a no-op", () => {
  const desired = buildDesiredCollections(manifest);
  const idx = indexCurrent(syncedMeta());
  const plan = diff(desired, idx);
  assert.ok(planIsEmpty(plan), JSON.stringify(plan, null, 2));
});

test("a changed color value produces exactly one set-value op", () => {
  const desired = buildDesiredCollections(manifest);
  const meta = syncedMeta();
  meta.variables.V1.valuesByMode.m1 = { r: 0.5, g: 0.5, b: 0.5, a: 1 };
  const plan = diff(desired, indexCurrent(meta));
  assert.equal(plan.setValues.length, 1);
  assert.equal(plan.setValues[0].variableName, "bg/primary");
  assert.equal(plan.setValues[0].mode, "Light");
});

test("changed scopes / codeSyntax produce a metadata update", () => {
  const desired = buildDesiredCollections(manifest);
  const meta = syncedMeta();
  meta.variables.V2.scopes = ["WIDTH_HEIGHT"];
  meta.variables.V2.codeSyntax = { WEB: "var(--stale)" };
  const plan = diff(desired, indexCurrent(meta));
  assert.equal(plan.updateVariables.length, 1);
  assert.deepEqual(Object.keys(plan.updateVariables[0].changes).sort(), [
    "codeSyntax",
    "scopes",
  ]);
});

test("extra Figma variables are reported but NEVER deleted", () => {
  const desired = buildDesiredCollections(manifest);
  const meta = syncedMeta();
  meta.variables.VX = {
    id: "VX",
    name: "bg/legacy",
    variableCollectionId: "C1",
    remote: false,
    resolvedType: "COLOR",
    scopes: [],
    codeSyntax: {},
    description: "",
    valuesByMode: { m1: { r: 0.1, g: 0.1, b: 0.1, a: 1 } },
  };
  const idx = indexCurrent(meta);
  const plan = diff(desired, idx);
  assert.ok(plan.extras.variables.includes("cs.color / bg/legacy"));
  const payload = buildPostPayload(plan, idx);
  assert.ok(assertNoDeletes(payload));
  // Nothing in the payload should reference the legacy variable at all.
  const touchesLegacy = payload.variables.some((v) => v.id === "VX");
  assert.ok(!touchesLegacy);
});

test("a type mismatch is a warning, not a destructive change", () => {
  const desired = buildDesiredCollections(manifest);
  const meta = syncedMeta();
  meta.variables.V2.resolvedType = "STRING"; // manifest says FLOAT
  const plan = diff(desired, indexCurrent(meta));
  assert.equal(plan.warnings.length, 1);
  assert.match(plan.warnings[0], /type mismatch/);
  // no create/update proposed for the mismatched var
  assert.equal(plan.createVariables.length, 0);
});

test("assertNoDeletes throws if a DELETE ever appears", () => {
  assert.throws(
    () => assertNoDeletes({ variables: [{ action: "DELETE", id: "x" }] }),
    /create-or-update only/,
  );
});
