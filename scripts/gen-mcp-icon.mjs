#!/usr/bin/env node
/**
 * Regenerate the MCP server brand icon module from the app favicon.
 *
 * Reads `public/favicon.svg` (the app brand mark) and writes
 * `packages/mcp-server/src/icon.ts` with the SVG embedded as a base64
 * `data:` URI. The MCP server advertises this on its `Implementation`
 * handshake (per the MCP icons spec, 2025-11) so clients that render server
 * icons show the Chaos Scheduler mark. Embedding as a data URI keeps the
 * published, bundled npm package self-contained (no runtime file access).
 *
 * Run from the repo root: `node scripts/gen-mcp-icon.mjs`
 */
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const svgPath = join(repoRoot, "public", "favicon.svg");
const outPath = join(repoRoot, "packages", "mcp-server", "src", "icon.ts");

const svg = readFileSync(svgPath, "utf8").trim();
const b64 = Buffer.from(svg, "utf8").toString("base64");

const out = `/**
 * Brand icon + website for the Chaos Scheduler MCP server, surfaced on the
 * \`Implementation\` handshake per the MCP icons spec (2025-11). Clients that
 * render server icons (e.g. MCP Inspector) show this next to the server; clients
 * that do not yet render them fall back to a generated avatar.
 *
 * The icon is the app brand mark (\`public/favicon.svg\`) embedded as a base64
 * data URI so the published, bundled package is self-contained (no runtime file
 * access). Regenerate with: \`node scripts/gen-mcp-icon.mjs\`.
 *
 * GENERATED FILE — do not edit by hand; edit the source SVG and regenerate.
 */
import type { Implementation } from "@modelcontextprotocol/sdk/types.js";

export const SERVER_WEBSITE_URL =
  "https://github.com/KleinPerkins/chaos-scheduler";

const ICON_SVG_DATA_URI =
  "data:image/svg+xml;base64,${b64}";

export const SERVER_ICONS: NonNullable<Implementation["icons"]> = [
  {
    src: ICON_SVG_DATA_URI,
    mimeType: "image/svg+xml",
    sizes: ["any"],
  },
];
`;

writeFileSync(outPath, out);
console.log(
  `Wrote ${outPath}\n  source: ${svgPath} (${svg.length} bytes)\n  base64: ${b64.length} chars`,
);
