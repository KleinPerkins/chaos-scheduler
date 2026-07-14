//! D05 CLOUD fix path — cloud non-draft hardening (PR2e). The pure,
//! independently-testable decisions the cloud post-run seam composes, plus the
//! thin runner-injected orchestration over them — mirroring `fix_local`'s
//! posture of keeping each security-critical decision a pure function that
//! carries its own failing-first test, free of the seam's process I/O.
//!
//! **What #284 shipped (the CLOUD fix path).** The CLOUD fix path dispatches a
//! Cursor Cloud Agent with a FORCED `auto_create_pr=true` (see
//! `scheduler::fix_agent_config_overlay`) so a machine-proposed fix always lands
//! as a REVIEWABLE PR rather than a silent branch. Its "the PR is a DRAFT"
//! guarantee, however, was only an ACCEPTED EXTERNAL DEPENDENCY: Cursor Cloud's
//! *documented default* of opening a programmatic PR as a draft — never
//! something the app itself enforced.
//!
//! **The risk this module closes.** This repo's GitHub App auto-merge bot
//! (`.github/workflows/app-auto-merge.yml`) arms squash auto-merge AND posts an
//! approval at PR CREATION (`opened` / `ready_for_review`) for ANY `draft ==
//! false`, same-repo PR (release-please excepted). So if the cloud agent opens a
//! NON-draft PR — or its PR is ever flipped ready — a machine-proposed fix could
//! AUTO-MERGE with NO human review, defeating the LOCKED D05 invariant that
//! EVERY fix path (cloud AND local) converges on a HUMAN-REVIEWED DRAFT PR that
//! is never auto-merged.
//!
//! **The hardening.** After the cloud agent returns a PR, DETECT its real
//! GitHub draft state and, if it is NON-draft, CONVERT it back to a draft
//! (`gh pr ready --undo`) — which removes it from the auto-merge bot's
//! `draft == false` eligibility. If the state cannot be verified, or the
//! conversion fails, FAIL CLOSED: raise an operator-visible alert rather than
//! silently assume the invariant holds (mirroring `fix_local`'s fail-closed
//! PR-base preflight).

use crate::service::ProcessRunner;
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Pure decision — what to do given the PR's observed draft state.
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
}
