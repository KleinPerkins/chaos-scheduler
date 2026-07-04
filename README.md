# Chaos Scheduler

[![CI](https://github.com/KleinPerkins/chaos-scheduler/actions/workflows/ci.yml/badge.svg)](https://github.com/KleinPerkins/chaos-scheduler/actions/workflows/ci.yml)
[![release-please](https://github.com/KleinPerkins/chaos-scheduler/actions/workflows/release-please.yml/badge.svg)](https://github.com/KleinPerkins/chaos-scheduler/actions/workflows/release-please.yml)
[![Latest release](https://img.shields.io/github/v/release/KleinPerkins/chaos-scheduler?sort=semver)](https://github.com/KleinPerkins/chaos-scheduler/releases/latest)
[![Download](https://img.shields.io/github/downloads/KleinPerkins/chaos-scheduler/total)](https://github.com/KleinPerkins/chaos-scheduler/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

A local-first **workflow scheduler** desktop/tray app — a Tauri 2 (Rust) backend
with a React frontend, backed by SQLite. It runs scheduled and on-demand
workflows, tracks runs/queues/telemetry, and surfaces everything in a Mission
Control UI.

> Roadmap: the scheduler is being made standalone with first-class
> **environments**, a REST API + webhooks + TypeScript SDK, a generic/typed
> workflow model with in-scheduler step-flow and operators, on-success/on-failure
> actions, a bidirectional **Cursor** integration (MCP server + Cloud Agents
> operator), and a signed installer with auto-update. See the project plan.

## Tech stack

- **Backend:** Rust + [Tauri 2](https://tauri.app/) (`src-tauri/`)
- **Frontend:** React 19 + TypeScript + Vite (`src/`)
- **Storage:** SQLite

## Development

Toolchains are pinned — Node via [`.nvmrc`](.nvmrc) / [`.node-version`](.node-version),
Rust via [`rust-toolchain.toml`](rust-toolchain.toml).

```bash
nvm use            # or fnm/asdf — reads .nvmrc
npm install        # installs deps + git hooks (lefthook, via the prepare script)
npm run dev        # Vite dev server
npm run lint       # eslint
npm run typecheck  # tsc -b --noEmit
```

Run the full desktop app with the Tauri CLI (`npx tauri dev`), and Rust checks
from `src-tauri/` (`cargo fmt`, `cargo clippy`, `cargo test`).

## Contributing & governance

This repo uses **SemVer + Conventional Commits** with automated releases.

- [CONTRIBUTING.md](CONTRIBUTING.md) — branching, commit conventions, PR flow, hooks
- [docs/VERSIONING.md](docs/VERSIONING.md) — SemVer policy (app / API / SDK / MCP / DB)
- [docs/RELEASING.md](docs/RELEASING.md) — release runbook + external prerequisites
- [SECURITY.md](SECURITY.md) — vulnerability reporting

Commit and PR titles must follow
[Conventional Commits](https://www.conventionalcommits.org/); they drive
versioning via [release-please](https://github.com/googleapis/release-please).
CI (`ci-required`) and branch protection are the authoritative gates.

## License

[MIT](LICENSE) © 2026 Laurence Duggan (KleinPerkins)
