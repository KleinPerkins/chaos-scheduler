# Contributing to Chaos Scheduler

Thanks for contributing! This repo uses **SemVer + Conventional Commits** with
automated versioning/releases via [release-please](https://github.com/googleapis/release-please).
Please read this before opening a PR.

## Prerequisites / toolchain

- **Node** — version pinned in [`.nvmrc`](.nvmrc) / [`.node-version`](.node-version) (use `nvm use`).
- **Rust** — pinned in [`rust-toolchain.toml`](rust-toolchain.toml) (installed automatically by `rustup`).
- Editor: honor [`.editorconfig`](.editorconfig).

## Setup

```bash
npm install        # also runs `lefthook install` via the `prepare` script
```

This installs the local git hooks (see below). If hooks don't appear, run
`npx lefthook install`.

## Branching & PR flow

1. Branch off `main` (e.g. `feat/environments`, `fix/orphaned-runs`).
2. Commit using Conventional Commits (enforced locally + in CI).
3. Open a PR. The **PR title must also be a valid Conventional Commit** — it
   becomes the squash-merge commit and drives the release.
4. CI must be green (the `ci-required` check) and a CODEOWNER must approve.
5. Squash-merge to `main`. Direct pushes to `main` are blocked by branch protection.

## Conventional Commits

Format: `type(optional-scope): description`

| Type                                                                  | Release effect          |
| --------------------------------------------------------------------- | ----------------------- |
| `fix:`                                                                | patch (`0.1.0 → 0.1.1`) |
| `feat:`                                                               | minor (`0.1.0 → 0.2.0`) |
| `feat!:` / `fix!:` / `BREAKING CHANGE:` in body                       | major (`0.1.0 → 1.0.0`) |
| `chore` `docs` `refactor` `test` `ci` `build` `perf` `style` `revert` | no release              |

Examples:

```
feat(api): add environment CRUD endpoints
fix(scheduler): recover orphaned background runs on restart
feat!: rename CHAOS_LABS_* child env vars to CHAOS_SCHEDULER_*
```

See [docs/VERSIONING.md](docs/VERSIONING.md) for the full policy.

## Local hooks (lefthook)

Configured in [`lefthook.yml`](lefthook.yml). Fast, developer-side, and
**bypassable** (`git commit --no-verify`) — CI is the authoritative gate.

- **commit-msg** — `commitlint` (Conventional Commits).
- **pre-commit** — `prettier --write` + `eslint --fix` on staged JS/TS; `cargo fmt --check`
  and `cargo clippy -D warnings` when Rust files change.
- **pre-push** — `tsc` typecheck; `cargo test`.

> Prettier auto-formats staged files at commit time. There is intentionally no
> repo-wide prettier gate in CI yet because the legacy frontend predates
> formatting; files are brought to standard as they're touched.

## Remote enforcement (authoritative)

[`.github/workflows/ci.yml`](.github/workflows/ci.yml) runs on every PR:

- **frontend** — `eslint`, `tsc`, `vite build`
- **rust** — `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `cargo build`
- **commitlint** — validates every commit in the PR
- **ci-required** — aggregation job (`needs:` all others); **this is the single
  required status check** for branch protection on `main`.

## Running things locally

```bash
npm run dev          # Vite dev server (frontend)
npm run lint         # eslint
npm run typecheck    # tsc -b --noEmit
npm run format       # prettier --write across the repo
cargo test           # (run inside src-tauri/)
```
