# ADR 0003: SQLite migrations are append-only, transactional, and never mutated after shipping
Status: accepted — 2026-08-22

## Decision

Schema evolution uses an ordered list of per-version migrations (`migrations()` in
`src-tauri/src/db.rs`), each applied atomically inside a transaction that advances
`PRAGMA user_version`. A pre-migration backup is taken (and pruned); the app refuses to open a
database whose `user_version` is newer than the build understands. A shipped migration is never
renumbered or mutated — new schema changes are only ever appended as a higher-numbered migration.

## Why

- **Alternatives considered.** (a) An ORM auto-migrate / diff-the-schema tool — rejected: implicit
  destructive DDL against a local user database on a signed desktop app is unacceptable; we need every
  DDL step to be explicit, reviewed, and reversible-by-backup. (b) Editing an existing migration to
  "fix" it — rejected: users who already applied vN would silently diverge from users who get the
  edited vN. (c) Append-only numbered migrations with a backup and a newer-than-build guard — chosen.
- **Evidence.** The current schema is version 17, reached by a linear append-only chain (environments,
  spec, api-keys, queue, idempotency, drop-corpus, prod/sandbox, queue-occupancy, run-environment
  snapshot, the fix-agent tables, suppress-completion, fix-agent spend). Each step advances
  `user_version` transactionally with a backup.

## Consequences

- **Enables.** Safe forward migration of user databases across releases; deterministic recovery via
  the pre-migration backup; a hard stop when a downgraded build meets a newer DB.
- **Forecloses.** No edit or renumber of an already-released migration; no out-of-band DDL that skips
  the transactional, `user_version`-advancing path.
- **Invariant to keep true.** Migrations are append-only and monotonic; opening a newer-than-build DB
  is refused, not coerced.
