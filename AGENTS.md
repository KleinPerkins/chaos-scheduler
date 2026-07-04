## Learned User Preferences

- chaos-scheduler is primarily a personal app for the user and a few friends; skip enterprise-grade hardening/steps that add no functional value at that scale unless there is a specific loss of functionality (e.g. skipped paid Apple Developer signing/notarization).
- Prefer the GitHub REST API over local git for repo/file changes when the working tree at ~/dev/personal/chaos-scheduler must stay undisturbed.
- Solve "owner can't self-approve their own PR" by using a dedicated GitHub App auto-merge bot to satisfy the branch-protection review gate, rather than lowering the required-review gate.
- Chose a passwordless Tauri minisign signing key; do releases/updates unattended without a key password prompt.

## Learned Workspace Facts

- chaos-scheduler (KleinPerkins/chaos-scheduler, public) is a Tauri 2 desktop/tray app — Rust backend (src-tauri/) + React/TypeScript frontend (src/) with SQLite persistence — being decoupled and rebranded off of chaos-labs.
- Core rearchitecture: replace the overloaded `corpus` ("source"|"instance") with `environment` plus a separate `managed_externally` flag; extract a single `SchedulerService` business-logic boundary with Tauri IPC, REST `/api/v1`, SDK, and MCP as thin adapters; provide bidirectional Cursor integration (MCP server + Cursor Cloud Agents).
- Release/update pipeline: Tauri v2 auto-updater signed with a minisign key at ~/.tauri/chaos-scheduler.key (public key lives in src-tauri/tauri.conf.json); a `release` GitHub Environment holds TAURI_SIGNING_PRIVATE_KEY (+ _PASSWORD); `latest.json` served via GitHub Releases. Losing the private key permanently breaks auto-update for installed users.
- `main` is protected by ruleset 18513148: PR required, strict `ci-required` status check, linear history, non-fast-forward, `required_approving_review_count: 1`, `require_code_owner_review: false`, empty bypass_actors.
- Auto-merge runs via a GitHub App bot (`chaos-scheduler-automerge`) using repo secrets AUTOMERGE_APP_ID + AUTOMERGE_APP_PRIVATE_KEY (.github/workflows/app-auto-merge.yml); the App's approval satisfies the review gate. The interim GITHUB_TOKEN-based auto-merge.yml was retired.
- Git hooks are active via lefthook: commit-time prettier + commitlint (Conventional Commits required; never use --no-verify); pre-push runs tsc + cargo test, filtered by changed-file globs.
- `gh` is authenticated for KleinPerkins/chaos-scheduler.
