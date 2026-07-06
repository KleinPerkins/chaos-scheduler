#!/usr/bin/env node
// Release smoke gate (updater UX plan, Section 7): after the desktop release
// uploads its assets and "Latest" is re-pinned, prove the *live* updater
// endpoint (https://…/releases/latest/download/latest.json) actually serves
// this release — status 200, `version` matches the desktop tag, and every
// platform entry carries both a signature and a download URL. This is the
// exact failure mode documented in docs/RELEASING.md ("multi-release Latest
// flag" pitfall): a green build + green "Latest" re-pin can still leave the
// endpoint transiently 404ing (GitHub CDN cache) or pointing at a stale
// version, and neither is caught by the build succeeding.
//
// Usage: node scripts/smoke-latest-json.mjs <expected-version> [endpoint-url]
const version = process.argv[2];
const endpoint =
  process.argv[3] ??
  "https://github.com/KleinPerkins/chaos-scheduler/releases/latest/download/latest.json";

if (!version) {
  console.error(
    "usage: node scripts/smoke-latest-json.mjs <expected-version> [endpoint-url]",
  );
  process.exit(1);
}

// The "Latest" flag re-pin can take a few minutes to propagate through
// GitHub's CDN (documented in docs/RELEASING.md), so retry with backoff
// instead of failing on the first transient 404/stale response.
const ATTEMPTS = 6;
const DELAY_MS = 20_000;

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function fetchLatestJson() {
  const res = await fetch(endpoint, { redirect: "follow" });
  if (!res.ok) {
    throw new Error(`HTTP ${res.status} fetching ${endpoint}`);
  }
  const body = await res.json();
  return body;
}

function assertManifestShape(manifest) {
  if (manifest.version !== version) {
    throw new Error(
      `latest.json version "${manifest.version}" does not match expected "${version}"`,
    );
  }
  const platforms = manifest.platforms ?? {};
  const entries = Object.entries(platforms);
  if (entries.length === 0) {
    throw new Error("latest.json has no platform entries");
  }
  for (const [platform, entry] of entries) {
    if (!entry.url) {
      throw new Error(`platform "${platform}" is missing a download url`);
    }
    if (!entry.signature) {
      throw new Error(`platform "${platform}" is missing a signature`);
    }
  }
  return entries.map(([platform]) => platform);
}

let lastError;
for (let attempt = 1; attempt <= ATTEMPTS; attempt++) {
  try {
    console.log(`Fetching ${endpoint} (attempt ${attempt}/${ATTEMPTS})…`);
    const manifest = await fetchLatestJson();
    const platforms = assertManifestShape(manifest);
    console.log(
      `OK  latest.json version=${manifest.version} platforms=[${platforms.join(", ")}] all carry url+signature.`,
    );
    process.exit(0);
  } catch (err) {
    lastError = err;
    console.warn(`Attempt ${attempt} failed: ${err.message}`);
    if (attempt < ATTEMPTS) {
      await sleep(DELAY_MS);
    }
  }
}

console.error(
  `::error::latest.json smoke check failed after ${ATTEMPTS} attempts: ${lastError?.message}`,
);
process.exit(1);
