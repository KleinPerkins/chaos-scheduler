# Plans

This directory contains multi-session plans for `chaos-scheduler`.

**Governance:** `~/dev/_ops/doc-warden/POLICY.md` §2 (taxonomy), §9 (archival).
Plans are **byte-identical-forever** once their Status line reads `ACCEPTED` — no in-place edits
after approval. A revision is a new `<slug>-v2.md` with a `Supersedes:` pointer; the prior
version is never modified.

## Index

| Plan | Status | Date | Notes |
|---|---|---|---|
| [design-to-code-completion-v1](design-to-code-completion-v1.md) | DRAFT | 2026-08-22 | Program/roadmap map for the remaining Design-to-Code work: D05 Run-detail agent-actions UI (M1), P4 Mission Control composition + demo states (M2), P5 design↔code re-sync/audit close (M3). Each milestone promotes to its own task-block execution plan at dispatch. |

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
