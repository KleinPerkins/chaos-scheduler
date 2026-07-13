//! D05 LOCAL rerun-gated fix agent — the pure, independently-testable gates the
//! orchestrator composes. Keeping the security-critical decisions here (as pure
//! functions over their inputs) means each one carries its own failing-first
//! test, free of the orchestrator's I/O.
//!
//! - **M1 credential isolation** — [`agent_credential_isolation`] builds the env
//!   scrub for the `cursor-agent` child so a `--force` autonomous agent never
//!   inherits the push token / gh auth.
//! - **M3 source scope/env gate** — [`ensure_fix_rerun_source_allowed`] confines
//!   the rerun to non-production, local-tree-reading source workflows.
//! - **M4 human-review backstop** — the `build_fix_*` builders app-author the PR
//!   title / body / commit message (never echoing agent free-text) and the argv
//!   builders open a DRAFT PR that is never merged.
//!
//! These gates are landed with their failing-first tests ahead of the
//! orchestrator that composes them; a module-level `dead_code` allow covers the
//! brief window before that wiring lands in the same PR.
#![allow(dead_code)]

use crate::workflow_spec::{WorkflowKind, WorkflowSpec};

// ---------------------------------------------------------------------------
// M1 — credential isolation for the cursor-agent child.
// ---------------------------------------------------------------------------

/// Push-credential env vars UNSET for the cursor-agent child (M1).
/// `run_cli` launches `cursor-agent … --force` (autonomous auto-approve) which
/// inherits the parent env; left intact the agent would be strictly MORE
/// privileged than the scheduler's own git/gh — it could itself `git push` /
/// `gh pr merge` / echo the token, defeating every downstream containment. So we
/// strip these for the agent child specifically; the scheduler provides the
/// credentials ONLY to its OWN single push + PR-create step.
///
/// This covers BOTH publish channels: the HTTPS token vars AND the SSH agent
/// socket / custom ssh command. The `chaos-scheduler` origin is an `ssh://`
/// remote, so scrubbing only the token would leave a `--force` agent able to
/// `git push` over the operator's loaded SSH keys — the SSH vars close that.
/// The scheduler's own push runs with the full inherited env, so it keeps both.
pub const AGENT_CREDENTIAL_SCRUB_VARS: &[&str] = &[
    "GITHUB_TOKEN",
    "GH_TOKEN",
    "GH_ENTERPRISE_TOKEN",
    "GITHUB_ENTERPRISE_TOKEN",
    "SSH_AUTH_SOCK",
    "SSH_AGENT_PID",
    "GIT_SSH",
    "GIT_SSH_COMMAND",
];

/// The env mutation applied to the cursor-agent child for credential isolation:
/// keys to unset (`env_remove`) plus keys to set (`env_set`).
pub struct AgentCredentialIsolation {
    pub env_remove: Vec<&'static str>,
    pub env_set: Vec<(String, String)>,
}

/// Highest-precedence git config injected into the cursor-agent child to defeat
/// EVERY stored-credential push channel — including the repo-LOCAL config that
/// `GIT_CONFIG_GLOBAL`/`GIT_CONFIG_NOSYSTEM` do NOT touch (a worktree shares the
/// main repo's `.git/config`, which commonly carries a `credential.helper` that
/// `gh` installed, or a `core.sshCommand` pointing at a key). Git reads env
/// config LAST, so these override system+global+local:
/// - `credential.helper=""` — an empty value RESETS the accumulated helper list,
///   so no system/global/local helper (osxkeychain, store, …) survives.
/// - `credential.interactive=false` — never prompt for a credential.
/// - `core.sshCommand=…` — an ssh that uses neither the agent nor any on-disk
///   identity (`-F /dev/null` so `~/.ssh/config` cannot re-add a `Host github.com
///   IdentityFile …`, plus `IdentityAgent=none`, `IdentitiesOnly=yes`,
///   `IdentityFile=/dev/null`, `BatchMode=yes`), so an `ssh://` push cannot
///   authenticate (sec-F3).
const AGENT_GIT_CONFIG_OVERRIDES: &[(&str, &str)] = &[
    ("credential.helper", ""),
    ("credential.interactive", "false"),
    (
        "core.sshCommand",
        "ssh -F /dev/null -o BatchMode=yes -o IdentityAgent=none -o IdentitiesOnly=yes -o IdentityFile=/dev/null",
    ),
];

/// Build the credential-isolation env for the cursor-agent child (M1). Unsets
/// the push-token / SSH vars and neutralizes gh + git auth discovery:
/// - `GH_CONFIG_DIR` → an empty scratch dir (gh finds no stored auth),
/// - `GIT_CONFIG_GLOBAL=/dev/null` + `GIT_CONFIG_NOSYSTEM=1` → the agent's git
///   sees no global/system config,
/// - `GIT_CONFIG_COUNT`/`GIT_CONFIG_KEY_n`/`GIT_CONFIG_VALUE_n` → the
///   highest-precedence `AGENT_GIT_CONFIG_OVERRIDES`, which ALSO defeat the
///   repo-LOCAL `credential.helper` / `core.sshCommand` a `--force` agent could
///   otherwise push behind our back (global/system scrubbing alone missed this),
/// - `GIT_TERMINAL_PROMPT=0` → a credential-less git operation fails fast rather
///   than hanging on an interactive prompt.
///
/// The agent does NOT need any credential to EDIT code; validation + the PR are
/// the scheduler's job, run with a separate, credentialed env for that one step.
pub fn agent_credential_isolation(empty_gh_config_dir: &str) -> AgentCredentialIsolation {
    let mut env_set = vec![
        ("GH_CONFIG_DIR".to_string(), empty_gh_config_dir.to_string()),
        ("GIT_CONFIG_GLOBAL".to_string(), "/dev/null".to_string()),
        ("GIT_CONFIG_NOSYSTEM".to_string(), "1".to_string()),
        ("GIT_TERMINAL_PROMPT".to_string(), "0".to_string()),
        (
            "GIT_CONFIG_COUNT".to_string(),
            AGENT_GIT_CONFIG_OVERRIDES.len().to_string(),
        ),
    ];
    for (i, (key, value)) in AGENT_GIT_CONFIG_OVERRIDES.iter().enumerate() {
        env_set.push((format!("GIT_CONFIG_KEY_{i}"), (*key).to_string()));
        env_set.push((format!("GIT_CONFIG_VALUE_{i}"), (*value).to_string()));
    }
    AgentCredentialIsolation {
        env_remove: AGENT_CREDENTIAL_SCRUB_VARS.to_vec(),
        env_set,
    }
}

// ---------------------------------------------------------------------------
// M3 — source rerun scope / environment gate.
// ---------------------------------------------------------------------------

/// Why a source run is ineligible for a LOCAL fix rerun (M3).
#[derive(Debug, PartialEq, Eq)]
pub enum FixRerunSourceRefusal {
    /// The source workflow runs in production. The LOCAL rerun executes the
    /// source's REAL command against agent-edited code (real side effects), so
    /// it is confined to NON-production environments.
    ProductionEnvironment,
    /// The source workflow does not read the local repo tree (an http / cloud
    /// operator). A local edit cannot be validated by hitting a remote.
    NonLocalSourceType,
}

impl std::fmt::Display for FixRerunSourceRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FixRerunSourceRefusal::ProductionEnvironment => write!(
                f,
                "local fix rerun is confined to non-production environments (the rerun runs the \
                 source's real command against edited code)"
            ),
            FixRerunSourceRefusal::NonLocalSourceType => write!(
                f,
                "local fix rerun requires a local-tree-reading source workflow (command or \
                 step-flow); http / cloud sources cannot validate a local edit"
            ),
        }
    }
}

/// Whether the source workflow reads the local repo tree, so a local edit can be
/// validated by rerunning it: a legacy single-script COMMAND (no `spec_json`) or
/// a GENERIC step-flow. A TYPED operator (http, cursor_agent, …) is refused, and
/// an unparseable spec fails closed.
pub fn source_is_local_tree_reading(spec_json: Option<&str>) -> bool {
    match spec_json {
        None => true,
        Some(json) => match WorkflowSpec::from_json(json) {
            Ok(spec) => matches!(spec.kind, WorkflowKind::Generic),
            Err(_) => false,
        },
    }
}

/// Whether `environment` is eligible for a LOCAL fix rerun — every environment
/// except production. Unlike the propose-only CLOUD path (which dropped its env
/// gate because it never executes locally), the LOCAL rerun runs REAL side
/// effects, so production is refused.
pub fn source_environment_allows_rerun(environment: &str) -> bool {
    !environment.eq_ignore_ascii_case("production")
}

/// M3 gate: the source run is eligible for a LOCAL fix rerun only if it is
/// non-production AND a local-tree-reading workflow type.
pub fn ensure_fix_rerun_source_allowed(
    environment: &str,
    spec_json: Option<&str>,
) -> Result<(), FixRerunSourceRefusal> {
    if !source_environment_allows_rerun(environment) {
        return Err(FixRerunSourceRefusal::ProductionEnvironment);
    }
    if !source_is_local_tree_reading(spec_json) {
        return Err(FixRerunSourceRefusal::NonLocalSourceType);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// M4 — app-authored, human-review-gated draft PR.
// ---------------------------------------------------------------------------

/// The plainly-stated limit of the rerun gate, embedded verbatim in the PR body:
/// a passing rerun proves only "exit 0 after edits," never correctness. Human
/// review of the diff is the sole backstop (M4).
pub const FIX_RERUN_CAVEAT: &str =
    "A passing rerun proves only that the workflow command exited 0 \
AFTER the agent's edits — it does NOT prove the fix is correct. The agent could have neutered the \
failing check (removed an assertion, added `exit 0`, …). Review the diff below before merging.";

/// Neutralize a code-fence delimiter inside app-embedded text so a crafted file
/// path in the diff summary cannot close the fenced block and escape into the
/// surrounding (trusted) PR body.
fn deflate_fence(s: &str) -> String {
    s.replace("```", "'''")
}

/// App-authored PR title. Derived only from the source workflow name + run id —
/// never agent stdout/stderr.
pub fn build_fix_pr_title(source_workflow_name: &str, source_run_id: &str) -> String {
    format!("fix({source_workflow_name}): automated local fix for failed run {source_run_id}")
}

/// App-authored commit message for the agent's edits (the scheduler commits, not
/// the agent). Body-free, derived only from app-structured fields.
pub fn build_fix_commit_message(source_workflow_name: &str, source_run_id: &str) -> String {
    format!(
        "fix({source_workflow_name}): local fix agent edits for failed run {source_run_id}\n\n\
         Automated edits proposed by Chaos Scheduler's local fix agent. Opened as a DRAFT PR for \
         human review; a passing rerun proves only exit-0-after-edits, not correctness."
    )
}

/// Inputs to [`build_fix_pr_body`] — all app-structured (never agent free-text).
pub struct FixPrBody<'a> {
    pub source_workflow_name: &'a str,
    pub source_run_id: &'a str,
    pub fix_branch: &'a str,
    /// `git diff --stat` output from the worktree (structural: file paths + line
    /// counts). App-generated, fence-neutralized before embedding.
    pub diff_stat: &'a str,
    /// The EXACT command the rerun executed to validate the edit, so a reviewer
    /// can see whether the check was neutered.
    pub rerun_command: &'a str,
}

/// App-author the DRAFT PR body (M4). Surfaces the full diff summary + the exact
/// rerun command + the "exit 0 ≠ correct" caveat so a human can judge the fix.
/// NEVER embeds raw stderr or agent free-text (origin is a PUBLIC repo; a draft
/// PR body is world-readable).
pub fn build_fix_pr_body(body: &FixPrBody) -> String {
    let diff = deflate_fence(body.diff_stat);
    let rerun = deflate_fence(body.rerun_command);
    format!(
        "## Automated fix — DRAFT for human review\n\n\
         Chaos Scheduler's local fix agent investigated a failed run of **{workflow}** and proposes \
         the change on branch `{branch}`. **This PR is a DRAFT and is never auto-merged.**\n\n\
         > {caveat}\n\n\
         **Source failed run:** `{run_id}`\n\n\
         **Validation rerun command:**\n\n\
         ```\n{rerun}\n```\n\n\
         ### Changed files (git diff --stat)\n\n\
         ```\n{diff}\n```\n",
        workflow = body.source_workflow_name,
        branch = body.fix_branch,
        caveat = FIX_RERUN_CAVEAT,
        run_id = body.source_run_id,
        rerun = rerun,
        diff = diff,
    )
}

/// A faithful, human-readable rendering of the command the source rerun actually
/// executes — surfaced verbatim in the PR body (M4) so a reviewer can see the
/// exact check that passed. A legacy single-script source runs its `script_path`;
/// a GENERIC step-flow runs its ordered steps (its `script_path` is unused), so
/// we render the steps instead of the misleading placeholder field.
pub fn describe_rerun_command(script_path: &str, spec_json: Option<&str>) -> String {
    if let Some(json) = spec_json {
        if let Ok(spec) = WorkflowSpec::from_json(json) {
            if let Some(generic) = spec.generic {
                let steps: Vec<String> = generic
                    .steps
                    .iter()
                    .map(|s| {
                        let cmd = s
                            .command
                            .as_deref()
                            .or(s.script.as_deref())
                            .unwrap_or("(no command)");
                        format!("{}: {}", s.id, cmd)
                    })
                    .collect();
                if !steps.is_empty() {
                    return steps.join("\n");
                }
            }
        }
    }
    script_path.to_string()
}

/// `git push` argv for the fix branch. Uses the `--` separator (precedent in
/// `operators.rs`) so a hostile-looking branch name can never be read as an
/// option. `--no-verify` (sec-F2) skips the pre-push hook on the scheduler's OWN
/// credentialed push — a caller ALSO points `core.hooksPath` at an empty dir
/// (belt-and-suspenders; that override also defeats non-`--no-verify` hooks).
/// `--force-with-lease` (corr-F2) lets a re-dispatch safely overwrite a STALE
/// `chaos-fix/<run_id>` left on the remote by a prior failed attempt (the branch
/// is app-owned + deterministic); the caller first best-effort `fetch`es it so
/// the lease compares against the real remote state, never blindly clobbering.
pub fn git_push_argv(remote: &str, branch: &str) -> Vec<String> {
    vec![
        "push".to_string(),
        "--no-verify".to_string(),
        "--force-with-lease".to_string(),
        "--set-upstream".to_string(),
        remote.to_string(),
        "--".to_string(),
        branch.to_string(),
    ]
}

/// `gh pr create` argv for a DRAFT PR whose body is read from a file (the runner
/// captures output and has no stdin). ALWAYS `--draft`; there is deliberately no
/// merge verb anywhere in the fix flow — human review is the sole backstop (M4).
pub fn gh_pr_create_argv(base: &str, head: &str, title: &str, body_file: &str) -> Vec<String> {
    vec![
        "pr".to_string(),
        "create".to_string(),
        "--draft".to_string(),
        "--base".to_string(),
        base.to_string(),
        "--head".to_string(),
        head.to_string(),
        "--title".to_string(),
        title.to_string(),
        "--body-file".to_string(),
        body_file.to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- M1 -------------------------------------------------------------

    #[test]
    fn agent_credential_isolation_scrubs_push_tokens_and_neutralizes_gh_git_auth() {
        let iso = agent_credential_isolation("/tmp/empty-gh-config");
        // The push tokens are UNSET for the agent child.
        assert!(iso.env_remove.contains(&"GITHUB_TOKEN"));
        assert!(iso.env_remove.contains(&"GH_TOKEN"));
        // The SSH publish channel is closed too — the origin is an ssh:// remote,
        // so a `--force` agent must not inherit the operator's loaded keys.
        assert!(iso.env_remove.contains(&"SSH_AUTH_SOCK"));
        assert!(iso.env_remove.contains(&"GIT_SSH_COMMAND"));
        // gh + git auth discovery is neutralized.
        let set: std::collections::HashMap<_, _> = iso.env_set.iter().cloned().collect();
        assert_eq!(
            set.get("GH_CONFIG_DIR").map(String::as_str),
            Some("/tmp/empty-gh-config")
        );
        assert_eq!(
            set.get("GIT_CONFIG_GLOBAL").map(String::as_str),
            Some("/dev/null")
        );
        assert_eq!(
            set.get("GIT_CONFIG_NOSYSTEM").map(String::as_str),
            Some("1")
        );
        // The repo-LOCAL config is ALSO defeated (global/system scrubbing misses
        // it): the highest-precedence env config resets the credential-helper
        // list to empty and overrides core.sshCommand to a key-less ssh, so a
        // `--force` agent has no stored-credential push channel of any kind.
        let count: usize = set
            .get("GIT_CONFIG_COUNT")
            .and_then(|c| c.parse().ok())
            .expect("GIT_CONFIG_COUNT is set");
        let overrides: std::collections::HashMap<String, String> = (0..count)
            .filter_map(|i| {
                let k = set.get(&format!("GIT_CONFIG_KEY_{i}"))?;
                let v = set.get(&format!("GIT_CONFIG_VALUE_{i}"))?;
                Some((k.clone(), v.clone()))
            })
            .collect();
        assert_eq!(
            overrides.get("credential.helper").map(String::as_str),
            Some(""),
            "an empty credential.helper resets any local/global/system helper"
        );
        assert!(
            overrides.get("core.sshCommand").is_some_and(
                |s| s.contains("IdentityAgent=none") && s.contains("IdentityFile=/dev/null")
            ),
            "core.sshCommand is overridden to a key-less, agent-less ssh"
        );
        // sec-F3: `-F /dev/null` makes the agent's ssh ignore ~/.ssh/config, so a
        // `Host github.com\n  IdentityFile …` there cannot re-introduce a key.
        assert!(
            overrides
                .get("core.sshCommand")
                .is_some_and(|s| s.contains("-F /dev/null")),
            "core.sshCommand ignores the user's ~/.ssh/config (-F /dev/null)"
        );
        // The scrub list never carries a value — it is purely a removal set.
        assert!(!iso
            .env_set
            .iter()
            .any(|(k, _)| k == "GITHUB_TOKEN" || k == "GH_TOKEN"));
    }

    // ---- M3 -------------------------------------------------------------

    #[test]
    fn m3_refuses_production_even_for_a_local_source_type() {
        let err = ensure_fix_rerun_source_allowed("production", None).unwrap_err();
        assert_eq!(err, FixRerunSourceRefusal::ProductionEnvironment);
        // Case-insensitive.
        assert!(ensure_fix_rerun_source_allowed("Production", None).is_err());
    }

    #[test]
    fn m3_refuses_http_and_cloud_source_types_in_non_prod() {
        let http = r#"{"kind":"typed","typed":{"operator_type":"http","config":{}}}"#;
        let cloud = r#"{"kind":"typed","typed":{"operator_type":"cursor_agent","config":{}}}"#;
        assert_eq!(
            ensure_fix_rerun_source_allowed("sandbox", Some(http)).unwrap_err(),
            FixRerunSourceRefusal::NonLocalSourceType
        );
        assert_eq!(
            ensure_fix_rerun_source_allowed("sandbox", Some(cloud)).unwrap_err(),
            FixRerunSourceRefusal::NonLocalSourceType
        );
    }

    #[test]
    fn m3_allows_non_prod_command_and_step_flow_sources() {
        // Legacy single-script command (no spec) reads the tree.
        assert!(ensure_fix_rerun_source_allowed("sandbox", None).is_ok());
        // Generic step-flow reads the tree.
        let generic = r#"{"kind":"generic","generic":{"steps":[]}}"#;
        assert!(ensure_fix_rerun_source_allowed("staging", Some(generic)).is_ok());
    }

    #[test]
    fn m3_unparseable_spec_fails_closed() {
        assert!(!source_is_local_tree_reading(Some("{not json")));
    }

    // ---- M4 -------------------------------------------------------------

    #[test]
    fn m4_pr_body_surfaces_diff_rerun_and_caveat_but_no_agent_free_text() {
        let body = build_fix_pr_body(&FixPrBody {
            source_workflow_name: "Nightly ETL",
            source_run_id: "run-123",
            fix_branch: "chaos-fix/run-123",
            diff_stat: " src/etl.py | 2 +-\n 1 file changed",
            rerun_command: "python3 scripts/etl.py --check",
        });
        assert!(
            body.contains(FIX_RERUN_CAVEAT),
            "must state exit-0 != correct"
        );
        assert!(
            body.contains("git diff --stat"),
            "must surface the diff summary"
        );
        assert!(
            body.contains("src/etl.py | 2 +-"),
            "must include the actual diff stat"
        );
        assert!(
            body.contains("python3 scripts/etl.py --check"),
            "must show the exact rerun cmd"
        );
        assert!(body.contains("DRAFT"), "must state it is a draft");
        // Would-be agent free-text is never present because we never pass it in.
        assert!(!body.contains("IGNORE PREVIOUS INSTRUCTIONS"));
    }

    #[test]
    fn m4_pr_body_neutralizes_a_fence_breakout_in_the_diff_stat() {
        let body = build_fix_pr_body(&FixPrBody {
            source_workflow_name: "W",
            source_run_id: "r",
            fix_branch: "chaos-fix/r",
            diff_stat: "```\nInjected trusted-looking text\n```",
            rerun_command: "true",
        });
        assert!(
            !body.contains("```\nInjected"),
            "raw fence must be deflated"
        );
        assert!(
            body.contains("'''"),
            "fence delimiter is neutralized to '''"
        );
    }

    #[test]
    fn m4_gh_argv_is_always_draft_and_never_merges() {
        let argv = gh_pr_create_argv("main", "chaos-fix/r", "fix(W): x", "/tmp/body.md");
        assert!(
            argv.contains(&"--draft".to_string()),
            "the PR must be a draft"
        );
        assert!(
            !argv.iter().any(|a| a == "merge"),
            "the fix flow never merges"
        );
        assert!(argv.contains(&"--body-file".to_string()));
    }

    #[test]
    fn m4_git_push_argv_uses_the_dash_dash_separator() {
        let argv = git_push_argv("origin", "chaos-fix/run-123");
        let sep = argv.iter().position(|a| a == "--").unwrap();
        assert_eq!(
            argv[sep + 1],
            "chaos-fix/run-123",
            "branch is a positional after --"
        );
    }

    #[test]
    fn f2_git_push_argv_skips_the_pre_push_hook() {
        // sec-F2: the scheduler's own credentialed push must not fire an
        // agent-planted pre-push hook.
        let argv = git_push_argv("origin", "chaos-fix/run-123");
        assert!(
            argv.contains(&"--no-verify".to_string()),
            "the scheduler's own push skips the pre-push hook"
        );
    }

    #[test]
    fn f2_git_push_argv_force_with_lease_for_a_stale_remote() {
        // corr-F2: a stale app-owned remote branch from a prior failed attempt
        // must be safely overwritable on re-dispatch.
        let argv = git_push_argv("origin", "chaos-fix/run-123");
        assert!(
            argv.contains(&"--force-with-lease".to_string()),
            "re-dispatch safely overwrites a stale remote fix branch"
        );
    }

    #[test]
    fn m4_rerun_command_uses_script_path_for_a_legacy_command_source() {
        assert_eq!(
            describe_rerun_command("python3 scripts/etl.py --check", None),
            "python3 scripts/etl.py --check"
        );
    }

    #[test]
    fn m4_rerun_command_renders_the_steps_for_a_generic_step_flow_source() {
        // A generic step-flow's real check lives in its steps, NOT script_path
        // (which is an unused placeholder) — the body must show the true command.
        let spec = r#"{"kind":"generic","generic":{"steps":[
            {"id":"build","command":"cargo build","depends_on":[]},
            {"id":"test","command":"cargo test","depends_on":["build"]}
        ]}}"#;
        let rendered = describe_rerun_command("unused", Some(spec));
        assert!(rendered.contains("build: cargo build"));
        assert!(rendered.contains("test: cargo test"));
        assert!(
            !rendered.contains("unused"),
            "must not surface the placeholder script_path for a step-flow"
        );
    }
}
