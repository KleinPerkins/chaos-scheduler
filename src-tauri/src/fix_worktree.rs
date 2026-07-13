//! D05 LOCAL fix-agent: dedicated throwaway `git worktree` isolation (M2).
//!
//! The scheduler runs multiple workflows concurrently (`DEFAULT_WORKER_COUNT`
//! defaults to 2), every one of them against the SHARED primary checkout via
//! `current_dir(workspace_root)`. The LOCAL fix agent EDITS the working tree and
//! then RE-RUNS the failed source job against those edits. Doing either in the
//! shared tree would race the other workers — an editing `git checkout` /
//! `clean -fd` could delete another run's in-flight files, and a rerun would
//! read a tree half-mutated by someone else.
//!
//! This module confines the whole fix + rerun to a DEDICATED throwaway worktree
//! checked out to a `chaos-fix/<source_run_id>` branch under a temp base. The
//! worktree is torn down in a FINALLY ([`FixWorktree`]'s `Drop`) and, after a
//! crash, reclaimed by a startup sweep ([`sweep_orphaned_fix_worktrees`]). A
//! process-global exclusivity lease ([`acquire_worktree_lease`]) bounds this to
//! one in-flight fix at a time so two fixes can never collide.
//!
//! This file is the isolation PRIMITIVE only. Wiring it into the actual fix
//! orchestration (agent edit → rerun in the worktree → app-authored draft PR)
//! lands in the follow-up orchestrator change; the startup sweep is wired here
//! because it belongs next to the existing crash-recovery on scheduler boot.

// The create/lease/`FixWorktree`/remove primitives below have no PRODUCTION
// caller until the fix orchestrator is wired in the follow-up PR (only
// `sweep_orphaned_fix_worktrees` is wired now, on scheduler boot). They are
// fully exercised by this module's tests. This allow is removed when the
// orchestrator lands and consumes them.
#![allow(dead_code)]

use crate::service::{ProcessRunner, SystemProcessRunner};
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::atomic::{AtomicBool, Ordering};

/// Branch (and, by construction, worktree) namespace for LOCAL fix runs. The
/// startup sweep reclaims anything under it, so it MUST stay unique to this
/// feature and never collide with a user branch.
pub const FIX_BRANCH_PREFIX: &str = "chaos-fix/";

/// Base directory that holds throwaway fix worktrees. Kept OUTSIDE the primary
/// checkout (under the OS temp dir) so a stray worktree can never be mistaken
/// for tracked content.
pub fn fix_worktree_base() -> PathBuf {
    std::env::temp_dir().join("chaos-scheduler-fixes")
}

/// The deterministic throwaway-worktree path for a source run — the single
/// source of truth for where a LOCAL fix + its rerun live. Derived purely from
/// [`fix_worktree_base`] + a sanitized `source_run_id`, so the orchestrator (at
/// create time) and the rerun's execution cwd (derived from the run's own
/// identity, no persisted column) resolve the SAME directory.
pub fn fix_worktree_path_for(source_run_id: &str) -> PathBuf {
    fix_worktree_base().join(sanitize_component(source_run_id))
}

/// Derive and validate the throwaway branch name for a source run. Reuses the
/// same `validate_git_ref` guard as `git_pull` (defense in depth on top of the
/// `--` positional separator used on every git argv below).
pub fn fix_branch_name(source_run_id: &str) -> Result<String, String> {
    let branch = format!("{FIX_BRANCH_PREFIX}{source_run_id}");
    crate::operators::validate_git_ref(&branch)
        .map_err(|_| format!("invalid source_run_id for fix branch: {source_run_id:?}"))?;
    Ok(branch)
}

/// Reduce an arbitrary id to a safe single path component (defense in depth:
/// the id is our own UUID, but a `/` or `..` must never escape the temp base).
fn sanitize_component(id: &str) -> String {
    let cleaned: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "unknown".to_string()
    } else {
        cleaned
    }
}

// ---------------------------------------------------------------------------
// Global exclusivity lease
// ---------------------------------------------------------------------------

static FIX_LEASE_HELD: AtomicBool = AtomicBool::new(false);

/// RAII proof that this process holds the single fix-worktree slot. Dropping it
/// releases the slot. It is deliberately `Send` (an `AtomicBool` flag, not a
/// `MutexGuard`) so the orchestrator may hold it across its own thread's work.
#[must_use = "dropping the lease immediately releases the exclusivity slot"]
pub struct WorktreeLease {
    _private: (),
}

impl Drop for WorktreeLease {
    fn drop(&mut self) {
        FIX_LEASE_HELD.store(false, Ordering::Release);
    }
}

/// Take the process-global fix-worktree exclusivity slot, or fail fast if a fix
/// is already in flight. Callers hold the returned guard for the whole fix.
pub fn acquire_worktree_lease() -> Result<WorktreeLease, String> {
    match FIX_LEASE_HELD.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => Ok(WorktreeLease { _private: () }),
        Err(_) => Err("another local fix is already in progress".to_string()),
    }
}

// ---------------------------------------------------------------------------
// git plumbing (runner-injected for tests)
// ---------------------------------------------------------------------------

fn run_git(runner: &dyn ProcessRunner, root: &str, args: &[&str]) -> Result<Output, String> {
    let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    runner
        .run("git", &owned, Some(root), &[])
        .map_err(|e| format!("failed to spawn git: {e}"))
}

/// Run a git subcommand and turn a non-zero exit into an `Err` carrying a
/// size-capped stderr snippet. The snippet is for INTERNAL logs only; the
/// human-reviewed PR body (a later change) never echoes raw git/stderr.
fn git_checked(runner: &dyn ProcessRunner, root: &str, args: &[&str]) -> Result<(), String> {
    let out = run_git(runner, root, args)?;
    if out.status.success() {
        return Ok(());
    }
    let code = out
        .status
        .code()
        .map(|c| c.to_string())
        .unwrap_or_else(|| "signal".to_string());
    let stderr = String::from_utf8_lossy(&out.stderr);
    let snippet: String = stderr.trim().chars().take(300).collect();
    Err(format!(
        "git {} exited {code}: {snippet}",
        args.first().copied().unwrap_or("")
    ))
}

// ---------------------------------------------------------------------------
// Create / cleanup a single fix worktree
// ---------------------------------------------------------------------------

/// A live throwaway fix worktree. Cleaned up on `Drop` (the FINALLY): the
/// worktree is force-removed, its branch deleted, and the admin state pruned —
/// best-effort and logged, so a cleanup failure never masks the original error.
#[must_use = "hold the FixWorktree for the fix's lifetime; dropping it tears the worktree down"]
pub struct FixWorktree {
    workspace_root: String,
    branch: String,
    path: PathBuf,
}

impl FixWorktree {
    /// Absolute path of the throwaway checkout — the `cwd` a fix + rerun run in.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The throwaway checkout path as a `&str` (UTF-8 guaranteed at creation).
    pub fn path_str(&self) -> &str {
        self.path.to_str().unwrap_or_default()
    }

    /// The `chaos-fix/<source_run_id>` branch this worktree is checked out to.
    pub fn branch(&self) -> &str {
        &self.branch
    }
}

impl Drop for FixWorktree {
    fn drop(&mut self) {
        let runner = SystemProcessRunner;
        if let Err(e) =
            remove_fix_worktree(&runner, &self.workspace_root, self.path_str(), &self.branch)
        {
            log::warn!(
                "Failed to fully clean up fix worktree {}: {e}",
                self.path.display()
            );
        }
    }
}

/// Create a dedicated throwaway worktree for `source_run_id`, checked out to a
/// fresh `chaos-fix/<source_run_id>` branch under [`fix_worktree_base`]. This is
/// resumable/idempotent (M6): any stale worktree/branch from a crashed prior
/// attempt for the SAME source run is cleared first, then re-created.
///
/// NOTE: this does not take the exclusivity lease — the orchestrator takes
/// [`acquire_worktree_lease`] once around the whole fix so create/rerun/cleanup
/// are one critical section.
pub fn create_fix_worktree(
    workspace_root: &str,
    source_run_id: &str,
) -> Result<FixWorktree, String> {
    if workspace_root.trim().is_empty() {
        return Err("workspace_root is not configured".to_string());
    }
    let branch = fix_branch_name(source_run_id)?;
    let base = fix_worktree_base();
    std::fs::create_dir_all(&base)
        .map_err(|e| format!("failed to create fix worktree base {}: {e}", base.display()))?;
    let path = fix_worktree_path_for(source_run_id);
    let path_str = path
        .to_str()
        .ok_or_else(|| "fix worktree path is not valid UTF-8".to_string())?
        .to_string();

    let runner = SystemProcessRunner;
    // Resumable (M6): clear a stale worktree/branch from a prior crashed attempt
    // for this same source run before re-adding, so a re-dispatch is idempotent.
    let _ = remove_fix_worktree(&runner, workspace_root, &path_str, &branch);

    git_checked(
        &runner,
        workspace_root,
        &["worktree", "add", "-b", &branch, "--", &path_str],
    )
    .map_err(|e| format!("failed to create fix worktree: {e}"))?;

    Ok(FixWorktree {
        workspace_root: workspace_root.to_string(),
        branch,
        path,
    })
}

/// Force-remove a fix worktree and delete its branch. Best-effort: every step
/// runs even if an earlier one fails, and the accumulated errors are returned so
/// the caller can log them. Safe to call when nothing exists (idempotent).
pub fn remove_fix_worktree(
    runner: &dyn ProcessRunner,
    workspace_root: &str,
    path: &str,
    branch: &str,
) -> Result<(), String> {
    let mut errs: Vec<String> = Vec::new();

    if let Err(e) = git_checked(
        runner,
        workspace_root,
        &["worktree", "remove", "--force", "--", path],
    ) {
        errs.push(e);
    }
    // Prune dangling admin files even if the checkout dir was already gone.
    let _ = git_checked(runner, workspace_root, &["worktree", "prune"]);
    if let Err(e) = git_checked(runner, workspace_root, &["branch", "-D", "--", branch]) {
        errs.push(e);
    }
    // Belt-and-suspenders: drop any directory git left behind.
    let _ = std::fs::remove_dir_all(path);

    if errs.is_empty() {
        Ok(())
    } else {
        Err(errs.join("; "))
    }
}

// ---------------------------------------------------------------------------
// Startup orphan sweep (M6)
// ---------------------------------------------------------------------------

/// Parsed entry from `git worktree list --porcelain`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct WorktreeEntry {
    path: String,
    /// Short branch name (e.g. `chaos-fix/run-1`) if the worktree is on a
    /// branch; `None` for a detached HEAD.
    branch: Option<String>,
}

/// Parse `git worktree list --porcelain` into entries. Each worktree is a block
/// of `key value` lines terminated by a blank line; we only need `worktree` and
/// `branch` (`branch refs/heads/<name>`).
fn parse_worktree_porcelain(porcelain: &str) -> Vec<WorktreeEntry> {
    let mut out = Vec::new();
    let mut path: Option<String> = None;
    let mut branch: Option<String> = None;
    let mut flush = |path: &mut Option<String>, branch: &mut Option<String>| {
        if let Some(p) = path.take() {
            out.push(WorktreeEntry {
                path: p,
                branch: branch.take(),
            });
        }
    };
    for line in porcelain.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            // A new block begins; flush the previous one first.
            flush(&mut path, &mut branch);
            path = Some(p.to_string());
        } else if let Some(b) = line.strip_prefix("branch ") {
            branch = Some(b.trim_start_matches("refs/heads/").to_string());
        } else if line.trim().is_empty() {
            flush(&mut path, &mut branch);
        }
    }
    flush(&mut path, &mut branch);
    out
}

/// Startup reconciliation (M6): remove every orphaned `chaos-fix/*` worktree and
/// branch left by a crash/kill mid-fix, so a fresh session starts clean and a
/// re-dispatch of the same source run is not blocked by stale state. Best-effort
/// and fully logged; never fails scheduler startup.
pub fn sweep_orphaned_fix_worktrees(runner: &dyn ProcessRunner, workspace_root: &str) {
    if workspace_root.trim().is_empty() {
        return;
    }

    // 1. Remove any worktree still checked out to a chaos-fix/* branch (or that
    //    lives under our temp base, covering a detached/branch-deleted remnant).
    let base = fix_worktree_base();
    let base_prefix = base.to_string_lossy().to_string();
    match run_git(runner, workspace_root, &["worktree", "list", "--porcelain"]) {
        Ok(out) if out.status.success() => {
            let listing = String::from_utf8_lossy(&out.stdout);
            for entry in parse_worktree_porcelain(&listing) {
                let is_fix_branch = entry
                    .branch
                    .as_deref()
                    .map(|b| b.starts_with(FIX_BRANCH_PREFIX))
                    .unwrap_or(false);
                let under_base = entry.path.starts_with(&base_prefix);
                if !(is_fix_branch || under_base) {
                    continue;
                }
                log::info!("Reclaiming orphaned fix worktree {}", entry.path);
                if let Err(e) = git_checked(
                    runner,
                    workspace_root,
                    &["worktree", "remove", "--force", "--", &entry.path],
                ) {
                    log::warn!("Failed to remove orphaned fix worktree {}: {e}", entry.path);
                }
            }
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            log::warn!(
                "Fix-worktree sweep: `git worktree list` failed: {}",
                stderr.trim()
            );
        }
        Err(e) => log::warn!("Fix-worktree sweep skipped: {e}"),
    }

    // 2. Prune admin state, then delete every leftover chaos-fix/* branch (some
    //    may have no worktree if the crash happened after removal but before
    //    branch deletion).
    let _ = git_checked(runner, workspace_root, &["worktree", "prune"]);
    if let Ok(out) = run_git(
        runner,
        workspace_root,
        &[
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/heads/chaos-fix/",
        ],
    ) {
        if out.status.success() {
            let branches = String::from_utf8_lossy(&out.stdout);
            for branch in branches.lines().map(str::trim).filter(|b| !b.is_empty()) {
                if !branch.starts_with(FIX_BRANCH_PREFIX) {
                    continue;
                }
                log::info!("Deleting orphaned fix branch {branch}");
                if let Err(e) = git_checked(runner, workspace_root, &["branch", "-D", "--", branch])
                {
                    log::warn!("Failed to delete orphaned fix branch {branch}: {e}");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Output};
    use std::sync::Mutex;

    /// A scripted `ProcessRunner` that records every git argv and returns queued
    /// outputs in order (defaulting to success/empty once the script is drained).
    struct ScriptedRunner {
        calls: Mutex<Vec<Vec<String>>>,
        outputs: Mutex<std::collections::VecDeque<Output>>,
    }

    impl ScriptedRunner {
        fn new(outputs: Vec<Output>) -> Self {
            ScriptedRunner {
                calls: Mutex::new(Vec::new()),
                outputs: Mutex::new(outputs.into_iter().collect()),
            }
        }

        fn argvs(&self) -> Vec<Vec<String>> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl ProcessRunner for ScriptedRunner {
        fn run(
            &self,
            program: &str,
            args: &[String],
            _cwd: Option<&str>,
            _env: &[(String, String)],
        ) -> std::io::Result<Output> {
            let mut argv = vec![program.to_string()];
            argv.extend(args.iter().cloned());
            self.calls.lock().unwrap().push(argv);
            Ok(self
                .outputs
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| ok_output("")))
        }
    }

    fn ok_output(stdout: &str) -> Output {
        use std::os::unix::process::ExitStatusExt;
        Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        }
    }

    fn is_git() -> bool {
        Command::new("git")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// A `git` command with any inherited git-hook context stripped, so the
    /// real-git tests below are hermetic even when `cargo test` runs inside this
    /// project's own pre-push hook (which exports `GIT_DIR`/`GIT_INDEX_FILE` —
    /// and `GIT_DIR` overrides `current_dir`, redirecting git at the primary
    /// checkout). This mirrors what [`SystemProcessRunner`] now does for our own
    /// git plumbing, keeping the direct-`Command` test helpers consistent.
    fn git_cmd() -> Command {
        let mut cmd = Command::new("git");
        for key in crate::service::INHERITED_GIT_CONTEXT_VARS {
            cmd.env_remove(key);
        }
        cmd
    }

    fn init_repo() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("chaos-fix-it-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let run = |args: &[&str]| {
            let ok = git_cmd()
                .args(args)
                .current_dir(&dir)
                .output()
                .unwrap()
                .status
                .success();
            assert!(ok, "git {args:?} failed");
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.email", "t@t.t"]);
        run(&["config", "user.name", "t"]);
        std::fs::write(dir.join("a.txt"), "hi").unwrap();
        run(&["add", "-A"]);
        run(&["commit", "-qm", "init"]);
        dir
    }

    #[test]
    fn fix_branch_name_is_namespaced_and_validated() {
        assert_eq!(
            fix_branch_name("run-123").unwrap(),
            "chaos-fix/run-123".to_string()
        );
        // Whitespace / control chars are rejected by validate_git_ref.
        assert!(fix_branch_name("bad id").is_err());
        assert!(fix_branch_name("bad\nid").is_err());
    }

    #[test]
    fn add_uses_dash_dash_separator_and_namespaced_branch() {
        // FAILING-FIRST: exercises the exact argv the worktree add must emit —
        // a namespaced branch plus the `--` positional separator (defense in
        // depth against option smuggling), matching the git precedent.
        let runner = ScriptedRunner::new(vec![]); // remove(preclean) + add all "succeed"
        let out = git_checked(
            &runner,
            "/repo",
            &["worktree", "add", "-b", "chaos-fix/run-1", "--", "/tmp/wt"],
        );
        assert!(out.is_ok());
        let argvs = runner.argvs();
        assert_eq!(
            argvs[0],
            vec![
                "git",
                "worktree",
                "add",
                "-b",
                "chaos-fix/run-1",
                "--",
                "/tmp/wt"
            ]
        );
    }

    #[test]
    fn remove_fix_worktree_emits_force_remove_prune_and_branch_delete() {
        let runner = ScriptedRunner::new(vec![]);
        let _ = remove_fix_worktree(&runner, "/repo", "/tmp/wt", "chaos-fix/run-1");
        let argvs = runner.argvs();
        assert_eq!(
            argvs[0],
            vec!["git", "worktree", "remove", "--force", "--", "/tmp/wt"]
        );
        assert_eq!(argvs[1], vec!["git", "worktree", "prune"]);
        assert_eq!(
            argvs[2],
            vec!["git", "branch", "-D", "--", "chaos-fix/run-1"]
        );
    }

    #[test]
    fn parse_worktree_porcelain_extracts_paths_and_branches() {
        let listing = "worktree /repo\nHEAD abc\nbranch refs/heads/main\n\nworktree /tmp/wt\nHEAD abc\nbranch refs/heads/chaos-fix/run-1\n\n";
        let entries = parse_worktree_porcelain(listing);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, "/repo");
        assert_eq!(entries[0].branch.as_deref(), Some("main"));
        assert_eq!(entries[1].path, "/tmp/wt");
        assert_eq!(entries[1].branch.as_deref(), Some("chaos-fix/run-1"));
    }

    #[test]
    fn sweep_removes_only_fix_worktrees_and_branches() {
        // Scripted: (1) worktree list returns one normal + one chaos-fix wt,
        // (2) remove, (3) prune, (4) for-each-ref returns one chaos-fix branch,
        // (5) branch -D. Assert we only touch the chaos-fix targets.
        let listing = "worktree /repo\nHEAD abc\nbranch refs/heads/main\n\nworktree /tmp/chaos-fix-x\nHEAD abc\nbranch refs/heads/chaos-fix/run-9\n\n";
        let runner = ScriptedRunner::new(vec![
            ok_output(listing),             // worktree list --porcelain
            ok_output(""),                  // worktree remove
            ok_output(""),                  // worktree prune
            ok_output("chaos-fix/run-9\n"), // for-each-ref
            ok_output(""),                  // branch -D
        ]);
        sweep_orphaned_fix_worktrees(&runner, "/repo");
        let argvs = runner.argvs();
        assert_eq!(argvs[0], vec!["git", "worktree", "list", "--porcelain"]);
        assert_eq!(
            argvs[1],
            vec![
                "git",
                "worktree",
                "remove",
                "--force",
                "--",
                "/tmp/chaos-fix-x"
            ]
        );
        // The normal `main` worktree (/repo) must NEVER be removed.
        assert!(!argvs
            .iter()
            .any(|a| a.contains(&"/repo".to_string()) && a.contains(&"remove".to_string())));
        assert!(argvs
            .iter()
            .any(|a| a == &vec!["git", "branch", "-D", "--", "chaos-fix/run-9"]));
    }

    #[test]
    fn lease_is_exclusive_and_releases_on_drop() {
        let first = acquire_worktree_lease().expect("first acquire succeeds");
        assert!(
            acquire_worktree_lease().is_err(),
            "a second concurrent lease must fail fast"
        );
        drop(first);
        let _second = acquire_worktree_lease().expect("re-acquire after release succeeds");
    }

    #[test]
    fn create_fix_worktree_then_drop_cleans_up_end_to_end() {
        if !is_git() {
            return;
        }
        let repo = init_repo();
        let repo_str = repo.to_str().unwrap();
        let source_run_id = format!("run-{}", uuid::Uuid::new_v4());
        let branch = fix_branch_name(&source_run_id).unwrap();

        let wt = create_fix_worktree(repo_str, &source_run_id).expect("create worktree");
        let wt_path = wt.path().to_path_buf();
        assert!(wt_path.exists(), "worktree checkout dir exists");
        assert!(
            wt_path.join("a.txt").exists(),
            "worktree has the repo files"
        );

        // The branch is live while the worktree exists.
        let branches = git_cmd()
            .args(["for-each-ref", "--format=%(refname:short)", "refs/heads/"])
            .current_dir(&repo)
            .output()
            .unwrap();
        assert!(String::from_utf8_lossy(&branches.stdout).contains(&branch));

        // FINALLY: dropping tears down the worktree AND deletes the branch.
        drop(wt);
        assert!(!wt_path.exists(), "worktree dir removed on drop");
        let after = git_cmd()
            .args(["for-each-ref", "--format=%(refname:short)", "refs/heads/"])
            .current_dir(&repo)
            .output()
            .unwrap();
        assert!(
            !String::from_utf8_lossy(&after.stdout).contains(&branch),
            "fix branch deleted on drop"
        );

        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn sweep_reclaims_a_real_orphaned_worktree() {
        if !is_git() {
            return;
        }
        let repo = init_repo();
        let repo_str = repo.to_str().unwrap();
        let source_run_id = format!("run-{}", uuid::Uuid::new_v4());
        let branch = fix_branch_name(&source_run_id).unwrap();

        // Simulate a crash: a fix worktree + branch left behind with no guard.
        let wt = create_fix_worktree(repo_str, &source_run_id).unwrap();
        let wt_path = wt.path().to_path_buf();
        std::mem::forget(wt); // skip Drop => orphaned, as after a kill

        assert!(wt_path.exists());
        let runner = SystemProcessRunner;
        sweep_orphaned_fix_worktrees(&runner, repo_str);

        assert!(!wt_path.exists(), "sweep removed the orphaned worktree");
        let after = git_cmd()
            .args(["for-each-ref", "--format=%(refname:short)", "refs/heads/"])
            .current_dir(&repo)
            .output()
            .unwrap();
        assert!(
            !String::from_utf8_lossy(&after.stdout).contains(&branch),
            "sweep deleted the orphaned fix branch"
        );

        let _ = std::fs::remove_dir_all(&repo);
    }
}
