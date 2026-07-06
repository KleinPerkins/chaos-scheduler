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
      // in, not left for npm to resolve at install time.
      const escaped = pkg.replace(/[/]/g, "\\/");
      const bannerPattern = new RegExp(`//\\s.*node_modules/${escaped}/`);
      expect(bannerPattern.test(allSource)).toBe(true);
    },
  );

  it.each(["zod", "@modelcontextprotocol/sdk"])(
    "no live code import/require of %s remains (only bundled)",
    (pkg) => {
      const escaped = pkg.replace(/[/]/g, "\\/");
      const importPattern = new RegExp(
        `from\\s+["']${escaped}(?:/[^"']*)?["']|require\\(["']${escaped}(?:/[^"']*)?["']\\)`,
      );
      const offenders: string[] = [];
      for (const file of distFiles) {
        const contents = readFileSync(join(distDir, file), "utf8");
        for (const line of contents.split("\n")) {
          const trimmed = line.trim();
          // Skip comments/JSDoc — esbuild preserves some upstream doc
          // comments verbatim, which can *mention* a package name in prose
          // without it being a real, unresolved import.
          if (trimmed.startsWith("//") || trimmed.startsWith("*")) continue;
          if (importPattern.test(line))
            offenders.push(`${file}: ${line.trim()}`);
        }
      }
      expect(offenders).toEqual([]);
    },
  );
});
