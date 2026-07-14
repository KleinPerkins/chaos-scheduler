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
//! itself with `gh pr create -R <owner/repo> --draft --base main --head <branch>`
//! and an app-authored title/body ([`build_cloud_fix_pr_title`] /
//! [`build_cloud_fix_pr_body`], modeled on `fix_local`'s M4 builders).
//! Born-`--draft` ⇒ auto-merge-INELIGIBLE ⇒ RACE-FREE: there is no moment at
//! which a non-draft cloud fix PR exists. This REVERSES #284's "the cloud agent
//! opens the PR" mechanism and UNIFIES the CLOUD path with the LOCAL path (which
//! already opens its own draft PR).
//!
//! **Repository targeting (`-R`).** Unlike the LOCAL path — which runs `gh` INSIDE
//! the target repo's worktree, so `gh` infers the repo from the checkout — the
//! CLOUD path has NO local checkout of the fix workflow's repository (the agent
//! worked in the cloud; the scheduler's own workspace is a DIFFERENT repo). So the
//! cloud `gh pr create`/`gh pr list` MUST pass an explicit `-R <owner/repo>`
//! ([`normalize_repo_slug`], derived from the workflow's configured `repository`
//! and validated by [`is_valid_repo_slug`]) or it fails / targets the WRONG repo.
//! This is why [`gh_pr_create_draft_argv`] DIVERGES from `fix_local`'s builder by
//! adding `-R`.
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
//!
//! **Orphaned-PR reconcile (primary-path defense).** A future Cursor change could
//! ALSO ignore `auto_create_pr=false`, open a NON-draft PR, AND omit its url from
//! `git.branches[]` — so `pr_url` is absent, the primary path runs, and its
//! `gh pr create` FAILS ("a pull request already exists"). Rather than stop at
//! that failure (leaving the auto-merge-eligible non-draft PR live), the seam then
//! PROBES the head branch for an existing open PR ([`reconcile_orphaned_cloud_fix_pr`]
//! → `gh pr list -R <repo> --head <branch>`) and, if one is found, runs
//! [`harden_cloud_fix_pr_draft`] to ensure it is a draft; an unverifiable probe
//! FAILS CLOSED with an alert. This guarantees NO cloud fix path can leave an
//! auto-merge-eligible machine PR, even when both `pr_url` surfacing and the
//! `auto_create_pr=false` request are ignored upstream.

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

/// Whether a derived `owner/repo` slug is safe to pass to `gh -R`. Like
/// [`is_valid_cloud_fix_branch`], the source (`repository`) is app config but is
/// treated as UNTRUSTED at the `gh` boundary: EXACTLY two non-empty segments,
/// each `[A-Za-z0-9._-]` with no leading `-` (so it can never be read as a flag),
/// and no `..`. HOST-less (`owner/repo`, github.com implied) — the app only ever
/// targets GitHub repos.
pub fn is_valid_repo_slug(slug: &str) -> bool {
    let parts: Vec<&str> = slug.split('/').collect();
    parts.len() == 2
        && !slug.contains("..")
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.len() <= 100
                && !p.starts_with('-')
                && p.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        })
}

/// Derive a validated `owner/repo` slug from the fix workflow's `repository`
/// config for `gh -R`. Accepts the two shapes the `cursor_agent` operator
/// accepts: a full `https://github.com/owner/repo[.git]` URL (host dropped, first
/// two path segments kept) or an `owner/repo` shorthand. Returns `None` when the
/// result is not a safe two-segment slug ([`is_valid_repo_slug`]) — the caller
/// then FAILS CLOSED rather than hand `gh` an unusable/injection-y `-R` value.
pub fn normalize_repo_slug(repository: &str) -> Option<String> {
    let r = repository.trim();
    let slug = if let Some(rest) = r
        .strip_prefix("https://")
        .or_else(|| r.strip_prefix("http://"))
    {
        // rest = "host/owner/repo[/…]" — drop the host, keep owner + repo.
        let mut segs = rest.split('/').filter(|s| !s.is_empty());
        let _host = segs.next()?;
        let owner = segs.next()?;
        let repo = segs.next()?;
        let repo = repo.strip_suffix(".git").unwrap_or(repo);
        format!("{owner}/{repo}")
    } else {
        r.strip_suffix(".git").unwrap_or(r).to_string()
    };
    is_valid_repo_slug(&slug).then_some(slug)
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

/// `gh pr create` argv for the scheduler-opened born-DRAFT cloud fix PR. Builds
/// on [`crate::fix_local::gh_pr_create_argv`] (so the shared `--draft --base …
/// --head … --title … --body-file …` shape stays defined once) but DIVERGES by
/// splicing an explicit `-R <owner/repo>` right after `create`: the CLOUD path
/// has no local checkout of the target repo, so `gh` cannot infer it (the LOCAL
/// path runs inside the worktree and needs no `-R`). `repo` MUST be a validated
/// [`is_valid_repo_slug`] value. ALWAYS `--draft`; there is deliberately no
/// merge/approve verb anywhere in the fix flow — human review is the sole
/// backstop (M4).
pub fn gh_pr_create_draft_argv(
    repo: &str,
    base: &str,
    head: &str,
    title: &str,
    body_file: &str,
) -> Vec<String> {
    let mut argv = crate::fix_local::gh_pr_create_argv(base, head, title, body_file);
    // Splice `-R <repo>` after `["pr", "create", …]` (index 2). gh treats flag
    // order freely; keeping it first makes the target repo unmistakable.
    argv.splice(2..2, ["-R".to_string(), repo.to_string()]);
    argv
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

/// `gh pr list` argv that finds an OPEN PR whose HEAD is `<branch>` in `<repo>`,
/// returning JSON (`number,isDraft,url`). Used only on the PRIMARY path AFTER a
/// `gh pr create` failure, to detect a PR a future Cursor may have opened for the
/// branch despite `auto_create_pr=false` (see [`reconcile_orphaned_cloud_fix_pr`]).
/// `repo` ([`is_valid_repo_slug`]) and `branch` ([`is_valid_cloud_fix_branch`])
/// are pre-validated; both ride as OPTION VALUES (never positionals), so neither
/// can be read as a flag.
pub fn gh_pr_list_head_argv(repo: &str, branch: &str) -> Vec<String> {
    vec![
        "pr".to_string(),
        "list".to_string(),
        "-R".to_string(),
        repo.to_string(),
        "--head".to_string(),
        branch.to_string(),
        "--state".to_string(),
        "open".to_string(),
        "--json".to_string(),
        "number,isDraft,url".to_string(),
    ]
}

/// The outcome of parsing `gh pr list … --json number,isDraft,url` output.
#[derive(Debug, PartialEq, Eq)]
pub enum ExistingPrProbe {
    /// An open PR for the branch exists; carries its URL (+ observed draft state
    /// when present — advisory only; the convert path re-probes it).
    Found { url: String, is_draft: Option<bool> },
    /// The probe parsed cleanly and there is NO open PR for the branch.
    None,
    /// The output could not be parsed (not a JSON array, or an entry with no
    /// URL) — the caller FAILS CLOSED rather than assume "no PR".
    Unparseable,
}

/// Parse the JSON array `gh pr list … --json number,isDraft,url` prints. Takes
/// the FIRST entry (`--head` scopes to one branch). A clean empty array is
/// [`ExistingPrProbe::None`]; a non-array / an entry missing `url` is
/// [`ExistingPrProbe::Unparseable`] (fail closed).
pub fn parse_existing_pr(stdout: &str) -> ExistingPrProbe {
    match serde_json::from_str::<Vec<Value>>(stdout.trim()) {
        Ok(arr) => match arr.first() {
            Some(pr) => match pr.get("url").and_then(|v| v.as_str()) {
                Some(url) => ExistingPrProbe::Found {
                    url: url.to_string(),
                    is_draft: pr.get("isDraft").and_then(|v| v.as_bool()),
                },
                None => ExistingPrProbe::Unparseable,
            },
            None => ExistingPrProbe::None,
        },
        Err(_) => ExistingPrProbe::Unparseable,
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
/// Reason recorded when the cloud fix run surfaced no usable target repository,
/// so the scheduler could not open the born-draft PR (fail closed).
pub const CLOUD_MISSING_REPO_REASON: &str =
    "the cloud fix run did not surface a valid target repository (owner/repo), so the scheduler could not open the draft PR";
/// Reason recorded when the `gh pr create` failed AND the scheduler could not
/// verify whether a PR already exists for the branch (the `gh pr list` probe
/// failed or returned junk) — an orphaned NON-draft PR may be live. Fail closed.
pub const CLOUD_ORPHAN_PROBE_FAILED_REASON: &str =
    "the scheduler could not open the draft PR and could not verify whether a PR already exists for the branch (gh pr list failed or returned no usable JSON)";

/// App-authored, operator-visible alert for when the PRIMARY `gh pr create`
/// failed because a PR ALREADY existed for the branch (a future Cursor opened one
/// despite `auto_create_pr=false` and omitted its url) and the scheduler
/// CONVERTED that PR back to a draft. Surfaces the anomaly (the fallback fired)
/// so an operator can confirm; derived only from the app-captured PR URL.
pub fn build_orphan_recovered_alert(pr_url: &str) -> String {
    format!(
        "D05 cloud fix: a PR unexpectedly already existed for the machine fix branch (Cursor opened \
         one despite auto_create_pr=false); the scheduler ensured it is a DRAFT so it cannot \
         auto-merge without human review. Review it: {pr_url}"
    )
}

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

/// Structured `draft_hardening` detail for the PRIMARY path when `gh pr create`
/// failed but an EXISTING PR was found for the branch and reconciled
/// ([`reconcile_orphaned_cloud_fix_pr`]) — distinct `recovered_*` states so an
/// operator can tell this apart from a clean born-draft open. Carries the PR URL
/// (and the alert reason when the reconcile itself could not guarantee a draft).
pub fn recovered_detail(pr_url: &str, hardening: &CloudDraftHardening) -> Value {
    let state = match hardening {
        CloudDraftHardening::AlreadyDraft => "recovered_existing_already_draft",
        CloudDraftHardening::Converted => "recovered_existing_converted_to_draft",
        CloudDraftHardening::Alerted { .. } => "recovered_existing_alert",
    };
    let mut v = json!({ "cloud_pr_draft": state, "pr_url": pr_url });
    if let CloudDraftHardening::Alerted { reason } = hardening {
        v["reason"] = json!(reason);
    }
    v
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
/// `<repo>` ([`is_valid_repo_slug`]) and `<branch>` ([`is_valid_cloud_fix_branch`])
/// MUST already be validated by the caller (the seam only reaches here via
/// [`CloudFixPrAction::OpenDraftPr`] with a resolved repo). The argv is passed as
/// a Vec (NO shell), so neither validated value can smuggle flags/args. `gh` runs
/// with the scheduler's own inherited credentials (no env additions), mirroring
/// the scheduler's other credentialed `gh`/`git` steps.
pub fn open_cloud_fix_draft_pr(
    runner: &dyn ProcessRunner,
    cwd: Option<&str>,
    repo: &str,
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
    let argv = gh_pr_create_draft_argv(repo, base, branch, title, &body_path.to_string_lossy());
    let result = match runner.run("gh", &argv, cwd, &[]) {
        Ok(out) if out.status.success() => CloudDraftPrOpen::Opened {
            pr_url: extract_pr_url(&String::from_utf8_lossy(&out.stdout)),
        },
        _ => CloudDraftPrOpen::Failed,
    };
    let _ = std::fs::remove_file(&body_path);
    result
}

/// The outcome of reconciling a PRIMARY-path `gh pr create` FAILURE: probe the
/// branch for an already-existing PR and, if found, ensure it is a draft.
#[derive(Debug, PartialEq, Eq)]
pub enum OrphanReconcile {
    /// The probe succeeded and there is NO open PR for the branch — the create
    /// failed for some OTHER reason (the seam then alerts open-failed).
    NoExistingPr,
    /// An open PR existed for the branch and was reconciled to a draft. Carries
    /// its URL + the [`harden_cloud_fix_pr_draft`] result (Converted / already a
    /// draft / Alerted if the convert could not be guaranteed).
    Found {
        pr_url: String,
        hardening: CloudDraftHardening,
    },
    /// The probe itself failed (non-zero `gh` or unparseable JSON) — whether an
    /// orphaned NON-draft PR is live could NOT be verified. FAIL CLOSED.
    ProbeFailed,
}

/// PRIMARY-path defense: after a failed `gh pr create`, detect + neutralize a PR
/// a future Cursor may have opened for the branch (despite `auto_create_pr=false`
/// AND without surfacing its url). Runner-injected so the whole path is
/// unit-testable with a fake:
///
/// 1. PROBE `gh pr list -R <repo> --head <branch> --state open --json …`; a
///    non-success exit or unparseable JSON is [`OrphanReconcile::ProbeFailed`]
///    (fail closed).
/// 2. If a PR is FOUND, run [`harden_cloud_fix_pr_draft`] on it (a no-op if it is
///    already a draft; convert if not; alert if it cannot be guaranteed).
/// 3. A clean "no PR" is [`OrphanReconcile::NoExistingPr`].
pub fn reconcile_orphaned_cloud_fix_pr(
    runner: &dyn ProcessRunner,
    cwd: Option<&str>,
    repo: &str,
    branch: &str,
) -> OrphanReconcile {
    let probe = match runner.run("gh", &gh_pr_list_head_argv(repo, branch), cwd, &[]) {
        Ok(out) if out.status.success() => parse_existing_pr(&String::from_utf8_lossy(&out.stdout)),
        _ => return OrphanReconcile::ProbeFailed,
    };
    match probe {
        ExistingPrProbe::Found { url, .. } => {
            let hardening = harden_cloud_fix_pr_draft(runner, cwd, &url);
            OrphanReconcile::Found {
                pr_url: url,
                hardening,
            }
        }
        ExistingPrProbe::None => OrphanReconcile::NoExistingPr,
        ExistingPrProbe::Unparseable => OrphanReconcile::ProbeFailed,
    }
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
    fn gh_pr_create_draft_argv_targets_the_repo_with_dash_r() {
        // FAILING-FIRST (Finding 1): the CLOUD create MUST carry an explicit
        // `-R <owner/repo>` (there is no local checkout to infer it from), and be
        // exactly `gh pr create -R <repo> --draft --base main --head <branch>
        // --title … --body-file …`.
        let argv = gh_pr_create_draft_argv(
            "acme/app",
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
                "-R".to_string(),
                "acme/app".to_string(),
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
            "born-draft PR argv must be `gh pr create -R <repo> --draft --base main --head <branch> …`"
        );
        // The repo rides as the VALUE of `-R` (never a positional / inferred).
        let r = argv.iter().position(|a| a == "-R").expect("has -R");
        assert_eq!(argv[r + 1], "acme/app");
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
    fn normalize_repo_slug_handles_urls_and_shorthand_and_rejects_junk() {
        // Finding 1: derive a safe `owner/repo` from either config shape.
        assert_eq!(
            normalize_repo_slug("https://github.com/acme/app"),
            Some("acme/app".to_string())
        );
        assert_eq!(
            normalize_repo_slug("https://github.com/acme/app.git"),
            Some("acme/app".to_string())
        );
        // Extra path segments (e.g. a pasted PR URL) collapse to owner/repo.
        assert_eq!(
            normalize_repo_slug("https://github.com/acme/app/pull/1"),
            Some("acme/app".to_string())
        );
        assert_eq!(
            normalize_repo_slug("acme/app"),
            Some("acme/app".to_string())
        );
        assert_eq!(
            normalize_repo_slug("  acme/app  "),
            Some("acme/app".to_string())
        );
        // A credential-bearing URL is still SAFE: the host (where any `user@`
        // lives) is dropped entirely, leaving just the owner/repo the `-R` targets.
        assert_eq!(
            normalize_repo_slug("https://x@github.com/acme/app"),
            Some("acme/app".to_string())
        );
        // Rejected: not two segments, injection-y, or a URL missing the repo.
        assert_eq!(normalize_repo_slug("acme"), None);
        assert_eq!(normalize_repo_slug("acme/app/extra"), None);
        assert_eq!(normalize_repo_slug("-acme/app"), None);
        assert_eq!(normalize_repo_slug("acme/../secret"), None);
        assert_eq!(normalize_repo_slug("acme/app --foo"), None);
        assert_eq!(normalize_repo_slug("https://github.com/acme"), None);
        assert!(is_valid_repo_slug("acme/app"));
        assert!(!is_valid_repo_slug("acme"));
        assert!(!is_valid_repo_slug("a/b/c"));
    }

    #[test]
    fn gh_pr_list_head_argv_scopes_to_repo_and_branch_as_option_values() {
        let argv = gh_pr_list_head_argv("acme/app", "cursor/fix-1");
        // repo + branch are OPTION VALUES (after -R / --head), never positionals.
        let r = argv.iter().position(|a| a == "-R").expect("has -R");
        assert_eq!(argv[r + 1], "acme/app");
        let h = argv.iter().position(|a| a == "--head").expect("has --head");
        assert_eq!(argv[h + 1], "cursor/fix-1");
        assert!(argv.iter().any(|a| a == "list"));
        assert!(argv.iter().any(|a| a == "open"), "only OPEN PRs");
        // Reads structured JSON (never merges / arms auto-merge).
        assert!(argv.iter().any(|a| a == "number,isDraft,url"));
        assert!(!argv.iter().any(|a| a == "merge" || a == "--auto"));
    }

    #[test]
    fn parse_existing_pr_finds_reads_or_fails_closed() {
        // A real `gh pr list` hit.
        assert_eq!(
            parse_existing_pr(
                r#"[{"number":7,"isDraft":false,"url":"https://github.com/o/r/pull/7"}]"#
            ),
            ExistingPrProbe::Found {
                url: "https://github.com/o/r/pull/7".to_string(),
                is_draft: Some(false)
            }
        );
        // Clean empty array => no PR.
        assert_eq!(parse_existing_pr("[]"), ExistingPrProbe::None);
        // Junk / non-array / missing url => fail closed (Unparseable).
        assert_eq!(parse_existing_pr(""), ExistingPrProbe::Unparseable);
        assert_eq!(
            parse_existing_pr("could not resolve"),
            ExistingPrProbe::Unparseable
        );
        assert_eq!(
            parse_existing_pr(r#"[{"number":7}]"#),
            ExistingPrProbe::Unparseable
        );
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
            "acme/app",
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
            &calls[0][..10],
            &[
                "gh",
                "pr",
                "create",
                "-R",
                "acme/app",
                "--draft",
                "--base",
                "main",
                "--head",
                "cursor/fix-xyz"
            ],
            "born --draft PR against the target repo (-R), head = the pushed branch"
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
            "acme/app",
            FIX_CLOUD_PR_BASE,
            "cursor/fix-xyz",
            "t",
            "b",
        );
        assert_eq!(result, CloudDraftPrOpen::Failed);
    }

    // ---- Finding 2 — orphaned-PR reconcile after a failed create -------

    /// Scripted `gh` runner for the reconcile path: routes `pr list` (the probe),
    /// `pr view` (draft state), and `pr ready` (the convert), recording argv.
    struct ReconcileRunner {
        list_code: i32,
        list_stdout: String,
        view_code: i32,
        view_stdout: String,
        ready_code: i32,
        calls: Mutex<Vec<Vec<String>>>,
    }
    impl ReconcileRunner {
        fn ran(&self, verb: &str) -> bool {
            self.calls
                .lock()
                .unwrap()
                .iter()
                .any(|argv| argv.iter().any(|a| a == verb))
        }
    }
    impl ProcessRunner for ReconcileRunner {
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
            if args.iter().any(|a| a == "list") {
                Ok(out(self.list_code, &self.list_stdout))
            } else if args.iter().any(|a| a == "view") {
                Ok(out(self.view_code, &self.view_stdout))
            } else if args.iter().any(|a| a == "ready") {
                Ok(out(self.ready_code, ""))
            } else {
                Ok(out(0, ""))
            }
        }
    }

    #[test]
    fn reconcile_converts_an_orphaned_nondraft_pr_found_after_a_failed_create() {
        // FAILING-FIRST (Finding 2): `gh pr create` failed because a PR already
        // existed for the branch (a future Cursor opened one despite
        // auto_create_pr=false, without surfacing prUrl). The reconcile MUST probe
        // (`gh pr list`), FIND the NON-draft PR, and CONVERT it to a draft — so no
        // auto-merge-eligible machine PR is left live.
        let runner = ReconcileRunner {
            list_code: 0,
            list_stdout: r#"[{"number":7,"isDraft":false,"url":"https://github.com/o/r/pull/7"}]"#
                .to_string(),
            view_code: 0,
            view_stdout: "false".to_string(),
            ready_code: 0,
            calls: Mutex::new(vec![]),
        };
        let result =
            reconcile_orphaned_cloud_fix_pr(&runner, Some("/tmp"), "acme/app", "cursor/fix-1");
        assert_eq!(
            result,
            OrphanReconcile::Found {
                pr_url: "https://github.com/o/r/pull/7".to_string(),
                hardening: CloudDraftHardening::Converted,
            }
        );
        assert!(runner.ran("list"), "the reconcile probes with gh pr list");
        assert!(
            runner.ran("ready"),
            "a found non-draft orphan is converted to a draft (gh pr ready --undo)"
        );
    }

    #[test]
    fn reconcile_reports_no_existing_pr_on_a_clean_empty_list() {
        // The create failed for some OTHER reason (no PR exists) — the seam then
        // alerts open-failed; NO convert is attempted.
        let runner = ReconcileRunner {
            list_code: 0,
            list_stdout: "[]".to_string(),
            view_code: 0,
            view_stdout: "".to_string(),
            ready_code: 0,
            calls: Mutex::new(vec![]),
        };
        let result =
            reconcile_orphaned_cloud_fix_pr(&runner, Some("/tmp"), "acme/app", "cursor/fix-1");
        assert_eq!(result, OrphanReconcile::NoExistingPr);
        assert!(!runner.ran("ready"), "no PR found => nothing to convert");
    }

    #[test]
    fn reconcile_fails_closed_when_the_probe_errors() {
        // The probe itself failed (non-zero gh) — whether a non-draft orphan is
        // live could NOT be verified. FAIL CLOSED (never a blind convert).
        let runner = ReconcileRunner {
            list_code: 1,
            list_stdout: "".to_string(),
            view_code: 0,
            view_stdout: "".to_string(),
            ready_code: 0,
            calls: Mutex::new(vec![]),
        };
        let result =
            reconcile_orphaned_cloud_fix_pr(&runner, Some("/tmp"), "acme/app", "cursor/fix-1");
        assert_eq!(result, OrphanReconcile::ProbeFailed);
        assert!(
            !runner.ran("ready"),
            "an unverifiable probe must not trigger a blind convert"
        );
    }
}
