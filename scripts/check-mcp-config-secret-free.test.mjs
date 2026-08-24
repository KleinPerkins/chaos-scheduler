import { test } from "node:test";
import assert from "node:assert/strict";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";
import {
  isScannableMcpConfig,
  findSecretsInConfig,
  findViolations,
} from "./check-mcp-config-secret-free.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

// A run-unique, credential-shaped value derived purely from runtime integers, so
// no secret scanner (gitleaks) or literal ever lands in this test source.
function runtimeToken() {
  const nanos = process.hrtime.bigint().toString(16);
  return `sk_live_${nanos}${Math.random().toString(16).slice(2)}`;
}

test("scopes to tracked .cursor/**/mcp*.json, excluding *.example.json", () => {
  assert.equal(isScannableMcpConfig(".cursor/mcp.json"), true);
  assert.equal(isScannableMcpConfig(".cursor/mcp.remote.json"), true);
  assert.equal(isScannableMcpConfig(".cursor/sub/mcp.local.json"), true);
  // Example files are excluded (they legitimately carry placeholders).
  assert.equal(isScannableMcpConfig(".cursor/mcp.example.json"), false);
  assert.equal(isScannableMcpConfig(".cursor/mcp.remote.example.json"), false);
  // Out of scope.
  assert.equal(isScannableMcpConfig("mcp.json"), false);
  assert.equal(isScannableMcpConfig(".cursor/rules/foo.json"), false);
  assert.equal(isScannableMcpConfig("packages/mcp-server/config.json"), false);
});

test("FAILS a config carrying a real inline CHAOS_SCHEDULER_API_KEY", () => {
  const token = runtimeToken();
  const text = JSON.stringify({
    mcpServers: {
      "chaos-scheduler": {
        command: "node",
        env: {
          CHAOS_SCHEDULER_URL: "http://127.0.0.1:9618",
          CHAOS_SCHEDULER_API_KEY: token,
        },
      },
    },
  });
  const reasons = findSecretsInConfig(text);
  assert.equal(reasons.length, 1);
  assert.match(reasons[0], /CHAOS_SCHEDULER_API_KEY/);
});

test("FAILS a config carrying a real Authorization: Bearer token", () => {
  const token = runtimeToken();
  const text = JSON.stringify({
    mcpServers: {
      "chaos-scheduler": {
        url: "https://host.example.com/mcp",
        headers: { Authorization: `Bearer ${token}` },
      },
    },
  });
  const reasons = findSecretsInConfig(text);
  assert.ok(reasons.some((r) => /Authorization\/Bearer/.test(r)));
});

test("FAILS even when the JSON is unparseable (raw fallback)", () => {
  const token = runtimeToken();
  const text = `{ "mcpServers": { "chaos-scheduler": { "env": { "CHAOS_SCHEDULER_API_KEY": "${token}" }  <<< broken ::::`;
  const reasons = findSecretsInConfig(text);
  assert.ok(reasons.some((r) => /CHAOS_SCHEDULER_API_KEY/.test(r)));
});

test("PASSES placeholders and empty values (onboarding examples)", () => {
  for (const value of [
    "",
    "REPLACE_WITH_SCOPED_API_KEY",
    "<your-key>",
    "{{task}}",
    "changeme",
  ]) {
    const text = JSON.stringify({
      mcpServers: {
        "chaos-scheduler": { env: { CHAOS_SCHEDULER_API_KEY: value } },
      },
    });
    assert.deepEqual(
      findSecretsInConfig(text),
      [],
      `placeholder ${JSON.stringify(value)}`,
    );
  }
  // The committed remote example's Bearer placeholder must also pass.
  const remoteExample = JSON.stringify({
    mcpServers: {
      "chaos-scheduler": {
        headers: { Authorization: "Bearer REPLACE_WITH_SCOPED_API_KEY" },
      },
    },
  });
  assert.deepEqual(findSecretsInConfig(remoteExample), []);
});

test("findViolations only reports in-scope files and uses injected reader", () => {
  const token = runtimeToken();
  const read = (p) => {
    if (p === ".cursor/mcp.json") {
      return JSON.stringify({
        mcpServers: {
          "chaos-scheduler": { env: { CHAOS_SCHEDULER_API_KEY: token } },
        },
      });
    }
    if (p === ".cursor/mcp.example.json") {
      return JSON.stringify({
        mcpServers: {
          "chaos-scheduler": {
            env: { CHAOS_SCHEDULER_API_KEY: "REPLACE_WITH_SCOPED_API_KEY" },
          },
        },
      });
    }
    throw new Error(`unexpected read: ${p}`);
  };
  const hits = findViolations(
    [".cursor/mcp.json", ".cursor/mcp.example.json", "README.md"],
    read,
  );
  assert.equal(hits.length, 1);
  assert.equal(hits[0].path, ".cursor/mcp.json");
});

test("the committed repository tree is secret-free (authoritative)", () => {
  // Scans the real tracked tree via `git ls-files`. `.cursor/mcp.json` must be
  // untracked (git-ignored), so only the example files remain — all placeholders.
  const tracked = execFileSync("git", ["ls-files", "-z"], {
    cwd: root,
    encoding: "utf8",
  })
    .split("\0")
    .filter(Boolean);
  const hits = findViolations(tracked);
  assert.deepEqual(
    hits,
    [],
    `tracked MCP configs must be secret-free, got ${JSON.stringify(hits)}`,
  );
  // And the project-local mcp.json must NOT be tracked.
  assert.ok(
    !tracked.includes(".cursor/mcp.json"),
    ".cursor/mcp.json must be untracked (git-ignored) — the managed key lives in the Keychain",
  );
});
