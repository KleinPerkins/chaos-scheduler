import { test } from "node:test";
import assert from "node:assert/strict";
import { findForbidden } from "./check-no-committed-data.mjs";

test("rejects scheduler.db and other SQLite databases (incl. WAL/SHM sidecars)", () => {
  const hits = findForbidden([
    "scheduler.db",
    "src-tauri/scheduler.db",
    "data/app.sqlite",
    "x.sqlite3",
    "scheduler.db-wal",
    "scheduler.db-shm",
  ]);
  assert.equal(hits.length, 6);
});

test("rejects env files but allows .env.example/.sample/.template", () => {
  assert.equal(findForbidden([".env"]).length, 1);
  assert.equal(findForbidden([".env.local"]).length, 1);
  assert.equal(findForbidden(["packages/x/.env.production"]).length, 1);
  assert.equal(
    findForbidden([".env.example", ".env.sample", "config/.env.template"])
      .length,
    0,
  );
});

test("rejects code-signing material and SSH private keys", () => {
  const hits = findForbidden([
    "certs/dist.p12",
    "a.pfx",
    "app.mobileprovision",
    ".ssh/id_rsa",
    "keys/id_ed25519",
  ]);
  assert.equal(hits.length, 5);
});

test("passes clean source/doc paths", () => {
  const hits = findForbidden([
    "src/components/RunDetail.tsx",
    "docs/plans/README.md",
    "package.json",
    "README.md",
    "src-tauri/src/db.rs",
    "e2e/visual/__screenshots__/overview-linux.png",
  ]);
  assert.equal(hits.length, 0);
});

test("honors the allowlist for a legitimate non-secret path", () => {
  const allow = new Set(["fixtures/seed.sqlite"]);
  assert.equal(findForbidden(["fixtures/seed.sqlite"], allow).length, 0);
});
