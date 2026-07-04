# Versioning policy

Chaos Scheduler follows [Semantic Versioning 2.0.0](https://semver.org/) driven
by [Conventional Commits](https://www.conventionalcommits.org/) and automated by
[release-please](https://github.com/googleapis/release-please).

## Single source of truth

Versions are owned by release-please via
[`.release-please-manifest.json`](../.release-please-manifest.json). On release,
release-please updates every version location in one Release PR:

| Location                                   | Component               | Type |
| ------------------------------------------ | ----------------------- | ---- |
| `package.json` (`version`)                 | `chaos-scheduler`       | node |
| `src-tauri/Cargo.toml` (`package.version`) | `chaos-scheduler-tauri` | rust |
| `src-tauri/tauri.conf.json` (`$.version`)  | (extra-file of root)    | json |
| `packages/sdk-ts` (`package.json`)         | `sdk-ts`                | node |
| `packages/mcp-server` (`package.json`)     | `mcp-server`            | node |

The desktop-app components (`chaos-scheduler` + `chaos-scheduler-tauri`) are kept
in **lockstep** via the release-please `linked-versions` plugin, and
`tauri.conf.json` is bumped as an `extra-files` JSON updater on the root
component. **Do not hand-edit versions** — let the Release PR do it. The current
reconciled baseline is `0.1.0` across all desktop-app version files.

> `packages/sdk-ts` and `packages/mcp-server` are **pre-declared** in
> [`release-please-config.json`](../release-please-config.json) so they're ready
> to release independently. They activate once their `package.json` is scaffolded
> (Phases 7–8); until then release-please treats them as not-yet-created.

## SemVer contract

- **Desktop app** — user-facing behavior + persisted schema.
  - MAJOR: breaking UX/config removal, or a non-idempotent DB migration.
  - MINOR: new features, backward-compatible schema migrations.
  - PATCH: bug fixes.
- **REST API** — versioned independently under the `/v1` path prefix. Additive
  changes are MINOR; removing/renaming a field or endpoint is MAJOR (new path
  prefix). Clients should check `GET /version` for the app version and rely on
  the path prefix for API compatibility.
- **TypeScript SDK (`sdk-ts`)** — SemVer against the public SDK surface; tracks
  the API it wraps.
- **MCP server (`mcp-server`)** — SemVer against its tool/resource/prompt surface.
- **Database** — migrations in `Database::init` must be **idempotent and
  forward-only**. A migration that can lose data is a MAJOR change and must be
  called out in the changelog and `docs/RELEASING.md`.

## Changelogs

release-please maintains per-component `CHANGELOG.md` files from commit history.
`ci`/`chore` commits are hidden from the changelog (see the `changelog-sections`
in the config).
