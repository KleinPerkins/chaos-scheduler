//! D05 CLOUD fix path — cloud non-draft hardening (PR2e), **Option C
//! (race-free, born-draft)**. The pure, independently-testable decisions the
//! cloud post-run seam composes, plus the thin runner-injected orchestration
//! over them — mirroring `fix_local`'s posture of keeping each security-critical
//! decision a pure function that carries its own failing-first test, free of the
//! seam's process I/O.
//!
//! **The risk.** This repo's GitHub App auto-merge bot
//! (`.github/workflows/app-auto-merge.yml`) arms squash auto-merge AND posts an
//! approval at PR CREATION (`opened` / `ready_for_review`) for ANY `draft ==
//! false`, same-repo PR (release-please excepted). So a machine-authored fix PR
//! that is ever born NON-draft could AUTO-MERGE with NO human review, defeating
//! the LOCKED D05 invariant that EVERY fix path (cloud AND local) converges on a
//! HUMAN-REVIEWED DRAFT PR that is never auto-merged.
//!
//! **What #284 shipped, and why it was replaced.** #284 forced the cloud agent
//! config `auto_create_pr=true`, so the Cursor CLOUD AGENT ITSELF opened the PR
//! and the app merely RELIED on Cursor Cloud's *documented default* of drafting a
//! programmatic PR (an accepted external dependency, never app-enforced). But a
//! cloud-opened PR is born NON-draft, so there was a real WINDOW in which an
//! auto-merge-eligible machine PR existed before any post-hoc fix could react.
//! An interim best-effort backstop (detect-non-draft → `gh pr ready --undo` +
//! alert) narrowed but could not CLOSE that window — it is inherently a race.
//!
//! **Option C (this module) — the SCHEDULER opens the draft PR.** The config
//! overlay now forces `auto_create_pr=false` (see
//! [`crate::scheduler`]'s `fix_agent_config_overlay`), so the cloud agent ONLY
//! pushes its `cursor/…` branch and opens NO PR. The scheduler then opens the PR
//! itself with `gh pr create --draft --base main --head <branch>` and an
//! app-authored title/body ([`build_cloud_fix_pr_title`] / [`build_cloud_fix_pr_body`],
//! modeled on `fix_local`'s M4 builders). Born-`--draft` ⇒ auto-merge-INELIGIBLE
//! ⇒ RACE-FREE: there is no moment at which a non-draft cloud fix PR exists. This
//! REVERSES #284's "the cloud agent opens the PR" mechanism and UNIFIES the CLOUD
//! path with the LOCAL path (which already opens its own draft PR).
//!
//! **Branch-name safety (primary path).** `<branch>` originates from the cloud
//! agent's `git.branches[]`, so it is VALIDATED ([`is_valid_cloud_fix_branch`])
//! to a safe `cursor/…` charset before it is ever passed to `gh` (as a Vec argv
//! — never shell-interpolated) so it cannot smuggle extra flags/args. A branch
//! that fails validation is REFUSED (no PR opened) and alerted.
//!
//! **Defense-in-depth fallback.** If a FUTURE Cursor change ignores
//! `auto_create_pr=false` and the agent UNEXPECTEDLY returns a `pr_url`, the
//! born-draft primary cannot apply — so the original best-effort path remains as
//! a FALLBACK: DETECT the PR's real draft state and, if NON-draft, CONVERT it
//! back to a draft ([`harden_cloud_fix_pr_draft`]); on an unverifiable state or a
//! failed conversion, FAIL CLOSED with an operator-visible alert. The pure
//! [`decide_cloud_fix_pr_action`] expresses BOTH branches (born-draft PRIMARY vs
//! convert-to-draft FALLBACK).

use crate::service::ProcessRunner;
use serde_json::{json, Value};

/// Base branch the scheduler-opened DRAFT cloud fix PR targets — the same
/// constant base as the LOCAL path (`service::FIX_LOCAL_PR_BASE`).
pub const FIX_CLOUD_PR_BASE: &str = "main";

/// The branch namespace a Cursor Cloud agent pushes its work to. The scheduler
/// only opens a born-draft PR for a branch under this prefix (defense in depth:
/// even a hostile `git.branches[]` value must look exactly like a Cursor push).
pub const CLOUD_FIX_BRANCH_PREFIX: &str = "cursor/";

// ---------------------------------------------------------------------------
// Option C — PRIMARY pure decision: what to do with the cloud fix outcome.
// ---------------------------------------------------------------------------

/// The action the cloud post-run seam takes, decided PURELY from the cloud
/// outcome's surfaced `pr_url` + pushed branch. This expresses BOTH the Option C
/// born-draft PRIMARY and the convert-to-draft FALLBACK in one place.
#[derive(Debug, PartialEq, Eq)]
pub enum CloudFixPrAction {
    /// PRIMARY (Option C): the agent pushed a VALID `cursor/…` branch and opened
    /// NO PR. The SCHEDULER opens a born-`--draft` PR against it (race-free —
    /// never auto-merge-eligible).
    OpenDraftPr { branch: String },
    /// FALLBACK / defense-in-depth: the agent UNEXPECTEDLY returned a `pr_url`
    /// (a future Cursor change ignored `auto_create_pr=false`). The born-draft
    /// primary cannot apply, so ensure the EXISTING PR is a draft via the
    /// detect→convert path ([`harden_cloud_fix_pr_draft`]).
    HardenExistingPr { pr_url: String },
    /// FAIL CLOSED: a branch was pushed but its name did NOT pass validation
    /// (injection-y / not a `cursor/…` name). Do NOT open a PR against it; alert.
    /// Carries the raw (rejected) name for the operator alert only — it is NEVER
    /// used in a `gh` argv.
    AlertInvalidBranch { branch: String },
    /// Nothing to do: no PR and no pushed branch (a failed / poll-exhausted run,
    /// or a run that pushed nothing).
    Noop,
}

/// PURE Option-C decision. `pr_url` takes PRECEDENCE: an already-existing PR
/// must be hardened, never duplicated by opening a second one. Otherwise a
/// pushed branch that passes [`is_valid_cloud_fix_branch`] opens the born-draft
/// PR (PRIMARY); a pushed branch that FAILS validation fails closed to an alert;
/// and neither present is a no-op. Inputs are the exact `Option<&str>` values the
/// seam reads from `outcome.details` (`pr_url` / `pushed_branch`), trimmed and
/// emptiness-filtered here so the seam stays a thin reader.
pub fn decide_cloud_fix_pr_action(
    pr_url: Option<&str>,
    pushed_branch: Option<&str>,
) -> CloudFixPrAction {
    if let Some(pr) = pr_url.map(str::trim).filter(|s| !s.is_empty()) {
        return CloudFixPrAction::HardenExistingPr {
            pr_url: pr.to_string(),
        };
    }
    match pushed_branch.map(str::trim).filter(|s| !s.is_empty()) {
        Some(branch) if is_valid_cloud_fix_branch(branch) => CloudFixPrAction::OpenDraftPr {
            branch: branch.to_string(),
        },
        Some(branch) => CloudFixPrAction::AlertInvalidBranch {
            branch: branch.to_string(),
        },
        None => CloudFixPrAction::Noop,
    }
}

/// Whether a cloud-agent-supplied branch name is safe to pass to `gh pr create
/// --head`. The name comes from the cloud agent's `git.branches[]`, so it is
/// UNTRUSTED input: it must match a strict `cursor/…` shape so it can never be
/// read as an extra flag/arg (belt-and-suspenders on top of passing the argv as
/// a Vec — never a shell string). Requires: the expected `cursor/` prefix; the
/// charset `[A-Za-z0-9._/-]` only; no leading `-`; no `..` (git ref rule + path
/// traversal); and a sane length bound.
pub fn is_valid_cloud_fix_branch(branch: &str) -> bool {
    !branch.is_empty()
        && branch.len() <= 255
        && branch.starts_with(CLOUD_FIX_BRANCH_PREFIX)
        && !branch.starts_with('-')
        && !branch.contains("..")
        && branch
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '/' | '-'))
}

// ---------------------------------------------------------------------------
// Option C — app-authored DRAFT PR (title / body / argv), scheduler-opened.
// ---------------------------------------------------------------------------

/// The plainly-stated limit of a machine fix, embedded verbatim in the cloud PR
/// body — the CLOUD counterpart to `fix_local::FIX_RERUN_CAVEAT`. A completed
/// Cursor Cloud agent run does NOT prove the fix is correct; human review of the
/// diff on the branch is the sole backstop (M4).
pub const CLOUD_FIX_CAVEAT: &str =
    "This is a machine-authored fix from a Cursor Cloud agent, opened as a DRAFT and NEVER \
auto-merged. A completed agent run does NOT prove the fix is correct — review the diff on the \
branch (the Files changed tab) before marking this PR ready or merging.";

/// App-authored cloud fix PR title. Derived only from the fix workflow name +
/// run id — never agent stdout/stderr. Mirrors `fix_local::build_fix_pr_title`
/// but says "cloud" (the LOCAL builder says "local").
pub fn build_cloud_fix_pr_title(source_workflow_name: &str, source_run_id: &str) -> String {
    format!("fix({source_workflow_name}): automated cloud fix for failed run {source_run_id}")
}

/// App-author the DRAFT PR body for the CLOUD path (M4). Unlike the LOCAL body
/// there is NO local diff / rerun to describe (the agent worked in the cloud), so
/// this frames the fix as a machine change pushed to `<fix_branch>` and points
/// the reviewer at the branch diff, plus the "a passing run ≠ correct" caveat.
/// Never embeds agent free-text; `fix_branch` is pre-validated by
/// [`is_valid_cloud_fix_branch`] (safe charset — cannot break the fenced/inline
/// markdown), so no fence-neutralization is needed here.
pub fn build_cloud_fix_pr_body(
    source_workflow_name: &str,
    source_run_id: &str,
    fix_branch: &str,
) -> String {
    format!(
        "## Automated fix — DRAFT for human review\n\n\
         Chaos Scheduler's CLOUD fix agent (Cursor Cloud) investigated a failed run of \
         **{workflow}** and pushed a proposed fix to branch `{branch}`. **This PR is a DRAFT and is \
         never auto-merged.**\n\n\
         > {caveat}\n\n\
         **Source failed run:** `{run_id}`\n\n\
         The change lives on `{branch}` — review the diff (the Files changed tab) before marking \
         this PR ready or merging.\n",
        workflow = source_workflow_name,
        branch = fix_branch,
        caveat = CLOUD_FIX_CAVEAT,
        run_id = source_run_id,
    )
}

/// `gh pr create` argv for the scheduler-opened born-DRAFT cloud fix PR. Delegates
/// to [`crate::fix_local::gh_pr_create_argv`] so the CLOUD and LOCAL paths emit a
/// BYTE-IDENTICAL `gh pr create --draft --base <base> --head <branch> --title …
/// --body-file …` invocation (the unification Option C is built on). ALWAYS
/// `--draft`; there is deliberately no merge/approve verb anywhere in the fix
/// flow — human review is the sole backstop (M4).
pub fn gh_pr_create_draft_argv(
    base: &str,
    head: &str,
    title: &str,
    body_file: &str,
) -> Vec<String> {
    crate::fix_local::gh_pr_create_argv(base, head, title, body_file)
}

// ---------------------------------------------------------------------------
// FALLBACK pure decision — what to do given an existing PR's observed draft state.
// ---------------------------------------------------------------------------

/// The action the cloud post-run seam takes for a cloud fix PR, decided from its
/// OBSERVED GitHub draft state.
#[derive(Debug, PartialEq, Eq)]
pub enum CloudPrDraftAction {
    /// The PR is PROVEN to be a draft — the D05 invariant already holds; the
    /// auto-merge bot skips it (`draft == false` is false), so no action.
    AlreadyDraft,
    /// The PR is PROVEN to be non-draft — the auto-merge bot may have already
    /// armed squash auto-merge + posted an approval at creation. CONVERT it back
    /// to a draft (`gh pr ready --undo`) to remove it from that eligibility.
    ConvertToDraft,
    /// The PR's draft state could NOT be determined (the probe errored or
    /// returned junk). FAIL CLOSED: an unverifiable invariant is a risk to flag,
    /// never a state to silently assume safe.
    FlagUnverifiable,
}

/// PURE decision: from the PR's observed draft state, decide the seam's action.
/// `Some(true)` = proven draft, `Some(false)` = proven non-draft, `None` = the
/// draft state could not be determined. FAIL CLOSED on `None`: an unverifiable
/// state is treated as a risk to flag, never silently assumed safe — the same
/// fail-closed posture as [`crate::fix_local::fix_pr_base_preflight`].
pub fn cloud_pr_draft_action(is_draft: Option<bool>) -> CloudPrDraftAction {
    match is_draft {
        Some(true) => CloudPrDraftAction::AlreadyDraft,
        Some(false) => CloudPrDraftAction::ConvertToDraft,
        None => CloudPrDraftAction::FlagUnverifiable,
    }
}

// ---------------------------------------------------------------------------
// gh argv builders + probe parsing.
// ---------------------------------------------------------------------------

/// `gh pr view` argv that reads ONLY the PR's draft state as a BARE boolean
/// (`--json isDraft --jq .isDraft` prints `true`/`false`). The app-captured PR
/// URL rides AFTER a `--` separator so a hostile-looking value can never be read
/// as an option (the `--` precedent from `fix_local`/`operators`).
pub fn gh_pr_view_isdraft_argv(pr: &str) -> Vec<String> {
    vec![
        "pr".to_string(),
        "view".to_string(),
        "--json".to_string(),
        "isDraft".to_string(),
        "--jq".to_string(),
        ".isDraft".to_string(),
        "--".to_string(),
        pr.to_string(),
    ]
}

/// `gh pr ready --undo` argv that CONVERTS a non-draft PR BACK to a draft (the
/// only mutation this path performs). The app-captured PR URL rides AFTER a `--`
/// separator. There is deliberately NO `ready`-without-`--undo`, no merge, and
/// no approve verb anywhere in the fix flow — the draft posture is the D05
/// human-review backstop.
pub fn gh_pr_ready_undo_argv(pr: &str) -> Vec<String> {
    vec![
        "pr".to_string(),
        "ready".to_string(),
        "--undo".to_string(),
        "--".to_string(),
        pr.to_string(),
    ]
}

/// Parse the bare `isDraft` boolean printed by `gh pr view … --jq .isDraft`.
/// Returns `None` for anything that is not an exact `true`/`false` (an error
/// string, empty output, junk), so the caller FAILS CLOSED on an unparseable
/// probe rather than guessing.
pub fn parse_is_draft(stdout: &str) -> Option<bool> {
    match stdout.trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Operator-visible alert (app-authored; never agent free-text).
// ---------------------------------------------------------------------------

/// Reason recorded when the cloud fix PR's draft state could not be verified.
pub const CLOUD_DRAFT_UNVERIFIABLE_REASON: &str =
    "could not verify the PR's draft state (gh probe failed or returned no boolean)";
/// Reason recorded when the convert-to-draft attempt itself failed.
pub const CLOUD_DRAFT_CONVERT_FAILED_REASON: &str =
    "convert-to-draft failed (gh pr ready --undo did not succeed)";

/// App-authored, operator-visible alert raised when the cloud fix PR could NOT
/// be guaranteed to be a draft. Derived ONLY from the app-captured PR URL + a
/// fixed reason — NEVER agent free-text (origin is a PUBLIC repo). Surfaced so
/// an operator can manually re-draft (or close) the PR BEFORE the auto-merge bot
/// merges a machine-proposed, unreviewed fix.
pub fn build_nondraft_alert(pr_url: &str, reason: &str) -> String {
    format!(
        "D05 cloud fix PR may be NON-DRAFT and could auto-merge without human review: {reason}. \
         Convert it back to a draft (gh pr ready --undo) or close it: {pr_url}"
    )
}

/// Reason recorded when the scheduler could not open the born-draft PR itself
/// (the `gh pr create` failed, or the PR body file could not be written).
pub const CLOUD_DRAFT_OPEN_FAILED_REASON: &str =
    "the scheduler could not open the draft PR (gh pr create failed or the PR body could not be written)";
/// Reason recorded when the cloud agent's pushed branch name failed validation
/// and the scheduler REFUSED to open a PR against it (fail closed).
pub const CLOUD_INVALID_BRANCH_REASON: &str =
    "the cloud agent's pushed branch name failed validation (expected a safe `cursor/…` name)";

/// App-authored, operator-visible alert for the PRIMARY (born-draft) path when
/// the scheduler did NOT open the draft PR — either the `gh pr create` failed or
/// the pushed branch name was rejected. Derived ONLY from a fixed reason + the
/// (length-capped) branch name for the operator to act on; the branch is DATA
/// here (logged locally, never re-fed to `gh`). Points the operator at the exact
/// manual, always-`--draft` command so a machine fix never lands non-draft.
pub fn build_cloud_open_alert(branch: &str, reason: &str) -> String {
    let branch_display: String = branch.chars().take(120).collect();
    format!(
        "D05 cloud fix: the scheduler did NOT open a draft PR for the machine fix (branch \
         {branch_display:?}): {reason}. Open it MANUALLY as a draft (never let it auto-merge): \
         gh pr create --draft --base {base} --head <branch>",
        base = FIX_CLOUD_PR_BASE,
    )
}

/// Structured `draft_hardening` detail for the PRIMARY path when the scheduler
/// successfully opened the born-draft PR. Kept beside the fallback
/// [`hardening_detail`] so the persisted `cloud_pr_draft` vocabulary lives here.
pub fn opened_draft_detail(branch: &str, pr_url: Option<&str>) -> Value {
    json!({ "cloud_pr_draft": "opened_draft", "branch": branch, "pr_url": pr_url })
}

/// Structured `draft_hardening` detail for an alert (branch rejected, or the
/// scheduler's own `gh pr create` failed). Mirrors the fallback `Alerted` shape.
pub fn alert_detail(reason: &str) -> Value {
    json!({ "cloud_pr_draft": "alert", "reason": reason })
}

// ---------------------------------------------------------------------------
// Runner-injected orchestration over the pure decisions.
// ---------------------------------------------------------------------------

/// The terminal result of [`harden_cloud_fix_pr_draft`], returned to the seam
/// for logging + audit annotation (the seam owns the operator-visible surfaces;
/// this function performs no logging so it stays a testable unit).
#[derive(Debug, PartialEq, Eq)]
pub enum CloudDraftHardening {
    /// The PR was PROVEN a draft — the D05 invariant already held; nothing done.
    AlreadyDraft,
    /// The PR was non-draft and was SUCCESSFULLY converted back to a draft.
    Converted,
    /// The invariant could NOT be guaranteed — the draft state was unverifiable,
    /// or the convert attempt failed — and an operator alert must be raised.
    /// Carries the app-authored reason (never agent free-text).
    Alerted { reason: String },
}

/// Ensure a cloud fix agent's PR is a DRAFT so the auto-merge bot never merges a
/// machine-proposed, unreviewed fix. Composes the pure decisions with the two
/// `gh` probes/mutations via the injected [`ProcessRunner`], so the whole path
/// is unit-testable with a fake runner (no real network / GitHub):
///
/// 1. PROBE the PR's draft state (`gh pr view … --jq .isDraft`); a non-success
///    exit or unparseable output leaves it UNKNOWN.
/// 2. DECIDE via [`cloud_pr_draft_action`] (fail-closed on unknown).
/// 3. On a proven NON-draft, CONVERT (`gh pr ready --undo`); a non-success exit
///    becomes an [`CloudDraftHardening::Alerted`].
///
/// `gh` runs with the scheduler's own inherited credentials (no env additions),
/// mirroring the scheduler's other credentialed `gh` steps.
pub fn harden_cloud_fix_pr_draft(
    runner: &dyn ProcessRunner,
    cwd: Option<&str>,
    pr_url: &str,
) -> CloudDraftHardening {
    let is_draft = match runner.run("gh", &gh_pr_view_isdraft_argv(pr_url), cwd, &[]) {
        Ok(out) if out.status.success() => parse_is_draft(&String::from_utf8_lossy(&out.stdout)),
        // A non-zero gh (unknown PR, auth issue, …) OR a spawn error leaves the
        // draft state UNKNOWN — fail closed rather than assume it is safe.
        _ => None,
    };

    match cloud_pr_draft_action(is_draft) {
        CloudPrDraftAction::AlreadyDraft => CloudDraftHardening::AlreadyDraft,
        CloudPrDraftAction::FlagUnverifiable => CloudDraftHardening::Alerted {
            reason: CLOUD_DRAFT_UNVERIFIABLE_REASON.to_string(),
        },
        CloudPrDraftAction::ConvertToDraft => {
            match runner.run("gh", &gh_pr_ready_undo_argv(pr_url), cwd, &[]) {
                Ok(out) if out.status.success() => CloudDraftHardening::Converted,
                _ => CloudDraftHardening::Alerted {
                    reason: CLOUD_DRAFT_CONVERT_FAILED_REASON.to_string(),
                },
            }
        }
    }
}

/// Render a [`CloudDraftHardening`] as the structured `draft_hardening` value the
/// seam folds into the run-task `details` (operator-visible in run detail /
/// audit). Kept beside the decision so the persisted shape is defined once.
pub fn hardening_detail(result: &CloudDraftHardening) -> Value {
    match result {
        CloudDraftHardening::AlreadyDraft => json!({ "cloud_pr_draft": "already_draft" }),
        CloudDraftHardening::Converted => json!({ "cloud_pr_draft": "converted_to_draft" }),
        CloudDraftHardening::Alerted { reason } => {
            json!({ "cloud_pr_draft": "alert", "reason": reason })
        }
    }
}

// ---------------------------------------------------------------------------
// Option C — PRIMARY runner-injected orchestration: scheduler opens the draft PR.
// ---------------------------------------------------------------------------

/// The terminal result of [`open_cloud_fix_draft_pr`], returned to the seam for
/// logging + audit annotation (the seam owns the operator-visible surfaces).
#[derive(Debug, PartialEq, Eq)]
pub enum CloudDraftPrOpen {
    /// The born-DRAFT PR was opened by the scheduler. Carries the parsed PR URL
    /// when `gh pr create` printed one (best-effort — success does not depend on
    /// parsing it).
    Opened { pr_url: Option<String> },
    /// The scheduler could NOT open the PR (body write failed, or `gh pr create`
    /// exited non-zero / could not spawn). The seam alerts + fails closed.
    Failed,
}

/// Extract the PR URL `gh pr create` prints on its own line (the first http(s)
/// line). App-parsed structurally — the URL is the only thing kept. Mirrors
/// `fix_local`'s private `extract_pr_url`.
fn extract_pr_url(stdout: &str) -> Option<String> {
    stdout
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("https://") || line.starts_with("http://"))
        .map(str::to_string)
}

/// PRIMARY (Option C): OPEN the born-`--draft` cloud fix PR ourselves, against
/// the agent's pushed `<branch>`. Runner-injected so the whole path is
/// unit-testable with a fake runner (no real network / GitHub):
///
/// 1. WRITE the app-authored body to a temp file (the runner captures output and
///    has no stdin; `--body-file` avoids a huge/looking-hostile argv). A write
///    failure fails closed to [`CloudDraftPrOpen::Failed`].
/// 2. RUN `gh pr create --draft --base <base> --head <branch> --title … --body-file …`
///    ([`gh_pr_create_draft_argv`]) via the injected runner; a non-success exit
///    or spawn error fails closed.
/// 3. Best-effort remove the temp body file on every path.
///
/// `<branch>` MUST already be validated ([`is_valid_cloud_fix_branch`]) by the
/// caller (the seam only reaches here via [`CloudFixPrAction::OpenDraftPr`]). The
/// argv is passed as a Vec (NO shell), so the validated branch cannot smuggle
/// flags/args. `gh` runs with the scheduler's own inherited credentials (no env
/// additions), mirroring the scheduler's other credentialed `gh`/`git` steps.
pub fn open_cloud_fix_draft_pr(
    runner: &dyn ProcessRunner,
    cwd: Option<&str>,
    base: &str,
    branch: &str,
    title: &str,
    body: &str,
) -> CloudDraftPrOpen {
    let body_path = std::env::temp_dir().join(format!(
        "chaos-cloud-fix-pr-body-{}.md",
        uuid::Uuid::new_v4()
    ));
    if std::fs::write(&body_path, body).is_err() {
        return CloudDraftPrOpen::Failed;
    }
    let argv = gh_pr_create_draft_argv(base, branch, title, &body_path.to_string_lossy());
    let result = match runner.run("gh", &argv, cwd, &[]) {
        Ok(out) if out.status.success() => CloudDraftPrOpen::Opened {
            pr_url: extract_pr_url(&String::from_utf8_lossy(&out.stdout)),
        },
        _ => CloudDraftPrOpen::Failed,
    };
    let _ = std::fs::remove_file(&body_path);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- pure decision --------------------------------------------------

    #[test]
    fn cloud_pr_draft_action_converts_a_proven_nondraft_and_fails_closed_on_unknown() {
        // The risk case: a PROVEN non-draft PR MUST be converted (else the
        // auto-merge bot may already have armed auto-merge at creation).
        assert_eq!(
            cloud_pr_draft_action(Some(false)),
            CloudPrDraftAction::ConvertToDraft
        );
        // A proven draft is left alone — the invariant already holds.
        assert_eq!(
            cloud_pr_draft_action(Some(true)),
            CloudPrDraftAction::AlreadyDraft
        );
        // FAIL CLOSED: an unverifiable state is flagged, never assumed safe.
        assert_eq!(
            cloud_pr_draft_action(None),
            CloudPrDraftAction::FlagUnverifiable
        );
    }

    // ---- Option C — PRIMARY decision + validation + argv ----------------

    #[test]
    fn decide_opens_a_draft_pr_for_a_pushed_branch_without_a_pr() {
        // FAILING-FIRST (Option C PRIMARY): a cloud outcome with a pushed,
        // valid `cursor/…` branch and NO pr_url must decide "the SCHEDULER opens
        // the draft PR" — the born-draft, race-free path.
        assert_eq!(
            decide_cloud_fix_pr_action(None, Some("cursor/fix-abc123")),
            CloudFixPrAction::OpenDraftPr {
                branch: "cursor/fix-abc123".to_string()
            }
        );
        // A null/empty pr_url is treated as absent (the same as None).
        assert_eq!(
            decide_cloud_fix_pr_action(Some("  "), Some("cursor/fix-1")),
            CloudFixPrAction::OpenDraftPr {
                branch: "cursor/fix-1".to_string()
            }
        );
    }

    #[test]
    fn gh_pr_create_draft_argv_is_exactly_draft_base_head_title_bodyfile() {
        // FAILING-FIRST (Option C PRIMARY): the built argv must be exactly
        // `gh pr create --draft --base main --head <branch> --title … --body-file …`.
        let argv = gh_pr_create_draft_argv(
            FIX_CLOUD_PR_BASE,
            "cursor/fix-abc123",
            "fix(WF): automated cloud fix for failed run run-7",
            "/tmp/body.md",
        );
        assert_eq!(
            argv,
            vec![
                "pr".to_string(),
                "create".to_string(),
                "--draft".to_string(),
                "--base".to_string(),
                "main".to_string(),
                "--head".to_string(),
                "cursor/fix-abc123".to_string(),
                "--title".to_string(),
                "fix(WF): automated cloud fix for failed run run-7".to_string(),
                "--body-file".to_string(),
                "/tmp/body.md".to_string(),
            ],
            "born-draft PR argv must be `gh pr create --draft --base main --head <branch> …`"
        );
        // Never a merge / auto-merge verb anywhere.
        assert!(!argv.iter().any(|a| a == "merge" || a == "--auto"));
        assert!(argv.contains(&"--draft".to_string()));
    }

    #[test]
    fn decide_falls_back_to_convert_when_an_unexpected_pr_url_is_present() {
        // Option C FALLBACK (defense-in-depth): a pr_url present (a future Cursor
        // change ignored auto_create_pr=false) takes PRECEDENCE — harden the
        // EXISTING PR rather than opening a second one, even if a branch is also
        // reported.
        assert_eq!(
            decide_cloud_fix_pr_action(Some("https://github.com/o/r/pull/9"), None),
            CloudFixPrAction::HardenExistingPr {
                pr_url: "https://github.com/o/r/pull/9".to_string()
            }
        );
        assert_eq!(
            decide_cloud_fix_pr_action(Some("https://github.com/o/r/pull/9"), Some("cursor/fix-1")),
            CloudFixPrAction::HardenExistingPr {
                pr_url: "https://github.com/o/r/pull/9".to_string()
            },
            "an existing PR is hardened, never duplicated by a second create"
        );
    }

    #[test]
    fn decide_is_a_noop_when_nothing_was_pushed_and_no_pr_exists() {
        assert_eq!(
            decide_cloud_fix_pr_action(None, None),
            CloudFixPrAction::Noop
        );
        assert_eq!(
            decide_cloud_fix_pr_action(Some(""), Some("   ")),
            CloudFixPrAction::Noop
        );
    }

    #[test]
    fn decide_rejects_a_malformed_or_injection_y_branch_and_fails_closed() {
        // Option C SECURITY: a pushed branch that is not a safe `cursor/…` name
        // must fail closed to an alert — the scheduler must NEVER pass it to
        // `gh pr create --head`.
        let bad = "cursor/x --title pwned --body x";
        assert_eq!(
            decide_cloud_fix_pr_action(None, Some(bad)),
            CloudFixPrAction::AlertInvalidBranch {
                branch: bad.to_string()
            }
        );
    }

    #[test]
    fn is_valid_cloud_fix_branch_accepts_cursor_names_and_rejects_injection() {
        // Accepts the real Cursor push shape.
        assert!(is_valid_cloud_fix_branch("cursor/fix-etl-a1b2c3"));
        assert!(is_valid_cloud_fix_branch("cursor/some_fix.v2/nested-1"));
        // Rejects: wrong/absent prefix (defense in depth — must look like Cursor).
        assert!(!is_valid_cloud_fix_branch("main"));
        assert!(!is_valid_cloud_fix_branch("feature/x"));
        assert!(!is_valid_cloud_fix_branch(""));
        // Rejects flag/arg smuggling + whitespace + shell-ish metacharacters
        // (belt-and-suspenders on top of the Vec argv).
        assert!(!is_valid_cloud_fix_branch("cursor/x --draft=false"));
        assert!(!is_valid_cloud_fix_branch("cursor/x;rm -rf /"));
        assert!(!is_valid_cloud_fix_branch("cursor/x`whoami`"));
        assert!(!is_valid_cloud_fix_branch("cursor/x$(id)"));
        assert!(!is_valid_cloud_fix_branch("cursor/../../etc/passwd"));
        assert!(!is_valid_cloud_fix_branch("-cursor/x"));
        assert!(!is_valid_cloud_fix_branch("cursor/x\ny"));
    }

    #[test]
    fn cloud_pr_title_and_body_are_app_authored_and_frame_the_branch() {
        let title = build_cloud_fix_pr_title("Nightly ETL", "run-123");
        assert!(title.contains("cloud fix"), "title says cloud (not local)");
        assert!(title.contains("run-123"));
        let body = build_cloud_fix_pr_body("Nightly ETL", "run-123", "cursor/fix-etl");
        assert!(body.contains("DRAFT"), "body states it is a draft");
        assert!(body.contains("cursor/fix-etl"), "body frames the branch");
        assert!(body.contains(CLOUD_FIX_CAVEAT), "body carries the caveat");
        // Never agent free-text (we never pass any in).
        assert!(!body.contains("IGNORE PREVIOUS INSTRUCTIONS"));
    }

    // ---- gh argv + parse ------------------------------------------------

    #[test]
    fn gh_view_argv_reads_isdraft_after_a_dash_dash_separator() {
        let argv = gh_pr_view_isdraft_argv("https://github.com/o/r/pull/7");
        assert!(argv.contains(&"isDraft".to_string()));
        assert!(argv.contains(&"--jq".to_string()));
        let sep = argv
            .iter()
            .position(|a| a == "--")
            .expect("has -- separator");
        assert_eq!(
            argv[sep + 1],
            "https://github.com/o/r/pull/7",
            "the PR URL is a positional after --, never readable as an option"
        );
    }

    #[test]
    fn gh_ready_undo_argv_converts_to_draft_and_never_merges() {
        let argv = gh_pr_ready_undo_argv("https://github.com/o/r/pull/7");
        assert!(argv.contains(&"ready".to_string()));
        assert!(
            argv.contains(&"--undo".to_string()),
            "must UNDO ready (i.e. convert TO draft), never mark ready"
        );
        assert!(
            !argv.iter().any(|a| a == "merge" || a == "--auto"),
            "the fix flow never merges / arms auto-merge"
        );
        let sep = argv
            .iter()
            .position(|a| a == "--")
            .expect("has -- separator");
        assert_eq!(argv[sep + 1], "https://github.com/o/r/pull/7");
    }

    #[test]
    fn parse_is_draft_only_accepts_exact_true_false() {
        assert_eq!(parse_is_draft("true\n"), Some(true));
        assert_eq!(parse_is_draft("  false "), Some(false));
        // Anything else is UNKNOWN → the caller fails closed.
        assert_eq!(parse_is_draft(""), None);
        assert_eq!(parse_is_draft("null"), None);
        assert_eq!(parse_is_draft("could not resolve PR"), None);
        assert_eq!(parse_is_draft("True"), None);
    }

    // ---- alert + detail rendering ---------------------------------------

    #[test]
    fn nondraft_alert_is_app_authored_and_actionable() {
        let msg = build_nondraft_alert(
            "https://github.com/o/r/pull/7",
            CLOUD_DRAFT_CONVERT_FAILED_REASON,
        );
        assert!(msg.contains("https://github.com/o/r/pull/7"));
        assert!(msg.contains("NON-DRAFT"));
        assert!(msg.contains("gh pr ready --undo"));
    }

    #[test]
    fn hardening_detail_renders_each_state_distinctly() {
        assert_eq!(
            hardening_detail(&CloudDraftHardening::AlreadyDraft)["cloud_pr_draft"],
            json!("already_draft")
        );
        assert_eq!(
            hardening_detail(&CloudDraftHardening::Converted)["cloud_pr_draft"],
            json!("converted_to_draft")
        );
        let alert = hardening_detail(&CloudDraftHardening::Alerted {
            reason: "x".to_string(),
        });
        assert_eq!(alert["cloud_pr_draft"], json!("alert"));
        assert_eq!(alert["reason"], json!("x"));
    }
}

// Behavior tests over the runner seam need `std::process::Output`, whose
// `ExitStatusExt` construction is unix-only (CI runs on macos — unix).
#[cfg(all(test, unix))]
mod runner_tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;
    use std::process::{ExitStatus, Output};
    use std::sync::Mutex;

    fn out(code: i32, stdout: &str) -> Output {
        Output {
            status: ExitStatus::from_raw((code & 0xff) << 8),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        }
    }

    /// Scripted `gh` runner: returns a configured result for the `gh pr view`
    /// draft probe and the `gh pr ready` convert, recording every argv so a test
    /// can assert WHETHER the convert was invoked.
    struct GhRunner {
        view_code: i32,
        view_stdout: String,
        ready_code: i32,
        calls: Mutex<Vec<Vec<String>>>,
    }

    impl GhRunner {
        fn new(view_code: i32, view_stdout: &str, ready_code: i32) -> Self {
            GhRunner {
                view_code,
                view_stdout: view_stdout.to_string(),
                ready_code,
                calls: Mutex::new(vec![]),
            }
        }
        fn ran_convert(&self) -> bool {
            self.calls
                .lock()
                .unwrap()
                .iter()
                .any(|argv| argv.iter().any(|a| a == "ready"))
        }
    }

    impl ProcessRunner for GhRunner {
        fn run(
            &self,
            program: &str,
            args: &[String],
            _cwd: Option<&str>,
            _env: &[(String, String)],
        ) -> std::io::Result<Output> {
            let mut argv = vec![program.to_string()];
            argv.extend_from_slice(args);
            self.calls.lock().unwrap().push(argv);
            if args.iter().any(|a| a == "view") {
                Ok(out(self.view_code, &self.view_stdout))
            } else if args.iter().any(|a| a == "ready") {
                Ok(out(self.ready_code, ""))
            } else {
                Ok(out(0, ""))
            }
        }
    }

    #[test]
    fn nondraft_cloud_fix_pr_is_converted_to_draft() {
        // The PR came back NON-draft (isDraft=false). The hardening MUST convert
        // it (gh pr ready --undo) so the auto-merge bot no longer sees a
        // `draft == false` PR to auto-merge.
        let runner = GhRunner::new(0, "false", 0);
        let result =
            harden_cloud_fix_pr_draft(&runner, Some("/tmp"), "https://github.com/o/r/pull/7");
        assert_eq!(result, CloudDraftHardening::Converted);
        assert!(
            runner.ran_convert(),
            "a proven non-draft PR must trigger the convert-to-draft (gh pr ready --undo)"
        );
    }

    #[test]
    fn already_draft_cloud_fix_pr_is_left_untouched() {
        // Cursor Cloud's documented default held (isDraft=true) — no mutation.
        let runner = GhRunner::new(0, "true", 0);
        let result =
            harden_cloud_fix_pr_draft(&runner, Some("/tmp"), "https://github.com/o/r/pull/7");
        assert_eq!(result, CloudDraftHardening::AlreadyDraft);
        assert!(
            !runner.ran_convert(),
            "an already-draft PR must NOT be touched"
        );
    }

    #[test]
    fn unverifiable_draft_state_raises_an_alert_and_never_silently_passes() {
        // The probe failed (non-zero gh). FAIL CLOSED: alert, and do NOT run a
        // blind convert against a PR whose state we could not read.
        let runner = GhRunner::new(1, "", 0);
        let result =
            harden_cloud_fix_pr_draft(&runner, Some("/tmp"), "https://github.com/o/r/pull/7");
        assert_eq!(
            result,
            CloudDraftHardening::Alerted {
                reason: CLOUD_DRAFT_UNVERIFIABLE_REASON.to_string()
            }
        );
        assert!(
            !runner.ran_convert(),
            "an unverifiable state must not trigger a blind convert"
        );
    }

    #[test]
    fn a_failed_conversion_raises_an_alert() {
        // Proven non-draft, but the convert itself failed (non-zero gh) — the
        // invariant is NOT guaranteed, so an operator alert must be raised.
        let runner = GhRunner::new(0, "false", 1);
        let result =
            harden_cloud_fix_pr_draft(&runner, Some("/tmp"), "https://github.com/o/r/pull/7");
        assert_eq!(
            result,
            CloudDraftHardening::Alerted {
                reason: CLOUD_DRAFT_CONVERT_FAILED_REASON.to_string()
            }
        );
        assert!(
            runner.ran_convert(),
            "the convert was attempted (and failed) before the alert"
        );
    }

    // ---- Option C — PRIMARY born-draft OPEN (scheduler opens the PR) ----

    /// Scripted `gh` runner for the born-draft OPEN path: records every argv and
    /// returns a configured exit code + stdout for `gh pr create`.
    struct CreateRunner {
        create_code: i32,
        create_stdout: String,
        calls: Mutex<Vec<Vec<String>>>,
    }
    impl CreateRunner {
        fn new(create_code: i32, create_stdout: &str) -> Self {
            CreateRunner {
                create_code,
                create_stdout: create_stdout.to_string(),
                calls: Mutex::new(vec![]),
            }
        }
    }
    impl ProcessRunner for CreateRunner {
        fn run(
            &self,
            program: &str,
            args: &[String],
            _cwd: Option<&str>,
            _env: &[(String, String)],
        ) -> std::io::Result<Output> {
            let mut argv = vec![program.to_string()];
            argv.extend_from_slice(args);
            self.calls.lock().unwrap().push(argv);
            Ok(out(self.create_code, &self.create_stdout))
        }
    }

    #[test]
    fn open_cloud_fix_draft_pr_runs_gh_pr_create_draft_against_the_branch() {
        // FAILING-FIRST (Option C PRIMARY, behavioral): the scheduler opens the
        // PR itself with `gh pr create --draft --base main --head <branch>` and
        // parses the printed PR URL. It NEVER merges / arms auto-merge.
        let runner = CreateRunner::new(0, "https://github.com/o/r/pull/42\n");
        let result = open_cloud_fix_draft_pr(
            &runner,
            Some("/tmp"),
            FIX_CLOUD_PR_BASE,
            "cursor/fix-xyz",
            "fix(WF): automated cloud fix for failed run run-7",
            "body text",
        );
        assert_eq!(
            result,
            CloudDraftPrOpen::Opened {
                pr_url: Some("https://github.com/o/r/pull/42".to_string())
            }
        );
        let calls = runner.calls.lock().unwrap();
        assert_eq!(calls.len(), 1, "exactly one gh invocation");
        assert_eq!(
            &calls[0][..8],
            &[
                "gh",
                "pr",
                "create",
                "--draft",
                "--base",
                "main",
                "--head",
                "cursor/fix-xyz"
            ],
            "born --draft PR against main, head = the pushed branch"
        );
        assert!(
            calls[0].iter().any(|a| a == "--body-file"),
            "the app-authored body is passed via --body-file"
        );
        assert!(
            !calls[0].iter().any(|a| a == "merge" || a == "--auto"),
            "the scheduler never merges / arms auto-merge"
        );
    }

    #[test]
    fn open_cloud_fix_draft_pr_fails_closed_when_gh_pr_create_errors() {
        // gh pr create exits non-zero => Failed (the seam then alerts + fails
        // closed, never assuming a PR was opened).
        let runner = CreateRunner::new(1, "");
        let result = open_cloud_fix_draft_pr(
            &runner,
            Some("/tmp"),
            FIX_CLOUD_PR_BASE,
            "cursor/fix-xyz",
            "t",
            "b",
        );
        assert_eq!(result, CloudDraftPrOpen::Failed);
    }
}
