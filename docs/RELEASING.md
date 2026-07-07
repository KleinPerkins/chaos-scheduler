# Releasing Chaos Scheduler

This is the release runbook. Releases are automated by
[release-please](https://github.com/googleapis/release-please) plus a
build/sign/publish [`release.yml`](../.github/workflows/release.yml) reusable
workflow (macOS desktop signing + notarization, Tauri updater publishing, and npm
publish for the SDK / MCP server).

## Release flow (overview)

```
Conventional-commit PRs → merge to main
        ↓
release-please opens/updates a "Release PR" (bumps versions + CHANGELOGs)
        ↓
merge the Release PR  →  git tag(s) + GitHub Release(s) created
        ↓
release-please.yml calls release.yml (same run) gated on release-please outputs
        ↓
  1. publish-sdk    → npm publish @chaos-scheduler/sdk (provenance)
        ↓ (needs: success or skipped)
  2. publish-mcp    → npm publish @chaos-scheduler/mcp-server (rewrites the
                       file:../sdk-ts dev dependency to the published SDK
                       semver first)
        ↓ (needs: success or skipped)
  3. mcp-consumer-smoke → clean `npm install --prefix <tmp>` of the exact
                       pinned mcp-server version under Node 18 (the package's
                       documented floor); asserts the SDK resolved from the
                       npm registry (not file:) and the installed CLI runs
        ↓ (needs: success — only if a desktop build is requested)
  4. build-macos    → stamps that smoke-tested mcp-server version into
                       src-tauri/mcp-pinned-version.txt, then tauri-action
                       builds universal macOS → codesign + notarize + minisign,
                       uploads signed .dmg + .app.tar.gz + .sig + latest.json,
                       then re-pins GitHub "Latest" to this release
  5. guard-latest-for-package-only-release → if this run published only npm
                       packages (no desktop bump), re-pins "Latest" back to
                       the most recent desktop release if a component release
                       stole it
```

Release ordering invariant: **sdk-ts → mcp-server → consumer-install smoke →
desktop pin/build**. This exists because the desktop app is (or will become)
the lifecycle owner of a managed, npm-provisioned MCP/SDK integration — see
"Release ordering + package-installability gate (managed MCP/SDK)" below — so
it must never advertise or pin a `mcp-server` version that is not actually
installable by a clean npm consumer. The ordering is enforced by job `needs:`
in [`release.yml`](../.github/workflows/release.yml), not by convention.

## How versioning triggers a release

1. Merge normal PRs to `main`. The [`release-please`](../.github/workflows/release-please.yml)
   workflow keeps a **Release PR** up to date, computing the next version from the
   Conventional Commits since the last release.
2. When ready to ship, **merge the Release PR**. release-please then:
   - updates `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`,
     `packages/*/package.json`, and per-component `CHANGELOG.md`;
   - creates the git tag(s) and the GitHub Release(s).

## Gating the downstream release build (important)

`release.yml` is a **reusable workflow** invoked by `release-please.yml` in the
**same run** (`uses: ./.github/workflows/release.yml`), gated on release-please's
outputs. This avoids the "a `GITHUB_TOKEN`-created release cannot trigger another
workflow" limitation without needing a PAT/GitHub App token.

### Release gating (verified against the v1.0.0 and v1.0.1 run outputs)

release-please-action prefixes outputs by **manifest path**, with the root path
`.` un-prefixed. The gate uses each component's **`tag_name`** output — empty
when that component did not release this run, the tag when it did:

| Component           | Path                  | Gating output key (`*_tag_name`)              |
| ------------------- | --------------------- | --------------------------------------------- |
| Desktop app (root)  | `.`                   | `tag_name` (root path is **un-prefixed**)     |
| Rust crate (linked) | `src-tauri`           | linked to the desktop app; not gated directly |
| TypeScript SDK      | `packages/sdk-ts`     | `packages/sdk-ts--tag_name`                   |
| MCP server          | `packages/mcp-server` | `packages/mcp-server--tag_name`               |

> **Why `tag_name`, not `release_created` (2026-07-07 fix):** the v1.0.1 release
> created `chaos-scheduler-v1.0.1` + `mcp-server-v1.0.1`, yet the per-path
> `*--release_created` booleans did **not** evaluate to `'true'` when consumed as
> job outputs, so `release.yml` and every build/publish job silently **skipped** —
> shipping no DMG/`latest.json` and publishing nothing to npm (a broken auto-updater
> 404). The `*_tag_name` outputs tracked exactly which components released in both
> the v1.0.0 and v1.0.1 runs, so the gate now keys off `<component>_tag_name != ''`.
> `release-please.yml` also exposes `releases_created` / `paths_released` and dumps
> `toJSON(steps.release.outputs)` in a "Debug release-please outputs" step, so any
> future skip regression is root-causable straight from the release-please job log.

The desktop build gates on the root `tag_name`; each npm publish gates on its
`packages/*--tag_name`. If nothing was released, `release.yml` is not called.

## Desktop build, signing & notarization

`release.yml` → `build-macos` job (runs in the `release` Environment):

- Builds a **universal macOS** binary (`--target universal-apple-darwin`; both
  `aarch64-apple-darwin` and `x86_64-apple-darwin` rust targets installed) via
  `tauri-apps/tauri-action`.
- **Codesigns + notarizes** with **hardened runtime** using the Apple secrets
  below. (The hardened-runtime entitlements file + `bundle.macOS` config live in
  `src-tauri/` and are owned by the desktop worker.)
- Attaches artifacts to the Release release-please created (`releaseId` resolved
  from the tag via `gh release view`).

## Updater publishing (Tauri + minisign)

Auto-update uses a **GitHub Releases `latest.json` endpoint**:
`https://github.com/KleinPerkins/chaos-scheduler/releases/latest/download/latest.json`.

- The **app side** (`src-tauri/tauri.conf.json`, owned by the desktop worker):
  `bundle.createUpdaterArtifacts: true`, `plugins.updater.pubkey` = the minisign
  **public** key, `plugins.updater.endpoints` = the `latest.json` URL above.
- The **CI side** (`release.yml`): the minisign **private** key
  (`TAURI_SIGNING_PRIVATE_KEY` + `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) signs the
  update artifacts; `tauri-action` (`includeUpdaterJson: true`) generates and
  uploads `latest.json` (plus `.app.tar.gz` + `.sig`) to the Release. The public
  key in the app must correspond to this private key or updates fail signature
  verification.
- Generate the keypair once with `npx tauri signer generate -w ~/.tauri/chaos-scheduler.key`.

### Pitfall: multi-release "Latest" flag (auto-update 404)

The updater endpoint resolves through GitHub's `/releases/latest/download/`
redirect, which serves assets from whichever release currently holds the
repo-wide **"Latest"** flag. But release-please creates **several releases per
version** in one batch — the desktop root `chaos-scheduler-v<version>` (the only
one carrying `latest.json` + `.app.tar.gz`), its linked
`chaos-scheduler-tauri-v<version>`, and the independent `sdk-ts-v*` /
`mcp-server-v*` — and GitHub marks the **last-created** non-prerelease release
"Latest". When that lands on an asset-less component release, the endpoint 404s
and auto-update silently breaks. This bit `v0.3.1` (`chaos-scheduler-tauri-v0.3.1`
grabbed "Latest") and was recovered with `gh release edit chaos-scheduler-v0.3.1
--latest` (GitHub CDN-caches the redirect, so a manual re-pin can take a few
minutes to propagate).

**Fix (automated):** the `build-macos` job's final step re-pins "Latest" to the
desktop release **after** its assets upload, so every desktop release ends with
the updater endpoint resolvable:

```sh
gh release edit chaos-scheduler-v<version> --latest   # --repo, tag from inputs.desktop_tag
```

This is version-agnostic (unlike a release-please `prerelease: true` flag on the
component packages, which only excludes them from "Latest" while versions are
**pre-1.0.0** and would silently stop working at `1.0.0`). If the endpoint ever
404s despite a green build, verify with `gh release list --json tagName,isLatest`
and re-run the one-liner above (idempotent).

**Residual edge case (now automated):** a release that bumps **only**
`sdk-ts` / `mcp-server` (no desktop bump) doesn't run `build-macos`, so a
component release could transiently hold "Latest" until the next desktop
release. The `guard-latest-for-package-only-release` job in
[`release.yml`](../.github/workflows/release.yml) runs for exactly this case
(`!build_desktop && (publish_sdk || publish_mcp)`) and re-pins "Latest" back
to the most recent `chaos-scheduler-v*` release if it drifted. It is a no-op
(and needs no secrets) when "Latest" already points at the desktop release.

### `latest.json` release smoke check

A green build + a green "Latest" re-pin still don't prove the **live**
updater endpoint actually works — the pitfall above shows a passing release
can leave `/releases/latest/download/latest.json` 404ing or serving a stale
version (GitHub CDN-caches the redirect for a few minutes after a re-pin).
`build-macos`'s last step, [`scripts/smoke-latest-json.mjs`](../scripts/smoke-latest-json.mjs),
fetches that exact URL after the "Latest" pin and asserts:

- the request returns **HTTP 200**;
- the manifest's `version` matches the desktop tag just released;
- every `platforms.*` entry has both a `url` and a `signature` (i.e. the
  updater has something installable and verifiable to offer).

It retries with backoff (6 attempts, 20s apart) to ride out the CDN-cache
propagation delay before failing the job. This is the release-side half of
the in-app updater UX (`useAppUpdate()` + the dashboard/tray affordances) —
it exists to catch exactly the failure mode that would otherwise leave every
installed user silently stuck on an old version with no error to look at.

## Release ordering + package-installability gate (managed MCP/SDK)

The desktop app is the lifecycle owner of a managed, npm-provisioned
`@chaos-scheduler/mcp-server` (+ its `@chaos-scheduler/sdk` dependency)
integration — it calls out to npm at runtime rather than bundling those bytes
into the DMG (see [`packages/INTEGRATION.md`](../packages/INTEGRATION.md) for
the wire protocol; the app-side provisioner lands as its own change). That
only works if the desktop build never pins a version that turns out not to be
cleanly installable. `release.yml` enforces this with three additions ahead of
`build-macos`:

1. **`publish-sdk` → `publish-mcp` ordering.** `publish-mcp` declares
   `needs: [publish-sdk]` and only runs once `publish-sdk` has succeeded or
   was skipped (a release that doesn't touch the SDK still lets mcp-server
   publish). This matches the existing in-job dependency (`publish-mcp`
   rewrites `@chaos-scheduler/sdk` to the just-published semver before
   `npm publish`) with an explicit job-graph guarantee, not just step order
   inside one job.
2. **`mcp-consumer-smoke`.** A new job that resolves the target `mcp-server`
   version from `packages/mcp-server/package.json` at the relevant tag
   (the `mcp-server` release tag if one exists this run, else the desktop
   tag — since this is a monorepo, that file always reflects the last
   actually-published version even on a desktop-only release), then runs
   [`scripts/smoke-mcp-install.mjs`](../scripts/smoke-mcp-install.mjs) under
   **Node 18** (the package's documented `engines.node` floor, not whatever
   newer version `.nvmrc` pins for everyday CI). That script does a clean
   `npm install --prefix <tmp>` of the exact pinned version, asserts from the
   resulting lockfile that `@chaos-scheduler/sdk` resolved from
   `registry.npmjs.org` (not a leaked `file:../sdk-ts` link), and runs the
   installed CLI's `--help` to prove it executes. This job runs whenever a
   desktop build needs a version to pin, or whenever `mcp-server` was freshly
   published (so a package-only release is validated too, independent of any
   desktop build).
3. **Stamp only after the smoke gate passes.** `build-macos` declares
   `needs: [mcp-consumer-smoke]` and only runs if that job succeeded. Its
   first build step overwrites the checked-in
   `src-tauri/mcp-pinned-version.txt` with the smoke-tested version, in the
   ephemeral CI checkout only (never committed back to git). The desktop app's
   managed-integration provisioner embeds that file at compile time (via
   `include_str!`), so the shipped binary's "pinned MCP version" — the exact
   version it installs — is always a version this gate already proved
   installable. Local/dev builds fall back to whatever value is checked into
   the file (kept roughly in sync with the last-released `mcp-server`
   version; harmless if stale since dev builds are never distributed).

## npm publishing (SDK + MCP server)

`release.yml` → `publish-sdk` / `publish-mcp` jobs (in the `release` Environment,
`id-token: write` for **npm provenance**, `NODE_AUTH_TOKEN` = `NPM_TOKEN`):

- Each package sets `publishConfig: { access: public, provenance: true }`, so
  publishes are public with a provenance attestation.
- **`file:../sdk-ts` rewrite:** `packages/mcp-server/package.json` depends on the
  SDK via `file:../sdk-ts` for local dev. The `publish-mcp` job builds mcp-server
  against the local SDK, then **rewrites that dependency to the published semver**
  (`^<sdk version>`, read from `packages/sdk-ts/package.json`) immediately before
  `npm publish`, so the tarball on npm depends on the real published package.
  Building against the local SDK first avoids any npm-registry propagation race.

> **Provenance + private repo caveat:** `npm publish --provenance` requires the
> source repository to be **public** (the attestation is written to a public
> transparency log). While `KleinPerkins/chaos-scheduler` is private, provenance
> publishes will fail — either make the repo public before the first npm release,
> or temporarily drop `--provenance` / `publishConfig.provenance`.

## Secrets & prerequisites checklist

These are **manual, one-time** steps a repo admin / release owner must complete
outside this repo (the workflows cannot create them). Scope every secret below to
the **`release` GitHub Environment** so it is only exposed to approved release runs.

### GitHub `release` Environment

- [ ] Create an Environment named **`release`** (Settings → Environments) with
      **required reviewers**. All `release.yml` jobs (`build-macos`, `publish-sdk`,
      `publish-mcp`) run in it and pause for approval.

### Apple codesigning + notarization (desktop)

- [ ] **Apple Developer Program** enrollment (needed for a Gatekeeper-trusted DMG).
- [ ] `APPLE_CERTIFICATE` — base64 of the Developer ID Application `.p12`.
- [ ] `APPLE_CERTIFICATE_PASSWORD` — password for that `.p12`.
- [ ] `APPLE_SIGNING_IDENTITY` — e.g. `Developer ID Application: Name (TEAMID)`.
- [ ] `APPLE_TEAM_ID` — your Apple Developer Team ID.
- [ ] `APPLE_ID` — Apple ID email used for notarization.
- [ ] `APPLE_PASSWORD` — an **app-specific password** for that Apple ID.
      (tauri-action also supports an App Store Connect API key via
      `APPLE_API_ISSUER` / `APPLE_API_KEY_ID` / `APPLE_API_KEY` as an alternative
      to `APPLE_ID` + `APPLE_PASSWORD`.)

### Tauri updater minisign key (auto-update)

- [ ] Generate the keypair once: `npx tauri signer generate -w ~/.tauri/chaos-scheduler.key`.
- [ ] `TAURI_SIGNING_PRIVATE_KEY` — the generated **private** key.
- [ ] `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — its password.
- [ ] Put the matching **public** key in `src-tauri/tauri.conf.json`
      (`plugins.updater.pubkey`) — owned by the desktop worker; it must correspond
      to the private key above or updates fail verification.
- [ ] **Back up the private key in a secret manager.** Losing it permanently
      breaks auto-update for every installed user. Never commit it (`.env` files
      are not read by Tauri).

### npm publishing (SDK + MCP server)

- [ ] `NPM_TOKEN` — an npm **automation** token with publish rights to the
      `@chaos-scheduler` scope (create the org/scope on npm first).
- [ ] Provenance requires **`id-token: write`** (already set on the publish jobs)
      **and a public source repo** — see the provenance caveat above.

### Other ecosystem prerequisites

- [ ] **GitHub admin / branch protection** on `main` (see below).
- [ ] **Cursor service account** — a Cursor service-account API key (for the
      Phase 8 `cursor_agent` operator / Cloud Agents). Stored in scheduler
      settings, not in CI, but listed here for completeness.

### Secrets summary

| Secret                               | Used by           | Purpose                             |
| ------------------------------------ | ----------------- | ----------------------------------- |
| `APPLE_CERTIFICATE`                  | `build-macos`     | Developer ID cert (.p12, base64)    |
| `APPLE_CERTIFICATE_PASSWORD`         | `build-macos`     | Cert password                       |
| `APPLE_SIGNING_IDENTITY`             | `build-macos`     | Signing identity string             |
| `APPLE_TEAM_ID`                      | `build-macos`     | Apple Team ID                       |
| `APPLE_ID`                           | `build-macos`     | Notarization Apple ID               |
| `APPLE_PASSWORD`                     | `build-macos`     | App-specific password               |
| `TAURI_SIGNING_PRIVATE_KEY`          | `build-macos`     | Updater artifact signing (minisign) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | `build-macos`     | Updater key password                |
| `NPM_TOKEN`                          | `publish-sdk/mcp` | npm publish auth                    |
| `GITHUB_TOKEN` (built-in)            | all               | Upload assets / create Release PR   |

## Branch protection (GitHub admin — must be set manually)

Configure on `main` (Settings → Branches → Add rule / Rulesets):

- [ ] Require a pull request before merging (**1** approval).
- [ ] Require review from **Code Owners** (uses [`.github/CODEOWNERS`](../.github/CODEOWNERS)).
- [ ] Require status checks to pass → select **`ci-required`** (the single
      aggregation check) and require branches be up to date.
- [ ] Require linear history; block force-pushes and deletions of `main`.
- [ ] (Optional) Require signed commits.

## Rollback

- **Bad release:** create a `fix:` (or `revert:`) commit; merge → release-please
  ships a new patch. Prefer roll-forward over deleting tags.
- **Broken auto-update:** if a published `latest.json` points at a bad build,
  publish a corrected higher version; the updater only moves forward. Do not
  reuse a version number.
- **DB migration regression:** migrations are forward-only and idempotent (see
  [VERSIONING.md](VERSIONING.md)); a regression is fixed by a new release that
  repairs state on next launch, not by downgrading.

## First public release note

The bundle identifier changes from `com.chaoslabs.scheduler` to the new Chaos
Scheduler identifier (Phase 2). Pre-existing installs will **not** auto-update
across the identifier change; the first release is a fresh install and the
Phase 2 legacy-DB migration preserves user data. Finalize branding before the
first public release.
