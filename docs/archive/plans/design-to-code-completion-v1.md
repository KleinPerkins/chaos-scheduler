# Design-to-Code Completion — Chaos Scheduler

**Version:** v1
**Status:** DRAFT — 2026-08-22
**Date:** 2026-08-22
**Owning repository:** `KleinPerkins/chaos-scheduler` (`~/dev/personal/chaos-scheduler`)
**Plan-type:** standard
**Review provenance:** Consolidated from the tracked `design/divergence-ledger.md` (G01, authority ledger), the shipped-state evidence in that ledger's §5a–§5d, and merged PRs referenced inline. This is the program/roadmap map; each milestone is promoted to its own task-block execution plan at dispatch (see §4). Not yet operator-accepted.

**Goal:** Finish the Design-to-Code work still open on `main` — the Run-detail agent-actions UI (D05), Mission Control composition + demo states (P4), and the design↔code re-sync/audit close (P5) — and make that previously-scattered roadmap durable and dispatchable under doc-warden governance.
**Architecture:** Three sequential, design-first milestones. Each surfaces or completes UI over primitives/backends that already exist on `main`; none changes the `SchedulerService` business-logic boundary or the shipped fix-agent backend contract. Each milestone becomes its own task-block execution plan once its Figma mock is approved.
**Tech stack:** React + TypeScript frontend (`src/`), Tauri 2 / Rust backend (`src-tauri/`) reached only through existing IPC commands, SQLite read models, bespoke SVG chart primitives (`d3-scale`/`d3-shape`), Style-Dictionary token pipeline + Figma Code Connect, Vitest + Playwright (`e2e/`, `e2e/visual/`).

---

## Global Constraints

Project-wide requirements that implicitly apply to every work item below:

- **Service boundary (ADR-0001):** all business logic stays in `SchedulerService`; UI components and IPC/REST/SDK/MCP adapters stay thin. No new business logic in a component or adapter.
- **Append-only migrations (ADR-0003):** any read-model/schema change is a new numbered, transactional migration; never mutate a shipped migration.
- **Admission control (ADR-0007):** every run/rerun routes through `dispatch_manual_run`; there is no immediate-execution path (`trigger_workflow` was removed, #266).
- **Charts (ADR-0008):** bespoke in-repo SVG primitives on `d3-scale`/`d3-shape`; do not add a charting library.
- **Tokens/theme:** bind fills/strokes/spacing/type to token CSS vars (`cs.*` mirror), never raw hex; dark is the default theme; ship self-hosted Inter.
- **Fix-agent invariants (D05, non-negotiable):** PROPOSE-ONLY — targets the real repo (production included) but is NEVER auto-merged or auto-applied; the seam never sets `workOnCurrentBranch` (always a new branch); the cloud PR is born `--draft` (auto-merge-ineligible, #313); consent/opt-in, rate cap, duplicate-dispatch hard-guard, per-dispatch audit row, prompt-fence of untrusted `stderr`, symlink-safe path confinement, and namespaced idempotency are all retained, each with its biting failing-first test.
- **Figma-mock-first:** iterate and get an approved Figma mock (shared file, `cs.*`-bound masters, Dark-pinned) for any UX surface before implementing it.
- **Delivery:** Conventional Commits; never `--no-verify`; land changes via the GitHub git-data API single-commit PR pattern (working tree stays pristine); `main` is protected (PR + `ci-required` + linear history + 1 approving review satisfied by the `chaos-scheduler-automerge` App).
- **Tauri parity:** keep the `tauri` crate and `@tauri-apps/api` major.minor-aligned (the `tauri-versions` gate blocks drift).

## 1. Purpose

The Design-to-Code Completion roadmap — screen/component coverage (F01–F24, C01–C41), divergence disposition (D00–D08), and the design↔code re-sync gates (G00–G13) — has been tracked across a gitignored planning doc, the tracked `design/divergence-ledger.md` (G01), and a GitHub Project. That fragmentation makes the remaining work hard to see, hand off, or dispatch. This plan consolidates the **remaining** work into one governed, doc-warden-tracked plan so it is durable, navigable, and dispatchable, and records the authority (the divergence ledger) each milestone answers to.

## 2. Scope and non-goals

**In scope:**

- **M1 — D05:** the Run-detail agent-actions UI over the already-shipped, disabled-by-default fix-agent backend/cloud/local paths.
- **M2 — P4:** Mission Control composition completion (F01/F03/F04/F05/F08/F11/F18) and the missing demo/transient states (F19/F22/F23, F21/F24).
- **M3 — P5:** design↔code re-sync and audit close (G00 co-design half, G03 token/version readback, G04 exhaustive audit + residual R01, G12 unify-in-code, G13 rollback drill, Code Connect version pinning).

**Non-goals:**

- Any change to the shipped fix-agent **backend** contract or its safety invariants — this plan only _surfaces_ it; no PR-merge path is added and `workOnCurrentBranch` is never set.
- Re-opening resolved decisions (D00 sequencing, D02 384×590 popup, D03 MC depth/history consolidation, D07 charting, D08 race-track), or re-adding **F06** as a standalone surface (removed only via D03 consolidation into F11).
- `repo-baseline`/pr-automation integration for this repo — it stays independently governed; doc-warden governs its documentation only (ADR-0008 in `~/dev`).
- Adding a charting library (ADR-0008), or moving business logic out of `SchedulerService`.

## 3. Background and context

Verified from `design/divergence-ledger.md` (current `main`) and the merged PRs it cites:

- **Repo is ahead of the roadmap baseline for Mission Control.** The D07 chart primitives are already composed into `Overview` (race-track, status donut, dual-axis trend), `OperationalHealth` (dual-axis), `NeedsAttention` (impact bars), and `Resources` (gauge + queue line) — ledger §1 note. P4 is _completion_, not _greenfield_.
- **Manual-run standardization is COMPLETE (Decision-3).** All manual runs (desktop UI + REST + SDK + MCP) route through admission control; `trigger_workflow` was removed (#266); `rerun_workflow` routes through `dispatch_manual_run` with a visible queued/admitted/duplicate outcome (#263/#264); SDK `runWorkflow` (#265) and MCP `run_workflow_now` (#267) are deprecated with corrected docs (#268).
- **Run read model snapshots the run-time environment** (schema v13, Decision-4, #251) so re-homing a workflow never re-buckets its history; the lean log-free read model (#247) lives at the DB/read-model layer but is **not yet wired** to the F11 Global History surface.
- **D05 backend is shipped, disabled-by-default, UI pending.** Backend safety foundation (#275/#277/#278/#279/#281), cloud propose-only path (#284), local rerun-gated path (#286/#287/#288/#289), and PR2e "Option C" born-draft cloud PR (#313, ledger §5d) are merged and green, each safety invariant carrying a biting failing-first test. The pending surface is: a Settings "Cursor integration" section, RunDetail actions, and a consent `Modal`.
- **G00 mirror-half is satisfied for shipped surfaces** (ledger §5b). The **co-design half** — MC surfaces F01/F03/F04/F05, demo states F19/F22/F23, and MC components C12–C29 — is OPERATOR-GATED (issues #301/#305/#306), so G00 is not fully complete.
- **G04 binding-half is COMPLETE** (ledger §5a). Residual **R01** (node-level proof that every descendant is a zero-remote-_instance_ node) and node IDs for unmapped masters remain; live Code Connect reports `version: "unknown"` for every mapped node (no pin).

## 4. Approach

- **Three sequential milestones**, ordered by leverage: **M1 (D05)** first because the backend is already shipped and only the UI is missing — it completes the fix-agent vertical slice for the least code; **M2 (P4)** completes the MC surfaces the repo is already partway through; **M3 (P5)** closes the design↔code loop (audit, re-sync, version pinning).
- **Design-first.** Because UX is Figma-mock-first, each milestone's UI is mocked and approved in the shared Figma file (`cs.*`-bound, Dark-pinned) before code.
- **Program plan, not a task runner.** This document is the standard program/roadmap map, mirroring the `~/dev` split of a program plan plus per-workstream execution plans. **Each milestone is promoted to its own task-block execution plan** — a new `docs/plans/design-to-code-completion-mN-v1.md` carrying the writing-plans task-block shape (per POLICY.md §10 rule 10) — at the point it is dispatched, after its mock is approved. This plan is not itself executed task-by-task.
- **Delivery** for every milestone: git-data-API single-commit PRs, one concern per PR, each with its failing-first test, merged via the automerge App.

## 5. Work items

### M1 — D05: Run-detail agent-actions UI (over the shipped backend)

- **Objective:** Expose the already-shipped, disabled-by-default fix-agent capability in the UI: a Settings "Cursor integration" opt-in section, RunDetail "Open in Cursor / Dispatch fix agent" actions on failed runs, and a consent `Modal` — without altering any backend safety invariant.
- **Scope (files/tests):** `src/components/Settings.tsx` (+ `Settings.test.tsx`) for the opt-in/consent section; `src/components/RunDetail.tsx` (+ `RunDetail.test.tsx`) for the failed-run actions and outcome (queued/admitted/duplicate, PR link); a consent `Modal` built from the existing `Modal` master; `e2e/run-detail.spec.ts` and an `e2e/visual/` spec. UI calls the existing IPC commands only (capability/flag read + dispatch); no new backend.
- **Prerequisites:** Figma mock of the RunDetail actions + Settings section + consent modal, approved in the shared file (retires the "UI pending" note on F10 and unblocks part of the G00 co-design half). Confirm the shipped IPC command names/params and the feature-flag/capability read from `src-tauri/` before authoring the execution plan.
- **Steps (to be expanded in the M1 execution plan):** (1) read capability/flag state and gate the surface; (2) Settings opt-in + consent copy that states PROPOSE-ONLY, born-draft, never-auto-applied; (3) RunDetail action buttons on failed runs wired to the dispatch command; (4) render the dispatch outcome and the born-draft PR link; (5) unit + e2e + visual coverage, each safety-relevant assertion as a failing-first test.
- **Definition of done:** the feature is reachable only after explicit opt-in; every UI path calls the existing admission-controlled, propose-only commands; no UI path can auto-merge/apply or set `workOnCurrentBranch`; consent copy is accurate; unit/e2e/visual green; feature remains default-off until opt-in.

### M2 — P4: Mission Control composition completion + demo states

- **Objective:** Finish the MC surfaces the repo is partway through and add the missing demo/transient states, per the approved MC design.
- **Scope (files/tests):**
  - **F01 Overview** (`src/components/overview/Overview.tsx`): sticky Lookback control, two-group IA (Critical/Needs-Attention + Operational Health), and at-a-glance→medium-expand behavior.
  - **F03 Operational Health** (`src/components/missionControl/OperationalHealth.tsx`, `useOperationalHealth.ts`): complete the designed KPI set.
  - **F04 Needs Attention** (`src/components/missionControl/NeedsAttention.tsx`, `useNeedsAttention.ts`): Collapsed / Debugging / FixReady / Fixed states.
  - **F05 Resources** (`src/components/missionControl/Resources.tsx`, `useResources.ts`): worker table (stays in scope per D03).
  - **F08 Workflow detail** (`src/components/WorkflowDetail.tsx`): complete the designed KPI set.
  - **F11 Global History** (`src/components/GlobalHistory.tsx`): sticky Lookback/KPIs, env + duration columns, rehomed F06 aggregate charts; wire the lean log-free read model (#247) and snapshotted `run_environment` (schema v13) to this surface.
  - **F18 Run History** (`src/components/RunHistory.tsx`): sticky Lookback/KPIs.
  - **F07/F19/F22 Workflows list** (`src/components/WorkflowList.tsx`, `WorkflowCard.tsx`): compact rows, timezone note, collapsible Daily groups (F19), Active/Disabled filter demo state (F22).
  - **F21/F24** (`src/components/WorkflowCard.tsx`): persistent queued/running row treatments (unify-in-code per D04).
  - Tests: the co-located `*Data.test.ts`/`*.test.tsx` per component plus the `e2e/visual/mission-control-*.spec.ts` set; **deterministic fixtures** for the missing demo states F19/F22/F23 per gate G06.
- **Prerequisites:** the G00 **co-design half** for F01/F03/F04/F05 + demo states + MC components C12–C29 is operator-gated (#301/#305/#306) — mocks/vNext approval must land before these are dispatched. Duration formatting, single-select Environment + Lookback filter bar, and hover-only InfoTip conventions are already established and must be honored.
- **Steps (to be expanded per-screen in the M2 execution plan):** compose existing primitives per approved mock; add the read-model wiring for F11; author deterministic demo fixtures; extend unit + visual coverage.
- **Definition of done:** every listed surface matches its approved mock; F11 reads the lean read model + snapshotted environment; demo states F19/F22/F23 render from deterministic fixtures; unit + visual baselines green; no regression to the shipped-richer surfaces (F09/F12/F13/F14).

### M3 — P5: design↔code re-sync + audit close

- **Objective:** Close the design↔code loop: finish the G00 co-design half, run the G03/G04 readback/audit, unify the accepted-final divergences (G12), prove the rollback drill (G13), and pin Code Connect versions.
- **Scope (files/tests):**
  - **G00 co-design half:** land the MC mocks (F01/F03/F04/F05, demo states, C12–C29) and close #301/#305/#306.
  - **G03/G04:** live token/version readback and the exhaustive Figma plugin/API audit; resolve residual **R01** (node-level `INSTANCE`/`mainComponent.remote` pass) and capture node IDs for the unmapped masters (C07/C13/C14/C15/C16/C17/C18/C22/C23/C25/C26/C27/C29).
  - **Code Connect version pin:** replace `version: "unknown"` across the mapped nodes (`*.figma.tsx` + the token pipeline / `figma.config.json`).
  - **G12/G13:** unify the §3a `D04` accepted-final divergences in code and mirror them back into the Figma masters; execute and record the rollback drill (evidence pattern in ledger §5c).
  - Ledger: append accepted re-sync evidence to `design/divergence-ledger.md` (updated only with accepted facts, per its own §0 rule).
  - Tests: `figma connect publish --dry-run` validation (the existing `figma-code-connect.yml` PR pass) and refreshed visual baselines.
- **Prerequisites:** M2 shipped (the surfaces being re-synced exist); operator approval for any Figma write (single-writer, serialize `use_figma`); protected-env readback for automated Figma token create/update (D06/G03/G13).
- **Steps (to be expanded in the M3 execution plan):** approve MC mocks → readback tokens/versions → run the audit passes → pin versions → unify divergences + mirror to masters → rollback drill → append ledger evidence.
- **Definition of done:** G00 is fully complete (mirror + co-design halves); R01 is resolved or explicitly re-scoped with evidence; every mapped node carries a pinned Code Connect version; the §3a accepted divergences are unified in code and reflected in Figma; the rollback drill is recorded.

### Promotion note

When a milestone is dispatched, author `docs/plans/design-to-code-completion-mN-v1.md` as a task-block execution plan (`**Files:**` + `**Interfaces:**` + `- [ ]` TDD steps with literal code and exact commands, per POLICY.md §10 rule 10), add its index row, and execute it task-by-task. Update this plan's index row Notes as milestones complete; a material change to the roadmap itself is a new `design-to-code-completion-v2.md` (this v1 is immutable once ACCEPTED).

## 6. Acceptance criteria

- **M1:** the fix-agent UI is opt-in only, calls exclusively the shipped propose-only/admission-controlled commands, cannot auto-merge/apply or set `workOnCurrentBranch`, shows the born-draft PR outcome, and ships default-off; unit/e2e/visual green.
- **M2:** F01/F03/F04/F05/F08/F11/F18 match their approved mocks; F11 reads the lean read model + snapshotted environment; demo states F19/F22/F23 render from deterministic fixtures; F21/F24 have persistent row treatments; visual baselines green; no regression to shipped-richer surfaces.
- **M3:** G00 complete; R01 resolved or evidenced; Code Connect versions pinned; §3a divergences unified in code + mirrored to Figma; rollback drill recorded; ledger updated with accepted evidence.
- **Program:** each milestone shipped through its own reviewed execution plan; `design/divergence-ledger.md` reflects the final accepted state; no resolved decision reopened; no fix-agent safety invariant weakened.

## 7. Rollback

- **Per milestone:** each ships as one-concern-per-PR; revert the offending PR(s). M1's feature stays default-off, so a UI revert cannot expose the backend; the born-draft guarantee (#313) means no fix PR is ever auto-merge-eligible regardless of UI state.
- **Read-model wiring (M2, F11):** revert the surface wiring; the underlying read model (#247) and schema v13 remain (append-only migrations are not rolled back — a forward migration is used if a change is ever needed).
- **Figma writes (M3):** every `use_figma` write is non-destructive and serialized; the G13 rollback drill is the rehearsed reversal path; token/version changes are reversible via the token pipeline (repo is the source of truth).
- **This plan:** DRAFT and reversible until ACCEPTED; supersede via a new version rather than editing in place.

## 8. Related / out-of-band workstreams

Prior design-to-code + security sessions (mined 2026-08-23 from session transcripts) surfaced adjacent work that is real and evidenced but **outside this plan's design-to-code milestone scope**. It is recorded here so it stays visible and dispatchable. None of these is a milestone of this plan.

- **Credential-security hardening** → its own governed plan, [`credential-security-hardening-v1.md`](credential-security-hardening-v1.md) (DRAFT). A read-only audit found the managed Cursor MCP key is minted `["read","write"]` (`src-tauri/src/mcp.rs:817`), so everyday MCP **tool** reads (`list_workflows`/`get_workflow` in `packages/mcp-server/src/server.ts`) return unredacted workflow secrets into agent/LLM context, plus at-rest, secret-scan-CI, audit-log/offboarding, and webhook-redaction gaps. Anchored to open security issue #292. Distinct subsystem (MCP/SDK/backend), so it lives in its own plan.
- **Design-discipline uplift (non-MC surfaces)** → GitHub Project #3 epic #329 (children #330–#335). The Mission Control design language did not fully filter into non-MC surfaces (Admin screens, Workflow authoring, Global/Run History, tray popup, Settings), with concrete shipped-code debt (e.g. `ui/Notice` hardcoded palette, an off-scale `.page-title` inherited by non-MC pages, broken Settings email/SMTP/theme CSS). This plan's **M2 is MC-composition-only**; the non-MC uplift is tracked in Project #3, not as a milestone here.
- **G11 native proof (R07)** — a signed / release-equivalent macOS build smoke (main 960×680 + popup 384×590 sizing, scroll/reflow, tray navigation, fonts, focus, drill-downs; confirm Playwright mocks stay isolated from the production SQLite DB). This design-to-code Definition-of-Done gate is **not covered by M1–M3** and remains open.
- **Figma file triage / prune** — the shared file carries duplicate/WIP versions of roughly half its ~50 frames; a triage page (`00 — Status & Triage`) tags them, and pruning to the canonical `v1 code-mirrored` set (a reversible archive move, operator-gated) is a design-ops precondition for the M3 re-sync. Tracked in Project #3.
- **Execution/tracking model (OPEN — operator decision).** A prior session proposed replacing milestone tracking with a **screen-per-session** model (design one surface at a time to design-to-code-ready; spawn a backend-dependency issue per locked screen) tracked solely in Project #3, and demoted the original fixed roadmap this plan descends from. This plan keeps the milestone structure; whether to adopt screen-per-session instead is an **unresolved operator decision**, not adopted here.
