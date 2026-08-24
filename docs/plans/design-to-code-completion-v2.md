# Design-to-Code Completion — Chaos Scheduler (Screen-per-Session)

**Version:** v2
**Status:** DRAFT — 2026-08-23
**Date:** 2026-08-23
**Supersedes:** design-to-code-completion-v1.md
**Owning repository:** `KleinPerkins/chaos-scheduler` (`~/dev/personal/chaos-scheduler`)
**Plan-type:** standard
**Review provenance:** Restructures v1 (`design-to-code-completion-v1.md`, retained byte-identical at `docs/archive/plans/`) around the **screen-per-session** execution model the operator decided to adopt, resolving the open tracking-model decision v1 left in its §8. Consolidated from v1, the prior screen-per-session design-audit session, the tracked `design/divergence-ledger.md` (G-gates / D-decisions), GitHub Project #3 (design-discipline uplift epic #329, children #330–#335), and the shared Figma triage page (`00 — Status & Triage`). Not yet operator-accepted.

**Goal:** Bring every Chaos Scheduler UI surface to a verifiable "design-to-code complete" state by uplifting **one screen per work-session** to the Mission Control (MC) design language and proving each renders natively on-brand — the **G11 native-proof** gate — replacing v1's three thematic milestones (M1 D05 / M2 P4 / M3 P5) with a screen-per-session cadence that iterates across all surfaces.
**Architecture:** One screen/surface per session, iterated across the whole app. Each session = **audit** the surface's current design debt → **uplift** it to the MC design language/tokens (composed `cs.*`-bound design-system masters; on-scale type/weights/spacing/radii; tokens-only, no raw hex; no saturated/retro drift) → **pass G11 native-proof** → done. Design-first: an approved Dark-pinned Figma mock precedes any code. A screen may **lock** with backend deferred (operator-scope-verified) behind a Project #3 backend-dependency issue, so design is never blocked on backend. No change to the `SchedulerService` business-logic boundary or the shipped, propose-only fix-agent backend contract.
**Tech stack:** React + TypeScript frontend (`src/`), Tauri 2 / Rust backend (`src-tauri/`) reached only through existing IPC commands, SQLite read models, bespoke SVG chart primitives (`d3-scale`/`d3-shape`), Style-Dictionary token pipeline + Figma Code Connect, Vitest + Playwright (`e2e/`, `e2e/visual/`).

---

## Global Constraints

Project-wide requirements that implicitly apply to every screen session below:

- **Service boundary (ADR-0001):** all business logic stays in `SchedulerService`; UI components and IPC/REST/SDK/MCP adapters stay thin. No new business logic in a component or adapter.
- **Append-only migrations (ADR-0003):** any read-model/schema change is a new numbered, transactional migration; never mutate a shipped migration.
- **Admission control (ADR-0007):** every run/rerun routes through `dispatch_manual_run`; there is no immediate-execution path (`trigger_workflow` was removed, #266).
- **Charts (ADR-0008):** bespoke in-repo SVG primitives on `d3-scale`/`d3-shape`; do not add a charting library.
- **Fix-agent invariants (ADR-0009, non-negotiable):** the D05 fix-agent is PROPOSE-ONLY — it targets the real repo (production included) but is NEVER auto-merged or auto-applied; the seam never sets `workOnCurrentBranch` (always a new branch); the cloud PR is born `--draft` (#313); consent/opt-in, rate cap, duplicate-dispatch hard-guard, per-dispatch audit row, prompt-fence of untrusted `stderr`, symlink-safe path confinement, and namespaced idempotency are all retained, each with its biting failing-first test.
- **Tokens/theme:** bind fills/strokes/spacing/type to token CSS vars (`cs.*` mirror), never raw hex; dark is the default theme; ship self-hosted Inter. Repo tokens are the source of truth (`tokens/*.json` → `style-dictionary.config.mjs` → generated `src/styles/tokens.css`/`tokens.ts`/`figma-tokens.json`); never hand-edit generated files.
- **Borrow-and-remaster:** external Affirm DS / Genome libraries are mined for inspiration, icons, components, and UX patterns, but any asset is **copied in and re-mastered locally** as a `cs.*`-bound bespoke component published in our own library — never a live link to a source library.
- **Figma-mock-first, single-writer:** iterate to an approved Figma mock (shared file, `cs.*`-bound masters, Dark-pinned) before implementing any surface; no product UI is coded from an unapproved/mid-iteration design; serialize every `use_figma` write.
- **Delivery:** Conventional Commits; never `--no-verify`; land changes via the GitHub git-data API single-commit PR pattern (working tree stays pristine); `main` is protected (PR + `ci-required` + linear history + 1 approving review satisfied by the `chaos-scheduler-automerge` App).
- **Tauri parity:** keep the `tauri` crate and `@tauri-apps/api` major.minor-aligned (the `tauri-versions` gate blocks drift).

## 1. Purpose

v1 consolidated the remaining Design-to-Code work into a durable, doc-warden-tracked roadmap, but organized it as **three thematic milestones** (M1 = D05 Run-detail UI, M2 = P4 Mission Control composition, M3 = P5 re-sync/audit) and left the execution/tracking model as an **open operator decision** in its §8. A prior design-audit session found the MC design language never filtered into the non-MC surfaces (Admin, Workflow authoring, Global/Run History, tray popup, Settings) — with real, shipped-code design debt — and proposed replacing milestone tracking with a **screen-per-session** cadence plus a **G11 native-proof** Definition-of-Done gate. The operator has now decided to adopt that model. This plan re-expresses the same remaining work as a screen-per-session roadmap: the surfaces themselves are the backbone, each carried to design-to-code-ready one session at a time and proven native.

## 2. Scope and non-goals

**In scope:**

- The **screen-per-session execution model** (§4) and its per-session Definition of Done, including the **G11 native-proof** gate applied per screen and once as a program-level aggregate.
- Every real app surface as a screen session (§5): **Run Detail** (D05), the **Mission Control** cluster + demo/transient states (P4), **Global/Run History**, **Workflow authoring**, **Admin** (Environments/Integrations/Queues), **Settings**, and the **tray popup**.
- The two cross-cutting workstreams that make screens land compliant and closed: the **Figma triage/prune** precondition and the **foundation-first primitive uplift** (epic #329 / #334: `ui/Notice`, `Modal`/`PageHeader`, `.page-title`, a lint gate).
- The **design↔code re-sync/audit close** (v1's P5) re-scoped as a program closeout after the surfaces land.

**Non-goals:**

- Any change to the shipped fix-agent **backend** contract or its safety invariants (ADR-0009) — screens only _surface_ it; no PR-merge path is added and `workOnCurrentBranch` is never set.
- Re-opening resolved decisions (D00 sequencing, D02 384×590 popup, D03 MC depth/history consolidation, D07 charting, D08 race-track), or re-adding **F06** as a standalone surface (consolidated into F11 per D03).
- **Credential-security hardening** — a distinct MCP/SDK/backend subsystem tracked in its own plan (`credential-security-hardening-v1.md`, §8); it is cross-referenced here, never folded into this design roadmap.
- `repo-baseline`/pr-automation integration for this repo — it stays independently governed; doc-warden governs its documentation only (ADR-0008 in `~/dev`).
- Adding a charting library (ADR-0008), or moving business logic out of `SchedulerService`.

## 3. Background and context

Verified from `design/divergence-ledger.md` (current `main`), the merged PRs it cites, Project #3, and the prior design-audit session:

- **The MC design language did not filter into the non-MC surfaces.** Mission Control got the "delightful" aesthetic; subpages, dialogs, editors, and Settings drifted saturated/retro — bound to valid tokens but using the wrong patterns (saturated fills, ad-hoc type scale) and hand-built primitives instead of composed DS masters. The retro look is baked into **shipped code** (not just mocks) across ~11 surfaces (Queues, Integrations, Workflow Editor, Environments, Run/Global History, tray popup, …).
- **Concrete shipped-code design debt** (from the audit): a systemic off-scale `.page-title` inherited by every non-MC page; `ui/Notice` carries hardcoded Google-palette RGB that is not even tokenized (worst offender); and Settings' email/SMTP/theme sections have genuinely broken CSS (undefined `var(--bg)`/`var(--text)`, hardcoded `#f8f8fa`, raw mono).
- **Tracking artifacts exist.** The uplift is scoped in Project #3 as design-discipline epic **#329** with children **#330–#335** (foundation-first), and the shared Figma file has a triage page `00 — Status & Triage` (node `852:7289`) tagging ~50 frames (19 FINAL / 22 DUP / 3 WIP / 1 DRAFT / 1 FIX / 1 JUNK / 3 LIBRARY).
- **The repo is ahead of the roadmap baseline for Mission Control.** The D07 chart primitives are already composed into `Overview` (race-track, status donut, dual-axis trend), `OperationalHealth` (dual-axis), `NeedsAttention` (impact bars), and `Resources` (gauge + queue line) — ledger §1 note. MC work is _completion_, not greenfield.
- **D05 backend is shipped, disabled-by-default, UI pending.** The safety foundation (#275/#277/#278/#279/#281), cloud propose-only path (#284), local rerun-gated path (#286/#287/#288/#289), and PR2e "Option C" born-draft cloud PR (#313, ledger §5d) are merged and green, each safety invariant carrying a biting failing-first test; ADR-0009 records the propose-only invariant. The pending surface is a Settings "Cursor integration" section, RunDetail actions, and a consent `Modal`.
- **The design↔code gates are partly closed.** G00 mirror-half is satisfied for shipped surfaces; the **co-design half** (MC surfaces F01/F03/F04/F05, demo states F19/F22/F23, MC components C12–C29) is operator-gated (#301/#305/#306). G04 binding-half is complete; residual **R01** (node-level zero-remote-instance proof) and node IDs for unmapped masters remain; live Code Connect reports `version: "unknown"` for every mapped node.
- **G11 native-proof (R07) is open.** No surface has yet been proven in a signed / release-equivalent macOS build; v1 explicitly left this Definition-of-Done gate uncovered by M1–M3.

## 4. Approach — the screen-per-session execution model

**One screen per session.** Each work-session takes exactly one UI surface from its current state to **design-to-code ready**, then moves to the next surface; the roadmap iterates across all surfaces rather than batching them into thematic milestones. Within a session:

1. **Audit** — capture the surface's current design debt (retro/saturated fills, off-scale type, hand-built primitives, broken CSS, `ui/Notice`/`.page-title` inheritance) as a discrete Project #3 issue in a screen context, reconciling **ideal vs. possible** via per-screen Q&A with the operator.
2. **Uplift** — bring the surface onto the MC design language: replace hand-built primitives with composed `cs.*`-bound DS masters; apply the MC type scale/weights/spacing/radii; tokens only (no raw hex); remove saturated/retro drift; borrow-and-remaster external patterns rather than reinventing. Design-first — the surface is mocked and operator-approved (Dark-pinned, `cs.*`-bound) in the shared Figma file before code.
3. **G11 native-proof** — prove the uplifted surface renders natively and on-brand (see the gate below).
4. **Lock** — if the surface needs backend work not yet built, the operator verifies scope, the screen is **locked** as design-complete, and a **backend-dependency issue/epic** is filed on Project #3; design does not wait on backend.

**Global sequence:** finalize all screens (design-to-code ready) → build screens → close backend gaps → test → iterate → test → repeat. Two cross-cutting workstreams bracket the screens: the **Figma triage/prune** precondition (so sessions iterate from a single canonical frame set) and the **foundation-first primitive uplift** (so every screen inherits compliant primitives), both in §5; and the **design↔code re-sync/audit close** (§5) as the program closeout once the surfaces land.

**G11 native-proof gate (the Definition-of-Done gate).** A surface is _native-proven_ when it renders correctly in a **signed / release-equivalent macOS build** (not merely a browser or component harness): correct viewport (main window 960×680, or the tray popup 384×590), correct self-hosted Inter fonts, `cs.*` tokens with no raw hex and no saturated/retro drift, working scroll/reflow, focus, tray navigation, and drill-downs — with a native-viewport visual baseline captured in Playwright (`e2e/visual/`) and green, and Playwright mocks proven isolated from the production SQLite DB. G11 is applied **per screen** as each session's closeout and once more as a **program-level aggregate** smoke across all surfaces (R07).

**Per-session Definition of Done.** A screen session is DONE when:

- its design debt is audited and captured as a Project #3 screen issue;
- an approved Dark-pinned Figma mock exists for the surface (design-first);
- the surface is composed from `cs.*`-bound DS masters (no hand-built primitives), on-scale type/weights/spacing/radii, tokens-only, with no saturated/retro drift and no `.page-title`/`ui/Notice` debt;
- **G11 native-proof passes for the surface** (native render + native-viewport visual baseline, Playwright↔prod-DB isolation);
- unit / e2e / visual tests are green, every safety-relevant assertion authored as a failing-first test;
- if backend is deferred, scope is operator-verified and a backend-dependency issue/epic is filed, and the screen is LOCKED.

**Program plan, not a task runner.** This document is the standard program/roadmap map. **Each screen session is promoted to its own task-block execution plan** — a new `docs/plans/design-to-code-completion-<screen>-v1.md` carrying the writing-plans task-block shape (POLICY.md §10 rule 10) — at the point it is dispatched, after its mock is approved. This plan is not itself executed screen-by-screen.

## 5. Work items — the screens as the roadmap backbone

Ordered by leverage. The two preconditions come first (they make screens land born-compliant from a canonical frame set); the closeout comes last (it re-syncs design↔code once the surfaces exist). Each screen session states its **design debt (audit)**, its **uplift**, and its **G11 per-screen DoD**.

### P0 — Figma triage / prune (design-ops precondition)

- **Debt:** the shared Figma file (`twQmWC8dWT4tqeqIigNsRy`) carries duplicate/WIP versions of roughly half its ~50 frames, so FINAL cannot be told from WIP/throwaway. The `00 — Status & Triage` page tags them (19 FINAL / 22 DUP / 3 WIP / 1 DRAFT / 1 FIX `424:19197` / 1 JUNK / 3 LIBRARY).
- **Work:** prune to the canonical **`v1 code-mirrored`** set (a reversible archive move, operator-gated — never a destructive delete), fix or formally flag the one broken frame (`424:19197`), and disposition JUNK with operator confirmation. Records the borrow-and-remaster asset-provenance map so every subsequent session inherits it.
- **DoD:** a single canonical frame set agreed with the operator; duplicates archived reversibly; the broken frame resolved. Precondition to every screen session and to the closeout re-sync (§5 closeout).

### F0 — Foundation-first primitives (epic #329 / #334)

- **Debt:** systemic, app-wide — the off-scale `.page-title` inherited by every non-MC page and the un-tokenized `ui/Notice` (hardcoded Google-palette RGB); shared `Modal`/`PageHeader` are hand-built rather than composed DS masters. Left unfixed, each screen session re-inherits the same debt.
- **Work:** tokenize `ui/Notice` to `cs.*`; fix the systemic `.page-title` to the MC type scale; lift `Modal` and `PageHeader` to composed `cs.*`-bound masters; add a **lint gate** that blocks raw hex, off-token fills, and off-type-scale so new screen code lands **born-compliant**.
- **DoD:** the shared primitives are MC-compliant and the lint gate is green in CI; sequenced before (or alongside) the first screen session so surfaces inherit compliant primitives. Tracked as epic #329 children #330–#335.

### S1 — Run Detail (F10 / D05) · _maps v1 M1_

- **Debt:** the shipped fix-agent capability is not yet surfaced; the earlier D05 mock drifted retro. (Shipped `RunDetail` + the Settings "Cursor integration" code already meet the MC rubric, so the uplift here is mostly composing the actions/consent surface compliantly, not a rebuild.)
- **Work:** expose the already-shipped, disabled-by-default fix-agent path — a Settings "Cursor integration" opt-in section, RunDetail "Open in Cursor / Dispatch fix agent" actions on failed runs, a consent `Modal` (built from the `Modal` master), and the dispatch outcome (queued/admitted/duplicate) + born-draft PR link. UI calls the existing admission-controlled IPC commands only; no new backend.
- **G11 DoD:** reachable only after explicit opt-in; every path calls the propose-only/admission-controlled commands and can never auto-merge/apply or set `workOnCurrentBranch` (ADR-0009), each as a failing-first test; consent copy is accurate; renders native at 960×680 with a green native-viewport baseline; ships default-off. First because the backend is already shipped — the least code to a proven vertical slice.

### S2 — Mission Control cluster + demo/transient states · _maps v1 M2 (P4)_

- **Surfaces:** F01 Overview (`overview/Overview.tsx`), F03 Operational Health (`missionControl/OperationalHealth.tsx`), F04 Needs Attention (`missionControl/NeedsAttention.tsx`), F05 Resources (`missionControl/Resources.tsx`), F08 Workflow Detail (`WorkflowDetail.tsx`); demo/transient states F19/F22/F23 and F21/F24.
- **Debt:** these already carry the MC language but are partway through — missing sticky Lookback + two-group IA (Overview), the full KPI sets (Operational Health, Workflow Detail), the Collapsed/Debugging/FixReady/Fixed states (Needs Attention), the worker table (Resources, in scope per D03), and the missing demo states (collapsible Daily groups F19, Active/Disabled filter F22, plus F21/F24 persistent queued/running row treatments per D04).
- **Work:** compose the remaining surfaces per the approved MC mocks; author **deterministic fixtures** for the demo states F19/F22/F23 (gate G06). Completion, not greenfield. Runs as one or more sessions (Overview; Operational Health / Needs Attention / Resources; Workflow Detail), each individually G11-gated.
- **G11 DoD:** each MC surface matches its approved mock and renders native at 960×680; demo states render from deterministic fixtures; visual baselines green; no regression to the already-richer surfaces (F09/F12/F13/F14). Gated on the G00 co-design half (#301/#305/#306) landing first.

### S3 — Global / Run History (F11, F18)

- **Surfaces:** `GlobalHistory.tsx` (F11), `RunHistory.tsx` (F18).
- **Debt:** among the ~11 retro shipped surfaces (Run/Global History) _and_ incomplete MC composition — F11 does not yet wire the lean log-free read model (#247) or the snapshotted `run_environment` (schema v13).
- **Work:** uplift both to the MC language; add sticky Lookback/KPIs, env + duration columns, and the rehomed F06 aggregate charts (D03); wire the lean read model (#247) and snapshotted environment (schema v13) to F11.
- **G11 DoD:** both render native at 960×680 on the MC language with green baselines; F11 reads the lean read model + snapshotted environment; re-homing a workflow never re-buckets its history.

### S4 — Workflow authoring (F09/F20)

- **Surfaces:** `WorkflowEditor.tsx` and `workflow/` (`StepFlowBuilder.tsx`, `ActionsEditor.tsx`, `OperatorConfigForm.tsx`), plus `ScheduleBuilder.tsx`.
- **Debt:** among the worst retro offenders (the editor) — hand-built form/dialog primitives, saturated fills, ad-hoc type scale.
- **Work:** uplift the editor, its dialogs, and the schedule builder to composed `cs.*`-bound masters and the MC type scale; borrow-and-remaster form/editor patterns from the Affirm DS rather than reinventing.
- **G11 DoD:** editor + dialogs render native at 960×680 on the MC language, tokens-only, green baselines; existing authoring behavior (incl. the AD8 result-webhooks / AD9 email-profile surfaces reflected per D04) preserved.

### S5 — Admin (Environments / Integrations / Queues)

- **Surfaces:** `Environments.tsx`, `Integrations.tsx`, `QueueView.tsx` (the "Admin screens" cluster).
- **Debt:** among the worst retro offenders (Queues, Integrations) — hand-built primitives, wrong token patterns, off-scale `.page-title`.
- **Work:** uplift each admin surface to the MC language and composed masters; ensure the F0 primitive fixes (`ui/Notice`, `.page-title`) are reflected here.
- **G11 DoD:** each admin surface renders native at 960×680 on the MC language with green baselines and no `ui/Notice`/`.page-title` debt.

### S6 — Settings (F15)

- **Surfaces:** `Settings.tsx`, `EmailProfiles.tsx`, `SettingsField.tsx`, `SettingsCheck.tsx`.
- **Debt:** the **worst CSS debt** in the app — the email/SMTP/theme sections have genuinely broken CSS (undefined `var(--bg)`/`var(--text)`, hardcoded `#f8f8fa`, raw mono). (The "Cursor integration" opt-in section landed MC-compliant in S1.)
- **Work:** fix the broken email/SMTP/theme CSS and uplift all Settings sections to composed `cs.*`-bound masters and the MC type scale; retain the S1 Cursor-integration section unchanged.
- **G11 DoD:** every Settings section renders native at 960×680 on the MC language, tokens-only with the broken CSS resolved, green baselines; the fix-agent opt-in behavior is unchanged.

### S7 — Tray popup (F17)

- **Surface:** `MenuBarPopup.tsx`.
- **Debt:** among the retro shipped surfaces; the menu-bar popup lost the MC aesthetic and must also hold its constrained popup geometry.
- **Work:** uplift the popup to the MC language and composed masters; preserve the AD10 queue-run affordance (D04); honor the D02 384×590 popup geometry.
- **G11 DoD:** renders native in the **popup 384×590** viewport (not just the main window) with correct scroll/reflow, focus, and tray navigation, on the MC language, green popup-viewport baseline.

### Closeout — design↔code re-sync + audit close (R07 aggregate G11) · _maps v1 M3 (P5)_

Runs after the surfaces land (the re-synced surfaces must exist):

- **G00 co-design half:** land the MC mocks (F01/F03/F04/F05, demo states, C12–C29) and close #301/#305/#306.
- **G03/G04:** live token/version readback and the exhaustive Figma plugin/API audit; resolve residual **R01** (node-level `INSTANCE`/`mainComponent.remote` pass) and capture node IDs for the unmapped masters.
- **Code Connect version pin:** replace `version: "unknown"` across the mapped nodes (`*.figma.tsx` + `figma.config.json`).
- **G12/G13:** unify the §3a `D04` accepted-final divergences in code and mirror them into the Figma masters; execute and record the rollback drill (evidence pattern in ledger §5c).
- **Program-level G11 native-proof (R07):** one signed / release-equivalent macOS build smoke across **all** surfaces (main 960×680 + popup 384×590 sizing, scroll/reflow, tray navigation, fonts, focus, drill-downs; Playwright mocks proven isolated from the production SQLite DB).
- **Ledger:** append accepted re-sync evidence to `design/divergence-ledger.md` (updated only with accepted facts, per its own §0 rule).

### Promotion note

When a screen session is dispatched, author `docs/plans/design-to-code-completion-<screen>-v1.md` as a task-block execution plan (`**Files:**` + `**Interfaces:**` + `- [ ]` TDD steps with literal code and exact commands, per POLICY.md §10 rule 10), add its index row, and execute it screen-by-screen. Update this plan's index-row Notes as screens complete; a material change to the roadmap itself is a new `design-to-code-completion-v3.md` (this v2 is immutable once ACCEPTED).

## 6. Acceptance criteria

- **Per screen:** each of S1–S7 matches its approved Dark-pinned mock, is composed from `cs.*`-bound DS masters with the MC type scale and no raw hex / saturated-retro drift, carries no `.page-title`/`ui/Notice` debt, and **passes G11 native-proof** (native render at its true viewport + green native-viewport baseline, Playwright↔prod-DB isolation); unit/e2e/visual green; any deferred backend is operator-scope-verified with a Project #3 dependency issue filed and the screen LOCKED.
- **Preconditions:** P0 leaves a single canonical Figma frame set (duplicates archived reversibly, broken frame resolved); F0 lands `ui/Notice`/`Modal`/`PageHeader`/`.page-title` compliant with a green lint gate.
- **S1 (Run Detail):** fix-agent UI is opt-in only, calls exclusively the shipped propose-only/admission-controlled commands, cannot auto-merge/apply or set `workOnCurrentBranch` (ADR-0009), shows the born-draft PR outcome, and ships default-off.
- **Closeout:** G00 complete (mirror + co-design halves); R01 resolved or explicitly re-scoped with evidence; every mapped node carries a pinned Code Connect version; the §3a divergences are unified in code and mirrored to Figma; the rollback drill is recorded; the program-level G11/R07 signed-build smoke is green.
- **Program:** each screen shipped through its own reviewed task-block execution plan; `design/divergence-ledger.md` reflects the final accepted state; no resolved decision reopened; no fix-agent safety invariant weakened.

## 7. Rollback

- **Per screen:** each screen session ships as one-concern-per-PR; revert the offending PR(s). A UI-only uplift cannot regress backend behavior; S1's feature stays default-off and the born-draft guarantee (#313, ADR-0009) means no fix PR is ever auto-merge-eligible regardless of UI state.
- **Read-model wiring (S3, F11):** revert the surface wiring; the underlying read model (#247) and schema v13 remain (append-only migrations are not rolled back — a forward migration is used if a change is ever needed).
- **Figma writes (P0, closeout):** every `use_figma` write is non-destructive and serialized; triage archiving is a reversible move; the G13 rollback drill is the rehearsed reversal path; token/version changes are reversible via the token pipeline (repo is the source of truth).
- **This plan:** DRAFT and reversible until ACCEPTED; supersede via a new version rather than editing in place.

## 8. Related / out-of-band workstreams

- **Credential-security hardening** → its own governed plan, [`credential-security-hardening-v1.md`](credential-security-hardening-v1.md) (DRAFT). A read-only audit found the managed Cursor MCP key is minted `["read","write"]`, so everyday MCP **tool** reads return unredacted workflow secrets into agent/LLM context, plus at-rest, secret-scan-CI, audit-log/offboarding, and webhook-redaction gaps. Anchored to open security issue #292. It is a **distinct MCP/SDK/backend subsystem** and is deliberately **not** folded into this design roadmap — it is cross-referenced here only.
- **Prior milestone framing (v1).** v1 (`design-to-code-completion-v1.md`, retained byte-identical at `docs/archive/plans/`) organized the same remaining work as M1 (D05) / M2 (P4) / M3 (P5) and listed design-discipline uplift, G11 native-proof, and Figma triage as _out-of-band_ while leaving screen-per-session an open decision. This v2 **adopts** screen-per-session and pulls those three workstreams **in-band**: the design-discipline uplift becomes the F0 foundation + the per-screen uplift backbone (§5), G11 becomes the per-session and program DoD gate (§4), and Figma triage becomes the P0 precondition (§5). Only credential-security remains out-of-band.
