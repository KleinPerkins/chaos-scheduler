# ADR 0009: The D05 fix-agent is propose-only — never auto-merged or auto-applied (born-draft PR, trusted-local-tool threat model)

Status: accepted — 2026-08-23

## Decision

The optional, disabled-by-default D05 "fix agent" (the run-detail "Open in Cursor / Dispatch fix
agent" capability on a failed run) is **propose-only** on both its cloud and local paths. It may
target the real repository (production included) but is **never auto-merged and never auto-applied**:

- The **scheduler** opens a born-`--draft` pull request — it forces the cloud agent's
  `auto_create_pr=false` and then runs `gh pr create --draft --base main --head <branch>`, so the
  fix PR is auto-merge-**ineligible** by construction (PR2e "Option C", #313, which reversed #284's
  "the cloud agent opens the PR" contract).
- The seam **never sets `workOnCurrentBranch`** — a fix always lands on a new branch, never the
  working branch.
- The **local** path is rerun-gated: a credential-scrubbed edit agent proposes a fix locally, the
  scheduler **re-runs the failed job** in the workflow's real environment, and only on a **green**
  rerun does it open the same human-reviewed draft PR.

The threat model is **trusted-local-tool** (no OS sandbox): the edit agent runs credential-scrubbed
(`GITHUB_TOKEN`/`GH_TOKEN` and planted `credential.helper`/`core.sshCommand` neutralized), the
validation rerun keeps the workflow's real env for faithful validation, there is no downstream
cascade, and the scheduler's own git steps are hook-hijack-proof — a **PR-base preflight refuses**
to run the local fix when `origin/main` is not an ancestor of `HEAD`
(`git merge-base --is-ancestor`). The capability ships **disabled-by-default with no UI surface**;
exposing it in the UI is roadmap milestone M1 of `docs/plans/design-to-code-completion-v1.md` and
must not weaken any invariant here.

## Why

- **Alternatives considered.** (a) Let the fix agent auto-apply or auto-merge a fix (e.g. commit to
  the working branch, or open a mergeable PR) — rejected: an automated code change reaching a
  protected branch or production without human review is an unacceptable blast radius for a desktop
  tool that holds real scheduler and repository credentials. (b) Keep #284's contract where the
  **cloud agent** opens its own PR — rejected: it raced the `chaos-scheduler-automerge` App and could
  produce a non-draft, auto-merge-eligible PR before the scheduler could constrain it. (c) The
  **scheduler** deterministically opens a born-`--draft` PR on a new branch, never auto-applied,
  always human-reviewed, with the local path additionally gated on a green validation rerun — chosen.
- **Evidence.** Backend safety foundation (#275/#277/#278/#279/#281), cloud propose-only path
  (#284), local rerun-gated path (#286/#287/#288/#289), and the born-draft "Option C" reversal
  (#313) are all merged and green; each safety invariant carries a biting failing-first test. The
  decision, the born-draft reversal, and the credential-boundary and PR-base-preflight fixes are
  recorded in `design/divergence-ledger.md` (§5d and the `D05` decision row). The feature is inert
  (`#[allow(dead_code)]`) until the M1 UI wires it.

## Consequences

- **Enables.** A safe, opt-in "fix my failed run" capability that always yields a reviewable draft
  PR; the cloud and local paths converge on the identical human-reviewed draft-PR outcome, so the UI
  and the reviewer see one contract.
- **Forecloses.** No backend or UI path may auto-merge or auto-apply a fix, set
  `workOnCurrentBranch`, or open a non-draft fix PR. The M1 UI only _surfaces_ this backend; it adds
  no PR-merge path and cannot weaken the born-draft/new-branch/rerun-gate guarantees.
- **Invariant to keep true.** Propose-only + born-draft + new-branch-only + credential-scrubbed
  agent + green-rerun-gated local path + hook-hijack-proof git with a PR-base preflight — each
  retaining its failing-first test. Any change that relaxes one of these is a regression against this
  ADR.
