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
  // The always-on raw scan (Finding 4) means multiple detectors may fire for the
  // same key; assert it is flagged rather than pinning an exact reason count.
  assert.ok(reasons.length >= 1);
  assert.ok(reasons.some((r) => /CHAOS_SCHEDULER_API_KEY/.test(r)));
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

test("FINDING 4: FAILS a live key under an ALTERNATE env name in parseable JSON", () => {
  // A secret-shaped key that is NOT the exact managed key. Pre-fix, the
  // structural walk only checked `CHAOS_SCHEDULER_API_KEY` and the raw scan
  // never ran on parseable JSON, so this slipped through.
  const token = runtimeToken();
  const text = JSON.stringify({
    mcpServers: {
      "chaos-scheduler": {
        command: "node",
        env: {
          CHAOS_SCHEDULER_URL: "http://127.0.0.1:9618",
          SCHEDULER_API_KEY: token,
        },
      },
    },
  });
  const reasons = findSecretsInConfig(text);
  assert.ok(
    reasons.some((r) => /secret-shaped field/.test(r)),
    `alternate secret key name must be flagged, got ${JSON.stringify(reasons)}`,
  );
});

test("FINDING 4: FAILS a Bearer token in a FREE-TEXT field of parseable JSON", () => {
  // The bearer token is in a plain `note` field, not an `authorization` header,
  // and the JSON parses fine — so only the always-on raw scan can catch it.
  const token = runtimeToken();
  const text = JSON.stringify({
    mcpServers: {
      "chaos-scheduler": {
        command: "node",
        env: { CHAOS_SCHEDULER_URL: "http://127.0.0.1:9618" },
        note: `remember to send Bearer ${token} upstream`,
      },
    },
  });
  const reasons = findSecretsInConfig(text);
  assert.ok(
    reasons.some((r) => /Bearer/.test(r)),
    `bearer-in-free-text must be flagged, got ${JSON.stringify(reasons)}`,
  );
});

test("FINDING 4: FAILS a base64-wrapped value under a secret-shaped key", () => {
  // A base64 blob is still a non-placeholder secret; a secret-shaped key name
  // (SESSION_TOKEN) under any prefix must be flagged.
  const b64 = Buffer.from(`session-${runtimeToken()}`).toString("base64");
  const text = JSON.stringify({
    mcpServers: {
      "chaos-scheduler": { credentials: { SESSION_TOKEN: b64 } },
    },
  });
  const reasons = findSecretsInConfig(text);
  assert.ok(
    reasons.some((r) => /secret-shaped field/.test(r)),
    `base64 value under a secret-shaped key must be flagged, got ${JSON.stringify(reasons)}`,
  );
});

test("FINDING 4: non-secret managed keys and URLs still PASS", () => {
  // The always-on raw scan and the secret-shaped-key heuristic must not
  // false-positive on config-only managed keys with real values.
  const text = JSON.stringify({
    mcpServers: {
      "chaos-scheduler": {
        command: "/Users/x/Library/Application Support/mcp/launch-managed.sh",
        env: {
          CHAOS_SCHEDULER_URL: "https://scheduler.internal.example.com/api",
          CHAOS_SCHEDULER_MCP_PROTECTED_ENVIRONMENTS: "production,staging",
          CHAOS_SCHEDULER_MANAGED_BY: "Chaos Scheduler",
          CHAOS_SCHEDULER_MANAGED_ID: "b1c2d3e4-managed-id",
        },
      },
    },
  });
  assert.deepEqual(findSecretsInConfig(text), []);
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
