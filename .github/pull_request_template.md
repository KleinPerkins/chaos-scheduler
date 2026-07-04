<!--
PR titles MUST follow Conventional Commits (they become the squash-merge commit
and drive release-please version bumps), e.g.:
  feat(api): add environment CRUD endpoints
  fix(scheduler): recover orphaned background runs
  feat!: rename CHAOS_LABS_* env vars (BREAKING CHANGE)
-->

## Summary

<!-- What does this change do and why? -->

## Type of change

- [ ] `fix` — bug fix (patch)
- [ ] `feat` — new feature (minor)
- [ ] breaking change (`!` / `BREAKING CHANGE:` — major)
- [ ] `chore` / `docs` / `refactor` / `test` / `ci` / `build` (no release)

## Checklist

- [ ] PR title follows [Conventional Commits](https://www.conventionalcommits.org/)
- [ ] `npm run lint`, `npm run typecheck`, and `cargo fmt/clippy/test` pass locally
- [ ] Tests added/updated where appropriate
- [ ] Docs updated (README / `docs/**` / `CONTRIBUTING.md`) where appropriate

## Related issues

<!-- Closes #123 -->
