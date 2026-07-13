# G01 Divergence Ledger — Chaos Scheduler

> **Gate:** `G01 Contract` of the Design-to-Code Completion roadmap
> (`.cursor/plans/design-to-code_completion_7b6a5788.plan.md`).
> **Status snapshot:** 2026-07-12 · off `origin/main`.
> **Scope:** every screen/state `F01–F24`, every component `C01–C41`, and every
> design-revision (`DR01/DR02/DR03`) entry, plus the `D04` accepted-final
> divergence register.

This ledger is the source-tracked contract that pins each design artifact to its
code, tests, Code Connect mapping, authority, status, accepted divergence, owner,
and last-verified/approved Figma version. It is a living document: it is diffed at
every phase boundary (roadmap §"re-sync checkpoint") and updated only with accepted
implementations/divergences.

No cell is left the literal word "unknown". Where a fact genuinely cannot be
determined from the repo + read-only metadata yet — chiefly **live-Figma facts**
(a master's live document version, and node-level remote-component-instance
freedom) — it is marked **`pending G04 audit`** (the one-time exhaustive Figma
plugin/API audit in roadmap `G04`, plus the `G03` live token/version readback).
The `G04` **binding-half** is now **COMPLETE** (per-descendant `cs.*` binding read
live via `get_variable_defs`, 2026-07-12) — and the 8 earlier-extracted masters
the audit found on legacy `affirm.color`/`radius` have since been **rebound to
`cs.*`** (structure-preserving, screenshot-verified, non-destructive): see §5a for
the evidence and §5 for the consolidated remaining list. The distinct
remote-icon-**instance** residual (`R01`) stays open.

---

## 0. Authority model & conventions

### Authority rules (how a conflict is resolved)

1. **Repo tokens are the source of truth (repo-SOT).** `tokens/*.json` →
   `style-dictionary.config.mjs` → generated `src/styles/tokens.css`,
   `src/styles/tokens.ts`, and `figma-tokens.json`. Figma is a **one-way mirror**
   of these values into its `cs.*` variable collection; code never reads Figma at
   build time. Any color/space/type/radius/motion value is owned by the repo.
2. **Figma is the design authority for layout / visual / state structure.** For a
   surface or component's _appearance and state model_, the approved Figma master
   (`file twQmWC8dWT4tqeqIigNsRy`, page "Mission Control" `0:1`, component section
   `113:514`) is authoritative and is mirrored one-way into code.
3. **Shipped safety/runtime behavior wins — and updates Figma.** Where the running
   app is intentionally richer or safer than the design (the `D04` accepted-final
   set), the **shipped behavior is authoritative** and Figma is updated to match
   (roadmap `G12`). These are recorded as _accepted divergences_, never informal.

The **Authority** column names the primary authority for each row:
`repo-SOT` · `Figma` · `shipped-behavior`. Token _values_ are always `repo-SOT`
regardless of the row's primary authority.

### Legend

- **Figma node** — the mapped Figma **component-set** node (variant child nodes
  inherit the mapping). `F` frame node IDs are the roadmap coverage-matrix IDs.
- **CC** — Code Connect. `✓ live` = a source-tracked `*.figma.tsx` mapping that
  auto-publishes on every push to `main` via `.github/workflows/figma-code-connect.yml`.
  The 22 design-system masters were additionally **verified live** this session via
  the published Code Connect map; chart primitives are on `main` and publish the
  same way. `—` = no mapping (unmapped master or non-code composition).
- **Owner** — `KP` = KleinPerkins (sole maintainer). Lane suffix from the roadmap
  phasing: `·DS` design-system, `·MC` Mission Control, `·Tabs` lighter-tab.
  Ownership is consolidated under the maintainer pending the roadmap `P0`
  "decision owners assigned" step.
- **Last-verified Figma ver.** — the live Code Connect map currently reports
  `version: "unknown"` for **every** mapped node (no version pin), and no `vNext`
  design has been approved (`G00` is open). So this field is either
  `Published lib — pending G04 audit` (mapped, live, but unpinned) or
  `Pre-P1 — pending G04 audit` (redesign/demo, no approved vNext). See §5.
- **Status vocabulary** — `Implemented` · `Implemented (richer)` · `Partial` ·
  `Partial (inline)` · `Missing` · `Removed` · `Demo state`. Statuses reflect
  **current repo reality**, which in several cases is _ahead of_ the roadmap's
  starting baseline (e.g. `StatusBar`, `InfoTip`, `LookbackSelect` and the chart
  primitives now exist with code + tests + CC).

> **Global G04 note:** the **binding-half** of `G04` is **COMPLETE** (live
> `get_variable_defs` readback, 2026-07-12) — descendant `cs.*` binding freedom is
> now audited per master, and the 8 earlier-extracted primitives the audit found on
> legacy `affirm.color`/`radius` have since been **rebound to `cs.*`**
> (structure-preserving, screenshot-verified, non-destructive; verified 0 remote
> color + 0 remote radius bindings on each master's own nodes). They now join the
> 10 `#174` masters + bespoke chart primitives + `WorkflowCard` as `cs.*`-only on
> their own nodes. Node-level remote-component-**instance** freedom is a separate,
> still-open residual (`R01`): 36 remote Affirm icon-glyph instances remain embedded
> in `NavItem`/`Sidebar`/`ThemeToggle` (88 icon-interior remote bindings), and
> per-descendant instance enumeration is still pending for the remaining rows. Full
> evidence in §5a; not re-stated per row.

---

## 1. Screen / state ledger (F01–F24)

Node IDs are the roadmap coverage-matrix frame IDs. 18 product surfaces + 6
demo/transient states = 24.

| ID  | Screen / state                    | Figma node  | Auth             | Code path                                                                                              | Test path                                                                                                              | CC                                 | Status                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | Accepted divergence                                                                                                      | Owner   | Last-verified Figma ver.   |
| --- | --------------------------------- | ----------- | ---------------- | ------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------- | ---------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ | ------- | -------------------------- |
| F01 | Home / Mission Control            | `110:443`   | Figma            | `src/components/MissionControl.tsx`, `src/components/overview/Overview.tsx`                            | `e2e/visual/mission-control-overview.spec.ts`, `src/components/overview/overviewData.test.ts`                          | via masters                        | Partial → P4: race-track + status donut + trend primitives composed; Lookback, two-group IA, medium-expand still landing                                                                                                                                                                                                                                                                                                                                                | — (DR01 redesign, not a divergence)                                                                                      | KP·MC   | Pre-P1 — pending G04 audit |
| F02 | Home / Sandbox                    | `138:693`   | shipped-behavior | `src/components/MissionControl.tsx`, `src/components/FilterBar.tsx`                                    | `src/components/FilterBar.test.tsx`, `e2e/dashboard-navigation.spec.ts`                                                | via `EnvSelect`                    | Partial demo: arbitrary environments work; default is dynamic, not the fixed `all` model                                                                                                                                                                                                                                                                                                                                                                                | Dynamic environments (D04 accept)                                                                                        | KP·MC   | Pre-P1 — pending G04 audit |
| F03 | Operational Health detail         | `145:892`   | Figma            | `src/components/missionControl/OperationalHealth.tsx`, `useOperationalHealth.ts`                       | `src/components/missionControl/operationalHealthData.test.ts`, `e2e/visual/mission-control-operational-health.spec.ts` | via `DualAxisLine`                 | Partial → P4: MC drill-down (D03); dual-axis runtime/wait trend composed; full KPI set pending                                                                                                                                                                                                                                                                                                                                                                          | — (DR01 redesign)                                                                                                        | KP·MC   | Pre-P1 — pending G04 audit |
| F04 | Needs Attention detail            | `154:1033`  | Figma            | `src/components/missionControl/NeedsAttention.tsx`, `useNeedsAttention.ts`                             | `src/components/missionControl/needsAttentionData.test.ts`, `e2e/visual/mission-control-needs-attention.spec.ts`       | via `ImpactBars`                   | Partial → P4: MC drill-down (D03); compound-impact bars composed; Collapsed/Debugging/FixReady/Fixed states pending                                                                                                                                                                                                                                                                                                                                                     | — (DR01 redesign)                                                                                                        | KP·MC   | Pre-P1 — pending G04 audit |
| F05 | Resources detail                  | `160:1215`  | Figma            | `src/components/missionControl/Resources.tsx`, `useResources.ts`                                       | `src/components/missionControl/resourcesData.test.ts`, `e2e/visual/mission-control-resources.spec.ts`                  | via `Gauge`, `QueueLine`           | Partial → P4: MC drill-down, stays in scope (D03); gauges + queue-utilization line composed; worker table pending                                                                                                                                                                                                                                                                                                                                                       | — (DR01 redesign)                                                                                                        | KP·MC   | Pre-P1 — pending G04 audit |
| F06 | aggregate Workflow History detail | `162:1394`  | Figma            | _(folded into `src/components/GlobalHistory.tsx`)_                                                     | _(see F11)_                                                                                                            | —                                  | **Removed** as standalone (D03): aggregate metrics rehome into F11                                                                                                                                                                                                                                                                                                                                                                                                      | Consolidation to one history surface (D03)                                                                               | KP·MC   | Pre-P1 — pending G04 audit |
| F07 | Workflows list                    | `207:1303`  | Figma            | `src/components/WorkflowList.tsx`, `src/components/WorkflowCard.tsx`                                   | `src/components/WorkflowList.test.tsx`, `e2e/workflows-errors.spec.ts`                                                 | via `WorkflowCard`                 | Partial: env/frequency grouping, searchable cards + Active/Disabled status filter (#230), and actions exist; manual runs enter admission control (#229; manual-run standardization completed #263/#266); compact rows, tz note, collapsible groups pending (unify in code, D04)                                                                                                                                                                                         | Delete action lives on list, not editor (D04 accept)                                                                     | KP·Tabs | Pre-P1 — pending G04 audit |
| F08 | Workflow detail                   | `224:1376`  | Figma            | `src/components/WorkflowDetail.tsx`                                                                    | `src/components/WorkflowDetail.test.tsx`, `e2e/workflow-detail.spec.ts`                                                | —                                  | Partial/close: config, heatmap (keyboard-accessible cells, #244), recent runs, drill-downs exist; designed KPI set incomplete                                                                                                                                                                                                                                                                                                                                           | — (DR02 redesign)                                                                                                        | KP·Tabs | Pre-P1 — pending G04 audit |
| F09 | Workflow builder / New            | `213:1303`  | shipped-behavior | `src/components/WorkflowEditor.tsx`, `src/components/ScheduleBuilder.tsx`, `src/components/workflow/*` | `src/components/WorkflowEditor.test.tsx`, `e2e/workflow-editor.spec.ts`                                                | via `ScheduleBuilder` (`581:4321`) | Implemented (richer) than design                                                                                                                                                                                                                                                                                                                                                                                                                                        | Editor richer than Figma (shipped)                                                                                       | KP·Tabs | Pre-P1 — pending G04 audit |
| F10 | Run detail                        | `213:3035`  | Figma            | `src/components/RunDetail.tsx`                                                                         | `src/components/RunDetail.test.tsx`, `e2e/run-detail.spec.ts`                                                          | —                                  | Partial (richer): timeline, logs, metrics, lineage, AI diagnosis exist; completed-task status is exposed to assistive tech (#239); the rerun path is now admission-controlled with a visible queued/admitted/duplicate outcome (#263/#264); the safe opt-in fix-agent **backend contract shipped disabled-by-default** across 5 PRs (#275/#277/#278/#279/#281), no UI surface yet; per-step logs/copy + fix-agent action UI pending (D05)                               | Agent actions: safe backend shipped, disabled-by-default; UI pending (D05)                                               | KP·Tabs | Pre-P1 — pending G04 audit |
| F11 | Global History                    | `228:1303`  | Figma            | `src/components/GlobalHistory.tsx`                                                                     | `src/components/GlobalHistory.test.tsx`, `e2e/global-history.spec.ts`                                                  | —                                  | Partial; scope-expanded (D03): bounded search + status/env/trigger filters + drill-down exist; async load failures announce via role=alert (#248); the lean log-free read model (#247) and snapshotted run_environment (schema v13, Decision-4, #251) land at the DB/read-model layer but are not yet wired to this surface; sticky Lookback/KPIs/env+duration cols + rehomed F06 charts pending                                                                        | Hosts rehomed F06 aggregate metrics (D03)                                                                                | KP·Tabs | Pre-P1 — pending G04 audit |
| F12 | Queues                            | `213:3441`  | shipped-behavior | `src/components/QueueView.tsx`                                                                         | `e2e/enqueue.spec.ts` (no dedicated unit spec)                                                                         | —                                  | Implemented (richer)                                                                                                                                                                                                                                                                                                                                                                                                                                                    | Richer than Figma (shipped)                                                                                              | KP·Tabs | Pre-P1 — pending G04 audit |
| F13 | Environments                      | `213:3845`  | shipped-behavior | `src/components/Environments.tsx`, `src/components/EnvironmentBadge.tsx`                               | `e2e/dashboard-navigation.spec.ts` (no dedicated unit spec)                                                            | —                                  | Implemented                                                                                                                                                                                                                                                                                                                                                                                                                                                             | Dynamic environments (D04 accept)                                                                                        | KP·Tabs | Pre-P1 — pending G04 audit |
| F14 | Integrations                      | `213:4245`  | shipped-behavior | `src/components/Integrations.tsx`                                                                      | `src/components/Integrations.test.tsx`                                                                                 | —                                  | Implemented, **deliberate divergence**                                                                                                                                                                                                                                                                                                                                                                                                                                  | App-owned pinned MCP provisioning (D04 accept)                                                                           | KP·Tabs | Pre-P1 — pending G04 audit |
| F15 | Settings                          | `213:5050`  | shipped-behavior | `src/components/Settings.tsx`                                                                          | `src/components/Settings.test.tsx`                                                                                     | via form masters                   | Implemented (richer); **mirrored code→design into Figma as v1** (G00 first bless/mirror surface — frame `633:5682`, page `0:1`, from `cs.*`-bound master instances, verified vs the committed `settings-linux.png` baseline)                                                                                                                                                                                                                                            | Richer than Figma (shipped)                                                                                              | KP·Tabs | Pre-P1 — pending G04 audit |
| F16 | Agent Activity                    | `213:4646`  | shipped-behavior | `src/components/missionControl/AgentActivity.tsx`, `agentActivityData.ts`                              | `src/components/missionControl/agentActivityData.test.ts`, `e2e/visual/mission-control-activity.spec.ts`               | —                                  | Partial/merged into MC Activity tab; no standalone route or agent-kind badge                                                                                                                                                                                                                                                                                                                                                                                            | Merged into Mission Control (D04 accept)                                                                                 | KP·MC   | Pre-P1 — pending G04 audit |
| F17 | Tray popup                        | `231:1303`  | shipped-behavior | `src/components/MenuBarPopup.tsx`                                                                      | `src/components/MenuBarPopup.test.tsx`, `e2e/popup.spec.ts`                                                            | —                                  | Shipped the **384×590 mini-dashboard** (#269): BrandMark branding + connection status + running/queued/failed summary chips + ACTIVE/UPCOMING/RECENT run rows, the admission-control "Queue run" preserved on upcoming rows, the update aside CTA relabeled "Install" (was "Update") + Skip, no "Pause all", sourced only from the existing `get_mission_control_snapshot` + `list_queued_runs` (no new DB/IPC), with a regenerated deterministic Linux visual baseline | 384×590 mini-dashboard ADOPTED per operator decision (D02; supersedes the earlier 340×440 glance-and-act recommendation) | KP·Tabs | Pre-P1 — pending G04 audit |
| F18 | Workflow-scoped Run History       | `296:2863`  | Figma            | `src/components/RunHistory.tsx`                                                                        | `src/components/RunHistory.test.tsx`, `e2e/workflow-history.spec.ts`                                                   | —                                  | Partial: heatmap (keyboard-accessible cells, #244) + table + search/status filter + rerun exist; workflow-level manual run enters admission control (#229; manual-run standardization completed #263/#266); async load failures announce via role=alert (#248); rows surface snapshotted run_environment (schema v13); sticky Lookback/KPIs pending                                                                                                                     | — (DR02 redesign)                                                                                                        | KP·Tabs | Pre-P1 — pending G04 audit |
| F19 | Collapsed Daily workflows         | `424:15842` | Figma            | `src/components/WorkflowList.tsx` _(collapsed-group state)_                                            | `e2e/workflows-errors.spec.ts` _(fixture pending)_                                                                     | —                                  | **Missing** demo state (collapsible groups not built)                                                                                                                                                                                                                                                                                                                                                                                                                   | —                                                                                                                        | KP·Tabs | Pre-P1 — pending G04 audit |
| F20 | Workflow builder / Edit           | `424:17176` | shipped-behavior | `src/components/WorkflowEditor.tsx` _(edit mode)_                                                      | `src/components/WorkflowEditor.test.tsx`, `e2e/workflow-editor.spec.ts`                                                | via `ScheduleBuilder`              | Implemented/partial: edit + read-only exist; delete intentionally on list                                                                                                                                                                                                                                                                                                                                                                                               | Delete on list, not editor (D04 accept)                                                                                  | KP·Tabs | Pre-P1 — pending G04 audit |
| F21 | Queued workflow row               | `424:17460` | Figma            | `src/components/WorkflowCard.tsx` _(`activity="waiting"`)_                                             | `src/components/WorkflowCard.test.tsx`                                                                                 | via `WorkflowCard`                 | Partial transient state (not a persistent row treatment)                                                                                                                                                                                                                                                                                                                                                                                                                | Card treatment vs row (unify in code, D04)                                                                               | KP·Tabs | Pre-P1 — pending G04 audit |
| F22 | Disabled workflow filter          | `424:19197` | Figma            | `src/components/WorkflowList.tsx` _(filter state)_                                                     | `e2e/workflows-errors.spec.ts` _(fixture pending)_                                                                     | —                                  | **Missing** demo state (Active/Disabled filter not built)                                                                                                                                                                                                                                                                                                                                                                                                               | —                                                                                                                        | KP·Tabs | Pre-P1 — pending G04 audit |
| F23 | Prefilled History search          | `424:19587` | Figma            | `src/components/GlobalHistory.tsx` _(prefilled search)_                                                | `e2e/global-history.spec.ts` _(fixture pending)_                                                                       | —                                  | **Missing** demo state                                                                                                                                                                                                                                                                                                                                                                                                                                                  | —                                                                                                                        | KP·Tabs | Pre-P1 — pending G04 audit |
| F24 | Running workflow row              | `424:19978` | Figma            | `src/components/WorkflowCard.tsx` _(`activity="submitting"`)_                                          | `src/components/WorkflowCard.test.tsx`                                                                                 | via `WorkflowCard`                 | Partial transient state (not a persistent row treatment)                                                                                                                                                                                                                                                                                                                                                                                                                | Card treatment vs row (unify in code, D04)                                                                               | KP·Tabs | Pre-P1 — pending G04 audit |

**Screen ledger notes**

- **Landed since the last snapshot (P4/P5 work):** manual run CTAs across
  `F07/F12/F17/F18` now enter scheduler admission control (`#229`); the tray
  popup reached semantic + visual parity at the retained 340×440 glance size
  (`#237`/`#238`); the run read model + `runs` rows snapshot the run-time
  environment (schema v13, `Decision-4`, `#251`) so re-homing a workflow never
  re-buckets its history; and accessibility hardening landed for run-detail
  assistive-tech status (`#239`), the warning status dot (`#240`, D04),
  keyboard-accessible failure-heatmap cells (`#244`), async load-failure
  `role=alert` (`#248`), and InfoTip Escape-dismiss + WCAG-AA contrast
  (`#246`/`#249`).
- **Decision-3 (global queue-only) manual-run standardization — COMPLETE:** all
  manual runs (desktop UI + REST + SDK + MCP) now route through scheduler
  admission control. The immediate-execution IPC command `trigger_workflow` was
  **removed** (`#266`); `rerun_workflow` now routes through the admission choke
  point `dispatch_manual_run`, and its queued/admitted/duplicate outcome is
  surfaced in the Run History UI (`#263`/`#264`). The misleadingly-named external
  surfaces were deprecated — SDK `runWorkflow` is `@deprecated` (`#265`) and MCP
  `run_workflow_now` is marked deprecated (`#267`), both already enqueueing
  underneath — and the false "dispatches immediately" docs were corrected
  (`#268`).
- **D05 (`F10` run-detail agent actions) — safe opt-in backend SHIPPED,
  disabled-by-default; UI pending:** the plan-first-reviewed backend contract for the
  run-detail "agent actions" ("Open in Cursor / Dispatch fix agent") is merged &
  green on `main` across 5 PRs (`#275`/`#277`/`#278`/`#279`/`#281`), with the feature
  **disabled by default and no UI surface yet**. Every safety invariant carries a
  biting failing-first test: backend `open_url` scheme guard (allow only
  `http`/`https`/`cursor`, reject a leading `-`), symlink-safe path confinement
  anchored to the app `workspace_root`, prompt-fencing of untrusted `stderr` + secret
  non-leak (`error_analysis` dropped), a mandatory dispatch rate cap, a namespaced
  idempotency key, no-hijack of `rerun`/`backfill`, and a per-dispatch audit row. UI
  exposure (Settings "Cursor integration" section + RunDetail actions + consent
  `Modal`) is **pending**. **Revised operator decision (PROPOSE-ONLY):** the
  fix agent now targets the REAL repo (production included) but the seam forces
  `auto_create_pr=true` so it can only open a reviewable **DRAFT PR** — a human
  reviews + merges; it is NEVER auto-merged or auto-applied (the app has no
  PR-merge code path). This DROPS the earlier non-production / sandbox target
  gate and the forced `auto_create_pr=false`; every other guardrail
  (opt-in/consent, rate cap, hard-guard duplicate dispatch, audit, prompt-fence,
  path confinement, idempotency) is retained. Figma-mock-first then code.
- **G00 (hybrid) — first bless/mirror surface landed:** `F15 Settings` was mirrored
  **code→design as v1** in Figma (frame `633:5682`, page `0:1`), built from
  `cs.*`-bound master instances and verified against the committed
  `settings-linux.png` baseline. This is the first G00 bless/mirror surface; further
  shipped surfaces remain in progress (G00 otherwise still open).
- **Repo is ahead of the roadmap baseline** for the Mission Control surfaces: the
  `D07` chart primitives are already composed into `Overview` (race-track,
  status donut, dual-axis trend), `OperationalHealth` (dual-axis), `NeedsAttention`
  (impact bars), and `Resources` (gauge + queue line). The roadmap's "missing
  screen" baseline predates this; status here reflects current `main`.
- `F06` is the only screen **removed** from production scope (D03), and only via
  consolidation into `F11`; no other designed surface is removed.
- Demo/transient states `F19/F22/F23` are **missing** (need deterministic fixtures
  per `G06`); `F21/F24` exist only as transient `WorkflowCard` activity states, not
  persistent row treatments.

---

## 2. Component ledger (C01–C41)

Variants are quoted exactly from the roadmap. `Figma node` is the mapped
component-set node; `pending G04 audit` in that column means the master exists in
Figma (one of the unmapped groups) but its node ID is not derivable from the repo
and must be read by the `G04` plugin audit.

| ID  | Component (variants)                                                            | Figma node        | Auth             | Code path                                                                                     | Test path                                                  | CC     | Status                                                                                                                                                                                                          | Accepted divergence                                             | Owner   | Last-verified Figma ver.          |
| --- | ------------------------------------------------------------------------------- | ----------------- | ---------------- | --------------------------------------------------------------------------------------------- | ---------------------------------------------------------- | ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------- | ------- | --------------------------------- |
| C01 | NavItem — Default/Active/Hover                                                  | `50:127`          | Figma            | `src/components/NavItem.tsx`                                                                  | `src/components/NavItem.test.tsx`                          | ✓ live | Implemented                                                                                                                                                                                                     | —                                                               | KP·DS   | Published lib — pending G04 audit |
| C02 | ThemeToggle — Dark/System/Light                                                 | `90:439`          | shipped-behavior | `src/components/ThemeToggle.tsx`                                                              | via `src/components/Sidebar.test.tsx` (no dedicated spec)  | ✓ live | Implemented                                                                                                                                                                                                     | Light theme is a code extension (D04 accept)                    | KP·DS   | Published lib — pending G04 audit |
| C03 | BrandMark                                                                       | `186:1241`        | repo-SOT         | `src/components/BrandMark.tsx`                                                                | `src/components/BrandMark.test.tsx`                        | ✓ live | Implemented (static asset; single glyph, no variants)                                                                                                                                                           | —                                                               | KP·DS   | Published lib — pending G04 audit |
| C04 | StatusPill — Succeeded/Running/Failed/Warning                                   | `49:124`          | Figma            | `src/components/StatusBadge.tsx`                                                              | `src/components/StatusBadge.test.tsx`                      | ✓ live | Implemented as `StatusBadge`; Warning→`poll_exhausted`                                                                                                                                                          | Name drift Pill→Badge (unify in code)                           | KP·DS   | Published lib — pending G04 audit |
| C05 | StatusBar                                                                       | `60:145`          | Figma            | `src/components/StatusBar.tsx`                                                                | `src/components/StatusBar.test.tsx`                        | ✓ live | Implemented (**ahead of baseline** — roadmap said missing); segments are a code-only data seam                                                                                                                  | —                                                               | KP·DS   | Published lib — pending G04 audit |
| C06 | InfoTip — Rest/Hover                                                            | `115:531`         | Figma            | `src/components/InfoTip.tsx`                                                                  | `src/components/InfoTip.test.tsx`                          | ✓ live | Implemented (**ahead of baseline**); hover/focus reveal + Escape-dismiss (#246) + WCAG-AA glyph contrast with axe un-suppressed (#249); glossary rows code-only                                                 | — (satisfies G09 InfoTip convention)                            | KP·DS   | Published lib — pending G04 audit |
| C07 | Tooltip                                                                         | pending G04 audit | Figma            | _(no dedicated component; click-driven usage at call sites)_                                  | —                                                          | —      | Partial, **divergent** (click-driven vs hover); superseded by `InfoTip` (C06) for metric tips                                                                                                                   | Click Tooltip divergence (unify in code)                        | KP·DS   | Pre-P1 — pending G04 audit        |
| C08 | ActionButton — Neutral/Primary/Ghost/Disabled/Running                           | `113:526`         | Figma            | `src/components/Button.tsx`                                                                   | `src/components/Button.test.tsx`                           | ✓ live | Implemented as `Button`; Disabled/Running→`disabled`/`loading`                                                                                                                                                  | Code-only `danger` variant + `size` unmapped (D04: composition) | KP·DS   | Published lib — pending G04 audit |
| C09 | EnvSelect — Production/Sandbox                                                  | `121:540`         | shipped-behavior | `src/components/EnvSelect.tsx`                                                                | `src/components/EnvSelect.test.tsx`                        | ✓ live | Implemented with dynamic-environment extension                                                                                                                                                                  | Dynamic environments beyond 2 fixed (D04 accept)                | KP·DS   | Published lib — pending G04 audit |
| C10 | LookbackSelect — 1d/3d/7d/30d                                                   | `121:585`         | Figma            | `src/components/LookbackSelect.tsx`                                                           | `src/components/LookbackSelect.test.tsx`                   | ✓ live | Implemented (**ahead of baseline**); trailing Custom segment                                                                                                                                                    | — (satisfies G09 Lookback convention)                           | KP·DS   | Published lib — pending G04 audit |
| C11 | ChartHover — Rest/Hover                                                         | `520:4262`        | Figma            | `src/components/charts/ChartTooltip.tsx`                                                      | `src/components/charts/ChartTooltip.test.tsx`              | ✓ live | Implemented as chart `ChartTooltip` primitive (**ahead of baseline**, D07)                                                                                                                                      | — (DR01 realization)                                            | KP·MC   | Published lib — pending G04 audit |
| C12 | StatCard — Resting/Hover/Expanded                                               | `53:132`          | Figma            | `src/components/StatCard.tsx`                                                                 | `src/components/StatCard.test.tsx`                         | ✓ live | Partial: resting implemented; delta pill + expanded sparkline are design-forward, no code state                                                                                                                 | Expanded/delta not yet coded (DR01)                             | KP·MC   | Published lib — pending G04 audit |
| C13 | KPITile — Info/Warning/Success/Danger/Neutral                                   | pending G04 audit | Figma            | _(partial analogue via `src/components/StatCard.tsx` + `missionControl/groupCard.tsx`)_       | `src/components/missionControl/*Data.test.ts`              | —      | Partial analogue only; tone variants not modeled                                                                                                                                                                | — (DR01 redesign)                                               | KP·MC   | Pre-P1 — pending G04 audit        |
| C14 | ConsumerRow — Normal/Elevated                                                   | pending G04 audit | Figma            | _(none — not built)_                                                                          | —                                                          | —      | **Missing**                                                                                                                                                                                                     | — (DR01 redesign)                                               | KP·MC   | Pre-P1 — pending G04 audit        |
| C15 | AlertRow — Error/Warning                                                        | pending G04 audit | Figma            | _(partial inline — SLA banner in `src/components/overview/Overview.tsx`)_                     | `src/components/overview/overviewData.test.ts`             | —      | Partial inline; no standalone Error/Warning row component                                                                                                                                                       | — (DR01 redesign)                                               | KP·MC   | Pre-P1 — pending G04 audit        |
| C16 | UpcomingRunCard — Resting/Hover                                                 | pending G04 audit | Figma            | _(partial inline — upcoming slice in `src/components/RunsTable.tsx`)_                         | `src/components/RunsTable.test.tsx`                        | —      | Partial inline (no dedicated card)                                                                                                                                                                              | — (DR01 redesign)                                               | KP·MC   | Pre-P1 — pending G04 audit        |
| C17 | SectionHeader — Expanded/Collapsed                                              | pending G04 audit | Figma            | _(partial inline — `missionControl/groupCard.tsx`, `src/components/PageHeader.tsx`)_          | `src/components/missionControl/*Data.test.ts`              | —      | Partial inline, no collapse behavior                                                                                                                                                                            | — (DR01 redesign)                                               | KP·MC   | Pre-P1 — pending G04 audit        |
| C18 | RecentRunRow — Resting/Hover                                                    | pending G04 audit | Figma            | _(partial inline — recent slice in `src/components/RunsTable.tsx`)_                           | `src/components/RunsTable.test.tsx`                        | —      | Partial inline (no dedicated row)                                                                                                                                                                               | — (DR01 redesign)                                               | KP·MC   | Pre-P1 — pending G04 audit        |
| C19 | DonutChart                                                                      | `524:4262`        | Figma            | `src/components/charts/StatusDonut.tsx`                                                       | `src/components/charts/StatusDonut.test.tsx`               | ✓ live | Implemented as `StatusDonut` primitive (**ahead of baseline**, D07)                                                                                                                                             | — (DR01 realization)                                            | KP·MC   | Published lib — pending G04 audit |
| C20 | AreaTrend                                                                       | `521:4262`        | Figma            | `src/components/charts/DualAxisLine.tsx`                                                      | `src/components/charts/DualAxisLine.test.tsx`              | ✓ live | Implemented as `DualAxisLine` (dual-axis line trend) primitive (**ahead of baseline**, D07)                                                                                                                     | Line-trend treatment supersedes area (DR01)                     | KP·MC   | Published lib — pending G04 audit |
| C21 | QueueLine                                                                       | `525:4262`        | Figma            | `src/components/charts/QueueLine.tsx`                                                         | `src/components/charts/QueueLine.test.tsx`                 | ✓ live | Implemented as `QueueLine` primitive (**ahead of baseline**, D07)                                                                                                                                               | — (DR01 realization)                                            | KP·MC   | Published lib — pending G04 audit |
| C22 | ChartTile — Resting/Hover                                                       | pending G04 audit | Figma            | _(no dedicated component; charts wrapped by `missionControl/surfaces.css` + `groupCard.tsx`)_ | `src/components/missionControl/*Data.test.ts`              | —      | Partial (tile is a surface composition, not a component)                                                                                                                                                        | — (DR01 redesign)                                               | KP·MC   | Pre-P1 — pending G04 audit        |
| C23 | CPULineChart                                                                    | pending G04 audit | Figma            | _(realized via `charts/QueueLine`/`DualAxisLine` in `missionControl/Resources.tsx`)_          | `src/components/missionControl/resourcesData.test.ts`      | —      | Partial: no dedicated CPU chart; reuses line primitives                                                                                                                                                         | Reuse of shared line primitive (DR01)                           | KP·MC   | Pre-P1 — pending G04 audit        |
| C24 | MemoryDonut                                                                     | `516:4262`        | Figma            | `src/components/charts/Gauge.tsx`                                                             | `src/components/charts/Gauge.test.tsx`                     | ✓ live | Implemented as `Gauge` (270° arc) primitive; gauge treatment supersedes donut for utilization (**ahead of baseline**, D07)                                                                                      | Gauge treatment supersedes memory donut (DR01)                  | KP·MC   | Published lib — pending G04 audit |
| C25 | ResourcePanel                                                                   | pending G04 audit | Figma            | `src/components/missionControl/Resources.tsx` (composition of `Gauge`+`QueueLine`)            | `src/components/missionControl/resourcesData.test.ts`      | —      | Partial telemetry analogue → composed panel (D03: Resources stays)                                                                                                                                              | — (DR01 redesign)                                               | KP·MC   | Pre-P1 — pending G04 audit        |
| C26 | CustomRangePicker                                                               | pending G04 audit | Figma            | _(partial — Custom segment in `src/components/LookbackSelect.tsx`)_                           | `src/components/LookbackSelect.test.tsx`                   | —      | Partial: Custom segment exists; full range picker not built                                                                                                                                                     | — (DR01 redesign)                                               | KP·MC   | Pre-P1 — pending G04 audit        |
| C27 | RunStatsPanel                                                                   | pending G04 audit | Figma            | _(partial stats analogue — `overview/overviewData.ts`, `GlobalHistory.tsx`)_                  | `src/components/overview/overviewData.test.ts`             | —      | Partial stats analogue; folds into F11 aggregate (D03)                                                                                                                                                          | — (DR01 redesign)                                               | KP·MC   | Pre-P1 — pending G04 audit        |
| C28 | Sidebar — Expanded/Collapsed                                                    | `305:6378`        | Figma            | `src/components/Sidebar.tsx`                                                                  | `src/components/Sidebar.test.tsx`                          | ✓ live | Partial, CC; `collapsed` prop mapped but collapsed behavior incomplete                                                                                                                                          | Collapsed behavior (unify in code, D04)                         | KP·DS   | Published lib — pending G04 audit |
| C29 | NeedsAttentionItem — Collapsed/Collapsed-Warn/Expanded/Debugging/FixReady/Fixed | pending G04 audit | Figma            | `src/components/missionControl/NeedsAttention.tsx`, `needsAttentionData.ts`                   | `src/components/missionControl/needsAttentionData.test.ts` | —      | Partial: item exists; Expanded/Debugging/FixReady/Fixed states pending                                                                                                                                          | — (DR01 redesign)                                               | KP·MC   | Pre-P1 — pending G04 audit        |
| C30 | RunsTable — Active/Upcoming/Recent                                              | `415:8668`        | Figma            | `src/components/RunsTable.tsx`                                                                | `src/components/RunsTable.test.tsx`                        | ✓ live | Partial, CC; `runs` data seam + `onViewRun`; Tab variant is caller-owned                                                                                                                                        | Caller-owned tabs composition (D04 accept)                      | KP·DS   | Published lib — pending G04 audit |
| C31 | WorkflowRow — Idle/Queued/Running/Disabled                                      | `579:4320`        | shipped-behavior | `src/components/WorkflowCard.tsx`                                                             | `src/components/WorkflowCard.test.tsx`                     | ✓ live | Partial/divergent: shipped as **card** with State + Activity props; Queued/Running are transient (F21/F24)                                                                                                      | Card treatment vs row (unify in code, D04)                      | KP·Tabs | Published lib — pending G04 audit |
| C32 | StatusDot — status-dot/mc-dot × Succeeded/Running/Failed/Queued/Warning         | `479:4257`        | repo-SOT         | `src/components/StatusDot.tsx`                                                                | `src/components/StatusDot.test.tsx`                        | ✓ live | Partial, CC; two bases mapped; warning status now maps to the warning color (#240, D04) and run-status canonicalization is unified across KPIs + scheduler gates (#242); residual status semantics/styles drift | Status semantics/styles (unify in code, D04)                    | KP·DS   | Published lib — pending G04 audit |
| C33 | Input — Default/Focus/Error/Disabled                                            | `481:4257`        | Figma            | `src/components/Input.tsx`                                                                    | `src/components/Input.test.tsx`                            | ✓ live | Partial, CC; thin native passthrough. Error/Disabled are design-forward Figma variants excluded from CC (no code prop)                                                                                          | State semantics (unify in code, D04)                            | KP·DS   | Published lib — pending G04 audit |
| C34 | Textarea — Default/Focus/Error/Disabled                                         | `485:4265`        | Figma            | `src/components/Textarea.tsx`                                                                 | `src/components/Textarea.test.tsx`                         | ✓ live | Partial, CC; thin native passthrough; Error/Disabled design-forward, excluded from CC                                                                                                                           | State semantics (unify in code, D04)                            | KP·DS   | Published lib — pending G04 audit |
| C35 | Select — Default/Focus/Disabled                                                 | `486:4266`        | Figma            | `src/components/Select.tsx`                                                                   | `src/components/Select.test.tsx`                           | ✓ live | Partial, CC; thin native passthrough                                                                                                                                                                            | State semantics (unify in code, D04)                            | KP·DS   | Published lib — pending G04 audit |
| C36 | Field                                                                           | `487:4257`        | Figma            | `src/components/Field.tsx`                                                                    | `src/components/Field.test.tsx`                            | ✓ live | Implemented; label + children code-only seams                                                                                                                                                                   | —                                                               | KP·DS   | Published lib — pending G04 audit |
| C37 | SettingsField — Hint With/Without                                               | `488:4268`        | Figma            | `src/components/SettingsField.tsx`                                                            | `src/components/SettingsField.test.tsx`                    | ✓ live | Implemented; Hint variant mirrors optional `hint` (not a code prop). CC example is state-incomplete by design                                                                                                   | —                                                               | KP·DS   | Published lib — pending G04 audit |
| C38 | EditorField — Hint With/Without                                                 | `489:4270`        | Figma            | `src/components/EditorField.tsx`                                                              | `src/components/EditorField.test.tsx`                      | ✓ live | Implemented; CC example state-incomplete by design                                                                                                                                                              | —                                                               | KP·DS   | Published lib — pending G04 audit |
| C39 | SettingsCheck — Checked × Disabled                                              | `490:4277`        | Figma            | `src/components/SettingsCheck.tsx`                                                            | `src/components/SettingsCheck.test.tsx`                    | ✓ live | Implemented; both variants mapped                                                                                                                                                                               | —                                                               | KP·DS   | Published lib — pending G04 audit |
| C40 | PageHeader — Subtitle × Actions                                                 | `491:4290`        | Figma            | `src/components/PageHeader.tsx`                                                               | `src/components/PageHeader.test.tsx`                       | ✓ live | Implemented; variants toggle optional slots (not code props). CC example state-incomplete by design                                                                                                             | —                                                               | KP·DS   | Published lib — pending G04 audit |
| C41 | Modal — Sm/Md/Lg × Footer With/Without                                          | `493:4307`        | Figma            | `src/components/Modal.tsx`                                                                    | `src/components/Modal.test.tsx`                            | ✓ live | Partial, CC; size/footer are consumer-owned shell composition                                                                                                                                                   | Caller-owned size/footer composition (D04 accept)               | KP·Tabs | Published lib — pending G04 audit |

**Component ledger notes**

- **Ahead of baseline:** `C05 StatusBar`, `C06 InfoTip`, `C10 LookbackSelect`, and
  the chart-realized `C11/C19/C20/C21/C24` now exist with code + tests + live CC —
  the roadmap's coverage matrix listed several as _missing_. This is genuine
  post-baseline progress, not an error.
- **Non-code / composition primitives:** `C22 ChartTile`, `C25 ResourcePanel` are
  surface compositions rather than standalone components; `C07 Tooltip`,
  `C13–C18`, `C23`, `C26`, `C27`, `C29` are partial-inline or missing and carry no
  dedicated `*.figma.tsx` (their masters are among Figma's unmapped groups → node
  `pending G04 audit`).
- **Deliberately-excluded CC props:** design-forward Figma variants that the code
  does not (yet) implement — `Input`/`Textarea` Error/Disabled, `StatCard` Expanded
  — are intentionally excluded from the Code Connect prop mappings, which mirror
  only real code props.

### 2a. New chart primitives introduced by DR01 / D07 / D08 (beyond the C01–C41 matrix)

The `D07` bespoke-SVG chart library (`d3-scale`/`d3-shape` math) added primitives
that have **no home in the original C-ID matrix** but are first-class, tested, and
Code-Connected. Recorded here so the matrix stays canonical while nothing is lost.
All bind color to `cs.*`/`--chart-*` tokens (repo-SOT); all live under
`src/components/charts/` and export via `src/components/charts/index.ts`.

| Primitive       | Figma node | Code path                                 | Test path                                      | CC     | Realizes / used by                         | Owner   | Last-verified Figma ver.          |
| --------------- | ---------- | ----------------------------------------- | ---------------------------------------------- | ------ | ------------------------------------------ | ------- | --------------------------------- |
| Axis            | `519:4262` | `src/components/charts/Axis.tsx`          | `src/components/charts/Axis.test.tsx`          | ✓ live | shared axis for all plot charts            | KP·MC   | Published lib — pending G04 audit |
| Legend          | `518:4262` | `src/components/charts/Legend.tsx`        | `src/components/charts/Legend.test.tsx`        | ✓ live | shared categorical legend                  | KP·MC   | Published lib — pending G04 audit |
| ThresholdBand   | `513:4262` | `src/components/charts/ThresholdBand.tsx` | `src/components/charts/ThresholdBand.test.tsx` | ✓ live | QueueLine / Resources zone bands           | KP·MC   | Published lib — pending G04 audit |
| ImpactBars      | `522:4262` | `src/components/charts/ImpactBars.tsx`    | `src/components/charts/ImpactBars.test.tsx`    | ✓ live | Needs Attention compound-impact bars (F04) | KP·MC   | Published lib — pending G04 audit |
| RaceTrack       | `527:4262` | `src/components/charts/RaceTrack.tsx`     | `src/components/charts/RaceTrack.test.tsx`     | ✓ live | D08 experimental running-jobs view (F01)   | KP·MC   | Published lib — pending G04 audit |
| Vehicle         | `559:4262` | `src/components/charts/Vehicle.tsx`       | `src/components/charts/Vehicle.test.tsx`       | ✓ live | RaceTrack racer glyph (D08)                | KP·MC   | Published lib — pending G04 audit |
| ScheduleBuilder | `581:4321` | `src/components/ScheduleBuilder.tsx`      | `src/components/ScheduleBuilder.test.tsx`      | ✓ live | Workflow builder (F09/F20) sub-component   | KP·Tabs | Published lib — pending G04 audit |

> `Gauge` (`516:4262`), `StatusDonut` (`524:4262`), `DualAxisLine` (`521:4262`),
> `QueueLine` (`525:4262`), and `ChartTooltip` (`520:4262`) are listed in §2 as the
> realizations of `C24/C19/C20/C21/C11` respectively.

---

## 3. Design-revision (DR) ledger

Format: `DRnn — surface/component — change summary — data/token impact — status —
approved version`. DRs are the co-designed revisions folded into this contract; the
coverage matrix (§1/§2) is the **starting baseline, not the implementation target**.

| DR       | Tier                     | Surfaces / components                                                                                                                | Change summary                                                                                                                                                                                                | Data / token impact                                                                                                                                                                                                                            | Reference                                                                                            | Status                                                                                                                                                                                                        | Approved Figma ver.        |
| -------- | ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------- |
| **DR01** | Heavy                    | `F01/F03/F05` (+ `F11` hosting rehomed `F06` per D03); `C11 ChartHover`, `C12 StatCard`, `C13 KPITile`, `C16/C18/C29/C30`, `C19–C27` | Rebuild Mission Control charts/graphs/tiles: queue↔worker rollup, blocked/long-running risk tables, threshold-zone line, semicircular gauges, compound-impact bars, interactive donut, experimental race view | New derived metrics (per-job expected runtime, blast radius, queue-util history, per-worker CPU) — several likely **no backend today** (R02, needs viz→data matrix); chart series/threshold colors bind to `cs.*`/`--chart-*` (repo-SOT, DR03) | `enterprise-scheduler-v3.canvas.tsx` (Tier-1 + Tier-2 synthesis)                                     | Primitives built + CC + composed into MC surfaces (P4 in progress); **design not yet approved at G00**                                                                                                        | Pre-P1 — pending G04 audit |
| **DR02** | Light                    | `F07–F11/F17/F18`, demo `F19–F24`; likely `C31 WorkflowRow`, `C41 Modal`                                                             | Lighter-tab refinements (compact rows, filters, collapsible groups, sticky Lookback/search/KPIs, popup scope/size); `F11` also carries the DR01-class rehomed `F06` charts                                    | Mostly layout/state; reuses DR01 chart primitives on `F11`; token impact via DR03                                                                                                                                                              | Per-tab batches (Workflows/Detail/Editor · History/Run Detail · Admin/Settings/Integrations · popup) | Design lane pending (serialized after P1); code landed ahead of design approval — searchable cards (#230), lightened workflow/history hierarchy (#231–#235), popup parity (#237/#238); `WorkflowCard` CC live | Pre-P1 — pending G04 audit |
| **DR03** | Tokens/motion/components | consequences of DR01/DR02                                                                                                            | Token/motion/component changes required by DR01/DR02, **repo-SOT-first** (originate in `tokens/`, mirror to Figma `cs.*`)                                                                                     | New chart palette (`--chart-1..4`) + any motion tier originate in `tokens/*.json` → regenerate → mirror                                                                                                                                        | `tokens/`, `style-dictionary.config.mjs`                                                             | Chart primitives already bind to `cs.*`/`--chart-*`; further token/motion tiers land with each DR01/DR02 change-set                                                                                           | Pre-P1 — pending G04 audit |

### 3a. `D04` accepted-final divergence register (feeds `G12`)

Every accepted-as-final divergence — where **shipped behavior is authoritative** and
Figma is to be updated to match. These must be visible here and reflected back into
Figma before `G12` can pass; none may remain an informal exception. The
"reflected in Figma?" column is a live-Figma fact → `pending G04 audit`.

| #    | Divergence                                                 | Surface / component                                                | Disposition (D04)                                                             | Reflected in Figma? |
| ---- | ---------------------------------------------------------- | ------------------------------------------------------------------ | ----------------------------------------------------------------------------- | ------------------- |
| AD1  | Calibre (design) → self-hosted Inter (code)                | Global typography                                                  | Accept-as-final; update Figma typography note (fonts differ by license)       | pending G04 audit   |
| AD2  | Dynamic environments + code-only light theme               | `C09 EnvSelect`, `F02/F13`, `C02 ThemeToggle`                      | Accept-as-final; code extensions beyond the 2 fixed envs / dark-only design   | pending G04 audit   |
| AD3  | App-owned pinned MCP provisioning (safer than Figma setup) | `F14 Integrations`                                                 | Accept-as-final; richer/safer shipped behavior                                | pending G04 audit   |
| AD4  | Caller-owned RunsTable tabs                                | `C30 RunsTable`                                                    | Accept-as-final; Tab is a composition concern, not a component prop           | pending G04 audit   |
| AD5  | Caller-owned Modal size/footer composition                 | `C41 Modal`                                                        | Accept-as-final; shell + passthrough props                                    | pending G04 audit   |
| AD6  | Agent Activity merged into Mission Control                 | `F16`                                                              | Accept-as-final; no standalone route                                          | pending G04 audit   |
| AD7  | Delete action on Workflows list, not the editor            | `F07/F20`, `C31`                                                   | Accept-as-final; safer placement                                              | pending G04 audit   |
| AD8  | Result webhooks (workflow result delivery)                 | `F09/F20 WorkflowEditor` (`workflow/ActionsEditor.tsx`)            | Accept-as-final; Figma-omitted shipped feature, reflect to Figma (decision-5) | pending D06/G12     |
| AD9  | Email Profiles (per-workflow email notification profiles)  | `F15 Settings` + `F09/F20` editor (`EmailProfiles.tsx`)            | Accept-as-final; Figma-omitted shipped feature, reflect to Figma (decision-5) | pending D06/G12     |
| AD10 | Popup queue-run affordance                                 | `F17 Tray popup` (`MenuBarPopup.tsx`)                              | Accept-as-final; Figma-omitted shipped feature, reflect to Figma (decision-5) | pending D06/G12     |
| AD11 | Workflow-history failure heatmap                           | `F08`/`F18` (`WorkflowDetail.tsx`, `RunHistory.tsx`)               | Accept-as-final; Figma-omitted shipped feature, reflect to Figma (decision-5) | pending D06/G12     |
| —    | **Placeholder — additional accepted-final divergences**    | _(to be appended as `P1–P5` surface implementation confirms them)_ | Recorded here at acceptance time per D04                                      | pending G04 audit   |

> **Unify-in-code (NOT accepted divergences):** collapsed `Sidebar` (C28), workflow
> status filters / collapsible groups (F07), form/status state semantics
> (C32/C33/C34/C35), and the designed duration/filter/InfoTip conventions (G09) are
> divergences to be **fixed in code to match design**, not accepted — tracked in the
> Status/Accepted-divergence columns of §1/§2, excluded from the AD register above.

---

## 4. Open decisions (D00–D08) that drive authority

Context for the authority classifications above (full text: roadmap §5).

| Decision                     | State          | Bearing on this ledger                                                                                                                                                                                                                                                                                                                                                                |
| ---------------------------- | -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D00 Sequencing               | resolved       | Staged overlap; MC design approved before MC code; lighter tabs serial                                                                                                                                                                                                                                                                                                                |
| D01 Main viewport            | recommend      | Keep 960×680 default; 1280 as wide responsive — sets `G07` screenshot geometry                                                                                                                                                                                                                                                                                                        |
| D02 Popup role               | resolved       | Operator adopted the 384×590 mini-dashboard (shipped #269); the earlier 340×440 recommendation is superseded — `F17` is now the 384×590 mini-dashboard, not a 340×440 divergence                                                                                                                                                                                                      |
| D03 Mission Control depth    | resolved       | NA + OH as MC drill-downs; **Resources stays**; `F06`→`F11` consolidation                                                                                                                                                                                                                                                                                                             |
| D04 Divergence disposition   | recommend      | Defines the §3a accepted-final set vs unify-in-code set (`G12`)                                                                                                                                                                                                                                                                                                                       |
| D05 Run-detail agent actions | in progress    | **Safe opt-in backend**, **shipped disabled-by-default** for `F10` "Open in Cursor/Dispatch fix agent" (#275/#277/#278/#279/#281; failing-first tests per §1); UI pending. **Revised → PROPOSE-ONLY:** real repo (production allowed); seam forces a reviewable **DRAFT PR**, never auto-merged/applied; drops old non-prod/sandbox gate; other guardrails retained. Figma-mock-first |
| D06 Figma write policy       | recommend      | Automate non-destructive post-merge token create/update behind protected env + readback (`G03/G13`)                                                                                                                                                                                                                                                                                   |
| D07 Charting architecture    | resolved       | Bespoke in-repo SVG on `d3-scale`/`d3-shape`; the §2a primitive set                                                                                                                                                                                                                                                                                                                   |
| D08 Experimental viz         | resolved: ship | Race-track front-and-center on `F01`; needs reduced-motion + per-job expected-runtime metric                                                                                                                                                                                                                                                                                          |

---

## 5. Fields marked "pending G04 audit" (consolidated)

Per `G01`, no field is left literally "unknown". The following are genuinely not
determinable from repo + read-only metadata yet and are marked `pending G04 audit`.

1. **Last-verified / approved Figma version — all rows.** No `vNext` has been
   approved (`G00` is open; the coverage matrix is an explicit _baseline, not
   target_). The live Code Connect map (pulled this session for section `113:514`)
   reports `version: "unknown"` for **every** mapped node — there is no version
   pin. Rows therefore carry `Published lib — pending G04 audit` (mapped + live,
   unpinned) or `Pre-P1 — pending G04 audit` (redesign/demo, unapproved). A concrete
   pinned version requires the `G03` live token/version readback + `G04` audit.
2. **Figma node ID for unmapped masters** — `C07 Tooltip`, `C13 KPITile`,
   `C14 ConsumerRow`, `C15 AlertRow`, `C16 UpcomingRunCard`, `C17 SectionHeader`,
   `C18 RecentRunRow`, `C22 ChartTile`, `C23 CPULineChart`, `C25 ResourcePanel`,
   `C26 CustomRangePicker`, `C27 RunStatsPanel`, `C29 NeedsAttentionItem`. These are
   among Figma's unmapped component groups; their node IDs are not derivable from
   the repo and the local desktop MCP was pointed at a FigJam file this session
   (the repo semantic snapshot stores names/types/structure, not descendant node
   IDs). The `G04` plugin/API audit reads them directly.
3. **Node-level remote-component-instance freedom — all rows (global note in §0).**
   The **binding-half is now RESOLVED** (§5a): a live `get_variable_defs` readback
   proves per-master descendant binding, and the 8 earlier primitives have since
   been rebound `affirm.color`/`radius` → `cs.*` — so the 10 `#174` masters + chart
   primitives + `WorkflowCard` + those 8 are all `cs.*`-only on their own nodes.
   What remains is node-level proof that every descendant is a
   zero-remote-**instance** node — `get_variable_defs`/`get_metadata` cannot
   enumerate embedded `INSTANCE` descendants — so this stays **residual `R01`**. A
   Figma plugin-API pass (`node.findAll` on `INSTANCE` + `mainComponent.remote`) has
   already quantified part of it: 36 remote Affirm icon-glyph instances embedded in
   `NavItem`/`Sidebar`/`ThemeToggle` (88 icon-interior remote bindings); the
   remaining rows still need the same pass.
4. **"Reflected in Figma?" for each `D04` accepted divergence (§3a).** Whether the
   shipped-behavior divergence has been mirrored back into the Figma master is a
   live-Figma fact confirmed during the `G04`/`G12` re-sync.

**Not** marked pending: Figma node IDs for mapped components (source-tracked in
each `*.figma.tsx`) and all frame IDs (roadmap coverage matrix); code paths and test
paths (verified in-repo); Code Connect presence (source-tracked, and the 22 DS
masters verified live this session); owner (consolidated under the maintainer per §0).

### 5a. `G04` binding-half audit + rebind — evidence (2026-07-12)

The **binding-half** of the roadmap `G04` audit is **COMPLETE**, established by a
live `get_variable_defs` readback of file `twQmWC8dWT4tqeqIigNsRy`, page
"Mission Control", with the `cs.*` variable collection pinned to Dark. This
resolves the per-descendant `cs.*` **binding-freedom** question (§0 global note; §5
item 3). The 8 earlier-extracted primitives the readback found on legacy
`affirm.color`/`radius` have since been **rebound to `cs.*`** (remediation executed
— see below). The node-level remote-instance half remains residual `R01` (below).

**✅ Clean — bound ONLY to `cs.*`** (`var(--…)` bindings, zero legacy paths):

- The 10 new self-contained `#174` masters — verified: `C32` StatusDot `479:4257`,
  `C33` Input `481:4257`, `C36` Field `487:4257`, `C41` Modal `493:4307`.
- The bespoke `D07` chart primitives (§2a) — verified: `C24` Gauge `516:4262`,
  `C20` DualAxisLine `521:4262`.
- `C31` WorkflowCard `579:4320`.

**✅ Rebound this pass — LEGACY `affirm.color`/`radius` → `cs.*`** (2026-07-12). All
8 earlier-extracted primitives — which predated the retired copy-detach approach and
which the audit had found on raw legacy paths such as `text/default`,
`bg/surface/primary`, `icon/brand/indigo`, `icon/usercomm/*`, `border/onsurface/*`,
`onSurface/*` (**not** `cs.*`) — were rebound off the remote
`affirm.color`/`affirm.radius`/`radius` collections onto the local `cs.*`
collection. The rebind was **structure-preserving, screenshot-verified, and
non-destructive**, and re-read live to confirm **0 remote color + 0 remote radius**
variable bindings on each master's own nodes. `StatCard`, `ThemeToggle`, and
`Sidebar` additionally had remote radius rebound to `cs.radius`.

| C-ID | Master      | Figma node | Radius → `cs.radius` |
| ---- | ----------- | ---------- | -------------------- |
| C08  | Button      | `113:526`  | —                    |
| C04  | StatusBadge | `49:124`   | —                    |
| C12  | StatCard    | `53:132`   | ✓                    |
| C01  | NavItem     | `50:127`   | —                    |
| C28  | Sidebar     | `305:6378` | ✓                    |
| C09  | EnvSelect   | `121:540`  | —                    |
| C30  | RunsTable   | `415:8668` | —                    |
| C02  | ThemeToggle | `90:439`   | ✓                    |

**Color-divergence side-effect — RESOLVED by the rebind.** The legacy bindings had
also differed slightly from the token SOT — e.g. legacy `text/default` `#f7f7f8` vs
`cs` `--text-primary` `#e8eaed`; legacy `bg/surface/primary` `#252531` vs `cs`
`--bg-secondary` `#1a1d27` / `--bg-tertiary` `#242736`. Rebinding these 8 masters'
own-node fills/strokes onto `cs.*` removes that marginal color divergence together
with the external dependency.

**Remediation — EXECUTED (2026-07-12).** Those 8 masters' fills/strokes were rebound
`affirm.color`/`radius` → `cs.*`. Because this was a **WRITE on published library
masters**, it was gated on decision **`D06` (Figma write policy)** + explicit
operator confirmation; it was carried out structure-preserving and non-destructive,
then re-read live to confirm 0 remote color + 0 remote radius variable bindings
remain on each master's own nodes, with a per-master screenshot check.

**Remote-instance half — residual gap `R01` (still OPEN, distinct from the
binding-half above).** A master can still embed **remote Affirm icon-glyph
`INSTANCE`s**, whose interior bindings the fill/stroke rebind above cannot touch. A
Figma plugin-API pass has now quantified part of this: **36 remote Affirm icon-glyph
instances** remain embedded — `NavItem` (3), `Sidebar` (52 interior), `ThemeToggle`
(33) — i.e. **88 icon-interior remote bindings**. Closing this requires replacing
those instances with **local vector copies**; that is a structural change, is
**operator-gated**, and is **NOT yet done** — so `R01` stays open. Full
per-descendant enumeration for the remaining masters still needs the same pass
(`node.findAll` on `INSTANCE` + `mainComponent.remote`). Proxy for the binding-half:
the rebound + clean masters now expose **zero** `affirm.color` bindings on their own
nodes.

**Corroborating (already-known, kept consistent).** `G03` token mirror **confirmed
accurate** (live Dark `cs.*` values match `figma-tokens.json`); `G05`/`G08` Code
Connect **live-confirmed** (Button set `113:526` → `src/components/Button.tsx`, all
5 variants mapped).

---

### Provenance

- **IDs, frame node IDs, DR/D definitions, baseline status:**
  `.cursor/plans/design-to-code_completion_7b6a5788.plan.md` (§1 coverage matrix, §5
  risks/decisions).
- **Component node IDs + Code Connect mappings:** the source-tracked
  `src/**/*.figma.tsx` files + `figma.config.json`; live map verified via the
  published Code Connect map for file `twQmWC8dWT4tqeqIigNsRy`, section `113:514`.
- **Code / test paths & current status:** the repo working tree at `origin/main`.
- **DR01 reference:** `enterprise-scheduler-v3.canvas.tsx`.
- **Semantic snapshot:** `design/figma/mission-control.snapshot.json`
  (`figma-semantic-snapshot/v1`; names/types/structure, no descendant node IDs or
  document version).
