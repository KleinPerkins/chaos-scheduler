#!/usr/bin/env node
// Data-guard: fail if the repository tracks a database snapshot or a secret file.
//
// The primary threat is a committed `scheduler.db` (the app's SQLite store, which
// integration tests write) or a secret file (`.env`, an Apple `.p12`, an SSH
// private key) reaching `main`. `.gitignore` and the lefthook pre-commit hook are
// the first line of defense, but BOTH are bypassed by the git-data-API
// single-commit PR flow this repo uses, and lefthook is `--no-verify`-bypassable —
// so this CI-required check is the authoritative gate. It scans the whole tracked
// tree (`git ls-files`) when run with no args (CI), or exactly the paths passed as
// args (the lefthook pre-commit hook passes the staged files).
//
// Matching is path-based and high-confidence only — no content heuristics, which
// are noisy and false-positive-prone (a dedicated secret scanner such as gitleaks
// is the right home for those). Add a legitimate, non-secret exception by its exact
// repo-relative path to a newline-separated `.data-guard-allow` file.
import { readFileSync, existsSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

// Each rule matches a repo-relative POSIX path and carries a human reason.
export const RULES = [
  {
    why: 'SQLite database (e.g. scheduler.db)',
    test: (p) => /\.(db|sqlite|sqlite3)$/.test(p) || /\.db-(wal|shm)$/.test(p),
  },
  {
    why: 'environment/secret file',
    test: (p) =>
      /(^|\/)\.env$/.test(p) ||
      (/(^|\/)\.env\.[^/]+$/.test(p) && !/\.(example|sample|template)$/.test(p)),
  },
  { why: 'code-signing material', test: (p) => /\.(p12|pfx|mobileprovision)$/.test(p) },
  { why: 'SSH private key', test: (p) => /(^|\/)id_(rsa|ed25519|ecdsa|dsa)$/.test(p) },
];

// Returns [{ path, why }] for every forbidden path, minus allowlisted paths.
export function findForbidden(paths, allow = new Set()) {
  const hits = [];
  for (const raw of paths) {
    const p = String(raw).trim();
    if (!p || allow.has(p)) continue;
    for (const rule of RULES) {
      if (rule.test(p)) {
        hits.push({ path: p, why: rule.why });
        break;
      }
    }
  }
  return hits;
}

function loadAllow() {
  const f = join(root, '.data-guard-allow');
  if (!existsSync(f)) return new Set();
  return new Set(
    readFileSync(f, 'utf8')
      .split('\n')
      .map((l) => l.replace(/#.*$/, '').trim())
      .filter(Boolean)
  );
}

function trackedFiles() {
  const out = execFileSync('git', ['ls-files', '-z'], { cwd: root, encoding: 'utf8' });
  return out.split('\0').filter(Boolean);
}

function main() {
  const args = process.argv.slice(2);
  const paths = args.length > 0 ? args : trackedFiles();
  const hits = findForbidden(paths, loadAllow());
  if (hits.length > 0) {
    console.error('::error::data-guard — refusing database/secret file(s) in the repository:');
    for (const h of hits) console.error(`  ${h.path}  — ${h.why}`);
    console.error(
      '\nThese must never be committed. If a match is a legitimate, non-secret file, add its exact ' +
        'repo-relative path to `.data-guard-allow`.'
    );
    process.exit(1);
  }
  console.log(`data-guard OK — scanned ${paths.length} path(s), no database/secret files.`);
}

// Run as a CLI only when invoked directly, so the test can import findForbidden.
if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main();
}
