import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { afterEach, describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import {
  containsProhibitedCredential,
  scanTrackedMcpConfigs,
  validateMcpConfigText,
} from "./check-tracked-mcp-config.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const tempRoots = [];

function tempRepo() {
  const dir = mkdtempSync(join(tmpdir(), "chaos-mcp-config-"));
  tempRoots.push(dir);
  execFileSync("git", ["init", "-q"], { cwd: dir });
  return dir;
}

function write(rootDir, relativePath, contents) {
  const path = join(rootDir, relativePath);
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, contents);
}

function git(rootDir, ...args) {
  execFileSync("git", args, {
    cwd: rootDir,
    stdio: ["ignore", "ignore", "ignore"],
  });
}

afterEach(() => {
  while (tempRoots.length) {
    rmSync(tempRoots.pop(), { recursive: true, force: true });
  }
});

describe("tracked MCP config parser", () => {
  it("rejects the prohibited property at any nested depth or value", () => {
    for (const value of ["synthetic", "", null]) {
      assert.equal(
        containsProhibitedCredential({
          outer: [{ env: { CHAOS_SCHEDULER_API_KEY: value } }],
        }),
        true,
      );
    }
  });

  it("allows similar names and ordinary values", () => {
    assert.equal(
      containsProhibitedCredential({
        CHAOS_SCHEDULER_API_KEY_FILE: "/safe/path",
        note: "CHAOS_SCHEDULER_API_KEY",
      }),
      false,
    );
  });

  it("allows only the documented bearer placeholder", () => {
    assert.equal(
      containsProhibitedCredential({
        headers: { Authorization: "Bearer REPLACE_WITH_SCOPED_API_KEY" },
      }),
      false,
    );
    assert.equal(
      containsProhibitedCredential({
        headers: { Authorization: "Bearer synthetic-value" },
      }),
      true,
    );
  });

  it("fails closed on malformed JSON without echoing source text", () => {
    const marker = "synthetic-value-never-log";
    const violations = validateMcpConfigText(`{${marker}`, "fixture.json");
    assert.equal(violations.length, 1);
    assert.equal(violations[0].includes(marker), false);
    assert.match(violations[0], /invalid JSON/);
  });
});

describe("tracked MCP config Git scanner", () => {
  it("rejects a tracked matching config and reports no value", () => {
    const repo = tempRepo();
    const marker = "synthetic-value-never-log";
    write(
      repo,
      ".cursor/mcp.team.json",
      JSON.stringify({
        mcpServers: {
          scheduler: { env: { CHAOS_SCHEDULER_API_KEY: marker } },
        },
      }),
    );
    git(repo, "add", ".cursor/mcp.team.json");

    const violations = scanTrackedMcpConfigs(repo);
    assert.equal(violations.length, 1);
    assert.equal(violations.join("\n").includes(marker), false);
  });

  it("scans tracked examples but never reads an ignored local config", () => {
    const repo = tempRepo();
    write(repo, ".gitignore", "/.cursor/mcp.json\n");
    write(
      repo,
      ".cursor/mcp.remote.example.json",
      JSON.stringify({
        mcpServers: { scheduler: { url: "https://example.test" } },
      }),
    );
    write(
      repo,
      ".cursor/mcp.json",
      JSON.stringify({
        env: { CHAOS_SCHEDULER_API_KEY: "untracked-local-value" },
      }),
    );
    git(repo, "add", ".gitignore", ".cursor/mcp.remote.example.json");

    assert.deepEqual(scanTrackedMcpConfigs(repo), []);
  });

  it("fails closed for a missing tracked file", () => {
    const repo = tempRepo();
    write(repo, ".cursor/mcp.json", "{}");
    git(repo, "add", ".cursor/mcp.json");
    rmSync(join(repo, ".cursor/mcp.json"));

    assert.match(scanTrackedMcpConfigs(repo)[0], /missing or unreadable/);
  });

  it("fails closed for a tracked symlink", () => {
    if (process.platform === "win32") return;
    const repo = tempRepo();
    write(repo, "outside.json", "{}");
    mkdirSync(join(repo, ".cursor"), { recursive: true });
    symlinkSync("../outside.json", join(repo, ".cursor/mcp.json"));
    git(repo, "add", ".cursor/mcp.json");

    assert.match(scanTrackedMcpConfigs(repo)[0], /not a regular file/);
  });

  it("allows a staged deletion and documents the narrow path scope", () => {
    const repo = tempRepo();
    write(repo, ".cursor/mcp.json", "{}");
    git(repo, "add", ".cursor/mcp.json");
    rmSync(join(repo, ".cursor/mcp.json"));
    git(repo, "add", "-u");
    write(
      repo,
      ".cursor/other.json",
      JSON.stringify({ CHAOS_SCHEDULER_API_KEY: "out-of-scope" }),
    );
    git(repo, "add", ".cursor/other.json");

    assert.deepEqual(scanTrackedMcpConfigs(repo), []);
  });

  it("scans staged content independently from the working tree", () => {
    const repo = tempRepo();
    const path = ".cursor/mcp.json";
    write(
      repo,
      path,
      JSON.stringify({
        headers: { Authorization: "Bearer synthetic-staged-value" },
      }),
    );
    git(repo, "add", path);
    write(repo, path, "{}");

    assert.match(scanTrackedMcpConfigs(repo)[0], /\(staged\)/);
  });

  it("fails closed outside a Git repository", () => {
    const dir = mkdtempSync(join(tmpdir(), "chaos-mcp-no-git-"));
    tempRoots.push(dir);
    assert.match(
      scanTrackedMcpConfigs(dir)[0],
      /unable to enumerate tracked MCP configuration/,
    );
  });
});

describe("required CI wiring", () => {
  it("runs the guard unconditionally and requires an explicit success", () => {
    const ci = readFileSync(join(root, ".github/workflows/ci.yml"), "utf8");
    const job = /^  mcp-config-security:\n((?: {4,}.*\n|\s*\n)*)/m.exec(ci);
    assert.ok(job, "missing mcp-config-security job");
    assert.doesNotMatch(job[1], /^ {4}if:/m);
    assert.match(job[1], /npm run test:mcp-config-security/);
    assert.match(job[1], /npm run check:mcp-config-security/);
    assert.match(
      ci,
      /^\s+if \[\[ "\$\{\{ needs\['mcp-config-security'\]\.result \}\}" != "success" \]\]; then/m,
    );
  });
});
