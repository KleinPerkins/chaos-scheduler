#!/usr/bin/env node
// Release-ordering smoke gate: prove that an exact, already-published
// `@chaos-scheduler/mcp-server@<version>` is installable by a clean npm
// consumer (a throwaway project, not the monorepo workspace), and that it
// resolves `@chaos-scheduler/sdk` as a real published dependency — not the
// repo-local `file:../sdk-ts` link used for local dev.
//
// This is the check the plan's "release ordering" gate depends on: publish
// sdk-ts, then publish mcp-server, then run this script against the exact
// pinned mcp-server version *before* anything (desktop build, "Latest" re-pin,
// managed-provisioning docs) treats that version as consumable.
//
// Usage: node scripts/smoke-mcp-install.mjs <mcp-server-version>
import { execFileSync } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const version = process.argv[2];
if (!version) {
  console.error(
    "usage: node scripts/smoke-mcp-install.mjs <mcp-server-version>",
  );
  process.exit(1);
}

const pkgSpec = `@chaos-scheduler/mcp-server@${version}`;
// Use a real (throwaway) package.json + run `npm install` with cwd == the
// project root, rather than `npm install --prefix <dir>` from a different
// cwd: with --prefix pointed elsewhere, npm relativizes package-lock.json
// paths against the *invoking* cwd rather than the install root (worse, it
// trips over the /tmp -> /private/tmp symlink on macOS), producing lockfile
// keys that don't match `node_modules/<pkg>`. Matching cwd to the project
// root keeps the lockfile in the normal, parseable shape.
const projectDir = mkdtempSync(join(tmpdir(), "chaos-mcp-smoke-"));
writeFileSync(
  join(projectDir, "package.json"),
  JSON.stringify({ name: "chaos-mcp-smoke", private: true }, null, 2) + "\n",
);

console.log(
  `Installing ${pkgSpec} into a clean consumer project: ${projectDir}`,
);
try {
  execFileSync(
    "npm",
    ["install", "--no-audit", "--no-fund", "--ignore-scripts", pkgSpec],
    {
      cwd: projectDir,
      stdio: "inherit",
    },
  );
} catch {
  console.error(
    `::error::npm install failed for ${pkgSpec} — see output above.`,
  );
  process.exit(1);
}

// 1. Prove @chaos-scheduler/sdk resolved from the public npm registry, not a
//    local file: dependency.
const lockPath = join(projectDir, "package-lock.json");
if (!existsSync(lockPath)) {
  console.error(
    `::error::expected a package-lock.json at ${lockPath} after install`,
  );
  process.exit(1);
}
const lock = JSON.parse(readFileSync(lockPath, "utf8"));
const sdkEntry = lock.packages?.["node_modules/@chaos-scheduler/sdk"];
if (!sdkEntry) {
  console.error(
    "::error::@chaos-scheduler/sdk did not resolve as a dependency of mcp-server — check the " +
      "publish-mcp job actually rewrote the file:../sdk-ts dependency before `npm publish`.",
  );
  process.exit(1);
}
if (
  !sdkEntry.resolved ||
  !sdkEntry.resolved.startsWith("https://registry.npmjs.org/")
) {
  console.error(
    `::error::@chaos-scheduler/sdk resolved from "${sdkEntry.resolved ?? "unknown"}", expected the ` +
      "public npm registry. This is exactly the file:../sdk-ts-leaked-into-a-published-tarball bug " +
      "this gate exists to catch.",
  );
  process.exit(1);
}
console.log(
  `OK  @chaos-scheduler/sdk@${sdkEntry.version} resolved from the npm registry.`,
);

// 2. Prove the installed CLI actually *uses* the resolved SDK dependency
//    rather than a copy of its source frozen into the bundle at mcp-server's
//    own build time (see tsup.config.ts's `noExternal` — @chaos-scheduler/sdk
//    is deliberately excluded from it). A lockfile-only check (above) can't
//    catch this: npm would still dutifully install the real SDK version
//    even if the shipped CLI never actually imports it, which would let an
//    SDK-only hotfix pass this gate while silently never reaching a single
//    installed user.
const distDir = join(
  projectDir,
  "node_modules",
  "@chaos-scheduler",
  "mcp-server",
  "dist",
);
const cliPath = join(distDir, "cli.js");
if (!existsSync(cliPath)) {
  console.error(`::error::installed CLI entrypoint not found at ${cliPath}`);
  process.exit(1);
}
const bundledSource = readdirSync(distDir)
  .filter((f) => f.endsWith(".js"))
  .map((f) => readFileSync(join(distDir, f), "utf8"))
  .join("\n");
if (!/from\s+["']@chaos-scheduler\/sdk["']/.test(bundledSource)) {
  console.error(
    "::error::the installed mcp-server build has no live import of @chaos-scheduler/sdk — " +
      "it looks like the SDK's source was bundled in at build time instead of being left for " +
      "npm to resolve. Check tsup.config.ts's `noExternal` list does not include " +
      '"@chaos-scheduler/sdk".',
  );
  process.exit(1);
}
console.log(
  "OK  installed CLI keeps a live import of @chaos-scheduler/sdk (not bundled).",
);

// 3. Prove the installed CLI actually runs on this Node (the caller pins the
//    Node version under test; CI runs this at the package's documented floor).
const help = execFileSync(process.execPath, [cliPath, "--help"], {
  encoding: "utf8",
});
if (!help.includes("chaos-mcp-server")) {
  console.error(
    "::error::installed CLI --help output did not look like chaos-mcp-server",
  );
  process.exit(1);
}
console.log(`OK  installed CLI runs under node ${process.version}.`);

console.log(
  `\nSmoke test passed: ${pkgSpec} is installable and consistent with npm-only deps.`,
);
