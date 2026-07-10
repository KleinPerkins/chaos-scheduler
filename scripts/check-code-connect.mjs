#!/usr/bin/env node
// Credential-free Code Connect coverage/health check.
//
// The repo is the source of truth for its Figma Code Connect mappings
// (src/**/*.figma.tsx). The live publish (figma-code-connect.yml) needs a PAT
// and is intentionally OUTSIDE ci-required; this check needs NO credentials and
// IS required, so a malformed mapping is caught on the PR that introduces it —
// long before the publish step runs.
//
// It complements `tsc -p tsconfig.figma.json` (which proves every mapping's
// `example` uses the REAL component with REAL props). Here we run the
// credential-free `figma connect parse` and validate that:
//   1. every *.figma.tsx on disk parses into at least one mapping (no file is
//      silently skipped), and every parsed mapping traces back to a known file;
//   2. each mapping points at a source file that actually exists in the repo;
//   3. each mapping produced a non-empty rendered template (a real snippet);
//   4. each mapping targets a real Figma design node URL.
//
// It deliberately does NOT require every component to HAVE a mapping — adding a
// component without a mapping must never fail CI (that would block the parallel
// component track).
import { execFileSync } from "node:child_process";
import { existsSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

/** Extract the repo-relative source path from a Code Connect `source` value,
 * which may be a GitHub blob URL (…/blob/<ref>/<path>) or a plain path. */
export function sourcePathFromDoc(source) {
  if (typeof source !== "string" || source.length === 0) return null;
  const blob = /\/blob\/[^/]+\/(.+?)(?:[?#].*)?$/.exec(source);
  if (blob) return blob[1];
  if (/^https?:\/\//.test(source)) return null; // a URL we can't map to a path
  return source.replace(/^\.\//, "");
}

const FIGMA_NODE_RE =
  /^https:\/\/(?:www\.)?figma\.com\/(?:design|file)\/[A-Za-z0-9]+\/.*node-id=/;

/**
 * Validate parsed Code Connect docs against the mapping files on disk. Pure
 * (no I/O) so it is unit-testable.
 *
 * @param {object} p
 * @param {any[]} p.docs parsed `figma connect parse` output
 * @param {string[]} p.figmaFilesRel repo-relative *.figma.tsx paths on disk
 * @param {(relPath: string) => boolean} p.fileExistsRel source-file probe
 * @returns {string[]} human-readable errors (empty when valid)
 */
export function validateCodeConnectDocs({
  docs,
  figmaFilesRel,
  fileExistsRel,
}) {
  const errors = [];

  if (!Array.isArray(docs)) {
    return ["`figma connect parse` did not return a JSON array of mappings."];
  }

  const norm = (p) => p.replaceAll("\\", "/");
  const onDisk = new Set(figmaFilesRel.map(norm));
  const covered = new Set();

  for (const doc of docs) {
    const rel = doc._codeConnectFilePath
      ? norm(doc._codeConnectFilePath).replace(`${norm(root)}/`, "")
      : "(unknown file)";
    const where = `${rel}`;

    if (doc._codeConnectFilePath) {
      if (!onDisk.has(rel)) {
        errors.push(`${where}: parsed mapping from an unexpected file path`);
      } else {
        covered.add(rel);
      }
    } else {
      errors.push(`(doc): mapping is missing _codeConnectFilePath`);
    }

    if (!doc.component || typeof doc.component !== "string") {
      errors.push(`${where}: mapping has no component name`);
    }

    const srcPath = sourcePathFromDoc(doc.source);
    if (!srcPath) {
      errors.push(`${where}: mapping has no resolvable source path`);
    } else if (!fileExistsRel(srcPath)) {
      errors.push(
        `${where}: mapping source "${srcPath}" does not exist in the repo`,
      );
    }

    if (!doc.template || typeof doc.template !== "string") {
      errors.push(`${where}: mapping produced no rendered template`);
    }

    if (!FIGMA_NODE_RE.test(doc.figmaNode ?? "")) {
      errors.push(
        `${where}: figmaNode is not a Figma design node URL: ${doc.figmaNode}`,
      );
    }
  }

  for (const file of onDisk) {
    if (!covered.has(file)) {
      errors.push(
        `${file}: on-disk mapping produced no parsed Code Connect doc ` +
          `(it may have failed to parse).`,
      );
    }
  }

  return errors;
}

function listFigmaFilesRel() {
  const srcDir = join(root, "src");
  return readdirSync(srcDir, { recursive: true })
    .map((p) => String(p))
    .filter((p) => p.endsWith(".figma.tsx"))
    .map((p) => `src/${p.replaceAll("\\", "/")}`)
    .sort();
}

function parseCodeConnect() {
  let stdout;
  try {
    stdout = execFileSync(
      "npx",
      ["--no-install", "figma", "connect", "parse"],
      {
        cwd: root,
        encoding: "utf8",
        stdio: ["ignore", "pipe", "pipe"],
        maxBuffer: 64 * 1024 * 1024,
      },
    );
  } catch (err) {
    console.error(
      "::error::`figma connect parse` failed (a mapping likely does not " +
        "parse). Output:",
    );
    console.error(err.stdout ?? "");
    console.error(err.stderr ?? "");
    process.exit(1);
  }
  const start = stdout.indexOf("[");
  const end = stdout.lastIndexOf("]");
  if (start === -1 || end === -1) {
    console.error("::error::`figma connect parse` produced no JSON array.");
    console.error(stdout);
    process.exit(1);
  }
  return JSON.parse(stdout.slice(start, end + 1));
}

function main() {
  const figmaFilesRel = listFigmaFilesRel();
  if (figmaFilesRel.length === 0) {
    console.log("No *.figma.tsx mappings found — nothing to validate.");
    return;
  }

  const docs = parseCodeConnect();
  const errors = validateCodeConnectDocs({
    docs,
    figmaFilesRel,
    fileExistsRel: (p) => existsSync(join(root, p)),
  });

  if (errors.length > 0) {
    console.error(
      "::error::Code Connect validation failed. Fix the mapping(s) below " +
        "(src/**/*.figma.tsx). Each mapping must point at a real component " +
        "source and produce a valid snippet:",
    );
    for (const e of errors) console.error(`  - ${e}`);
    process.exit(1);
  }

  console.log(
    `OK — ${docs.length} Code Connect mapping(s) valid across ` +
      `${figmaFilesRel.length} *.figma.tsx file(s).`,
  );
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main();
}
