#!/usr/bin/env node
// Fail-fast parity gate for the Tauri toolchain.
//
// The Rust `tauri` crate and the JavaScript `@tauri-apps/*` packages MUST share
// the same major.minor. When they drift, `tauri build` aborts with
// "Found version mismatched Tauri packages" — which is exactly what broke the
// 0.3.0 desktop release (crate 2.10 vs @tauri-apps/api 2.11). CI runs
// `cargo build` and the JS build, but never `tauri build`, so that class of
// failure only surfaced in the release job. This check reproduces the guard in
// milliseconds (pure lockfile reads, no compile) on every PR.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

function minor(v) {
  const m = /^(\d+)\.(\d+)\./.exec(v);
  if (!m) throw new Error(`unparseable version: ${v}`);
  return `${m[1]}.${m[2]}`;
}

function crateVersion(name) {
  const lock = readFileSync(join(root, 'src-tauri', 'Cargo.lock'), 'utf8');
  const re = new RegExp(`\\[\\[package\\]\\]\\nname = "${name}"\\nversion = "([^"]+)"`);
  const m = re.exec(lock);
  if (!m) throw new Error(`crate not found in Cargo.lock: ${name}`);
  return m[1];
}

function npmVersion(name) {
  const lock = JSON.parse(readFileSync(join(root, 'package-lock.json'), 'utf8'));
  const pkg = lock.packages?.[`node_modules/${name}`];
  if (!pkg?.version) throw new Error(`package not found in package-lock.json: ${name}`);
  return pkg.version;
}

const checks = {
  'tauri (rust crate)': crateVersion('tauri'),
  '@tauri-apps/api (js)': npmVersion('@tauri-apps/api'),
  '@tauri-apps/cli (js)': npmVersion('@tauri-apps/cli'),
};

const rows = Object.entries(checks).map(([k, v]) => [k, v, minor(v)]);
const distinct = new Set(rows.map(([, , mn]) => mn));
for (const [k, v, mn] of rows) console.log(`  ${k.padEnd(24)} ${v}  (minor ${mn})`);

if (distinct.size !== 1) {
  console.error(
    `\n::error::Tauri version mismatch — the Rust \`tauri\` crate and \`@tauri-apps/*\` must share a major.minor. ` +
      `Found minors: ${[...distinct].join(', ')}. This is what aborts \`tauri build\` in the release job. ` +
      `Fix by aligning them (e.g. \`cargo update -p tauri\` in src-tauri, or bump the @tauri-apps/* deps) so all read the same 2.x.`
  );
  process.exit(1);
}

console.log(`\nOK — all Tauri packages agree on minor ${[...distinct][0]}.`);
