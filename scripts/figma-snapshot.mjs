#!/usr/bin/env node
// Semantic snapshot / drift-diff for the live Chaos Scheduler Figma design file.
//
// Captures a STABLE, semantic outline (node name + type + child hierarchy, with
// volatile ids / geometry / styles stripped) of the design page and the
// self-contained component-library section, so the scheduled advisory workflow
// (.github/workflows/figma-snapshot.yml) can flag when the live design drifts
// from the committed baseline. This is metadata only — NOT a pixel/visual diff
// (the Playwright visual harness owns pixels).
//
// Credential model: reads FIGMA_ACCESS_TOKEN (a Figma PAT with "File content:
// read"). Purely advisory — kept OUTSIDE the ci-required fan-in.
//
// Usage:
//   node scripts/figma-snapshot.mjs snapshot --out <file>
//   node scripts/figma-snapshot.mjs check --baseline <file> [--out <file>]
//
// `check` exits 1 on drift, 0 on match, and 0 (with a note) when the baseline
// does not exist yet — so the first run can seed a baseline via its artifact.

import { readFileSync, writeFileSync, existsSync } from "node:fs";

// The published team-library file (see src/**/*.figma.tsx node URLs + AGENTS.md).
export const FILE_KEY = "twQmWC8dWT4tqeqIigNsRy";
// Page "Mission Control" (0:1) + the self-contained component section
// "v4 — New Components (Affirm DS)" (113:514).
export const NODE_IDS = ["0:1", "113:514"];
export const SCHEMA = "figma-semantic-snapshot/v1";
const DEFAULT_DEPTH = 4;
const API = "https://api.figma.com/v1";

/**
 * Reduce a raw Figma node tree to a stable semantic outline: only `name`,
 * `type`, and `children` (in document order). Ids, geometry, fills, styles, and
 * absolute positions are intentionally dropped so the diff surfaces STRUCTURAL
 * design changes (renames, added/removed/retyped nodes) rather than pixel or
 * id churn.
 */
export function normalizeNode(node) {
  const out = { name: node.name ?? "", type: node.type ?? "" };
  if (Array.isArray(node.children) && node.children.length > 0) {
    out.children = node.children.map(normalizeNode);
  }
  return out;
}

/** Build the full snapshot object from a Figma `GET /files/:key/nodes` body. */
export function buildSnapshot(fileKey, nodeIds, nodesResponse) {
  const nodes = nodeIds.map((id) => {
    const entry = nodesResponse?.[id];
    if (!entry || !entry.document) {
      throw new Error(
        `Figma response is missing node "${id}" — check the id and PAT file access.`,
      );
    }
    return { requestedId: id, ...normalizeNode(entry.document) };
  });
  return { schema: SCHEMA, fileKey, nodes };
}

/**
 * Coarse, order-sensitive structural diff between two snapshots. Returns a list
 * of human-readable differences (empty when identical). Index-based child
 * matching keeps it simple; an insertion may cascade (acceptable for an
 * advisory signal).
 */
export function diffSnapshots(baseline, current) {
  const diffs = [];
  const byId = (snap) =>
    new Map((snap.nodes ?? []).map((n) => [n.requestedId, n]));
  const baseNodes = byId(baseline);
  const curNodes = byId(current);
  const ids = new Set([...baseNodes.keys(), ...curNodes.keys()]);
  for (const id of ids) {
    diffTree(baseNodes.get(id), curNodes.get(id), `node ${id}`, diffs);
  }
  return diffs;
}

function diffTree(base, cur, path, diffs) {
  if (!base && !cur) return;
  if (!base) {
    diffs.push(`+ added: ${path} → "${cur.name}" [${cur.type}]`);
    return;
  }
  if (!cur) {
    diffs.push(`- removed: ${path} → "${base.name}" [${base.type}]`);
    return;
  }
  if (base.name !== cur.name) {
    diffs.push(`~ renamed (${path}): "${base.name}" → "${cur.name}"`);
  }
  if (base.type !== cur.type) {
    diffs.push(`~ retyped (${path} "${cur.name}"): ${base.type} → ${cur.type}`);
  }
  const bch = base.children ?? [];
  const cch = cur.children ?? [];
  const max = Math.max(bch.length, cch.length);
  for (let i = 0; i < max; i++) {
    const childPath = `${path} / ${(cch[i] ?? bch[i]).name}`;
    diffTree(bch[i], cch[i], childPath, diffs);
  }
}

async function fetchNodes(fileKey, nodeIds, depth, token) {
  const url = `${API}/files/${fileKey}/nodes?ids=${encodeURIComponent(nodeIds.join(","))}&depth=${depth}`;
  const res = await fetch(url, { headers: { "X-Figma-Token": token } });
  if (!res.ok) {
    const body = await res.text().catch(() => "");
    if (res.status === 403 || res.status === 401) {
      throw new Error(
        `Figma API ${res.status}: the FIGMA_ACCESS_TOKEN PAT likely lacks "File content: read" or file access. ${body}`,
      );
    }
    throw new Error(`Figma API ${res.status}: ${body}`);
  }
  const json = await res.json();
  return json.nodes;
}

function parseArgs(argv) {
  const [command, ...rest] = argv;
  const opts = { command, depth: DEFAULT_DEPTH };
  for (let i = 0; i < rest.length; i++) {
    const a = rest[i];
    if (a === "--out") opts.out = rest[++i];
    else if (a === "--baseline") opts.baseline = rest[++i];
    else if (a === "--depth") opts.depth = Number(rest[++i]);
  }
  return opts;
}

function serialize(snapshot) {
  return `${JSON.stringify(snapshot, null, 2)}\n`;
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  if (opts.command !== "snapshot" && opts.command !== "check") {
    console.error(
      "Usage:\n  figma-snapshot.mjs snapshot --out <file>\n  figma-snapshot.mjs check --baseline <file> [--out <file>]",
    );
    process.exit(2);
  }

  const token = process.env.FIGMA_ACCESS_TOKEN;
  if (!token) {
    console.error(
      "::error title=Missing FIGMA_ACCESS_TOKEN::Set the FIGMA_ACCESS_TOKEN repository secret (a Figma PAT with 'File content: read').",
    );
    process.exit(1);
  }

  const nodesResponse = await fetchNodes(FILE_KEY, NODE_IDS, opts.depth, token);
  const snapshot = buildSnapshot(FILE_KEY, NODE_IDS, nodesResponse);

  if (opts.out) {
    writeFileSync(opts.out, serialize(snapshot));
    console.log(`Wrote live snapshot → ${opts.out}`);
  }

  if (opts.command === "snapshot") return;

  // check
  if (!opts.baseline || !existsSync(opts.baseline)) {
    console.log(
      `::notice title=No Figma baseline::No committed baseline at "${opts.baseline ?? "(unset)"}". ` +
        `Download the uploaded snapshot artifact and commit it to establish the diff.`,
    );
    return;
  }
  const baseline = JSON.parse(readFileSync(opts.baseline, "utf8"));
  const diffs = diffSnapshots(baseline, snapshot);
  if (diffs.length === 0) {
    console.log("Figma design matches the committed baseline. ✅");
    return;
  }
  console.log(
    `::warning title=Figma design drift::${diffs.length} change(s) vs the committed baseline (advisory):`,
  );
  for (const d of diffs) console.log(`  ${d}`);
  console.log(
    "\nIf these changes are intentional, refresh the baseline: re-run this workflow, download the snapshot artifact, and commit it.",
  );
  process.exit(1);
}

// Only run main() as a CLI, so the unit test can import the pure helpers.
if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch((err) => {
    console.error(`::error title=Figma snapshot failed::${err.message}`);
    process.exit(1);
  });
}
