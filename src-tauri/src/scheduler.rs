use crate::db::Database;
use chrono::Utc;
use chrono_tz::Tz;
use cron::Schedule;
use serde::Deserialize;
use std::process::Command;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub static SHUTDOWN: AtomicBool = AtomicBool::new(false);

pub struct WorkflowScheduler {
    db: Arc<Database>,
    notify_on_failure: AtomicBool,
    notify_on_success: AtomicBool,
    email_on_failure: AtomicBool,
}

impl WorkflowScheduler {
    pub fn new(db: Arc<Database>) -> Self {
        let email_enabled = db.get_email_config().map(|c| c.enabled).unwrap_or(false);
        Self {
            db,
            notify_on_failure: AtomicBool::new(true),
            notify_on_success: AtomicBool::new(false),
            email_on_failure: AtomicBool::new(email_enabled),
        }
    }

    pub fn set_notify_on_failure(&self, val: bool) {
        self.notify_on_failure.store(val, Ordering::Relaxed);
    }

    pub fn set_notify_on_success(&self, val: bool) {
        self.notify_on_success.store(val, Ordering::Relaxed);
    }

    pub fn refresh_email_config(&self) {
        let enabled = self
            .db
            .get_email_config()
            .map(|c| c.enabled)
            .unwrap_or(false);
        self.email_on_failure.store(enabled, Ordering::Relaxed);
    }

    pub fn should_email_on_failure(&self) -> bool {
        self.email_on_failure.load(Ordering::Relaxed)
    }

    pub fn find_due_workflows(&self) -> Vec<DueWorkflow> {
        let workflows = match self.db.list_workflows() {
            Ok(w) => w,
            Err(e) => {
                log::error!("Failed to list workflows: {}", e);
                return vec![];
            }
        };

        let now = Utc::now();
        let mut due = vec![];

        for workflow in workflows {
            if !workflow.enabled {
                continue;
            }

            let tz = parse_tz(&workflow.timezone);
            let since = now - chrono::Duration::days(2);
            let Some(scheduled_time) =
                latest_scheduled_multi(&workflow.cron_schedule, tz, since, now)
            else {
                continue;
            };

            let last_run = workflow.last_run_at.as_ref().and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(s)
                    .ok()
                    .map(|d| d.with_timezone(&Utc))
            });

            if let Some(last) = last_run {
                if last >= scheduled_time {
                    continue;
                }
            }

            log::info!(
                "Running due workflow: {} (scheduled for {})",
                workflow.name,
                scheduled_time
            );
            let now_str = now.to_rfc3339();
            let _ = self.db.set_last_run_at(&workflow.id, &now_str);
            due.push(DueWorkflow {
                id: workflow.id.clone(),
            });
        }

        due
    }
}

pub struct DueWorkflow {
    pub id: String,
}

/// Compute the next run time for a cron expression in the given timezone.
/// Pure function — no scheduler state needed.
pub fn get_next_run_time(cron_expr: &str, timezone: &str) -> Option<String> {
    let tz = parse_tz(timezone);
    next_run_multi(cron_expr, tz).map(|t| t.to_rfc3339())
}

/// Execute a workflow subprocess. Does not require the scheduler mutex.
pub fn execute_workflow(
    db: &Arc<Database>,
    chaos_labs_root: &str,
    python_path: &str,
    workflow_id: &str,
    notify_on_success: bool,
    notify_on_failure: bool,
    email_on_failure_enabled: bool,
) -> Result<RunResult, String> {
    let workflow = db
        .get_workflow(workflow_id)
        .map_err(|e| format!("Failed to get workflow: {}", e))?;

    let run = db
        .create_run(&workflow.id)
        .map_err(|e| format!("Failed to create run record: {}", e))?;

    let output =
        build_workflow_command(&workflow.script_path, chaos_labs_root, python_path, &run.id)
            .output();

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = output.status.code().unwrap_or(-1);

            let result_url = extract_result_url(&stdout);

            let bg_pid = extract_background_pid(&stdout, chaos_labs_root);

            if let Some(pid) = bg_pid {
                let db = Arc::clone(db);
                let run_id = run.id.clone();
                let wf_name = workflow.name.clone();
                let wf_script = workflow.script_path.clone();
                let wf_email = workflow.email_on_failure;
                let root = chaos_labs_root.to_string();
                let email_enabled = email_on_failure_enabled;

                let bg_log = extract_log_path(&stdout);
                let log_start_offset = extract_log_start_offset(&stdout);

                std::thread::spawn(move || {
                    monitor_background_pid(
                        pid,
                        &run_id,
                        &wf_name,
                        &wf_script,
                        &root,
                        bg_log.as_deref(),
                        log_start_offset,
                        email_enabled && wf_email,
                        &db,
                    );
                });

                return Ok(RunResult {
                    run_id: run.id,
                    workflow_name: workflow.name,
                    script_path: workflow.script_path.clone(),
                    success: true,
                    should_notify: false,
                    email_on_failure: workflow.email_on_failure,
                });
            }

            db.finish_run(&run.id, exit_code, &stdout, &stderr, result_url.as_deref())
                .map_err(|e| format!("Failed to update run: {}", e))?;

            let success = exit_code == 0;
            Ok(RunResult {
                run_id: run.id,
                workflow_name: workflow.name,
                script_path: workflow.script_path.clone(),
                success,
                should_notify: if success {
                    notify_on_success
                } else {
                    notify_on_failure
                },
                email_on_failure: workflow.email_on_failure,
            })
        }
        Err(e) => {
            let _ = db.finish_run(&run.id, -1, "", &format!("Failed to execute: {}", e), None);
            Err(format!("Failed to execute workflow: {}", e))
        }
    }
}

pub struct RunResult {
    pub run_id: String,
    pub workflow_name: String,
    pub script_path: String,
    pub success: bool,
    pub should_notify: bool,
    pub email_on_failure: bool,
}

fn parse_tz(tz_str: &str) -> Tz {
    tz_str.parse::<Tz>().unwrap_or(chrono_tz::UTC)
}

/// Convert standard 5-field cron (min hour dom month dow) to the 6-field
/// format the `cron` crate requires (sec min hour dom month dow) by
/// prepending seconds=0. Passes 6- and 7-field expressions through unchanged.
fn normalize_cron(expr: &str) -> String {
    let field_count = expr.split_whitespace().count();
    if field_count == 5 {
        format!("0 {}", expr)
    } else {
        expr.to_string()
    }
}

/// Parse a potentially semicolon-delimited multi-cron schedule and return the
/// earliest upcoming fire time across all sub-expressions. The cron fields are
/// interpreted in the given timezone; the returned instant is UTC.
fn next_run_multi(cron_expr: &str, tz: Tz) -> Option<chrono::DateTime<Utc>> {
    let mut earliest: Option<chrono::DateTime<Utc>> = None;
    for expr in cron_expr.split(';') {
        let trimmed = expr.trim();
        if trimmed.is_empty() {
            continue;
        }
        let normalized = normalize_cron(trimmed);
        if let Ok(schedule) = Schedule::from_str(&normalized) {
            if let Some(next) = schedule.upcoming(tz).next() {
                let next_utc = next.with_timezone(&Utc);
                if earliest.is_none() || next_utc < earliest.unwrap() {
                    earliest = Some(next_utc);
                }
            }
        }
    }
    earliest
}

/// Parse a potentially semicolon-delimited multi-cron schedule and return the
/// latest recently-scheduled time across all sub-expressions within the window
/// `[since, until]`. The cron fields are interpreted in the given timezone;
/// since/until and the return value are UTC.
fn latest_scheduled_multi(
    cron_expr: &str,
    tz: Tz,
    since: chrono::DateTime<Utc>,
    until: chrono::DateTime<Utc>,
) -> Option<chrono::DateTime<Utc>> {
    let since_tz = since.with_timezone(&tz);
    let until_tz = until.with_timezone(&tz);
    let mut latest: Option<chrono::DateTime<Utc>> = None;
    for expr in cron_expr.split(';') {
        let trimmed = expr.trim();
        if trimmed.is_empty() {
            continue;
        }
        let normalized = normalize_cron(trimmed);
        let schedule = match Schedule::from_str(&normalized) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let most_recent = schedule
            .after(&since_tz)
            .take_while(|t| *t <= until_tz)
            .last();
        if let Some(t) = most_recent {
            let t_utc = t.with_timezone(&Utc);
            if latest.is_none() || t_utc > latest.unwrap() {
                latest = Some(t_utc);
            }
        }
    }
    latest
}

/// Build the Command for a workflow, handling two script_path conventions:
/// - Full shell command (contains env vars or absolute python path):
///   executed via `sh -c` so env assignments, args, etc. all work.
/// - Relative script path (e.g. `scripts/workflows/daily_digest.py`):
///   resolved against chaos_labs_root and executed with the detected python.
fn build_workflow_command(
    script_path: &str,
    chaos_labs_root: &str,
    python_path: &str,
    run_id: &str,
) -> Command {
    let is_shell_cmd = script_path.contains('=') || script_path.contains("/bin/python");

    if is_shell_cmd {
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg(script_path)
            .current_dir(chaos_labs_root)
            .env("CHAOS_LABS_ROOT", chaos_labs_root)
            .env("CHAOS_LABS_SCHEDULER_RUN_ID", run_id);
        cmd
    } else {
        let parts: Vec<&str> = script_path.split_whitespace().collect();
        let resolved = if parts[0].starts_with('/') {
            parts[0].to_string()
        } else {
            format!("{}/{}", chaos_labs_root, parts[0])
        };
        let mut cmd = Command::new(python_path);
        cmd.arg(&resolved);
        for arg in &parts[1..] {
            cmd.arg(arg);
        }
        cmd.current_dir(chaos_labs_root)
            .env("CHAOS_LABS_ROOT", chaos_labs_root)
            .env("CHAOS_LABS_SCHEDULER_RUN_ID", run_id);
        cmd
    }
}

fn extract_result_url(stdout: &str) -> Option<String> {
    for line in stdout.lines().rev() {
        let trimmed = line.trim();
        if trimmed.starts_with("RESULT_URL:") {
            return Some(trimmed.trim_start_matches("RESULT_URL:").trim().to_string());
        }
        if trimmed.starts_with("https://docs.google.com/")
            || trimmed.starts_with("https://drive.google.com/")
        {
            return Some(trimmed.to_string());
        }
    }
    None
}

/// Look for evidence that the script spawned a long-running background process.
/// Checks stdout for the launcher convention "launched (PID <n>)". Only falls
/// back to PID file detection when stdout already contains a launch signal, so
/// non-launcher workflows aren't accidentally matched against a stale PID file.
fn extract_background_pid(stdout: &str, chaos_labs_root: &str) -> Option<u32> {
    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("Context capture launched (PID ") {
            if let Some(pid_str) = rest.strip_suffix(')') {
                if let Ok(pid) = pid_str.parse::<u32>() {
                    return Some(pid);
                }
            }
        }
        // Generic launcher pattern: any "<name> launched (PID <n>)"
        if let Some(idx) = trimmed.find("launched (PID ") {
            let after = &trimmed[idx + "launched (PID ".len()..];
            if let Some(pid_str) = after.strip_suffix(')') {
                if let Ok(pid) = pid_str.parse::<u32>() {
                    return Some(pid);
                }
            }
        }
    }

    // Only check the PID file as a fallback when stdout has a launch indicator.
    let has_launch_signal = stdout.lines().any(|l| {
        let t = l.trim();
        t.contains("launched") || t.contains("Background PID")
    });
    if !has_launch_signal {
        return None;
    }

    let pid_paths = [
        format!("{}/data/context-capture/capture.pid", chaos_labs_root),
        format!("{}/data/context-refresh/refresh.pid", chaos_labs_root),
    ];
    for path in &pid_paths {
        if let Ok(contents) = std::fs::read_to_string(path) {
            if let Ok(pid) = contents.trim().parse::<u32>() {
                let alive = unsafe { libc::kill(pid as i32, 0) } == 0;
                if alive {
                    return Some(pid);
                }
            }
        }
    }

    None
}

/// Extract the background log path from launcher stdout.
fn extract_log_path(stdout: &str) -> Option<String> {
    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some(path) = trimmed.strip_prefix("Log: ") {
            let path = path.trim();
            if !path.is_empty() {
                return Some(path.to_string());
            }
        }
    }
    None
}

fn extract_log_start_offset(stdout: &str) -> Option<u64> {
    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some(offset) = trimmed.strip_prefix("LogStartOffset: ") {
            if let Ok(parsed) = offset.trim().parse::<u64>() {
                return Some(parsed);
            }
        }
    }
    None
}

/// Poll a background PID until it exits, then finalize the run record.
fn monitor_background_pid(
    pid: u32,
    run_id: &str,
    wf_name: &str,
    wf_script: &str,
    chaos_labs_root: &str,
    bg_log_path: Option<&str>,
    log_start_offset: Option<u64>,
    email_enabled: bool,
    db: &Database,
) {
    log::info!(
        "Monitoring background PID {} for workflow '{}'",
        pid,
        wf_name
    );

    loop {
        std::thread::sleep(Duration::from_secs(10));

        let alive = unsafe { libc::kill(pid as i32, 0) } == 0;
        if !alive {
            break;
        }
    }

    log::info!(
        "Background PID {} for workflow '{}' has exited",
        pid,
        wf_name
    );

    let log_path = bg_log_path
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{}/data/context-capture/capture_bg.log", chaos_labs_root));

    let log_dir = std::path::Path::new(&log_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string());
    let stdout = read_log_slice(&log_path, log_start_offset).unwrap_or_default();
    let run_status = log_dir
        .as_deref()
        .and_then(|dir| read_run_scoped_exit_status_from_dir(dir, run_id));
    let exit_code = run_status
        .as_ref()
        .and_then(|status| status.exit_code)
        .or_else(|| {
            if log_start_offset.is_none() {
                log_dir
                    .as_deref()
                    .and_then(read_exit_status_from_dir)
                    .or_else(|| {
                        read_exit_status_from_dir(&format!(
                            "{}/data/context-capture",
                            chaos_labs_root
                        ))
                    })
            } else {
                None
            }
        })
        .unwrap_or_else(|| infer_exit_code_from_current_output(&stdout));
    let result_url = run_status
        .as_ref()
        .and_then(|status| {
            status
                .result_url
                .clone()
                .or_else(|| status.report_path.as_ref().map(|p| format!("file://{}", p)))
        })
        .or_else(|| extract_result_url(&stdout))
        .or_else(|| Some(format!("file://{}", log_path)));

    let _ = db.finish_run(run_id, exit_code, &stdout, "", result_url.as_deref());

    if exit_code != 0 && email_enabled {
        let result = RunResult {
            run_id: run_id.to_string(),
            workflow_name: wf_name.to_string(),
            script_path: wf_script.to_string(),
            success: false,
            should_notify: true,
            email_on_failure: true,
        };
        send_failure_email(db, chaos_labs_root, &result);
    }
}

fn read_exit_status_from_dir(dir: &str) -> Option<i32> {
    let candidates = [
        format!("{}/exit_status.json", dir),
        format!("{}/capture_exit_status.json", dir),
    ];
    for path in &candidates {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(code) = parsed.get("exit_code").and_then(|v| v.as_i64()) {
                    let _ = std::fs::remove_file(path);
                    return Some(code as i32);
                }
            }
        }
    }
    None
}

fn read_exit_status(chaos_labs_root: &str) -> Option<i32> {
    read_exit_status_from_dir(&format!("{}/data/context-capture", chaos_labs_root))
}

#[derive(Debug, Deserialize)]
struct RunScopedStatus {
    run_id: Option<String>,
    exit_code: Option<i32>,
    result_url: Option<String>,
    report_path: Option<String>,
}

fn read_run_scoped_exit_status_from_dir(dir: &str, run_id: &str) -> Option<RunScopedStatus> {
    let path = std::path::Path::new(dir)
        .join("run-status")
        .join(format!("{}.json", run_id));
    let content = std::fs::read_to_string(path).ok()?;
    let parsed = serde_json::from_str::<RunScopedStatus>(&content).ok()?;
    if parsed.run_id.as_deref().is_some_and(|id| id != run_id) {
        return None;
    }
    Some(parsed)
}

fn read_log_slice(path: &str, start_offset: Option<u64>) -> std::io::Result<String> {
    use std::io::{Read, Seek, SeekFrom};

    let mut file = std::fs::File::open(path)?;
    if let Some(offset) = start_offset {
        let len = file.metadata()?.len();
        file.seek(SeekFrom::Start(offset.min(len)))?;
    }
    let mut output = String::new();
    file.read_to_string(&mut output)?;
    Ok(output)
}

fn infer_exit_code_from_current_output(stdout: &str) -> i32 {
    let success_markers = [
        "Context capture completed",
        "Context refresh completed",
        "Completed phase: report",
        "SUMMARY_JSON:",
        "RESULT_URL:",
    ];
    if success_markers.iter().any(|marker| stdout.contains(marker)) {
        return 0;
    }
    if stdout.contains("Traceback") || stdout.contains("] ERROR:") {
        1
    } else {
        0
    }
}

/// On startup, finalize any runs stuck in "running" from a previous session.
/// PID re-attachment is unreliable across restarts (the PID file is
/// workflow-agnostic), so orphaned runs are always finalized. The scheduler
/// will re-trigger any missed workflows on the next tick via the backward scan.
fn recover_orphaned_runs(db: &Database, chaos_labs_root: &str) {
    let running = match db.get_running_runs() {
        Ok(r) => r,
        Err(e) => {
            log::warn!("Failed to check for orphaned runs: {}", e);
            return;
        }
    };

    if running.is_empty() {
        return;
    }

    log::info!(
        "Found {} orphaned running run(s), recovering...",
        running.len()
    );

    let known_logs = [
        format!("{}/data/context-capture/capture_bg.log", chaos_labs_root),
        format!("{}/data/context-refresh/refresh.log", chaos_labs_root),
    ];

    for run in &running {
        let wf_name = run.workflow_name.as_deref().unwrap_or("unknown");

        let mut best_stdout = String::new();
        let mut best_log_path = String::new();
        for path in &known_logs {
            if let Ok(content) = std::fs::read_to_string(path) {
                if !content.is_empty() && content.len() > best_stdout.len() {
                    best_stdout = content;
                    best_log_path = path.clone();
                }
            }
        }

        let result_url = extract_result_url(&best_stdout).or_else(|| {
            if best_log_path.is_empty() {
                None
            } else {
                Some(format!("file://{}", best_log_path))
            }
        });

        let exit_code =
            read_exit_status_from_dir(&format!("{}/data/context-refresh", chaos_labs_root))
                .or_else(|| read_exit_status(chaos_labs_root))
                .unwrap_or_else(|| {
                    if best_stdout.is_empty() {
                        -1
                    } else if best_stdout.contains("Traceback") || best_stdout.contains("] ERROR:")
                    {
                        1
                    } else {
                        0
                    }
                });

        let status_label = match exit_code {
            0 => "success",
            -1 => "unknown (no output)",
            _ => "failed",
        };
        let _ = db.finish_run(&run.id, exit_code, &best_stdout, "", result_url.as_deref());
        log::info!(
            "Recovered orphaned run {} ({}) — finalized as {}",
            &run.id[..8],
            wf_name,
            status_label
        );
    }
}

pub fn start_scheduler_loop(
    scheduler: Arc<Mutex<WorkflowScheduler>>,
    db: Arc<Database>,
    chaos_labs_root: String,
    python_path: String,
    app_handle: tauri::AppHandle,
) {
    std::thread::spawn(move || {
        recover_orphaned_runs(&db, &chaos_labs_root);

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("Failed to create scheduler runtime");

        rt.block_on(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;

                if SHUTDOWN.load(Ordering::Relaxed) {
                    log::info!("Scheduler shutting down gracefully");
                    break;
                }

                // Phase 1 (locked): evaluate cron, find due workflows, read prefs
                let (due, notify_success, notify_failure, email_enabled) = {
                    let sched = scheduler.lock().unwrap();
                    let due = sched.find_due_workflows();
                    let ns = sched.notify_on_success.load(Ordering::Relaxed);
                    let nf = sched.notify_on_failure.load(Ordering::Relaxed);
                    let ef = sched.should_email_on_failure();
                    (due, ns, nf, ef)
                };
                // Lock released — all subsequent work is lock-free

                // Phase 2 (unlocked): execute workflows
                let mut results = vec![];
                for wf in &due {
                    match execute_workflow(
                        &db,
                        &chaos_labs_root,
                        &python_path,
                        &wf.id,
                        notify_success,
                        notify_failure,
                        email_enabled,
                    ) {
                        Ok(result) => results.push(result),
                        Err(e) => log::error!("Workflow {} failed: {}", wf.id, e),
                    }
                }

                for result in &results {
                    if result.should_notify {
                        send_notification(&app_handle, result);
                    }
                }

                if email_enabled {
                    for result in results.iter().filter(|r| !r.success && r.email_on_failure) {
                        send_failure_email(&db, &chaos_labs_root, result);
                    }
                }
            }
        });
    });
}

fn send_notification(app: &tauri::AppHandle, result: &RunResult) {
    use tauri_plugin_notification::NotificationExt;

    let (title, body) = if result.success {
        (
            format!("{} completed", result.workflow_name),
            "Workflow ran successfully.".to_string(),
        )
    } else {
        (
            format!("{} failed", result.workflow_name),
            "Check the dashboard for details.".to_string(),
        )
    };

    if let Err(e) = app
        .notification()
        .builder()
        .title(&title)
        .body(&body)
        .show()
    {
        log::warn!("Failed to send notification: {}", e);
    }
}

fn send_failure_email(db: &Database, chaos_labs_root: &str, result: &RunResult) {
    let config = match db.get_email_config() {
        Ok(c) if c.enabled && !c.alert_email.is_empty() => c,
        _ => return,
    };

    let run = match db.get_run(&result.run_id) {
        Ok(r) => r,
        Err(e) => {
            log::warn!("Failed to fetch run for email alert: {}", e);
            return;
        }
    };

    let run_context = serde_json::json!({
        "workflow_name": result.workflow_name,
        "script_path": result.script_path,
        "exit_code": run.exit_code,
        "stderr": run.stderr.as_deref().unwrap_or(""),
        "stdout": run.stdout.as_deref().unwrap_or(""),
        "started_at": run.started_at,
        "finished_at": run.finished_at.as_deref().unwrap_or(""),
        "run_id": run.id,
    });

    match crate::commands::run_email_script(chaos_labs_root, &config, Some(&run_context), "alert") {
        Ok(val) => {
            let success = val
                .get("success")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if success {
                log::info!("Failure email sent for workflow '{}'", result.workflow_name);
            } else {
                let error = val
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error");
                log::warn!(
                    "Failed to send failure email for '{}': {}",
                    result.workflow_name,
                    error
                );
            }
        }
        Err(e) => {
            log::warn!(
                "Email script invocation failed for '{}': {}",
                result.workflow_name,
                e
            );
        }
    }
}

// ---------------------------------------------------------------------------
// launchd plist management
// ---------------------------------------------------------------------------

pub fn install_launchd_plist(app_path: &str) -> Result<String, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    let plist_dir = format!("{}/Library/LaunchAgents", home);
    let plist_path = format!("{}/com.chaoslabs.scheduler.plist", plist_dir);

    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.chaoslabs.scheduler</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
</dict>
</plist>"#,
        app_path
    );

    std::fs::create_dir_all(&plist_dir).map_err(|e| e.to_string())?;
    std::fs::write(&plist_path, plist_content).map_err(|e| e.to_string())?;

    Command::new("launchctl")
        .args(["load", &plist_path])
        .output()
        .map_err(|e| e.to_string())?;

    Ok(plist_path)
}

pub fn uninstall_launchd_plist() -> Result<(), String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    let plist_path = format!(
        "{}/Library/LaunchAgents/com.chaoslabs.scheduler.plist",
        home
    );

    if std::path::Path::new(&plist_path).exists() {
        let _ = Command::new("launchctl")
            .args(["unload", &plist_path])
            .output();
        std::fs::remove_file(&plist_path).map_err(|e| e.to_string())?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Timelike, Utc};

    // --- normalize_cron ---

    #[test]
    fn normalize_5field_prepends_seconds() {
        assert_eq!(normalize_cron("0 5 * * *"), "0 0 5 * * *");
    }

    #[test]
    fn normalize_6field_passthrough() {
        assert_eq!(normalize_cron("0 0 5 * * *"), "0 0 5 * * *");
    }

    #[test]
    fn normalize_7field_passthrough() {
        assert_eq!(normalize_cron("0 0 5 * * * *"), "0 0 5 * * * *");
    }

    // --- parse_tz ---

    #[test]
    fn parse_tz_valid() {
        let tz = parse_tz("America/New_York");
        assert_eq!(tz, "America/New_York".parse::<Tz>().unwrap());
    }

    #[test]
    fn parse_tz_fallback_on_invalid() {
        let tz = parse_tz("Not/A_Timezone");
        assert_eq!(tz, chrono_tz::UTC);
    }

    #[test]
    fn parse_tz_utc_string() {
        let tz = parse_tz("UTC");
        assert_eq!(tz, chrono_tz::UTC);
    }

    // --- next_run_multi: validation ---

    #[test]
    fn next_run_single_cron_backward_compat() {
        let result = next_run_multi("0 5 * * *", chrono_tz::UTC);
        assert!(
            result.is_some(),
            "single cron should produce a next run time"
        );
    }

    #[test]
    fn next_run_multi_returns_earliest() {
        let a = next_run_multi("0 5 * * *", chrono_tz::UTC);
        let b = next_run_multi("0 17 * * *", chrono_tz::UTC);
        let combined = next_run_multi("0 5 * * *; 0 17 * * *", chrono_tz::UTC);
        assert!(combined.is_some());
        let expected = std::cmp::min(a.unwrap(), b.unwrap());
        assert_eq!(combined.unwrap(), expected);
    }

    #[test]
    fn next_run_multi_mixed_field_counts() {
        let result = next_run_multi("0 5 * * *; 0 0 13 * * * *", chrono_tz::UTC);
        assert!(
            result.is_some(),
            "mixed 5-field and 7-field should both parse"
        );
    }

    // --- next_run_multi: invalidation ---

    #[test]
    fn next_run_trailing_semicolon() {
        let result = next_run_multi("0 5 * * *; ", chrono_tz::UTC);
        assert!(result.is_some(), "trailing semicolon should be ignored");
    }

    #[test]
    fn next_run_double_semicolon() {
        let result = next_run_multi("0 5 * * *;; 0 13 * * *", chrono_tz::UTC);
        assert!(
            result.is_some(),
            "double semicolon should skip empty segment"
        );
    }

    #[test]
    fn next_run_no_whitespace_between() {
        let result = next_run_multi("0 5 * * *;0 13 * * *", chrono_tz::UTC);
        assert!(
            result.is_some(),
            "no whitespace after semicolon should work via trim"
        );
    }

    #[test]
    fn next_run_extra_whitespace() {
        let result = next_run_multi("  0 5 * * *  ;  0 13 * * *  ", chrono_tz::UTC);
        assert!(result.is_some(), "extra whitespace should be trimmed");
    }

    #[test]
    fn next_run_one_invalid_subexpr() {
        let result = next_run_multi("0 5 * * *; not_valid_cron; 0 13 * * *", chrono_tz::UTC);
        assert!(
            result.is_some(),
            "invalid sub-expression should be skipped, valid ones used"
        );
    }

    #[test]
    fn next_run_all_invalid() {
        let result = next_run_multi("invalid; also_invalid", chrono_tz::UTC);
        assert!(
            result.is_none(),
            "all invalid expressions should return None"
        );
    }

    #[test]
    fn next_run_empty_string() {
        let result = next_run_multi("", chrono_tz::UTC);
        assert!(result.is_none(), "empty string should return None");
    }

    #[test]
    fn next_run_just_semicolons() {
        let result = next_run_multi(";;;", chrono_tz::UTC);
        assert!(result.is_none(), "only semicolons should return None");
    }

    // --- timezone-aware next_run_multi ---

    #[test]
    fn tz_aware_different_from_utc() {
        // "9:00 AM Mon" in New York vs UTC should produce different UTC instants
        let cron = "0 0 9 * * Mon *";
        let utc_next = next_run_multi(cron, chrono_tz::UTC).unwrap();
        let ny_next = next_run_multi(cron, "America/New_York".parse::<Tz>().unwrap()).unwrap();
        assert_ne!(
            utc_next, ny_next,
            "same cron in UTC vs New York should produce different UTC instants"
        );
        // New York is behind UTC, so the UTC instant for NY 9:00 should be later
        assert!(
            ny_next > utc_next,
            "New York 9:00 AM should map to a later UTC instant than UTC 9:00 AM"
        );
    }

    #[test]
    fn tz_aware_dst_transition() {
        // In America/New_York, EST is UTC-5 and EDT is UTC-4.
        // A cron at 9:00 AM in that timezone should produce UTC 14:00 in winter
        // and UTC 13:00 in summer.
        let ny: Tz = "America/New_York".parse().unwrap();
        let cron = "0 0 9 * * * *"; // daily at 9am

        // Pick a known winter date (Jan 15, 2026 — EST, UTC-5)
        let winter_base = chrono::NaiveDate::from_ymd_opt(2026, 1, 15)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let winter_utc = Utc.from_utc_datetime(&winter_base);
        let winter_next = {
            let normalized = normalize_cron(cron);
            let schedule = cron::Schedule::from_str(&normalized).unwrap();
            let next = schedule
                .after(&winter_utc.with_timezone(&ny))
                .next()
                .unwrap();
            next.with_timezone(&Utc)
        };
        assert_eq!(winter_next.hour(), 14, "9 AM EST should be 14:00 UTC");

        // Pick a known summer date (Jul 15, 2026 — EDT, UTC-4)
        let summer_base = chrono::NaiveDate::from_ymd_opt(2026, 7, 15)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let summer_utc = Utc.from_utc_datetime(&summer_base);
        let summer_next = {
            let normalized = normalize_cron(cron);
            let schedule = cron::Schedule::from_str(&normalized).unwrap();
            let next = schedule
                .after(&summer_utc.with_timezone(&ny))
                .next()
                .unwrap();
            next.with_timezone(&Utc)
        };
        assert_eq!(summer_next.hour(), 13, "9 AM EDT should be 13:00 UTC");
    }

    // --- latest_scheduled_multi: validation ---

    #[test]
    fn latest_scheduled_single_cron() {
        let now = Utc::now();
        let since = now - chrono::Duration::days(2);
        let result = latest_scheduled_multi("0 0 * * *", chrono_tz::UTC, since, now);
        assert!(
            result.is_some(),
            "daily cron should have a recent scheduled time"
        );
    }

    #[test]
    fn latest_scheduled_multi_picks_latest() {
        let now = Utc::now();
        let since = now - chrono::Duration::days(2);
        let a = latest_scheduled_multi("0 5 * * *", chrono_tz::UTC, since, now);
        let b = latest_scheduled_multi("0 17 * * *", chrono_tz::UTC, since, now);
        let combined = latest_scheduled_multi("0 5 * * *; 0 17 * * *", chrono_tz::UTC, since, now);
        assert!(combined.is_some());
        let expected = std::cmp::max(a.unwrap(), b.unwrap());
        assert_eq!(combined.unwrap(), expected);
    }

    #[test]
    fn latest_scheduled_three_expressions() {
        let now = Utc::now();
        let since = now - chrono::Duration::days(2);
        let result = latest_scheduled_multi(
            "0 5 * * *; 0 12 * * *; 0 17 * * *",
            chrono_tz::UTC,
            since,
            now,
        );
        assert!(result.is_some());
    }

    #[test]
    fn latest_scheduled_tz_aware() {
        // "0 0 9 * * * *" in America/Chicago (CST=UTC-6, CDT=UTC-5)
        // evaluated with UTC since/until should find scheduled times correctly
        let chi: Tz = "America/Chicago".parse().unwrap();
        let now = Utc::now();
        let since = now - chrono::Duration::days(2);
        let result = latest_scheduled_multi("0 0 9 * * * *", chi, since, now);
        assert!(
            result.is_some(),
            "daily 9am Chicago should have a recent fire time within 2 days"
        );
        let t = result.unwrap();
        // 9 AM Chicago in winter is 15:00 UTC, in summer 14:00 UTC
        assert!(
            t.hour() == 14 || t.hour() == 15,
            "9 AM Chicago should map to UTC 14 or 15 depending on DST, got {}",
            t.hour()
        );
    }

    // --- latest_scheduled_multi: invalidation ---

    #[test]
    fn latest_scheduled_trailing_semicolon() {
        let now = Utc::now();
        let since = now - chrono::Duration::days(2);
        let result = latest_scheduled_multi("0 5 * * *; ", chrono_tz::UTC, since, now);
        assert!(result.is_some());
    }

    #[test]
    fn latest_scheduled_one_invalid() {
        let now = Utc::now();
        let since = now - chrono::Duration::days(2);
        let result =
            latest_scheduled_multi("0 5 * * *; garbage; 0 17 * * *", chrono_tz::UTC, since, now);
        assert!(
            result.is_some(),
            "should skip invalid and use valid sub-expressions"
        );
    }

    #[test]
    fn latest_scheduled_all_invalid() {
        let now = Utc::now();
        let since = now - chrono::Duration::days(2);
        let result = latest_scheduled_multi("nope; bad", chrono_tz::UTC, since, now);
        assert!(result.is_none());
    }

    #[test]
    fn latest_scheduled_empty() {
        let now = Utc::now();
        let since = now - chrono::Duration::days(2);
        let result = latest_scheduled_multi("", chrono_tz::UTC, since, now);
        assert!(result.is_none());
    }

    // --- workflow fires correctly after prior run ---

    #[test]
    fn multi_schedule_later_expr_still_triggers() {
        let today_5am = Utc::now()
            .date_naive()
            .and_hms_opt(5, 0, 0)
            .map(|dt| Utc.from_utc_datetime(&dt))
            .unwrap();
        let since = today_5am - chrono::Duration::days(2);

        let latest =
            latest_scheduled_multi("0 5 * * *; 0 17 * * *", chrono_tz::UTC, since, today_5am);
        assert!(latest.is_some());
        assert_eq!(latest.unwrap(), today_5am);

        let today_5pm = Utc::now()
            .date_naive()
            .and_hms_opt(17, 0, 0)
            .map(|dt| Utc.from_utc_datetime(&dt))
            .unwrap();
        let at_5pm_plus = today_5pm + chrono::Duration::minutes(1);
        let latest_after =
            latest_scheduled_multi("0 5 * * *; 0 17 * * *", chrono_tz::UTC, since, at_5pm_plus);
        assert!(latest_after.is_some());
        assert!(
            latest_after.unwrap() > today_5am,
            "17:00 schedule should be after 05:00 last_run_at, enabling the workflow to fire again"
        );
    }

    #[test]
    fn extract_log_start_offset_reads_launcher_metadata() {
        let stdout = "Gmail capture launched (PID 123)\nLog: /tmp/gmail.log\nLogStartOffset: 42\n";
        assert_eq!(extract_log_start_offset(stdout), Some(42));
    }

    #[test]
    fn read_log_slice_returns_current_run_only() {
        let path =
            std::env::temp_dir().join(format!("chaos-log-slice-{}.log", uuid::Uuid::new_v4()));
        let stale = "old Traceback\nSUMMARY_JSON:{\"title\":\"old\"}\n";
        let current = "current start\nSUMMARY_JSON:{\"title\":\"current\"}\nRESULT_URL: file:///tmp/current.md\n";
        std::fs::write(&path, format!("{}{}", stale, current)).unwrap();

        let slice = read_log_slice(path.to_str().unwrap(), Some(stale.len() as u64)).unwrap();
        let _ = std::fs::remove_file(path);

        assert_eq!(slice, current);
        assert!(!slice.contains("old Traceback"));
    }

    #[test]
    fn infer_exit_code_prefers_current_success_markers_over_error_text() {
        let stdout = "2026-05-08 current run\nTraceback from warning text\nCompleted phase: report\nContext capture completed\n";
        assert_eq!(infer_exit_code_from_current_output(stdout), 0);
    }

    #[test]
    fn run_scoped_status_ignores_other_run_ids() {
        let dir = std::env::temp_dir().join(format!("chaos-run-status-{}", uuid::Uuid::new_v4()));
        let status_dir = dir.join("run-status");
        std::fs::create_dir_all(&status_dir).unwrap();
        std::fs::write(
            status_dir.join("run-a.json"),
            r#"{"run_id":"run-b","exit_code":1}"#,
        )
        .unwrap();

        let parsed = read_run_scoped_exit_status_from_dir(dir.to_str().unwrap(), "run-a");
        let _ = std::fs::remove_dir_all(dir);

        assert!(parsed.is_none());
    }
}
