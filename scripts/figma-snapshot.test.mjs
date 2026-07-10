import { test } from "node:test";
import assert from "node:assert/strict";
import {
  normalizeNode,
  buildSnapshot,
  diffSnapshots,
  FILE_KEY,
  NODE_IDS,
} from "./figma-snapshot.mjs";

test("normalizeNode keeps name/type/children and drops volatile fields", () => {
  const raw = {
    id: "1:2",
    name: "Frame A",
    type: "FRAME",
    absoluteBoundingBox: { x: 10, y: 20, width: 100, height: 50 },
    fills: [{ type: "SOLID", color: { r: 1, g: 0, b: 0 } }],
    children: [{ id: "1:3", name: "Text", type: "TEXT", characters: "hi" }],
  };
  assert.deepEqual(normalizeNode(raw), {
    name: "Frame A",
    type: "FRAME",
    children: [{ name: "Text", type: "TEXT" }],
  });
});

test("normalizeNode omits children when there are none", () => {
  assert.deepEqual(normalizeNode({ id: "9", name: "Leaf", type: "TEXT" }), {
    name: "Leaf",
    type: "TEXT",
  });
});

test("buildSnapshot keys nodes by requestedId in order", () => {
  const response = {
    "0:1": { document: { id: "0:1", name: "Mission Control", type: "CANVAS" } },
    "113:514": {
      document: { id: "113:514", name: "v4 Components", type: "SECTION" },
    },
  };
  const snap = buildSnapshot(FILE_KEY, NODE_IDS, response);
  assert.equal(snap.fileKey, FILE_KEY);
  assert.deepEqual(
    snap.nodes.map((n) => n.requestedId),
    NODE_IDS,
  );
  assert.equal(snap.nodes[0].name, "Mission Control");
});

test("buildSnapshot throws when a requested node is absent", () => {
  assert.throws(
    () => buildSnapshot(FILE_KEY, NODE_IDS, { "0:1": { document: {} } }),
    /missing node "113:514"|missing node "0:1"/,
  );
});

const baseline = {
  schema: "figma-semantic-snapshot/v1",
  fileKey: FILE_KEY,
  nodes: [
    {
      requestedId: "0:1",
      name: "Mission Control",
      type: "CANVAS",
      children: [
        { name: "Workflows", type: "FRAME" },
        { name: "Queues", type: "FRAME" },
      ],
    },
  ],
};

test("diffSnapshots returns empty for identical snapshots", () => {
  assert.deepEqual(
    diffSnapshots(baseline, JSON.parse(JSON.stringify(baseline))),
    [],
  );
});

test("diffSnapshots detects rename, add, remove, retype", () => {
  const current = {
    ...baseline,
    nodes: [
      {
        requestedId: "0:1",
        name: "Mission Control (v2)", // renamed
        type: "CANVAS",
        children: [
          { name: "Workflows", type: "GROUP" }, // retyped
          { name: "Runs", type: "FRAME" }, // renamed from Queues
        ],
      },
    ],
  };
  const diffs = diffSnapshots(baseline, current).join("\n");
  assert.match(diffs, /renamed .*Mission Control.*Mission Control \(v2\)/);
  assert.match(diffs, /retyped.*FRAME → GROUP/);
});

test("diffSnapshots detects an added top-level node", () => {
  const current = {
    ...baseline,
    nodes: [
      baseline.nodes[0],
      { requestedId: "113:514", name: "New Section", type: "SECTION" },
    ],
  };
  const diffs = diffSnapshots(baseline, current).join("\n");
  assert.match(diffs, /\+ added: node 113:514/);
});
