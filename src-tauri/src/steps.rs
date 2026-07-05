//! In-scheduler step-flow executor.
//!
//! Executes a generic workflow's steps in dependency order within a single run,
//! honoring per-step retry and fail-fast vs continue-on-error semantics. Uses
//! the injected [`ProcessRunner`] so the DAG logic is unit-testable. The
//! scheduler remains the sole author of `run_tasks`/`run_attempts` for these
//! steps (the task-ownership contract): the caller persists each returned
//! [`StepResult`] rather than the child emitting task events.

use crate::service::ProcessRunner;
use crate::workflow_spec::{topological_order, GenericSpec, StepSpec};
use std::collections::{HashMap, HashSet};

/// Outcome of a single step.
#[derive(Debug, Clone)]
pub struct StepResult {
    pub step_id: String,
    pub success: bool,
    pub skipped: bool,
    pub attempts: u32,
    pub exit_code: Option<i32>,
    pub message: String,
}

/// Outcome of an entire step-flow.
#[derive(Debug, Clone)]
pub struct StepFlowOutcome {
    pub success: bool,
    pub results: Vec<StepResult>,
}

/// Resolve the program + args for a step. `command` runs via `sh -c`; `script`
/// runs the resolved path directly with `args`.
fn command_for(step: &StepSpec, workspace_root: &str) -> (String, Vec<String>, String) {
    let cwd = step
        .working_dir
        .clone()
        .unwrap_or_else(|| workspace_root.to_string());
    if let Some(cmd) = &step.command {
        ("sh".to_string(), vec!["-c".to_string(), cmd.clone()], cwd)
    } else {
        let script = step.script.clone().unwrap_or_default();
        let resolved = if script.starts_with('/') {
            script
        } else {
            format!("{}/{}", cwd, script)
        };
        let mut args = vec![resolved];
        args.extend(step.args.iter().cloned());
        // Run the script via `sh` so it works regardless of executable bit /
        // shebang; the first arg is the script path.
        let program = args.remove(0);
        (program, args, cwd)
    }
}

/// Execute a generic step-flow. `sleep` allows tests to skip real backoff waits.
pub fn execute_step_flow(
    spec: &GenericSpec,
    runner: &dyn ProcessRunner,
    workspace_root: &str,
    base_env: &[(String, String)],
) -> Result<StepFlowOutcome, String> {
    execute_step_flow_inner(spec, runner, workspace_root, base_env, &mut |d| {
        crate::scheduler::sleep_interruptible(d)
    })
}

fn execute_step_flow_inner(
    spec: &GenericSpec,
    runner: &dyn ProcessRunner,
    workspace_root: &str,
    base_env: &[(String, String)],
    sleep: &mut dyn FnMut(std::time::Duration),
) -> Result<StepFlowOutcome, String> {
    let order = topological_order(&spec.steps)?;
    let step_map: HashMap<&str, &StepSpec> =
        spec.steps.iter().map(|s| (s.id.as_str(), s)).collect();

    let mut results: Vec<StepResult> = Vec::with_capacity(order.len());
    // Steps whose failure should block dependents (i.e. failed and not tolerated).
    let mut blocking_failures: HashSet<String> = HashSet::new();
    let mut overall_success = true;

    for id in &order {
        let step = step_map[id.as_str()];

        // Skip if any dependency is a blocking failure.
        if step
            .depends_on
            .iter()
            .any(|d| blocking_failures.contains(d))
        {
            results.push(StepResult {
                step_id: id.clone(),
                success: false,
                skipped: true,
                attempts: 0,
                exit_code: None,
                message: "skipped: upstream dependency failed".to_string(),
            });
            overall_success = false;
            continue;
        }

        let (program, args, cwd) = command_for(step, workspace_root);
        let max_attempts = step
            .retry
            .as_ref()
            .map(|r| r.max_retries.saturating_add(1))
            .unwrap_or(1)
            .max(1);
        let backoff = step.retry.as_ref().map(|r| r.backoff_seconds).unwrap_or(0);

        let mut success = false;
        let mut exit_code = None;
        let mut attempts = 0u32;
        let mut message = String::new();

        for attempt in 0..max_attempts {
            attempts = attempt + 1;
            match runner.run(&program, &args, Some(&cwd), base_env) {
                Ok(output) => {
                    exit_code = output.status.code();
                    if output.status.success() {
                        success = true;
                        message = "ok".to_string();
                        break;
                    }
                    message = format!("exited with {:?}", exit_code);
                }
                Err(e) => {
                    exit_code = None;
                    message = format!("spawn error: {e}");
                }
            }
            if attempt + 1 < max_attempts && backoff > 0 {
                if crate::scheduler::SHUTDOWN.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
                sleep(std::time::Duration::from_secs(backoff));
            }
        }

        if !success {
            if step.continue_on_error {
                // Failure tolerated: dependents still run, but the run's final
                // status is unaffected (matching "continue" semantics).
            } else {
                overall_success = false;
                blocking_failures.insert(id.clone());
            }
        }

        results.push(StepResult {
            step_id: id.clone(),
            success,
            skipped: false,
            attempts,
            exit_code,
            message,
        });
    }

    Ok(StepFlowOutcome {
        success: overall_success,
        results,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow_spec::{RetryPolicy, StepSpec};
    use std::process::Output;
    use std::sync::Mutex;

    #[cfg(unix)]
    fn out(code: i32) -> Output {
        use std::os::unix::process::ExitStatusExt;
        Output {
            status: std::process::ExitStatus::from_raw((code & 0xff) << 8),
            stdout: Vec::new(),
            stderr: Vec::new(),
        }
    }

    /// Runner returning a per-command exit code keyed by the `sh -c` payload or
    /// the program name.
    struct ScriptedRunner {
        codes: HashMap<String, Vec<i32>>,
        calls: Mutex<Vec<String>>,
    }
    #[cfg(unix)]
    impl ProcessRunner for ScriptedRunner {
        fn run(
            &self,
            program: &str,
            args: &[String],
            _cwd: Option<&str>,
            _env: &[(String, String)],
        ) -> std::io::Result<Output> {
            let key = if program == "sh" {
                args.get(1).cloned().unwrap_or_default()
            } else {
                program.to_string()
            };
            self.calls.lock().unwrap().push(key.clone());
            let count = self
                .calls
                .lock()
                .unwrap()
                .iter()
                .filter(|k| **k == key)
                .count();
            let code = self
                .codes
                .get(&key)
                .and_then(|v| v.get(count - 1).copied().or_else(|| v.last().copied()))
                .unwrap_or(0);
            Ok(out(code))
        }
    }

    fn step(id: &str, cmd: &str, deps: &[&str]) -> StepSpec {
        StepSpec {
            id: id.to_string(),
            command: Some(cmd.to_string()),
            script: None,
            args: vec![],
            working_dir: None,
            depends_on: deps.iter().map(|s| s.to_string()).collect(),
            retry: None,
            timeout_seconds: None,
            continue_on_error: false,
        }
    }

    #[test]
    #[cfg(unix)]
    fn all_steps_succeed_in_dependency_order() {
        let spec = GenericSpec {
            steps: vec![step("a", "cmd_a", &[]), step("b", "cmd_b", &["a"])],
        };
        let runner = ScriptedRunner {
            codes: HashMap::new(),
            calls: Mutex::new(vec![]),
        };
        let outcome = execute_step_flow(&spec, &runner, "/tmp", &[]).unwrap();
        assert!(outcome.success);
        assert_eq!(outcome.results.len(), 2);
        assert!(outcome.results.iter().all(|r| r.success));
    }

    #[test]
    #[cfg(unix)]
    fn dependent_is_skipped_when_upstream_fails_fast() {
        let spec = GenericSpec {
            steps: vec![step("a", "cmd_a", &[]), step("b", "cmd_b", &["a"])],
        };
        let mut codes = HashMap::new();
        codes.insert("cmd_a".to_string(), vec![1]);
        let runner = ScriptedRunner {
            codes,
            calls: Mutex::new(vec![]),
        };
        let outcome = execute_step_flow(&spec, &runner, "/tmp", &[]).unwrap();
        assert!(!outcome.success);
        let b = outcome.results.iter().find(|r| r.step_id == "b").unwrap();
        assert!(b.skipped);
    }

    #[test]
    #[cfg(unix)]
    fn continue_on_error_lets_dependents_run_and_keeps_run_green() {
        let mut a = step("a", "cmd_a", &[]);
        a.continue_on_error = true;
        let spec = GenericSpec {
            steps: vec![a, step("b", "cmd_b", &["a"])],
        };
        let mut codes = HashMap::new();
        codes.insert("cmd_a".to_string(), vec![1]);
        let runner = ScriptedRunner {
            codes,
            calls: Mutex::new(vec![]),
        };
        let outcome = execute_step_flow(&spec, &runner, "/tmp", &[]).unwrap();
        assert!(outcome.success, "tolerated failure keeps run green");
        let b = outcome.results.iter().find(|r| r.step_id == "b").unwrap();
        assert!(!b.skipped && b.success);
    }

    #[test]
    #[cfg(unix)]
    fn retry_reattempts_until_success() {
        let mut a = step("a", "cmd_a", &[]);
        a.retry = Some(RetryPolicy {
            max_retries: 2,
            backoff_seconds: 0,
        });
        let spec = GenericSpec { steps: vec![a] };
        let mut codes = HashMap::new();
        // fail, fail, succeed
        codes.insert("cmd_a".to_string(), vec![1, 1, 0]);
        let runner = ScriptedRunner {
            codes,
            calls: Mutex::new(vec![]),
        };
        let outcome = execute_step_flow(&spec, &runner, "/tmp", &[]).unwrap();
        assert!(outcome.success);
        assert_eq!(outcome.results[0].attempts, 3);
    }
}
