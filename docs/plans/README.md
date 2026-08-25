# Plans

This directory contains multi-session plans for `chaos-scheduler`.

**Governance:** `~/dev/_ops/doc-warden/POLICY.md` §2 (taxonomy), §9 (archival).
Plans are **byte-identical-forever** once their Status line reads `ACCEPTED` — no in-place edits
after approval. A revision is a new `<slug>-v2.md` with a `Supersedes:` pointer; the prior
version is never modified.

## Index

| Plan                                                                    | Status   | Date       | Notes                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| ----------------------------------------------------------------------- | -------- | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [design-to-code-completion-F0-v1](design-to-code-completion-F0-v1.md)   | DRAFT    | 2026-08-24 | Task-block execution plan for F0 foundation-first primitives (epic #329 / child #334): tokenize `ui/Notice`, fix off-scale `.page-title`, lift `Modal`+`PageHeader` to `cs.*`-bound masters, add raw-hex lint gate.                                                                                                                                                                                                                                                 |
| [design-to-code-completion-v2](design-to-code-completion-v2.md)         | ACCEPTED | 2026-08-24 | Screen-per-session roadmap superseding v1's M1/M2/M3 milestones: one UI surface uplifted to the Mission Control design language per session, each passing the G11 native-proof gate. Backbone = Figma triage (P0) + foundation primitives (F0) + Run Detail / Mission Control / History / Workflow authoring / Admin / Settings / tray-popup screen sessions + design↔code re-sync closeout. Each screen promotes to its own task-block execution plan at dispatch. |
| [credential-security-hardening-v1](credential-security-hardening-v1.md) | ACCEPTED | 2026-08-24 | Credential-exposure hardening surfaced by a read-only security audit: redact MCP tool reads, secret-scan CI, at-rest file hardening, audit-log/offboarding + read-only managed key, envelope encryption (PR-E delivered/mandated, on by default — decision reversed 2026-08-24, ADR 0011). Distinct MCP/SDK/backend subsystem; anchored to security issue #292. Operator-accepted 2026-08-24.                                                                       |

## Conventions

- Filename: `<slug>-vN.md` where `N` starts at `1`
- Status values: `DRAFT` → `ACCEPTED` → `SUPERSEDED` (tracked in the index row, NOT in the plan file itself after acceptance)
- A new version (`-v2`, `-v3`, …) is created when a material revision is needed post-acceptance; the prior version is immutable
- Draft plans may be edited in place before acceptance; accepted plans may not
- **Plan type:** every non-`ACCEPTED` governed plan MUST declare a header line
  `**Plan-type:** standard` or `**Plan-type:** agentic-execution`. An `agentic-execution`
  plan MUST give every `## 5. Work items` entry the task-block shape from
  `~/dev/_ops/doc-warden/templates/plan-task-block.md` — a `**Files:**` block, an `**Interfaces:**`
  (Consumes/Produces) block, and checkbox (`- [ ]`) TDD steps with literal code and exact commands,
  no placeholders. doc-warden enforces this shape structurally (POLICY.md §10 rule 10): a marked plan
  missing the shape is `DRIFTED`; an unfilled skeleton is `NEEDS_AUTHORING`.

## Pre-acceptance self-review (agentic-execution plans)

Before flipping an `agentic-execution` plan's Status to `ACCEPTED`, run this `writing-plans`-derived
self-review — a zero-context subagent must be able to execute each work item from its block alone:

- [ ] **Spec coverage:** every requirement in the source spec maps to at least one work item; nothing
      silently dropped.
- [ ] **Placeholder scan:** no `TBD`, `add error handling`, `similar to Task N`, or code step without
      a code block anywhere in the Work items.
- [ ] **Type consistency:** every name/type a task lists under `Produces` matches what a consuming
      task lists under `Consumes`; no work item references a symbol no task defines.
- [ ] **Self-containment:** each work item names exact file paths and gives runnable commands with
      expected pass/fail output; no cross-task "see above".

## Archive

Superseded and retired plans that are no longer active but preserved for history live in
`docs/archive/plans/` (moved via `git mv`, not deleted).

| Plan                                                                             | Status           | Date       | Notes                                                                                                                                                                                                                     |
| -------------------------------------------------------------------------------- | ---------------- | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [design-to-code-completion-v1](../archive/plans/design-to-code-completion-v1.md) | SUPERSEDED by v2 | 2026-08-23 | Original M1/M2/M3 milestone roadmap for the remaining Design-to-Code work; superseded by the screen-per-session `design-to-code-completion-v2.md`. Retained byte-identical, moved to `docs/archive/plans/` per POLICY §9. |
