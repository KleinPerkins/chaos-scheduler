import { describe, expect, it } from "vitest";
import { readFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

// Regression test for the "exact-version pin is not enforced end-to-end"
// finding: `@modelcontextprotocol/sdk` and `zod` were declared as regular
// (unpinned, `^`-range) npm dependencies and left un-bundled, so a
// compromised/malicious patch published to either after release could be
// silently pulled onto every user's machine on the app's next re-provision
// or startup re-provision hook — even though CI's release-smoke gate only
// ever tested the tree resolved at release time.
//
// This asserts the *build output* itself proves both packages were inlined
// by tsup (see tsup.config.ts's `noExternal`), rather than relying only on
// the end-to-end `mcp-consumer-smoke` release gate (which only runs at
// release time against the npm registry, not on every PR).
describe("build output bundles every runtime dependency", () => {
  const distDir = join(dirname(fileURLToPath(import.meta.url)), "..", "dist");
  const distFiles = readdirSync(distDir).filter((f) => f.endsWith(".js"));
  const allSource = distFiles
    .map((f) => readFileSync(join(distDir, f), "utf8"))
    .join("\n");

  it("built at least the expected entrypoints", () => {
    expect(distFiles).toContain("cli.js");
    expect(distFiles).toContain("index.js");
  });

  it.each(["zod", "@modelcontextprotocol/sdk"])(
    "inlines %s's own source (esbuild module banner present)",
    (pkg) => {
      // esbuild/tsup emits a `// node_modules/<pkg>/...` banner comment
      // above every module it inlines from that package — a positive,
      // hard-to-fake signal that the package's actual source was bundled
      // in, not left for npm to resolve at install time. Literal string
      // containment (no regex) avoids constructing a pattern from `pkg`.
      expect(allSource.includes(`node_modules/${pkg}/`)).toBe(true);
    },
  );

  /** True if `line` contains a live `import`/`require` reference to `pkg`. */
  function hasLiveImportOf(line: string, pkg: string): boolean {
    const needles = [
      `from "${pkg}"`,
      `from '${pkg}'`,
      `from "${pkg}/`,
      `from '${pkg}/`,
      `require("${pkg}")`,
      `require('${pkg}')`,
      `require("${pkg}/`,
      `require('${pkg}/`,
    ];
    return needles.some((needle) => line.includes(needle));
  }

  it.each(["zod", "@modelcontextprotocol/sdk"])(
    "no live code import/require of %s remains (only bundled)",
    (pkg) => {
      const offenders: string[] = [];
      for (const file of distFiles) {
        const contents = readFileSync(join(distDir, file), "utf8");
        for (const line of contents.split("\n")) {
          const trimmed = line.trim();
          // Skip comments/JSDoc — esbuild preserves some upstream doc
          // comments verbatim, which can *mention* a package name in prose
          // without it being a real, unresolved import.
          if (trimmed.startsWith("//") || trimmed.startsWith("*")) continue;
          if (hasLiveImportOf(line, pkg))
            offenders.push(`${file}: ${line.trim()}`);
        }
      }
      expect(offenders).toEqual([]);
    },
  );
});

// Regression test for the "@chaos-scheduler/sdk gets bundled, defeating the
// release-ordering gate" finding: unlike zod/@modelcontextprotocol/sdk
// above, @chaos-scheduler/sdk is our own package, published and versioned
// independently (see the sdk-ts -> mcp-server ordering in release.yml /
// docs/RELEASING.md, which rewrites this dependency to the real published
// semver before `npm publish`). If tsup inlines its source instead of
// leaving it external, a published mcp-server freezes whatever SDK code
// happened to be on disk at its own build time — an SDK-only hotfix
// republish would then never reach a user who already has mcp-server
// installed, even though npm would still (uselessly) fetch the newer SDK
// into node_modules alongside the stale bundled copy. This is the inverse
// of the assertions above: the package must stay a live, unresolved import.
describe("build output leaves @chaos-scheduler/sdk external, not bundled", () => {
  const distDir = join(dirname(fileURLToPath(import.meta.url)), "..", "dist");
  const distFiles = readdirSync(distDir).filter((f) => f.endsWith(".js"));
  const allSource = distFiles
    .map((f) => readFileSync(join(distDir, f), "utf8"))
    .join("\n");

  it("does not inline @chaos-scheduler/sdk's own source", () => {
    // The SDK's own package name never appears in bundled source comments
    // when it's left external — esbuild only emits path-banner comments
    // for modules it actually inlines.
    expect(allSource.includes("sdk-ts")).toBe(false);
  });

  it("keeps a live import of @chaos-scheduler/sdk for node to resolve at runtime", () => {
    const importPattern =
      /from\s+["']@chaos-scheduler\/sdk["']|require\(["']@chaos-scheduler\/sdk["']\)/;
    const matches = distFiles.filter((file) =>
      importPattern.test(readFileSync(join(distDir, file), "utf8")),
    );
    expect(matches.length).toBeGreaterThan(0);
  });
});
