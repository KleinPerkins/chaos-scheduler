use crate::db::{Database, Run, RunAdmission, Workflow, WorkflowResourceSample};
use chrono::Utc;
use chrono_tz::Tz;
use cron::Schedule;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{hash_map::DefaultHasher, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::process::{Command, ExitStatus, Stdio};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::io::FromRawFd;
#[cfg(unix)]
use std::os::unix::process::{CommandExt, ExitStatusExt};

pub static SHUTDOWN: AtomicBool = AtomicBool::new(false);
static ALREADY_EXITING: AtomicBool = AtomicBool::new(false);
static SLA_NOTIFICATION_CACHE: OnceLock<Mutex<HashMap<String, i64>>> = OnceLock::new();

/// First quit path wins; subsequent `ExitRequested` events are ignored so the
/// off-main-thread grace timer is armed exactly once.
pub fn claim_exit_shutdown() -> bool {
    !ALREADY_EXITING.swap(true, Ordering::SeqCst)
}

/// Fixed grace before `app.exit(0)` on the off-main shutdown thread. Covers the
/// in-flight child `SIGTERM`->`SIGKILL` window plus a small margin.
pub fn process_exit_grace() -> Duration {
    PROCESS_SHUTDOWN_GRACE + Duration::from_secs(2)
}

/// Signal all workers/poll/retry paths to stop promptly.
pub fn initiate_shutdown() {
    SHUTDOWN.store(true, Ordering::Relaxed);
}

// Test-only accounting of the total *requested* sleep duration on the
// current thread. Synchronous operator paths (e.g. `CursorAgentOperator`'s
// poll loop) call `sleep_interruptible` directly on the calling thread, so a
// thread-local lets a test assert on how many milliseconds of sleep a code
// path *asked for* — exactly and independently of wall-clock jitter — where
// a `cargo test`-parallel-safe global counter could not. Compiled only under
// `cfg(test)`; a no-op in release/dev builds.
#[cfg(test)]
thread_local! {
    static ACCOUNTED_SLEEP_MS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Reset the current thread's sleep accounting to zero and return what it was.
/// Call at the start of a test to zero it, and again at the end to read the
/// total requested since the reset.
#[cfg(test)]
pub(crate) fn take_accounted_sleep_ms() -> u64 {
    ACCOUNTED_SLEEP_MS.with(|c| c.replace(0))
}

/// Sleep that returns early when [`SHUTDOWN`] is set (poll/retry backoff paths),
/// so a fixed shutdown grace is actually sufficient to stop in-flight workers.
pub fn sleep_interruptible(duration: Duration) {
    #[cfg(test)]
    ACCOUNTED_SLEEP_MS.with(|c| {
        c.set(
            c.get()
                .saturating_add(duration.as_millis().min(u64::MAX as u128) as u64),
        )
    });
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        if SHUTDOWN.load(Ordering::Relaxed) {
            return;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        std::thread::sleep(remaining.min(Duration::from_millis(50)));
    }
}

#[cfg(test)]
static SHUTDOWN_TEST_LOCK: Mutex<()> = Mutex::new(());

/// Serialize + reset the process-global `SHUTDOWN`/`ALREADY_EXITING` flags for any
/// test that reads or mutates them. `cargo test` runs tests in-process and in
/// parallel, so a test that flips `SHUTDOWN` true would otherwise race a command
/// that expects it false. Hold the returned guard for the whole test body.
#[cfg(test)]
pub(crate) fn lock_shutdown_test_state() -> std::sync::MutexGuard<'static, ()> {
    let guard = SHUTDOWN_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    SHUTDOWN.store(false, Ordering::Relaxed);
    ALREADY_EXITING.store(false, Ordering::Relaxed);
    guard
}

pub const SCHEDULER_BUNDLE_ID: &str = crate::branding::BUNDLE_ID;
pub const CANONICAL_EXECUTABLE_PATH: &str = crate::branding::CANONICAL_EXECUTABLE_PATH;

const RESOURCE_SAMPLE_INTERVAL: Duration = Duration::from_secs(5);
const DEFAULT_WORKER_COUNT: usize = 2;
const WORKER_QUEUE_MULTIPLIER: usize = 2;
const DEFAULT_RUN_TIMEOUT_SECONDS: u64 = 15 * 60;
const PROCESS_SHUTDOWN_GRACE: Duration = Duration::from_secs(3);
const OUTPUT_CAPTURE_LIMIT_BYTES: usize = 1024 * 1024;
const TASK_EVENT_CAPTURE_LIMIT_BYTES: usize = 256 * 1024;
const BACKGROUND_MONITOR_MAX_POLLS: usize = 8_640; // 24h at 10s/poll.
const COMPLETION_CHAIN_MAX_DEPTH: usize = 16;

#[derive(Clone)]
struct ResourceSampleMetadata {
    db: Arc<Database>,
    run_id: String,
    workflow_id: String,
    queue_name: Option<String>,
    environment: String,
}

struct ResourceSamplerHandle {
    stop_tx: mpsc::Sender<()>,
    join: std::thread::JoinHandle<()>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PidIdentity {
    pid: u32,
    start_time_ticks: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompletionChain {
    visited_workflow_ids: Vec<String>,
}

#[derive(Debug, Clone)]
struct ProcessStat {
    pid: i64,
    ppid: i64,
    cpu_percent: f64,
    rss_kb: i64,
    vms_kb: i64,
}

pub struct WorkflowScheduler {
    db: Arc<Database>,
    notify_on_failure: AtomicBool,
    notify_on_success: AtomicBool,
    email_on_failure: AtomicBool,
}

impl WorkflowScheduler {
    pub fn new(db: Arc<Database>) -> Self {
        let email_enabled = db.get_email_config().map(|c| c.enabled).unwrap_or(false);
        let (notify_on_failure, notify_on_success) =
            db.get_notification_prefs().unwrap_or((true, false));
        Self {
            db,
            notify_on_failure: AtomicBool::new(notify_on_failure),
            notify_on_success: AtomicBool::new(notify_on_success),
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

    pub fn find_due_workflows(&self, chaos_labs_root: &str) -> Vec<DueWorkflow> {
        match self.db.validate_queue_cap_lattice() {
            Ok(errors) if errors.is_empty() => {}
            Ok(errors) => {
                log::error!(
                    "Scheduler queue cap validation failed; skipping admission: {}",
                    errors.join("; ")
                );
                return vec![];
            }
            Err(e) => {
                log::error!(
                    "Scheduler queue cap validation failed; skipping admission: {}",
                    e
                );
                return vec![];
            }
        }

        let workflows = match self.db.list_workflows() {
            Ok(w) => w,
            Err(e) => {
                log::error!("Failed to list workflows: {}", e);
                return vec![];
            }
        };

        let now = Utc::now();
        let mut due = vec![];
        let mut queued_workflow_ids = HashSet::new();

        for row in self.db.list_queued_runs(500).unwrap_or_default() {
            if row.status != "queued" {
                continue;
            }
            let Ok(workflow) = self.db.get_workflow(&row.workflow_id) else {
                continue;
            };
            if !workflow.enabled {
                continue;
            }
            let queue_config =
                parse_queue_config(workflow.queue_config.as_deref(), &workflow.environment);
            match dependency_decision_for_db(&self.db, &workflow, &queue_config) {
                DependencyDecision::Ready
                    if self.has_queue_capacity(&queue_config).unwrap_or(false) =>
                {
                    queued_workflow_ids.insert(workflow.id.clone());
                    let candidate = DueWorkflow {
                        id: workflow.id.clone(),
                        trigger_kind: row.trigger_kind.clone(),
                        trigger_payload: row.trigger_payload.clone(),
                        queue_name: row.queue_name.clone(),
                        priority: row.priority,
                        queued_run_id: Some(row.id.clone()),
                        upstream_run_id: row.upstream_run_id.clone(),
                        input_json: row.input_json.clone(),
                        rerun_of_run_id: row.rerun_of_run_id.clone(),
                        suppress_completion_triggers: row.suppress_completion_triggers,
                    };
                    due.push(candidate);
                }
                DependencyDecision::CascadeSkip(reason) => {
                    let payload = serde_json::json!({
                        "reason": reason,
                        "original_trigger_kind": row.trigger_kind.as_deref(),
                        "original_trigger_payload": row.trigger_payload.as_deref(),
                    })
                    .to_string();
                    if let Ok(run) = self.db.create_terminal_run_with_context(
                        &workflow.id,
                        "cascade-skipped",
                        row.trigger_kind.as_deref(),
                        Some(&payload),
                        row.upstream_run_id.as_deref(),
                        row.input_json.as_deref(),
                        row.rerun_of_run_id.as_deref(),
                    ) {
                        let _ = self.db.mark_queued_run_terminal_by_id(
                            &row.id,
                            &run.id,
                            "cascade-skipped",
                        );
                    }
                }
                _ => {}
            }
        }

        for workflow in workflows {
            if queued_workflow_ids.contains(&workflow.id) {
                continue;
            }
            if !workflow.enabled {
                continue;
            }

            let queue_config =
                parse_queue_config(workflow.queue_config.as_deref(), &workflow.environment);
            let trigger_config = parse_trigger_config(workflow.trigger_config.as_deref());
            let has_explicit_triggers = !trigger_config.is_empty();
            let cron_triggers: Vec<String> = if has_explicit_triggers {
                trigger_config
                    .iter()
                    .filter(|trigger| trigger.get("kind").and_then(Value::as_str) == Some("cron"))
                    .filter_map(|trigger| {
                        trigger
                            .get("cron")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
                    .collect()
            } else {
                vec![workflow.cron_schedule.clone()]
            };

            if !cron_triggers.is_empty() {
                let cron_expr = cron_triggers.join("; ");
                let tz = parse_tz(&workflow.timezone);
                let since = now - chrono::Duration::days(8);
                if let Some(scheduled_time) = latest_scheduled_multi(&cron_expr, tz, since, now) {
                    let last_run = workflow.last_run_at.as_ref().and_then(|s| {
                        chrono::DateTime::parse_from_rfc3339(s)
                            .ok()
                            .map(|d| d.with_timezone(&Utc))
                    });

                    if last_run.map(|last| last < scheduled_time).unwrap_or(true) {
                        log::info!(
                            "Running due workflow: {} (scheduled for {})",
                            workflow.name,
                            scheduled_time
                        );
                        let candidate = DueWorkflow {
                            id: workflow.id.clone(),
                            trigger_kind: Some("cron".to_string()),
                            trigger_payload: Some(
                                serde_json::json!({
                                    "scheduled_time": scheduled_time.to_rfc3339()
                                })
                                .to_string(),
                            ),
                            queue_name: queue_config.queue.clone(),
                            priority: queue_config.priority,
                            queued_run_id: None,
                            upstream_run_id: None,
                            input_json: None,
                            rerun_of_run_id: None,
                            suppress_completion_triggers: false,
                        };
                        if self.admit_or_skip_due_workflow(&workflow, &queue_config, &candidate) {
                            let now_str = now.to_rfc3339();
                            let _ = self.db.set_last_run_at(&workflow.id, &now_str);
                            due.push(candidate);
                        }
                        continue;
                    }
                }
            }

            for trigger in trigger_config.iter().filter(|trigger| {
                trigger.get("kind").and_then(Value::as_str) == Some("file_arrival")
            }) {
                if let Some(reason) =
                    self.evaluate_file_arrival_trigger(&workflow.id, trigger, chaos_labs_root)
                {
                    let candidate = DueWorkflow {
                        id: workflow.id.clone(),
                        trigger_kind: Some("file_arrival".to_string()),
                        trigger_payload: Some(reason.to_string()),
                        queue_name: queue_config.queue.clone(),
                        priority: queue_config.priority,
                        queued_run_id: None,
                        upstream_run_id: None,
                        input_json: None,
                        rerun_of_run_id: None,
                        suppress_completion_triggers: false,
                    };
                    if self.admit_or_skip_due_workflow(&workflow, &queue_config, &candidate) {
                        due.push(candidate);
                    }
                    break;
                }
            }

            for trigger in trigger_config.iter().filter(|trigger| {
                trigger.get("kind").and_then(Value::as_str) == Some("asset_update")
            }) {
                if let Some(reason) = self.evaluate_asset_update_trigger(&workflow.id, trigger) {
                    let candidate = DueWorkflow {
                        id: workflow.id.clone(),
                        trigger_kind: Some("asset_update".to_string()),
                        trigger_payload: Some(reason.to_string()),
                        queue_name: queue_config.queue.clone(),
                        priority: queue_config.priority,
                        queued_run_id: None,
                        upstream_run_id: None,
                        input_json: None,
                        rerun_of_run_id: None,
                        suppress_completion_triggers: false,
                    };
                    if self.admit_or_skip_due_workflow(&workflow, &queue_config, &candidate) {
                        due.push(candidate);
                    }
                    break;
                }
            }
        }

        due.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| a.queue_name.cmp(&b.queue_name))
                .then_with(|| a.id.cmp(&b.id))
        });
        due
    }

    fn admit_or_skip_due_workflow(
        &self,
        workflow: &crate::db::Workflow,
        queue_config: &QueueConfig,
        candidate: &DueWorkflow,
    ) -> bool {
        match self.dependency_decision(workflow, queue_config) {
            DependencyDecision::Ready => match self.has_queue_capacity(queue_config) {
                Ok(true) => true,
                Ok(false) => {
                    log::info!(
                        "Deferring workflow {}: queue {} is at capacity",
                        workflow.id,
                        queue_config.queue
                    );
                    let _ = self.db.upsert_queued_run_with_context(
                        &workflow.id,
                        &queue_config.queue,
                        queue_config.priority,
                        candidate.trigger_kind.as_deref(),
                        candidate.trigger_payload.as_deref(),
                        candidate.upstream_run_id.as_deref(),
                        candidate.input_json.as_deref(),
                        candidate.rerun_of_run_id.as_deref(),
                        candidate.suppress_completion_triggers,
                    );
                    false
                }
                Err(e) => {
                    log::warn!(
                        "Deferring workflow {}: failed to evaluate queue capacity: {}",
                        workflow.id,
                        e
                    );
                    false
                }
            },
            DependencyDecision::Waiting(reason) => {
                log::info!(
                    "Deferring workflow {} in queue {}: {}",
                    workflow.id,
                    queue_config.queue,
                    reason
                );
                let _ = self.db.upsert_queued_run_with_context(
                    &workflow.id,
                    &queue_config.queue,
                    queue_config.priority,
                    candidate.trigger_kind.as_deref(),
                    candidate.trigger_payload.as_deref(),
                    candidate.upstream_run_id.as_deref(),
                    candidate.input_json.as_deref(),
                    candidate.rerun_of_run_id.as_deref(),
                    candidate.suppress_completion_triggers,
                );
                false
            }
            DependencyDecision::CascadeSkip(reason) => {
                log::warn!("Cascade-skipping workflow {}: {}", workflow.id, reason);
                let payload = serde_json::json!({
                    "reason": reason,
                    "queue": queue_config.queue,
                    "original_trigger_kind": candidate.trigger_kind.as_deref(),
                    "original_trigger_payload": candidate.trigger_payload.as_deref(),
                })
                .to_string();
                let _ = self.db.create_terminal_run_with_context(
                    &workflow.id,
                    "cascade-skipped",
                    candidate.trigger_kind.as_deref(),
                    Some(&payload),
                    None,
                    None,
                    None,
                );
                false
            }
        }
    }

    fn dependency_decision(
        &self,
        workflow: &crate::db::Workflow,
        queue_config: &QueueConfig,
    ) -> DependencyDecision {
        dependency_decision_for_db(&self.db, workflow, queue_config)
    }

    fn has_queue_capacity(&self, queue_config: &QueueConfig) -> Result<bool, String> {
        has_runtime_capacity(&self.db, queue_config)
    }

    fn evaluate_asset_update_trigger(&self, workflow_id: &str, trigger: &Value) -> Option<Value> {
        let asset = trigger.get("asset")?;
        let asset_kind = asset.get("kind")?.as_str()?;
        let asset_namespace = asset
            .get("namespace")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty());
        let asset_partition = asset
            .get("partition")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty());
        let trigger_id = trigger
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| {
                format!(
                    "asset:{}:{}:{}",
                    asset_kind,
                    asset_namespace.unwrap_or(""),
                    asset_partition.unwrap_or("")
                )
            });
        let record = self
            .db
            .latest_asset_write_matching(asset_kind, asset_namespace, asset_partition, None)
            .ok()
            .flatten()?;
        let fingerprint = format!(
            "{}:{}:{}",
            record.run_id,
            record.task_id.clone().unwrap_or_default(),
            record.emitted_at
        );
        if record.workflow_id == workflow_id {
            let _ = self
                .db
                .set_trigger_state(workflow_id, &trigger_id, &fingerprint, false);
            return None;
        }
        let prior = self
            .db
            .get_trigger_fingerprint(workflow_id, &trigger_id)
            .ok()
            .flatten();
        let changed = prior.as_deref() != Some(fingerprint.as_str());
        if changed {
            Some(serde_json::json!({
                "trigger_id": trigger_id,
                "fingerprint": fingerprint,
                "asset_kind": record.asset_kind,
                "asset_namespace": record.asset_namespace,
                "asset_partition": record.asset_partition,
                "last_writer_run_id": record.run_id,
                "writer_workflow_id": record.workflow_id,
                "updated_at": record.emitted_at,
            }))
        } else {
            None
        }
    }

    fn evaluate_file_arrival_trigger(
        &self,
        workflow_id: &str,
        trigger: &Value,
        chaos_labs_root: &str,
    ) -> Option<Value> {
        let path = trigger.get("path")?.as_str()?;
        let mode = trigger
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("mtime_changed");
        let trigger_id = trigger
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("file:{}:{}", path, mode));
        let (matched_path, fingerprint) = fingerprint_file_sensor(path, mode, chaos_labs_root)?;
        let prior = self
            .db
            .get_trigger_fingerprint(workflow_id, &trigger_id)
            .ok()
            .flatten();
        let changed = prior.as_deref() != Some(fingerprint.as_str());
        if changed {
            Some(serde_json::json!({
                "trigger_id": trigger_id,
                "path": matched_path,
                "mode": mode,
                "fingerprint": fingerprint,
            }))
        } else {
            None
        }
    }
}

fn latest_status_for_db(db: &Arc<Database>, workflow_id: &str) -> Option<String> {
    match db.latest_run_status(workflow_id) {
        Ok(status) => status,
        Err(e) => {
            log::warn!(
                "Failed to read latest run status for {}: {}",
                workflow_id,
                e
            );
            None
        }
    }
}

fn dependency_decision_for_db(
    db: &Arc<Database>,
    workflow: &crate::db::Workflow,
    queue_config: &QueueConfig,
) -> DependencyDecision {
    for upstream in &queue_config.depends_on {
        match latest_status_for_db(db, upstream) {
            Some(status) if matches!(status.as_str(), "success" | "succeeded") => {}
            Some(status) if is_failure_terminal(&status) => {
                return DependencyDecision::CascadeSkip(format!(
                    "depends_on upstream {} ended as {}",
                    upstream, status
                ));
            }
            Some(status) => {
                return DependencyDecision::Waiting(format!(
                    "depends_on upstream {} is {}",
                    upstream, status
                ));
            }
            None => {
                return DependencyDecision::Waiting(format!(
                    "depends_on upstream {} has no runs",
                    upstream
                ));
            }
        }
    }
    for upstream in &queue_config.waits_for {
        match latest_status_for_db(db, upstream) {
            Some(status) if is_terminal_status(&status) => {}
            Some(status) => {
                return DependencyDecision::Waiting(format!(
                    "waits_for upstream {} is {}",
                    upstream, status
                ));
            }
            None => {
                return DependencyDecision::Waiting(format!(
                    "waits_for upstream {} has no runs",
                    upstream
                ));
            }
        }
    }
    if queue_config.queue.trim().is_empty() {
        return DependencyDecision::Waiting(format!(
            "workflow {} has empty queue assignment",
            workflow.id
        ));
    }
    DependencyDecision::Ready
}

/// Derive the `(trigger_id, fingerprint)` pair that an admission should record
/// as fired for file-arrival / asset-update triggers, or `None` when the
/// trigger carries no dedupe fingerprint. Persistence happens atomically inside
/// [`Database::admit_run_with_context`].
fn trigger_state_for_admission(
    trigger_kind: Option<&str>,
    trigger_payload: Option<&str>,
) -> Option<(String, String)> {
    let kind = trigger_kind?;
    if kind != "file_arrival" && kind != "asset_update" {
        return None;
    }
    let payload = trigger_payload?;
    let value = serde_json::from_str::<Value>(payload).ok()?;
    let trigger_id = value.get("trigger_id").and_then(Value::as_str)?;
    let fingerprint = value.get("fingerprint").and_then(Value::as_str)?;
    Some((trigger_id.to_string(), fingerprint.to_string()))
}

#[derive(Clone)]
pub struct DueWorkflow {
    pub id: String,
    pub trigger_kind: Option<String>,
    pub trigger_payload: Option<String>,
    pub queue_name: String,
    pub priority: i64,
    pub queued_run_id: Option<String>,
    pub upstream_run_id: Option<String>,
    pub input_json: Option<String>,
    pub rerun_of_run_id: Option<String>,
    /// Carried from the queued row (v16): when true, draining this run must NOT
    /// fire on-completion trigger chains (D05 fix-rerun gate, M5). Always false
    /// for cron / file-arrival / asset-update admissions (they have no queued
    /// row to carry the intent).
    pub suppress_completion_triggers: bool,
}

fn due_workflow_pending_key(workflow: &DueWorkflow) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        workflow.id,
        workflow.queued_run_id.as_deref().unwrap_or(""),
        workflow.trigger_kind.as_deref().unwrap_or(""),
        workflow.trigger_payload.as_deref().unwrap_or(""),
        workflow.upstream_run_id.as_deref().unwrap_or("")
    )
}

#[derive(Debug, Clone, Serialize)]
pub struct DispatchOutcome {
    pub workflow_id: String,
    pub status: String,
    pub run_id: Option<String>,
    pub queued_run_id: Option<String>,
    pub queue_name: String,
    pub trigger_kind: Option<String>,
    pub trigger_payload: Option<String>,
    pub reason: Option<String>,
}

pub struct NonCronDispatchOptions<'a> {
    pub notify_on_success: bool,
    pub notify_on_failure: bool,
    pub email_on_failure_enabled: bool,
    pub trigger_kind: &'a str,
    pub trigger_payload: Option<&'a str>,
    pub upstream_run_id: Option<&'a str>,
    pub input_json: Option<&'a str>,
    pub rerun_of_run_id: Option<&'a str>,
    pub suppress_completion_triggers: bool,
    pub dedupe: bool,
    pub app_handle: Option<tauri::AppHandle>,
}

#[derive(Default)]
struct ChildDispatchSummary {
    failure_count: usize,
    notes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct QueueConfig {
    #[serde(default)]
    depends_on: Vec<String>,
    #[serde(default)]
    waits_for: Vec<String>,
    #[serde(default)]
    excludes: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    queue: String,
    #[serde(default)]
    priority: i64,
    #[serde(skip)]
    environment: String,
}

enum DependencyDecision {
    Ready,
    Waiting(String),
    CascadeSkip(String),
}

fn parse_queue_config(queue_config: Option<&str>, environment: &str) -> QueueConfig {
    let default_queue = format!("{}-default", environment);
    let mut parsed = queue_config
        .filter(|s| !s.trim().is_empty())
        .and_then(|raw| serde_json::from_str::<QueueConfig>(raw).ok())
        .unwrap_or(QueueConfig {
            depends_on: vec![],
            waits_for: vec![],
            excludes: vec![],
            tags: vec![],
            queue: default_queue.clone(),
            priority: 0,
            environment: environment.to_string(),
        });
    if parsed.queue.trim().is_empty() {
        parsed.queue = default_queue;
    }
    parsed.environment = environment.to_string();
    parsed
}

fn mutex_keys(workflow_id: &str, queue_config: &QueueConfig) -> Vec<String> {
    let mut keys = Vec::new();
    for other in &queue_config.excludes {
        let mut pair = [workflow_id.to_string(), other.to_string()];
        pair.sort();
        keys.push(format!("exclude:{}::{}", pair[0], pair[1]));
    }
    // Tag concurrency is enforced by tag_cap counts rather than mutex keys so caps above 1 work.
    keys.sort();
    keys.dedup();
    keys
}

fn has_runtime_capacity(db: &Arc<Database>, queue_config: &QueueConfig) -> Result<bool, String> {
    let capacity = db
        .queue_capacity(&queue_config.queue, &queue_config.environment)
        .map_err(|e| e.to_string())?;
    let running = running_count_for_queue(db, &queue_config.queue, &queue_config.environment)?;
    if running >= capacity {
        return Ok(false);
    }
    let global_cap = db.global_parallelism_cap().map_err(|e| e.to_string())?;
    if db.get_running_runs().map_err(|e| e.to_string())?.len() as i64 >= global_cap {
        return Ok(false);
    }
    if let Some(tag_cap) = db
        .queue_tag_cap(&queue_config.queue, &queue_config.environment)
        .map_err(|e| e.to_string())?
    {
        for tag in &queue_config.tags {
            if running_count_for_tag(db, &queue_config.queue, &queue_config.environment, tag)?
                >= tag_cap
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn running_count_for_tag(
    db: &Arc<Database>,
    queue_name: &str,
    environment: &str,
    tag: &str,
) -> Result<i64, String> {
    let running = db.get_running_runs().map_err(|e| e.to_string())?;
    let mut count = 0;
    for run in running {
        let workflow = db
            .get_workflow(&run.workflow_id)
            .map_err(|e| e.to_string())?;
        let config = parse_queue_config(workflow.queue_config.as_deref(), &workflow.environment);
        if config.queue == queue_name
            && config.environment == environment
            && config.tags.iter().any(|candidate| candidate == tag)
        {
            count += 1;
        }
    }
    Ok(count)
}

fn running_count_for_queue(
    db: &Arc<Database>,
    queue_name: &str,
    environment: &str,
) -> Result<i64, String> {
    let running = db.get_running_runs().map_err(|e| e.to_string())?;
    let mut count = 0;
    for run in running {
        let workflow = db
            .get_workflow(&run.workflow_id)
            .map_err(|e| e.to_string())?;
        let config = parse_queue_config(workflow.queue_config.as_deref(), &workflow.environment);
        if config.queue == queue_name && config.environment == environment {
            count += 1;
        }
    }
    Ok(count)
}

fn is_terminal_status(status: &str) -> bool {
    crate::db::run_status::is_terminal(status)
}

fn is_failure_terminal(status: &str) -> bool {
    crate::db::run_status::ended_not_ok(status)
}

impl CompletionChain {
    fn root(workflow_id: &str) -> Self {
        Self {
            visited_workflow_ids: vec![workflow_id.to_string()],
        }
    }

    fn from_trigger_payload(payload: Option<&str>, current_workflow_id: &str) -> Self {
        let mut visited = payload
            .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
            .and_then(|value| value.get("_chain").cloned())
            .and_then(|chain| {
                chain
                    .get("visited_workflow_ids")
                    .and_then(Value::as_array)
                    .map(|ids| {
                        ids.iter()
                            .filter_map(Value::as_str)
                            .filter(|id| !id.trim().is_empty())
                            .take(COMPLETION_CHAIN_MAX_DEPTH + 1)
                            .map(str::to_string)
                            .collect::<Vec<_>>()
                    })
            })
            .filter(|ids| !ids.is_empty())
            .unwrap_or_else(|| vec![current_workflow_id.to_string()]);

        if !visited.iter().any(|id| id == current_workflow_id) {
            visited.push(current_workflow_id.to_string());
        }
        Self {
            visited_workflow_ids: visited,
        }
    }

    fn depth(&self) -> usize {
        self.visited_workflow_ids.len().saturating_sub(1)
    }

    fn try_advance(&self, workflow_id: &str) -> Result<Self, String> {
        if self.visited_workflow_ids.iter().any(|id| id == workflow_id) {
            return Err(format!(
                "completion chain cycle detected at workflow {workflow_id}"
            ));
        }
        if self.depth() >= COMPLETION_CHAIN_MAX_DEPTH {
            return Err(format!(
                "completion chain depth {} reached max {}",
                self.depth(),
                COMPLETION_CHAIN_MAX_DEPTH
            ));
        }
        let mut visited = self.visited_workflow_ids.clone();
        visited.push(workflow_id.to_string());
        Ok(Self {
            visited_workflow_ids: visited,
        })
    }

    fn as_json(&self) -> Value {
        serde_json::json!({
            "depth": self.depth(),
            "max_depth": COMPLETION_CHAIN_MAX_DEPTH,
            "visited_workflow_ids": self.visited_workflow_ids,
        })
    }
}

fn payload_with_chain(mut payload: Value, chain: &CompletionChain) -> String {
    if let Some(object) = payload.as_object_mut() {
        object.insert("_chain".to_string(), chain.as_json());
    }
    payload.to_string()
}

fn completion_trigger_payload(
    upstream_workflow_id: &str,
    upstream_run_id: &str,
    status: &str,
    chain: &CompletionChain,
) -> String {
    payload_with_chain(
        serde_json::json!({
            "upstream_workflow_id": upstream_workflow_id,
            "upstream_run_id": upstream_run_id,
            "status": status,
        }),
        chain,
    )
}

fn run_workflow_action_payload(
    upstream_workflow_id: &str,
    upstream_run_id: &str,
    chain: &CompletionChain,
) -> String {
    payload_with_chain(
        serde_json::json!({
            "upstream_workflow_id": upstream_workflow_id,
            "upstream_run_id": upstream_run_id,
            "source": "run_workflow_action",
        }),
        chain,
    )
}

fn completion_chain_for_run(
    db: &Arc<Database>,
    run_id: &str,
    workflow_id: &str,
) -> CompletionChain {
    db.get_run(run_id)
        .ok()
        .map(|run| {
            CompletionChain::from_trigger_payload(run.trigger_payload.as_deref(), workflow_id)
        })
        .unwrap_or_else(|| CompletionChain::root(workflow_id))
}

/// Compute the next run time for a cron expression in the given timezone.
/// Pure function — no scheduler state needed.
pub fn get_next_run_time(cron_expr: &str, timezone: &str) -> Option<String> {
    let tz = parse_tz(timezone);
    next_run_multi(cron_expr, tz).map(|t| t.to_rfc3339())
}

fn parse_trigger_config(trigger_config: Option<&str>) -> Vec<Value> {
    let Some(raw) = trigger_config.filter(|s| !s.trim().is_empty()) else {
        return vec![];
    };
    let Ok(parsed) = serde_json::from_str::<Value>(raw) else {
        log::warn!("Ignoring invalid trigger_config JSON");
        return vec![];
    };
    if let Some(triggers) = parsed.as_array() {
        return triggers.clone();
    }
    parsed
        .get("triggers")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn fingerprint_file_sensor(
    path: &str,
    mode: &str,
    chaos_labs_root: &str,
) -> Option<(String, String)> {
    let resolved = resolve_sensor_path(path, chaos_labs_root)?;
    let meta = std::fs::metadata(&resolved).ok()?;
    let fingerprint = match mode {
        "size_changed" => format!("size:{}", meta.len()),
        "content_hash_changed" => {
            let content = std::fs::read(&resolved).ok()?;
            let mut hasher = DefaultHasher::new();
            content.hash(&mut hasher);
            format!("hash:{:x}", hasher.finish())
        }
        _ => {
            let modified = meta.modified().ok()?;
            let nanos = modified
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_nanos())
                .unwrap_or_default();
            format!("mtime:{}:{}", nanos, meta.len())
        }
    };
    Some((resolved, fingerprint))
}

fn resolve_sensor_path(path: &str, chaos_labs_root: &str) -> Option<String> {
    let expanded = if std::path::Path::new(path).is_absolute() {
        path.to_string()
    } else {
        format!("{}/{}", chaos_labs_root, path)
    };
    if !expanded.contains('*') {
        return std::path::Path::new(&expanded).exists().then_some(expanded);
    }

    let path_obj = std::path::Path::new(&expanded);
    let parent = path_obj.parent()?;
    let pattern = path_obj.file_name()?.to_string_lossy().to_string();
    let mut matches: Vec<String> = std::fs::read_dir(parent)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            simple_wildcard_match(&pattern, &name)
                .then(|| entry.path().to_string_lossy().to_string())
        })
        .collect();
    matches.sort();
    matches.pop()
}

fn simple_wildcard_match(pattern: &str, candidate: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    let Some((prefix, suffix)) = pattern.split_once('*') else {
        return pattern == candidate;
    };
    candidate.starts_with(prefix) && candidate.ends_with(suffix)
}

/// Execute a workflow subprocess. Does not require the scheduler mutex.
pub fn dispatch_non_cron_workflow(
    db: &Arc<Database>,
    chaos_labs_root: &str,
    python_path: &str,
    workflow_id: &str,
    options: NonCronDispatchOptions<'_>,
) -> Result<DispatchOutcome, String> {
    let workflow = db
        .get_workflow(workflow_id)
        .map_err(|e| format!("Failed to get workflow: {}", e))?;
    let queue_config = parse_queue_config(workflow.queue_config.as_deref(), &workflow.environment);
    let trigger_kind = Some(options.trigger_kind);

    if !workflow.enabled {
        return Err(format!("Workflow {} is disabled", workflow.id));
    }

    if options.dedupe {
        if let Some(run) = db
            .find_run_by_dispatch_context(
                &workflow.id,
                trigger_kind,
                options.trigger_payload,
                options.input_json,
                options.rerun_of_run_id,
            )
            .map_err(|e| format!("Failed to check existing runs: {}", e))?
        {
            return Ok(DispatchOutcome {
                workflow_id: workflow.id,
                status: "duplicate".to_string(),
                run_id: Some(run.id),
                queued_run_id: None,
                queue_name: queue_config.queue,
                trigger_kind: trigger_kind.map(str::to_string),
                trigger_payload: options.trigger_payload.map(str::to_string),
                reason: Some("matching run already exists".to_string()),
            });
        }
        if let Some(queued) = db
            .find_queued_run_by_dispatch_context(
                &workflow.id,
                trigger_kind,
                options.trigger_payload,
                options.input_json,
                options.rerun_of_run_id,
            )
            .map_err(|e| format!("Failed to check existing queued runs: {}", e))?
        {
            return Ok(DispatchOutcome {
                workflow_id: workflow.id,
                status: "queued".to_string(),
                run_id: queued.run_id,
                queued_run_id: Some(queued.id),
                queue_name: queue_config.queue,
                trigger_kind: trigger_kind.map(str::to_string),
                trigger_payload: options.trigger_payload.map(str::to_string),
                reason: Some("matching queued run already exists".to_string()),
            });
        }
    }

    let queue_due_to = |reason: String| -> Result<DispatchOutcome, String> {
        let queued_id = db
            .upsert_queued_run_with_context(
                &workflow.id,
                &queue_config.queue,
                queue_config.priority,
                trigger_kind,
                options.trigger_payload,
                options.upstream_run_id,
                options.input_json,
                options.rerun_of_run_id,
                options.suppress_completion_triggers,
            )
            .map_err(|e| format!("Failed to queue workflow: {}", e))?;
        let _ = db.insert_queue_event(
            &queue_config.queue,
            &queue_config.environment,
            Some(&workflow.id),
            None,
            "deferred",
            Some(&reason),
            Some(&serde_json::json!({
                "trigger_kind": options.trigger_kind,
                "trigger_payload": options.trigger_payload,
            })),
        );
        Ok(DispatchOutcome {
            workflow_id: workflow.id.clone(),
            status: "queued".to_string(),
            run_id: None,
            queued_run_id: Some(queued_id),
            queue_name: queue_config.queue.clone(),
            trigger_kind: trigger_kind.map(str::to_string),
            trigger_payload: options.trigger_payload.map(str::to_string),
            reason: Some(reason),
        })
    };

    match dependency_decision_for_db(db, &workflow, &queue_config) {
        DependencyDecision::Ready => {}
        DependencyDecision::Waiting(reason) => return queue_due_to(reason),
        DependencyDecision::CascadeSkip(reason) => {
            let payload = serde_json::json!({
                "reason": reason,
                "original_trigger_kind": options.trigger_kind,
                "original_trigger_payload": options.trigger_payload,
            })
            .to_string();
            let run = db
                .create_terminal_run_with_context(
                    &workflow.id,
                    "cascade-skipped",
                    trigger_kind,
                    Some(&payload),
                    options.upstream_run_id,
                    options.input_json,
                    options.rerun_of_run_id,
                )
                .map_err(|e| format!("Failed to create cascade-skip run: {}", e))?;
            return Ok(DispatchOutcome {
                workflow_id: workflow.id,
                status: "skipped".to_string(),
                run_id: Some(run.id),
                queued_run_id: None,
                queue_name: queue_config.queue,
                trigger_kind: trigger_kind.map(str::to_string),
                trigger_payload: Some(payload),
                reason: Some("dependency cascade skip".to_string()),
            });
        }
    }

    if !has_runtime_capacity(db, &queue_config)? {
        return queue_due_to(format!(
            "queue {} is at capacity or constrained by global/tag caps",
            queue_config.queue
        ));
    }

    let result = execute_workflow_with_context(
        db,
        chaos_labs_root,
        python_path,
        &workflow.id,
        options.notify_on_success,
        options.notify_on_failure,
        options.email_on_failure_enabled,
        trigger_kind,
        options.trigger_payload,
        options.upstream_run_id,
        options.input_json,
        options.rerun_of_run_id,
        None,
        options.suppress_completion_triggers,
        options.app_handle.clone(),
    )?;
    if result.completed && !options.suppress_completion_triggers {
        trigger_on_completion(
            db,
            chaos_labs_root,
            python_path,
            &workflow.id,
            &result.run_id,
            result.success,
            options.notify_on_success,
            options.notify_on_failure,
            options.email_on_failure_enabled,
        );
    }
    Ok(DispatchOutcome {
        workflow_id: workflow.id,
        status: "admitted".to_string(),
        run_id: Some(result.run_id),
        queued_run_id: None,
        queue_name: queue_config.queue,
        trigger_kind: trigger_kind.map(str::to_string),
        trigger_payload: options.trigger_payload.map(str::to_string),
        reason: None,
    })
}

fn dispatch_child_workflow_requests(
    db: &Arc<Database>,
    chaos_labs_root: &str,
    python_path: &str,
    parent_run_id: &str,
    parent_workflow_id: &str,
    raw_events: &str,
    app_handle: Option<tauri::AppHandle>,
) -> ChildDispatchSummary {
    let mut summary = ChildDispatchSummary::default();
    for event_line in valid_task_event_lines(raw_events) {
        let Ok(event) = serde_json::from_str::<Value>(&event_line) else {
            continue;
        };
        if event.get("status").and_then(Value::as_str) != Some("subworkflow_requested") {
            continue;
        }
        let task_id = event.get("task_id").and_then(Value::as_str);
        let details = event.get("details").cloned().unwrap_or(Value::Null);
        let Some(child_workflow_id) = details.get("workflow_id").and_then(Value::as_str) else {
            summary
                .notes
                .push("subworkflow request missing workflow_id".to_string());
            continue;
        };
        let wait = details.get("wait").and_then(Value::as_bool).unwrap_or(true);
        let inputs = details
            .get("inputs")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        let correlation_id = details.get("correlation_id").and_then(Value::as_str);
        let trigger_payload = serde_json::json!({
            "parent_run_id": parent_run_id,
            "parent_workflow_id": parent_workflow_id,
            "parent_task_id": task_id,
            "wait": wait,
            "correlation_id": correlation_id,
        })
        .to_string();
        let input_json = serde_json::to_string(&inputs).unwrap_or_else(|_| "{}".to_string());

        if child_workflow_id == parent_workflow_id {
            let reason = "child workflow loop rejected: child workflow matches parent";
            let _ = db.insert_run_relationship(
                parent_run_id,
                None,
                None,
                child_workflow_id,
                "child_workflow",
                task_id,
                wait,
                "rejected",
                Some(reason),
                Some(&details),
            );
            if wait {
                summary.failure_count += 1;
            }
            summary.notes.push(reason.to_string());
            continue;
        }

        match dispatch_non_cron_workflow(
            db,
            chaos_labs_root,
            python_path,
            child_workflow_id,
            NonCronDispatchOptions {
                notify_on_success: false,
                notify_on_failure: true,
                email_on_failure_enabled: false,
                trigger_kind: "child_workflow",
                trigger_payload: Some(&trigger_payload),
                upstream_run_id: Some(parent_run_id),
                input_json: Some(&input_json),
                rerun_of_run_id: None,
                suppress_completion_triggers: false,
                dedupe: false,
                app_handle: app_handle.clone(),
            },
        ) {
            Ok(outcome) => {
                let status = outcome.status.clone();
                let _ = db.insert_run_relationship(
                    parent_run_id,
                    outcome.run_id.as_deref(),
                    outcome.queued_run_id.as_deref(),
                    child_workflow_id,
                    "child_workflow",
                    task_id,
                    wait,
                    &status,
                    outcome.reason.as_deref(),
                    Some(&details),
                );
                if wait {
                    if status == "skipped" {
                        summary.failure_count += 1;
                    }
                    if let Some(child_run_id) = outcome.run_id.as_deref() {
                        if let Ok(child_run) = db.get_run(child_run_id) {
                            if is_failure_terminal(&child_run.status) {
                                summary.failure_count += 1;
                                summary.notes.push(format!(
                                    "child workflow {} finished as {}",
                                    child_workflow_id, child_run.status
                                ));
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let _ = db.insert_run_relationship(
                    parent_run_id,
                    None,
                    None,
                    child_workflow_id,
                    "child_workflow",
                    task_id,
                    wait,
                    "failed",
                    Some(&e),
                    Some(&details),
                );
                if wait {
                    summary.failure_count += 1;
                }
                summary.notes.push(e);
            }
        }
    }
    summary
}

/// Scheduler-owned environment exported to step-flow child processes. Steps do
/// NOT emit FD-3 task events (the scheduler records their run_tasks/run_attempts
/// from the returned results), but they still receive the run context.
fn step_flow_base_env(
    db: &Arc<Database>,
    run_id: &str,
    workflow: &Workflow,
) -> Vec<(String, String)> {
    vec![
        ("CHAOS_SCHEDULER_RUN_ID".to_string(), run_id.to_string()),
        (
            "CHAOS_SCHEDULER_WORKFLOW_ID".to_string(),
            workflow.id.clone(),
        ),
        (
            "CHAOS_SCHEDULER_ENVIRONMENT".to_string(),
            workflow.environment.clone(),
        ),
        ("CHAOS_SCHEDULER_DB_PATH".to_string(), db.path().to_string()),
        // Legacy dual-emit for one transition minor version.
        (
            "CHAOS_LABS_SCHEDULER_RUN_ID".to_string(),
            run_id.to_string(),
        ),
        (
            "CHAOS_LABS_SCHEDULER_WORKFLOW_ID".to_string(),
            workflow.id.clone(),
        ),
    ]
}

/// Persist per-step results into run_tasks/run_attempts. The scheduler is the
/// sole writer here — step child processes never author these rows.
fn record_step_results(db: &Arc<Database>, run_id: &str, results: &[crate::steps::StepResult]) {
    for result in results {
        let attempts = result.attempts.max(1);
        let mut last_attempt_id: Option<String> = None;
        for n in 0..attempts {
            let is_last = n + 1 == attempts;
            let status = if result.skipped {
                "skipped"
            } else if is_last {
                if result.success {
                    "success"
                } else {
                    "failed"
                }
            } else {
                "retry"
            };
            if let Ok(attempt_id) =
                db.insert_run_attempt(run_id, &result.step_id, n as i64, "running", None)
            {
                let (error_type, error_message) = if result.success || result.skipped {
                    (None, None)
                } else {
                    (Some("StepError"), Some(result.message.as_str()))
                };
                let _ = db.finish_run_attempt(
                    &attempt_id,
                    status,
                    result.exit_code,
                    error_type,
                    error_message,
                );
                last_attempt_id = Some(attempt_id);
            }
        }
        let task_status = if result.skipped {
            "skipped"
        } else if result.success {
            "success"
        } else {
            "failed"
        };
        let details = serde_json::json!({
            "message": result.message,
            "exit_code": result.exit_code,
            "attempts": result.attempts,
        });
        if let Ok(task_row_id) = db.insert_run_task(
            run_id,
            last_attempt_id.as_deref(),
            &result.step_id,
            task_status,
            attempts.saturating_sub(1) as i64,
            Some(&details),
        ) {
            let (error_type, error_message) = if result.success || result.skipped {
                (None, None)
            } else {
                (Some("StepError"), Some(result.message.as_str()))
            };
            let _ = db.finish_run_task(&task_row_id, task_status, error_type, error_message, None);
        }
    }
}

/// Execute a generic step-flow in-scheduler, recording per-step tasks/attempts.
/// Returns `(exit_code, stdout, stderr)` for the run record.
fn execute_generic_step_flow(
    db: &Arc<Database>,
    workspace_root: &str,
    run_id: &str,
    workflow: &Workflow,
    generic: &crate::workflow_spec::GenericSpec,
) -> (i32, String, String) {
    let runner = crate::service::SystemProcessRunner;
    let base_env = step_flow_base_env(db, run_id, workflow);
    match crate::steps::execute_step_flow(generic, &runner, workspace_root, &base_env) {
        Ok(outcome) => {
            record_step_results(db, run_id, &outcome.results);
            let mut lines = Vec::new();
            let mut failures = Vec::new();
            for r in &outcome.results {
                let state = if r.skipped {
                    "skipped"
                } else if r.success {
                    "ok"
                } else {
                    "failed"
                };
                lines.push(format!("[{}] {} ({})", state, r.step_id, r.message));
                if !r.success && !r.skipped {
                    failures.push(format!("{}: {}", r.step_id, r.message));
                }
            }
            let exit_code = if outcome.success { 0 } else { 1 };
            (exit_code, lines.join("\n"), failures.join("\n"))
        }
        Err(e) => {
            let msg = format!("step-flow error: {e}");
            (-1, String::new(), msg)
        }
    }
}

/// Resolves operator secrets (e.g. the Cursor service-account API key) from the
/// scheduler config table, falling back to the process environment. Values are
/// never logged.
struct SchedulerSecretResolver {
    db: Arc<Database>,
}

impl crate::operators::SecretResolver for SchedulerSecretResolver {
    fn get(&self, key: &str) -> Option<String> {
        self.db
            .get_scheduler_config(key)
            .ok()
            .flatten()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| crate::operators::EnvSecretResolver.get(key))
    }
}

/// Map an operator outcome to a first-class terminal run status when one
/// applies (e.g. a cloud agent whose poll budget was exhausted), so the run row
/// carries `poll_exhausted` instead of collapsing to `failed`.
fn operator_run_terminal_status(
    outcome: &crate::operators::OperatorOutcome,
) -> Option<&'static str> {
    if outcome.details.get("status").and_then(|v| v.as_str()) == Some("POLL_EXHAUSTED") {
        Some("poll_exhausted")
    } else {
        None
    }
}

/// Execute a typed operator via the operator registry, recording a task/attempt.
/// Returns `(exit_code, stdout, stderr, terminal_status)` where `terminal_status`
/// is a first-class run status when the outcome maps to one.
/// Run `f` via `tokio::task::block_in_place` when called from inside an
/// ambient multi-threaded tokio runtime (so the runtime can hand this
/// worker's other queued tasks to a fresh thread while `f` blocks); otherwise
/// run `f` directly. `block_in_place` panics if there is no current runtime
/// or the runtime is single-threaded, so both are guarded against.
///
/// Do not call this from inside a `tokio::task::spawn_blocking` closure:
/// `Handle::try_current()` still reports the ambient runtime there, but
/// `block_in_place`'s own internal state isn't set up on the blocking pool,
/// so it panics anyway (tokio issue #2327). No `spawn_blocking` call exists
/// in this codebase today, so this is currently unreachable — keep it that
/// way for any future caller of this helper.
fn run_possibly_blocking<T>(f: impl FnOnce() -> T) -> T {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
            tokio::task::block_in_place(f)
        }
        _ => f(),
    }
}

/// Ephemeral, in-memory operator-config overlay for the Cursor fix-agent seam
/// (D05 / F10, B3). Returns the stored config UNCHANGED for every dispatch
/// except a `ui_fix_agent`-triggered `cursor_agent` run, for which it:
///
/// - overlays ONLY the `prompt` from `input_json` — a strict WHITELIST: no other
///   `input_json` field (`repository` / `auto_create_pr` / `workOnCurrentBranch`
///   / `api_key_secret` / …) can reach the operator config, and
/// - FORCES `auto_create_pr = false` regardless of the stored config OR any value
///   smuggled in `input_json` (forced AT EXECUTION, not merely defaulted).
///
/// **D05 PR2e — Option C (race-free born-draft).** The cloud agent is forced to
/// push its `cursor/…` branch and open NO PR of its own; the SCHEDULER then
/// opens a DRAFT PR against that branch (see
/// [`apply_cloud_fix_draft_hardening`] + [`crate::fix_cloud`]). This REVERSES
/// #284's "the cloud agent opens the PR" mechanism: a Cursor-opened PR is born
/// NON-draft, and this repo's `app-auto-merge.yml` arms squash auto-merge + posts
/// an approval at PR CREATION for ANY `draft == false` same-repo PR — so a
/// machine-authored fix could auto-merge before a human reviews. A
/// scheduler-opened `--draft` PR is born-draft ⇒ auto-merge-INELIGIBLE ⇒
/// race-free (no window in which a non-draft cloud fix PR exists), unifying the
/// CLOUD path with the LOCAL path (which already opens its own draft PR).
/// Because the seam never sets `workOnCurrentBranch`, the agent always pushes to
/// a NEW branch; the app has no PR-merge code path, so a fix is NEVER
/// auto-merged and NEVER auto-applied to the running system.
///
/// repository / mode / model / api_key_secret therefore ALWAYS come from the
/// designated fix workflow's STORED config, never from caller-supplied input.
/// Gating on `trigger_kind == ui_fix_agent` (not merely `operator_type ==
/// cursor_agent`) is what keeps a plain rerun/backfill/child dispatch of the
/// same operator from having its prompt hijacked or PR-forced (M2).
fn fix_agent_config_overlay<'a>(
    operator_type: &str,
    trigger_kind: Option<&str>,
    input_json: Option<&str>,
    config: &'a Value,
) -> std::borrow::Cow<'a, Value> {
    let is_fix_agent = operator_type == "cursor_agent"
        && trigger_kind == Some(crate::service::FIX_AGENT_TRIGGER_KIND);
    if !is_fix_agent {
        return std::borrow::Cow::Borrowed(config);
    }
    let mut obj = match config {
        Value::Object(map) => map.clone(),
        _ => serde_json::Map::new(),
    };
    // Whitelist: take ONLY `prompt` from caller-supplied input_json.
    if let Some(prompt) = input_json
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .as_ref()
        .and_then(|v| v.get("prompt"))
        .and_then(|v| v.as_str())
    {
        obj.insert("prompt".to_string(), Value::String(prompt.to_string()));
    }
    // Option C: FORCE the agent to open NO PR — it only pushes its `cursor/…`
    // branch. The scheduler then opens a born-DRAFT PR against that branch, so a
    // machine fix is never a non-draft (auto-merge-eligible) PR at any moment.
    obj.insert("auto_create_pr".to_string(), Value::Bool(false));
    std::borrow::Cow::Owned(Value::Object(obj))
}

/// D05 PR2e — cloud non-draft hardening seam, **Option C (race-free born-draft)**.
/// For a `ui_fix_agent` `cursor_agent` dispatch, enforce the LOCKED D05 invariant
/// that the fix PR is a DRAFT — never auto-merged — with NO window in which a
/// non-draft cloud fix PR exists.
///
/// The config overlay forces `auto_create_pr=false`, so the cloud agent ONLY
/// pushes its `cursor/…` branch (surfaced as `outcome.details.pushed_branch`) and
/// opens NO PR. This seam then:
///
/// - **PRIMARY** — on a pushed, VALIDATED branch with NO `pr_url`, the SCHEDULER
///   opens the born-`--draft` PR itself (`gh pr create -R <owner/repo> --draft
///   --base main --head <branch>`, [`crate::fix_cloud::open_cloud_fix_draft_pr`]).
///   The explicit `-R` (the run's surfaced `repo` slug) is required — the
///   scheduler's own workspace is NOT a checkout of the target repo. Born-draft ⇒
///   auto-merge-INELIGIBLE ⇒ race-free. On a successful open the created PR's URL
///   is BACKFILLED to the top-level `outcome.details.pr_url` (the agent left it
///   null) so run history shows the PR.
/// - **PRIMARY RECONCILE** (defense-in-depth) — if that `gh pr create` FAILS
///   because a PR already exists (a future Cursor opened one despite
///   `auto_create_pr=false` AND omitted its url), PROBE the branch
///   ([`crate::fix_cloud::reconcile_orphaned_cloud_fix_pr`]) and CONVERT any found
///   PR to a draft, so no auto-merge-eligible machine PR is left live.
/// - **FALLBACK** — if the agent UNEXPECTEDLY returns a `pr_url` outright, the
///   born-draft primary cannot apply, so DETECT the PR's draft state and CONVERT a
///   non-draft back to a draft ([`crate::fix_cloud::harden_cloud_fix_pr_draft`]).
/// - **FAIL CLOSED** — a pushed branch whose name fails validation, a missing
///   target repo, or an unverifiable/failed convert/probe/open all REFUSE to
///   assume safety and raise an operator-visible `log::warn!` alert. Every outcome
///   is recorded under `outcome.details.draft_hardening` (surfaced in run detail).
///
/// The born-draft vs convert-to-draft split is expressed by the pure
/// [`crate::fix_cloud::decide_cloud_fix_pr_action`]. Gated on `trigger_kind ==
/// ui_fix_agent` (NOT merely `operator_type == cursor_agent`), exactly like
/// [`fix_agent_config_overlay`], so a plain rerun/backfill/child dispatch of the
/// same operator — or any non-fix `cursor_agent` workflow — is NEVER touched.
/// `workflow_name` + `run_id` feed the app-authored PR title/body (never agent
/// free-text). Runner-injected so the whole path is unit-testable with a fake.
fn apply_cloud_fix_draft_hardening(
    runner: &dyn crate::service::ProcessRunner,
    cwd: Option<&str>,
    operator_type: &str,
    trigger_kind: Option<&str>,
    workflow_name: &str,
    run_id: &str,
    outcome: &mut crate::operators::OperatorOutcome,
) {
    if operator_type != "cursor_agent"
        || trigger_kind != Some(crate::service::FIX_AGENT_TRIGGER_KIND)
    {
        return;
    }
    use crate::fix_cloud;
    let pr_url = outcome
        .details
        .get("pr_url")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let pushed_branch = outcome
        .details
        .get("pushed_branch")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    // The target `owner/repo` slug the run surfaced (Finding 1) — required to open
    // the PR against the RIGHT repo (`gh pr create -R …`), since the scheduler's
    // workspace is not a checkout of it.
    let repo = outcome
        .details
        .get("repo")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    // `(draft_hardening detail, top-level pr_url to backfill)`. The backfill
    // (Finding 3) is `Some` only when a DRAFT PR is confirmed (opened or
    // reconciled), so an alert path never fabricates a `pr_url`.
    let (detail, backfill_pr_url): (serde_json::Value, Option<String>) =
        match fix_cloud::decide_cloud_fix_pr_action(pr_url.as_deref(), pushed_branch.as_deref()) {
            // Nothing pushed and no PR (a failed / poll-exhausted run) — no-op.
            fix_cloud::CloudFixPrAction::Noop => return,
            // PRIMARY (race-free): the agent pushed a `cursor/…` branch and opened no
            // PR — the scheduler opens the born-DRAFT PR against it.
            fix_cloud::CloudFixPrAction::OpenDraftPr { branch } => match repo.as_deref() {
                // FAIL CLOSED: no valid target repo surfaced — cannot open the PR
                // against the right repo, so refuse rather than risk the wrong one.
                None => {
                    log::warn!(
                        "{}",
                        fix_cloud::build_cloud_open_alert(
                            &branch,
                            fix_cloud::CLOUD_MISSING_REPO_REASON
                        )
                    );
                    (
                        fix_cloud::alert_detail(fix_cloud::CLOUD_MISSING_REPO_REASON),
                        None,
                    )
                }
                Some(repo) => {
                    let title = fix_cloud::build_cloud_fix_pr_title(workflow_name, run_id);
                    let body = fix_cloud::build_cloud_fix_pr_body(workflow_name, run_id, &branch);
                    match fix_cloud::open_cloud_fix_draft_pr(
                        runner,
                        cwd,
                        repo,
                        fix_cloud::FIX_CLOUD_PR_BASE,
                        &branch,
                        &title,
                        &body,
                    ) {
                        fix_cloud::CloudDraftPrOpen::Opened { pr_url } => (
                            fix_cloud::opened_draft_detail(&branch, pr_url.as_deref()),
                            pr_url,
                        ),
                        // RECONCILE (Finding 2): the create failed — a PR may already
                        // exist for the branch (Cursor opened one despite the flag,
                        // without surfacing its url). Probe + convert any orphan to a
                        // draft so nothing auto-merge-eligible is left live.
                        fix_cloud::CloudDraftPrOpen::Failed => {
                            match fix_cloud::reconcile_orphaned_cloud_fix_pr(
                                runner, cwd, repo, &branch,
                            ) {
                                fix_cloud::OrphanReconcile::Found { pr_url, hardening } => {
                                    match &hardening {
                                        fix_cloud::CloudDraftHardening::Alerted { reason } => {
                                            log::warn!(
                                                "{}",
                                                fix_cloud::build_nondraft_alert(&pr_url, reason)
                                            )
                                        }
                                        _ => log::warn!(
                                            "{}",
                                            fix_cloud::build_orphan_recovered_alert(&pr_url)
                                        ),
                                    }
                                    let detail = fix_cloud::recovered_detail(&pr_url, &hardening);
                                    (detail, Some(pr_url))
                                }
                                fix_cloud::OrphanReconcile::NoExistingPr => {
                                    log::warn!(
                                        "{}",
                                        fix_cloud::build_cloud_open_alert(
                                            &branch,
                                            fix_cloud::CLOUD_DRAFT_OPEN_FAILED_REASON,
                                        )
                                    );
                                    (
                                        fix_cloud::alert_detail(
                                            fix_cloud::CLOUD_DRAFT_OPEN_FAILED_REASON,
                                        ),
                                        None,
                                    )
                                }
                                fix_cloud::OrphanReconcile::ProbeFailed => {
                                    log::warn!(
                                        "{}",
                                        fix_cloud::build_cloud_open_alert(
                                            &branch,
                                            fix_cloud::CLOUD_ORPHAN_PROBE_FAILED_REASON,
                                        )
                                    );
                                    (
                                        fix_cloud::alert_detail(
                                            fix_cloud::CLOUD_ORPHAN_PROBE_FAILED_REASON,
                                        ),
                                        None,
                                    )
                                }
                            }
                        }
                    }
                }
            },
            // FALLBACK (defense-in-depth): the agent unexpectedly opened a PR itself
            // (a future Cursor change ignored auto_create_pr=false) — ensure it is a
            // draft via the detect→convert path.
            fix_cloud::CloudFixPrAction::HardenExistingPr { pr_url } => {
                let result = fix_cloud::harden_cloud_fix_pr_draft(runner, cwd, &pr_url);
                if let fix_cloud::CloudDraftHardening::Alerted { reason } = &result {
                    log::warn!("{}", fix_cloud::build_nondraft_alert(&pr_url, reason));
                }
                // pr_url is already top-level (it came from details) — pass it
                // through so the backfill is idempotent.
                (fix_cloud::hardening_detail(&result), Some(pr_url))
            }
            // FAIL CLOSED: a pushed branch whose name failed validation — refuse to
            // open a PR against it (never trust an injection-y `--head` value).
            fix_cloud::CloudFixPrAction::AlertInvalidBranch { branch } => {
                log::warn!(
                    "{}",
                    fix_cloud::build_cloud_open_alert(
                        &branch,
                        fix_cloud::CLOUD_INVALID_BRANCH_REASON
                    )
                );
                (
                    fix_cloud::alert_detail(fix_cloud::CLOUD_INVALID_BRANCH_REASON),
                    None,
                )
            }
        };

    if let Some(obj) = outcome.details.as_object_mut() {
        obj.insert("draft_hardening".to_string(), detail);
        // Finding 3: surface the confirmed DRAFT PR at the top level so run
        // history / consumers show the PR instead of "no PR" (the cloud agent
        // leaves `pr_url` null by design under Option C).
        if let Some(u) = backfill_pr_url {
            obj.insert("pr_url".to_string(), serde_json::Value::String(u));
        }
    }
}

fn execute_typed_operator(
    db: &Arc<Database>,
    workspace_root: &str,
    run_id: &str,
    workflow: &Workflow,
    typed: &crate::workflow_spec::TypedSpec,
    trigger_kind: Option<&str>,
    input_json: Option<&str>,
) -> (i32, String, String, Option<&'static str>) {
    let registry = crate::operators::OperatorRegistry::with_builtins();
    let Some(operator) = registry.get(&typed.operator_type) else {
        return (
            -1,
            String::new(),
            format!("unknown operator_type: {}", typed.operator_type),
            None,
        );
    };
    let attempt_id = db
        .insert_run_attempt(run_id, &typed.operator_type, 0, "running", None)
        .ok();
    // Insert the task row *before* execution (status "running", no details
    // yet) rather than only recording it once execution finishes. A
    // long-running operator (`cursor_agent` cloud mode polls for minutes) can
    // report interim progress via `on_progress` — see below — so a scheduler
    // kill mid-execution still leaves a traceable run_task row instead of no
    // record at all.
    let task_row_id = db
        .insert_run_task(
            run_id,
            attempt_id.as_deref(),
            &typed.operator_type,
            "running",
            0,
            None,
        )
        .ok();
    let progress_db = Arc::clone(db);
    let progress_task_row_id = task_row_id.clone();
    let on_progress = move |details: &Value| {
        if let Some(task_row_id) = &progress_task_row_id {
            let _ = progress_db.update_run_task_details(task_row_id, details);
        }
    };
    // Typed operators (`cursor_agent` in particular) build and use a
    // `reqwest::blocking::Client` and may block synchronously for minutes
    // (HTTP polling). The REST API path reaches this function from inside a
    // multi-threaded tokio runtime worker (see `api::start_api_server`); a
    // blocking reqwest client constructed *and dropped* on such a worker
    // panics on drop ("Cannot drop a runtime in a context where blocking is
    // not allowed"), silently leaving the run stuck in `running` forever. Run
    // the operator (construction through drop of `http`) inside
    // `block_in_place` whenever an ambient tokio runtime is detected so the
    // runtime can offload other work first; call directly otherwise (e.g. the
    // cron scheduler loop or a Tauri sync-command context with no runtime).
    // Fix-agent seam (D05 / F10): for a `ui_fix_agent` dispatch of a
    // `cursor_agent` operator ONLY, overlay the diagnostic prompt from
    // `input_json` and FORCE `auto_create_pr=true` (propose-only DRAFT PR) at
    // execution. Every other dispatch (rerun/backfill/child of the same
    // operator) is untouched, so a stored prompt/PR-setting is never hijacked
    // (M2). See [`fix_agent_config_overlay`].
    let effective_config = fix_agent_config_overlay(
        &typed.operator_type,
        trigger_kind,
        input_json,
        &typed.config,
    );
    let mut outcome = run_possibly_blocking(|| {
        let runner = crate::service::SystemProcessRunner;
        let http = crate::operators::ReqwestHttpClient::default();
        let secrets = SchedulerSecretResolver { db: Arc::clone(db) };
        let ctx = crate::operators::OperatorContext {
            runner: &runner,
            http: &http,
            secrets: &secrets,
            workspace_root,
            on_progress: &on_progress,
        };
        operator.execute(&ctx, &effective_config)
    });
    // D05 PR2e cloud non-draft hardening (Option C): a `ui_fix_agent`
    // `cursor_agent` dispatch pushes a `cursor/…` branch and opens NO PR — the
    // scheduler opens the born-DRAFT PR itself (race-free), with a
    // convert-to-draft FALLBACK if a future Cursor change unexpectedly opens a PR.
    // A no-op for every other dispatch. Wrapped like the operator above so the
    // brief `gh` subprocess(es) never block a tokio worker (the REST path reaches
    // here from a multi-thread runtime). See [`apply_cloud_fix_draft_hardening`].
    run_possibly_blocking(|| {
        apply_cloud_fix_draft_hardening(
            &crate::service::SystemProcessRunner,
            Some(workspace_root),
            &typed.operator_type,
            trigger_kind,
            &workflow.name,
            run_id,
            &mut outcome,
        );
    });
    let status = if outcome.success { "success" } else { "failed" };
    if let Some(attempt_id) = &attempt_id {
        let _ = db.finish_run_attempt(
            attempt_id,
            status,
            Some(if outcome.success { 0 } else { 1 }),
            if outcome.success {
                None
            } else {
                Some("OperatorError")
            },
            if outcome.success {
                None
            } else {
                Some(outcome.summary.as_str())
            },
        );
    }
    match &task_row_id {
        Some(task_row_id) => {
            let _ = db.finish_run_task(
                task_row_id,
                status,
                if outcome.success {
                    None
                } else {
                    Some("OperatorError")
                },
                if outcome.success {
                    None
                } else {
                    Some(outcome.summary.as_str())
                },
                Some(&outcome.details),
            );
        }
        None => {
            // The early insert failed (rare); fall back to a single insert
            // with the final outcome so the task is still recorded.
            let _ = db.insert_run_task(
                run_id,
                attempt_id.as_deref(),
                &typed.operator_type,
                status,
                0,
                Some(&outcome.details),
            );
        }
    }
    let exit_code = if outcome.success { 0 } else { 1 };
    let terminal = operator_run_terminal_status(&outcome);
    let stderr = if outcome.success {
        String::new()
    } else {
        outcome.summary.clone()
    };
    let summary = outcome.summary;
    (exit_code, summary, stderr, terminal)
}

#[allow(clippy::too_many_arguments)] // Threads full run context through the engine entry point.
pub fn execute_workflow_with_context(
    db: &Arc<Database>,
    chaos_labs_root: &str,
    python_path: &str,
    workflow_id: &str,
    notify_on_success: bool,
    notify_on_failure: bool,
    email_on_failure_enabled: bool,
    trigger_kind: Option<&str>,
    trigger_payload: Option<&str>,
    upstream_run_id: Option<&str>,
    input_json: Option<&str>,
    rerun_of_run_id: Option<&str>,
    queued_run_id: Option<&str>,
    suppress_completion_triggers: bool,
    app_handle: Option<tauri::AppHandle>,
) -> Result<RunResult, String> {
    // M2 (D05 LOCAL fix): a fix-agent source RERUN must execute inside the fix's
    // dedicated throwaway worktree, NEVER the shared primary checkout (editing +
    // rerunning there would race the other scheduler workers). The worktree path
    // is DERIVED from the run's own identity — the reserved fix-rerun trigger
    // kind plus the source run it reruns — so both the inline dispatch and the
    // queued-drain path resolve the SAME directory with no persisted column.
    // Shadowing `chaos_labs_root` here routes the whole execution (command build,
    // step-flow, typed operator, child/background dispatch) into the worktree.
    // FAIL CLOSED: if the worktree is absent (e.g. a post-crash stale queued
    // rerun whose worktree the startup sweep already reclaimed), refuse rather
    // than silently running agent-edited code against the primary tree.
    let fix_rerun_root;
    let chaos_labs_root: &str = if trigger_kind == Some(crate::service::FIX_RERUN_TRIGGER_KIND) {
        let source_run_id =
            rerun_of_run_id.ok_or_else(|| "fix rerun is missing its source run id".to_string())?;
        let worktree = crate::fix_worktree::fix_worktree_path_for(source_run_id);
        if !worktree.is_dir() {
            return Err(format!(
                "fix rerun worktree is missing for source run {source_run_id}; \
                 refusing to run against the primary checkout"
            ));
        }
        fix_rerun_root = worktree.to_string_lossy().into_owned();
        &fix_rerun_root
    } else {
        chaos_labs_root
    };
    let workflow = db
        .get_workflow(workflow_id)
        .map_err(|e| format!("Failed to get workflow: {}", e))?;
    let queue_config = parse_queue_config(workflow.queue_config.as_deref(), &workflow.environment);
    let mutex_keys = mutex_keys(&workflow.id, &queue_config);
    let trigger_state = trigger_state_for_admission(trigger_kind, trigger_payload);

    // Capacity, mutex, queued claim, run/admitted state, and trigger state are
    // now decided in one BEGIN IMMEDIATE transaction, so concurrent admitters
    // cannot both pass the caps or both take a mutex before either commits.
    let run = match db
        .admit_run_with_context(
            &workflow.id,
            &queue_config.queue,
            &queue_config.environment,
            &queue_config.tags,
            trigger_kind,
            trigger_payload,
            upstream_run_id,
            input_json,
            rerun_of_run_id,
            queued_run_id,
            &mutex_keys,
            trigger_state
                .as_ref()
                .map(|(id, fingerprint)| (id.as_str(), fingerprint.as_str())),
        )
        .map_err(|e| format!("Failed to admit run: {}", e))?
    {
        RunAdmission::Admitted(run) => run,
        RunAdmission::AtCapacity => {
            let _ = db.upsert_queued_run_with_context(
                &workflow.id,
                &queue_config.queue,
                queue_config.priority,
                trigger_kind,
                trigger_payload,
                upstream_run_id,
                input_json,
                rerun_of_run_id,
                suppress_completion_triggers,
            );
            return Err(format!(
                "Queue {} is at capacity or constrained by global/tag caps",
                queue_config.queue
            ));
        }
        RunAdmission::MutexBusy => {
            return Err(format!(
                "Workflow {} could not acquire mutex locks in queue {}",
                workflow.id, queue_config.queue
            ));
        }
        RunAdmission::QueuedRunUnavailable => {
            return Err(format!(
                "Queued run {} is no longer available",
                queued_run_id.unwrap_or("<none>")
            ));
        }
    };
    let worker_id = std::thread::current()
        .name()
        .unwrap_or("scheduler-direct")
        .to_string();
    db.mark_run_started(&run.id, &worker_id)
        .map_err(|e| format!("Failed to mark run started: {}", e))?;

    // Structured workflows (generic step-flow / typed operator) are executed
    // in-scheduler with the scheduler as the sole author of run_tasks /
    // run_attempts (task-ownership contract). Legacy single-script workflows
    // (no spec_json) fall through to the child FD-3 event path below.
    if let Some(spec) = workflow
        .spec_json
        .as_deref()
        .and_then(|json| crate::workflow_spec::WorkflowSpec::from_json(json).ok())
    {
        let (exit_code, stdout, stderr, terminal_status) = match spec.kind {
            crate::workflow_spec::WorkflowKind::Generic => match spec.generic.as_ref() {
                Some(generic) => {
                    let (code, out, err) =
                        execute_generic_step_flow(db, chaos_labs_root, &run.id, &workflow, generic);
                    (code, out, err, None)
                }
                None => (
                    -1,
                    String::new(),
                    "generic workflow has no step body".to_string(),
                    None,
                ),
            },
            crate::workflow_spec::WorkflowKind::Typed => match spec.typed.as_ref() {
                Some(typed) => execute_typed_operator(
                    db,
                    chaos_labs_root,
                    &run.id,
                    &workflow,
                    typed,
                    trigger_kind,
                    input_json,
                ),
                None => (
                    -1,
                    String::new(),
                    "typed workflow has no operator body".to_string(),
                    None,
                ),
            },
        };
        if let Some(status) = terminal_status {
            db.finish_run_with_status_details(
                &run.id,
                Some(exit_code),
                status,
                &stdout,
                &stderr,
                None,
            )
            .map_err(|e| format!("Failed to update run: {}", e))?;
        } else {
            db.finish_run(&run.id, exit_code, &stdout, &stderr, None)
                .map_err(|e| format!("Failed to update run: {}", e))?;
        }
        let success = exit_code == 0;
        return Ok(RunResult {
            run_id: run.id,
            workflow_name: workflow.name,
            script_path: workflow.script_path.clone(),
            success,
            completed: true,
            should_notify: if success {
                notify_on_success
            } else {
                notify_on_failure
            },
            email_on_failure: workflow.email_on_failure,
        });
    }

    let sample_metadata = ResourceSampleMetadata {
        db: Arc::clone(db),
        run_id: run.id.clone(),
        workflow_id: workflow.id.clone(),
        queue_name: Some(queue_config.queue.clone()),
        environment: queue_config.environment.clone(),
    };

    let output = run_workflow_command(
        build_workflow_command(
            &workflow.script_path,
            chaos_labs_root,
            python_path,
            &run.id,
            &workflow.id,
            &queue_config.queue,
            &queue_config.environment,
            workflow.domain.as_deref(),
            db.path(),
            input_json,
        ),
        Some(sample_metadata.clone()),
        run_timeout(),
    );

    match output {
        Ok(output) => {
            let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let mut stderr = String::from_utf8_lossy(&output.stderr).to_string();
            append_capture_notice(&mut stdout, "stdout", output.stdout_truncated);
            append_capture_notice(&mut stderr, "stderr", output.stderr_truncated);
            append_capture_notice(&mut stderr, "task events", output.task_events_truncated);
            let exit_code = output.status.code().unwrap_or(-1);
            let task_events_raw = output.task_events_raw;
            let stdout = stdout_with_task_events(stdout, &task_events_raw);
            persist_task_events(db, &run.id, &workflow.id, &task_events_raw);
            // corr-F4 (D05 LOCAL fix): a VALIDATION rerun (the only caller that
            // sets `suppress_completion_triggers`) must cause NO downstream
            // cascade. `subworkflow_requested` events are emitted at RUNTIME
            // (not a static spec field, so they can't be refused at preflight),
            // so the child/subworkflow spawn is skipped HERE. Byte-identical for
            // every normal run (the flag is false), which still dispatches.
            let child_summary = if suppress_completion_triggers {
                ChildDispatchSummary::default()
            } else {
                dispatch_child_workflow_requests(
                    db,
                    chaos_labs_root,
                    python_path,
                    &run.id,
                    &workflow.id,
                    &task_events_raw,
                    app_handle.clone(),
                )
            };

            let result_url = extract_result_url(&stdout);

            // corr-F1 (D05 LOCAL fix): never spawn the background-PID completion
            // monitor for a validation rerun — its later finish fires a
            // completion trigger that would cascade. Background mode is detected
            // from RUNTIME stdout ("launched (PID N)"), not a static spec field,
            // so (like corr-F4) it is suppressed here rather than refused at
            // preflight. The rerun then resolves via its FOREGROUND exit; a
            // background-launcher source's fix is validated only by the launch's
            // exit, which — with the human-reviewed DRAFT PR — is the accepted v1
            // residual. Byte-identical for every normal run (flag is false).
            let bg_pid = if suppress_completion_triggers {
                None
            } else {
                extract_background_pid(&stdout, chaos_labs_root)
            };

            if let Some(pid) = bg_pid {
                let db = Arc::clone(db);
                let run_id = run.id.clone();
                let wf_name = workflow.name.clone();
                let wf_script = workflow.script_path.clone();
                let wf_email = workflow.email_on_failure;
                let root = chaos_labs_root.to_string();
                let py = python_path.to_string();
                let email_enabled = email_on_failure_enabled;
                let notify_success = notify_on_success;
                let notify_failure = notify_on_failure;
                let workflow_id = workflow.id.clone();

                let bg_log = extract_log_path(&stdout);
                let log_start_offset = extract_log_start_offset(&stdout);
                let pid_identity = capture_pid_identity(pid);

                std::thread::spawn(move || {
                    monitor_background_pid(
                        pid_identity,
                        &run_id,
                        &wf_name,
                        &wf_script,
                        &root,
                        bg_log.as_deref(),
                        log_start_offset,
                        email_enabled && wf_email,
                        email_enabled,
                        &db,
                        &py,
                        &workflow_id,
                        sample_metadata,
                        notify_success,
                        notify_failure,
                        app_handle.clone(),
                    );
                });

                return Ok(RunResult {
                    run_id: run.id,
                    workflow_name: workflow.name,
                    script_path: workflow.script_path.clone(),
                    success: true,
                    completed: false,
                    should_notify: false,
                    email_on_failure: workflow.email_on_failure,
                });
            }

            let final_exit_code = if output.timed_out || output.cancelled {
                -1
            } else if exit_code == 0 && child_summary.failure_count > 0 {
                1
            } else {
                exit_code
            };
            let stderr = if output.timed_out || output.cancelled {
                let mut combined = stderr;
                if !combined.is_empty() {
                    combined.push('\n');
                }
                combined.push_str(if output.timed_out {
                    "Workflow timed out and its process group was terminated"
                } else {
                    "Workflow was cancelled during scheduler shutdown"
                });
                combined
            } else if child_summary.failure_count > 0 {
                let mut combined = stderr;
                if !combined.is_empty() {
                    combined.push('\n');
                }
                combined.push_str(&format!(
                    "Child workflow dispatch reported {} failure(s): {}",
                    child_summary.failure_count,
                    child_summary.notes.join("; ")
                ));
                combined
            } else {
                stderr
            };

            let final_status = if output.timed_out {
                "timed_out"
            } else if output.cancelled {
                "cancelled"
            } else if final_exit_code == 0 {
                "success"
            } else {
                "failed"
            };
            db.finish_run_with_status_details(
                &run.id,
                Some(final_exit_code),
                final_status,
                &stdout,
                &stderr,
                result_url.as_deref(),
            )
            .map_err(|e| format!("Failed to update run: {}", e))?;

            let success = final_exit_code == 0;
            Ok(RunResult {
                run_id: run.id,
                workflow_name: workflow.name,
                script_path: workflow.script_path.clone(),
                success,
                completed: true,
                should_notify: if success {
                    notify_on_success
                } else {
                    notify_on_failure
                },
                email_on_failure: workflow.email_on_failure,
            })
        }
        Err(e) => {
            let stderr = format!("Failed to execute: {}", e);
            let _ = db.finish_run(&run.id, -1, "", &stderr, None);
            Ok(RunResult {
                run_id: run.id,
                workflow_name: workflow.name,
                script_path: workflow.script_path.clone(),
                success: false,
                completed: true,
                should_notify: notify_on_failure,
                email_on_failure: workflow.email_on_failure,
            })
        }
    }
}

#[allow(clippy::too_many_arguments)] // Threads full run context through the completion hook.
pub fn trigger_on_completion(
    db: &Arc<Database>,
    chaos_labs_root: &str,
    python_path: &str,
    upstream_workflow_id: &str,
    upstream_run_id: &str,
    upstream_success: bool,
    notify_on_success: bool,
    notify_on_failure: bool,
    email_on_failure_enabled: bool,
) {
    let chain = completion_chain_for_run(db, upstream_run_id, upstream_workflow_id);
    trigger_on_completion_with_chain(
        db,
        chaos_labs_root,
        python_path,
        upstream_workflow_id,
        upstream_run_id,
        upstream_success,
        notify_on_success,
        notify_on_failure,
        email_on_failure_enabled,
        chain,
    );
}

#[allow(clippy::too_many_arguments)] // Threads full run context through recursive completion chains.
fn trigger_on_completion_with_chain(
    db: &Arc<Database>,
    chaos_labs_root: &str,
    python_path: &str,
    upstream_workflow_id: &str,
    upstream_run_id: &str,
    upstream_success: bool,
    notify_on_success: bool,
    notify_on_failure: bool,
    email_on_failure_enabled: bool,
    chain: CompletionChain,
) {
    let status = if upstream_success {
        "success"
    } else {
        "failed"
    };
    let workflows = match db.list_workflows() {
        Ok(w) => w,
        Err(e) => {
            log::warn!("Failed to list workflows for completion triggers: {}", e);
            return;
        }
    };
    for workflow in workflows {
        if !workflow.enabled || workflow.id == upstream_workflow_id {
            continue;
        }
        let triggers = parse_trigger_config(workflow.trigger_config.as_deref());
        let should_run = triggers.iter().any(|trigger| {
            if trigger.get("kind").and_then(Value::as_str) != Some("on_completion") {
                return false;
            }
            if trigger.get("upstream_workflow_id").and_then(Value::as_str)
                != Some(upstream_workflow_id)
            {
                return false;
            }
            let statuses = trigger
                .get("status_filter")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_else(|| vec![Value::String("success".to_string())]);
            statuses.iter().any(|s| s.as_str() == Some(status))
        });
        if !should_run {
            continue;
        }
        let next_chain = match chain.try_advance(&workflow.id) {
            Ok(next_chain) => next_chain,
            Err(reason) => {
                log::warn!(
                    "Skipping completion-triggered workflow {} from {}: {}",
                    workflow.id,
                    upstream_workflow_id,
                    reason
                );
                continue;
            }
        };
        let payload =
            completion_trigger_payload(upstream_workflow_id, upstream_run_id, status, &next_chain);
        let queue_config =
            parse_queue_config(workflow.queue_config.as_deref(), &workflow.environment);
        match dependency_decision_for_db(db, &workflow, &queue_config) {
            DependencyDecision::Ready => {}
            DependencyDecision::Waiting(reason) => {
                log::info!(
                    "Deferring completion-triggered workflow {} in queue {}: {}",
                    workflow.id,
                    queue_config.queue,
                    reason
                );
                let _ = db.upsert_queued_run_with_context(
                    &workflow.id,
                    &queue_config.queue,
                    queue_config.priority,
                    Some("on_completion"),
                    Some(&payload),
                    Some(upstream_run_id),
                    None,
                    None,
                    false,
                );
                continue;
            }
            DependencyDecision::CascadeSkip(reason) => {
                let skip_payload = serde_json::json!({
                    "reason": reason,
                    "original_trigger_payload": payload,
                })
                .to_string();
                let _ = db.create_terminal_run_with_context(
                    &workflow.id,
                    "cascade-skipped",
                    Some("on_completion"),
                    Some(&skip_payload),
                    Some(upstream_run_id),
                    None,
                    None,
                );
                continue;
            }
        }
        match execute_workflow_with_context(
            db,
            chaos_labs_root,
            python_path,
            &workflow.id,
            notify_on_success,
            notify_on_failure,
            email_on_failure_enabled,
            Some("on_completion"),
            Some(&payload),
            Some(upstream_run_id),
            None,
            None,
            None,
            false,
            None,
        ) {
            Ok(result) if result.completed => trigger_on_completion_with_chain(
                db,
                chaos_labs_root,
                python_path,
                &workflow.id,
                &result.run_id,
                result.success,
                notify_on_success,
                notify_on_failure,
                email_on_failure_enabled,
                next_chain,
            ),
            Ok(_) => {}
            Err(e) => {
                log::error!(
                    "Completion trigger failed for downstream workflow {}: {}",
                    workflow.id,
                    e
                );
            }
        }
    }
}

pub struct RunResult {
    pub run_id: String,
    pub workflow_name: String,
    pub script_path: String,
    pub success: bool,
    pub completed: bool,
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
#[allow(clippy::too_many_arguments)] // Builds a child command from full run context.
fn build_workflow_command(
    script_path: &str,
    chaos_labs_root: &str,
    python_path: &str,
    run_id: &str,
    workflow_id: &str,
    queue_name: &str,
    environment: &str,
    domain: Option<&str>,
    scheduler_db_path: &str,
    input_json: Option<&str>,
) -> Command {
    let script_path = script_path.trim();
    let is_shell_cmd = script_path.contains('=') || script_path.contains("/bin/python");

    let mut cmd = if script_path.is_empty() {
        Command::new("false")
    } else if is_shell_cmd {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(script_path);
        cmd
    } else {
        let parts: Vec<&str> = script_path.split_whitespace().collect();
        let script = parts.first().copied().unwrap_or_default();
        let resolved = if script.starts_with('/') {
            script.to_string()
        } else {
            format!("{}/{}", chaos_labs_root, script)
        };
        let mut cmd = Command::new(python_path);
        cmd.arg(&resolved);
        for arg in &parts[1..] {
            cmd.arg(arg);
        }
        cmd
    };
    cmd.current_dir(chaos_labs_root);
    apply_workflow_env(
        &mut cmd,
        chaos_labs_root,
        run_id,
        workflow_id,
        queue_name,
        environment,
        domain,
        scheduler_db_path,
        input_json,
    );
    scrub_scheduler_secrets_from_child(&mut cmd);
    cmd
}

/// Strip the scheduler's own secrets from a child command's inherited env before
/// spawn. We keep the rest of the parent env (personal scripts rely on
/// `SSH_AUTH_SOCK`, proxies, venv/`PYTHONPATH`, Homebrew `PATH`, cloud CLI creds,
/// etc.), removing only named scheduler-internal secrets so they cannot leak to
/// arbitrary workflow scripts. None of the explicit `CHAOS_SCHEDULER_*` context
/// vars set by `apply_workflow_env` match the deny-list, so they survive.
fn scrub_scheduler_secrets_from_child(cmd: &mut Command) {
    for (key, _) in std::env::vars() {
        if crate::service::should_scrub_child_env_key(&key) {
            cmd.env_remove(key);
        }
    }
}

/// Export the scheduler's context to a child process. Emits the new
/// `CHAOS_SCHEDULER_*` variables and, for one transition minor version, also
/// the legacy `CHAOS_LABS_*` names so external scripts keep working.
#[allow(clippy::too_many_arguments)]
fn apply_workflow_env(
    cmd: &mut Command,
    workspace_root: &str,
    run_id: &str,
    workflow_id: &str,
    queue_name: &str,
    environment: &str,
    domain: Option<&str>,
    scheduler_db_path: &str,
    input_json: Option<&str>,
) {
    // New canonical names.
    cmd.env("CHAOS_SCHEDULER_WORKSPACE_ROOT", workspace_root)
        .env("CHAOS_SCHEDULER_RUN_ID", run_id)
        .env("CHAOS_SCHEDULER_WORKFLOW_ID", workflow_id)
        .env("CHAOS_SCHEDULER_QUEUE", queue_name)
        .env("CHAOS_SCHEDULER_ENVIRONMENT", environment)
        .env("CHAOS_SCHEDULER_DB_PATH", scheduler_db_path);
    // Legacy names (dual-emit for one minor version).
    cmd.env("CHAOS_LABS_ROOT", workspace_root)
        .env("CHAOS_LABS_SCHEDULER_RUN_ID", run_id)
        .env("CHAOS_LABS_SCHEDULER_WORKFLOW_ID", workflow_id)
        .env("CHAOS_LABS_SCHEDULER_QUEUE", queue_name)
        .env("CHAOS_LABS_SCHEDULER_CORPUS", environment)
        .env("CHAOS_LABS_SCHEDULER_DB_PATH", scheduler_db_path);
    if let Some(domain) = domain {
        cmd.env("CHAOS_SCHEDULER_DOMAIN", domain)
            .env("CHAOS_LABS_SCHEDULER_DOMAIN", domain);
    }
    if let Some(input) = input_json {
        cmd.env("CHAOS_SCHEDULER_WORKFLOW_INPUT_JSON", input)
            .env("CHAOS_LABS_WORKFLOW_INPUT_JSON", input);
    }
}

#[derive(Debug)]
struct WorkflowCommandOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    task_events_raw: String,
    timed_out: bool,
    cancelled: bool,
    stdout_truncated: bool,
    stderr_truncated: bool,
    task_events_truncated: bool,
}

fn run_timeout() -> Duration {
    let secs = std::env::var("CHAOS_SCHEDULER_RUN_TIMEOUT_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|secs| *secs > 0)
        .unwrap_or(DEFAULT_RUN_TIMEOUT_SECONDS);
    Duration::from_secs(secs)
}

fn append_capture_notice(output: &mut String, stream: &str, truncated: bool) {
    if !truncated {
        return;
    }
    if !output.ends_with('\n') && !output.is_empty() {
        output.push('\n');
    }
    output.push_str(&format!(
        "[chaos-scheduler] {stream} truncated at capture limit\n"
    ));
}

fn read_capped<R: Read>(mut reader: R, limit: usize) -> (Vec<u8>, bool) {
    let mut captured = Vec::with_capacity(limit.min(8192));
    let mut truncated = false;
    let mut buf = [0_u8; 8192];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let remaining = limit.saturating_sub(captured.len());
                if remaining > 0 {
                    let keep = remaining.min(n);
                    captured.extend_from_slice(&buf[..keep]);
                    truncated |= keep < n;
                } else {
                    truncated = true;
                }
            }
            Err(_) => break,
        }
    }
    (captured, truncated)
}

#[cfg(unix)]
fn process_start_time_fingerprint(pid: u32) -> Option<String> {
    let output = Command::new("ps")
        .args(["-o", "lstart=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

#[cfg(unix)]
fn process_is_alive(pid: i64) -> bool {
    pid > 0 && unsafe { libc::kill(pid as i32, 0) } == 0
}

#[cfg(unix)]
fn process_group_is_alive(pgid: i64) -> bool {
    pgid > 0 && unsafe { libc::kill(-(pgid as i32), 0) } == 0
}

#[cfg(unix)]
fn terminate_process_group(pgid: i64, grace: Duration) {
    if pgid <= 0 {
        return;
    }
    unsafe {
        libc::kill(-(pgid as i32), libc::SIGTERM);
    }
    let deadline = Instant::now() + grace;
    while Instant::now() < deadline {
        if !process_group_is_alive(pgid) {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    unsafe {
        libc::kill(-(pgid as i32), libc::SIGKILL);
    }
}

#[cfg(unix)]
fn run_workflow_command(
    mut cmd: Command,
    sample_metadata: Option<ResourceSampleMetadata>,
    timeout: Duration,
) -> std::io::Result<WorkflowCommandOutput> {
    let mut fds = [0; 2];
    let pipe_result = unsafe { libc::pipe(fds.as_mut_ptr()) };
    if pipe_result == -1 {
        return Err(std::io::Error::last_os_error());
    }
    let read_fd = fds[0];
    let write_fd = fds[1];

    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("CHAOS_SCHEDULER_TASK_CHANNEL_FD", "3")
        .env("CHAOS_LABS_TASK_CHANNEL_FD", "3");
    unsafe {
        cmd.pre_exec(move || {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::dup2(write_fd, 3) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            if write_fd != 3 {
                libc::close(write_fd);
            }
            if read_fd != 3 {
                libc::close(read_fd);
            }
            Ok(())
        });
    }

    let child = cmd.spawn();
    unsafe {
        libc::close(write_fd);
    }
    let mut child = match child {
        Ok(child) => child,
        Err(e) => {
            unsafe {
                libc::close(read_fd);
            }
            return Err(e);
        }
    };

    let pid = child.id();
    let pgid = pid as i64;
    if let Some(metadata) = sample_metadata.as_ref() {
        let started_at = process_start_time_fingerprint(pid);
        let _ = metadata.db.record_run_process(
            &metadata.run_id,
            pid as i64,
            pgid,
            started_at.as_deref(),
        );
    }

    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");
    let stdout_reader = std::thread::spawn(move || read_capped(stdout, OUTPUT_CAPTURE_LIMIT_BYTES));
    let stderr_reader = std::thread::spawn(move || read_capped(stderr, OUTPUT_CAPTURE_LIMIT_BYTES));
    let task_reader = std::thread::spawn(move || {
        let file = unsafe { std::fs::File::from_raw_fd(read_fd) };
        read_capped(file, TASK_EVENT_CAPTURE_LIMIT_BYTES)
    });

    let sampler = sample_metadata.map(|metadata| spawn_resource_sampler(metadata, pid));
    let deadline = Instant::now() + timeout;
    let mut timed_out = false;
    let mut cancelled = false;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if SHUTDOWN.load(Ordering::Relaxed) {
            cancelled = true;
            terminate_process_group(pgid, PROCESS_SHUTDOWN_GRACE);
            break child
                .wait()
                .unwrap_or_else(|_| ExitStatus::from_raw(libc::SIGKILL));
        }
        if Instant::now() >= deadline {
            timed_out = true;
            terminate_process_group(pgid, PROCESS_SHUTDOWN_GRACE);
            break child
                .wait()
                .unwrap_or_else(|_| ExitStatus::from_raw(libc::SIGKILL));
        }
        std::thread::sleep(Duration::from_millis(50));
    };

    stop_resource_sampler(sampler);
    let (stdout, stdout_truncated) = stdout_reader.join().unwrap_or_default();
    let (stderr, stderr_truncated) = stderr_reader.join().unwrap_or_default();
    let (task_events, task_events_truncated) = task_reader.join().unwrap_or_default();
    let task_events_raw = String::from_utf8_lossy(&task_events).to_string();

    Ok(WorkflowCommandOutput {
        status,
        stdout,
        stderr,
        task_events_raw,
        timed_out,
        cancelled,
        stdout_truncated,
        stderr_truncated,
        task_events_truncated,
    })
}

#[cfg(not(unix))]
fn run_workflow_command(
    mut cmd: Command,
    _sample_metadata: Option<ResourceSampleMetadata>,
    _timeout: Duration,
) -> std::io::Result<WorkflowCommandOutput> {
    let output = cmd.output()?;
    let (stdout, stdout_truncated) =
        read_capped(output.stdout.as_slice(), OUTPUT_CAPTURE_LIMIT_BYTES);
    let (stderr, stderr_truncated) =
        read_capped(output.stderr.as_slice(), OUTPUT_CAPTURE_LIMIT_BYTES);
    Ok(WorkflowCommandOutput {
        status: output.status,
        stdout,
        stderr,
        task_events_raw: String::new(),
        timed_out: false,
        cancelled: false,
        stdout_truncated,
        stderr_truncated,
        task_events_truncated: false,
    })
}

fn spawn_resource_sampler(
    metadata: ResourceSampleMetadata,
    root_pid: u32,
) -> ResourceSamplerHandle {
    let (stop_tx, stop_rx) = mpsc::channel();
    let join = std::thread::spawn(move || loop {
        persist_resource_sample(&metadata, root_pid);
        match stop_rx.recv_timeout(RESOURCE_SAMPLE_INTERVAL) {
            Ok(_) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
        }
    });
    ResourceSamplerHandle { stop_tx, join }
}

fn stop_resource_sampler(handle: Option<ResourceSamplerHandle>) {
    if let Some(handle) = handle {
        let _ = handle.stop_tx.send(());
        let _ = handle.join.join();
    }
}

fn persist_resource_sample(metadata: &ResourceSampleMetadata, root_pid: u32) {
    let Some(sample) = collect_resource_sample(metadata, root_pid) else {
        return;
    };
    if let Err(err) = metadata.db.insert_workflow_resource_sample(&sample) {
        log::warn!(
            "Failed to persist workflow resource sample for {}: {}",
            metadata.workflow_id,
            err
        );
    }
}

fn collect_resource_sample(
    metadata: &ResourceSampleMetadata,
    root_pid: u32,
) -> Option<WorkflowResourceSample> {
    let processes = read_process_table().ok()?;
    let selected = select_process_tree(root_pid as i64, &processes);
    if selected.is_empty()
        || !processes
            .iter()
            .any(|process| process.pid == root_pid as i64)
    {
        return None;
    }
    let mut cpu_percent = 0.0;
    let mut memory_rss_bytes = 0_i64;
    let mut memory_vms_bytes = 0_i64;
    for process in processes.iter().filter(|p| selected.contains(&p.pid)) {
        cpu_percent += process.cpu_percent;
        memory_rss_bytes += process.rss_kb.saturating_mul(1024);
        memory_vms_bytes += process.vms_kb.saturating_mul(1024);
    }
    Some(WorkflowResourceSample {
        id: String::new(),
        run_id: Some(metadata.run_id.clone()),
        workflow_id: metadata.workflow_id.clone(),
        queue_name: metadata.queue_name.clone(),
        environment: metadata.environment.clone(),
        pid: Some(root_pid as i64),
        sampled_at: Utc::now().to_rfc3339(),
        cpu_percent: Some(cpu_percent),
        memory_rss_bytes: Some(memory_rss_bytes),
        memory_vms_bytes: Some(memory_vms_bytes),
        swap_bytes: None,
        labels: Some(serde_json::json!({
            "root_pid": root_pid,
            "pid_count": selected.len(),
            "sampler": "ps-pid-tree-v1"
        })),
    })
}

fn read_process_table() -> std::io::Result<Vec<ProcessStat>> {
    let output = Command::new("ps")
        .args(["-axo", "pid=,ppid=,%cpu=,rss=,vsz="])
        .output()?;
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text.lines().filter_map(parse_process_stat).collect())
}

fn parse_process_stat(line: &str) -> Option<ProcessStat> {
    let mut parts = line.split_whitespace();
    let pid = parts.next()?.parse().ok()?;
    let ppid = parts.next()?.parse().ok()?;
    let cpu_percent = parts.next()?.parse().ok()?;
    let rss_kb = parts.next()?.parse().ok()?;
    let vms_kb = parts.next()?.parse().ok()?;
    Some(ProcessStat {
        pid,
        ppid,
        cpu_percent,
        rss_kb,
        vms_kb,
    })
}

fn select_process_tree(root_pid: i64, processes: &[ProcessStat]) -> HashSet<i64> {
    let mut selected = HashSet::from([root_pid]);
    loop {
        let before = selected.len();
        for process in processes {
            if selected.contains(&process.ppid) {
                selected.insert(process.pid);
            }
        }
        if selected.len() == before {
            break;
        }
    }
    selected
}

fn stdout_with_task_events(mut stdout: String, raw_events: &str) -> String {
    let events = valid_task_event_lines(raw_events);
    if events.is_empty() {
        return stdout;
    }
    if !stdout.ends_with('\n') {
        stdout.push('\n');
    }
    for event in events {
        stdout.push_str("TASK_EVENT_JSON: ");
        stdout.push_str(&event);
        stdout.push('\n');
    }
    stdout
}

fn valid_task_event_lines(raw_events: &str) -> Vec<String> {
    raw_events
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }
            let parsed: Value = serde_json::from_str(trimmed).ok()?;
            let has_required = parsed.get("ts").and_then(Value::as_str).is_some()
                && parsed.get("task_id").and_then(Value::as_str).is_some()
                && parsed.get("status").and_then(Value::as_str).is_some();
            has_required.then(|| parsed.to_string())
        })
        .collect()
}

fn persist_task_events(db: &Arc<Database>, run_id: &str, workflow_id: &str, raw_events: &str) {
    let mut attempts: HashMap<(String, i64), String> = HashMap::new();
    let mut task_rows: HashMap<(String, i64), String> = HashMap::new();

    for event_line in valid_task_event_lines(raw_events) {
        let Ok(event) = serde_json::from_str::<Value>(&event_line) else {
            continue;
        };
        let Some(task_id) = event.get("task_id").and_then(Value::as_str) else {
            continue;
        };
        let Some(status) = event.get("status").and_then(Value::as_str) else {
            continue;
        };
        let attempt_number = event.get("attempt").and_then(Value::as_i64).unwrap_or(0);
        let details = event.get("details").cloned();

        match status {
            "started" => {
                let _ = ensure_task_attempt(
                    db,
                    run_id,
                    task_id,
                    attempt_number,
                    details.as_ref(),
                    &mut attempts,
                    &mut task_rows,
                );
            }
            "failed" => {
                if let Ok((attempt_id, task_row_id)) = ensure_task_attempt(
                    db,
                    run_id,
                    task_id,
                    attempt_number,
                    details.as_ref(),
                    &mut attempts,
                    &mut task_rows,
                ) {
                    let error_type = details
                        .as_ref()
                        .and_then(|d| d.get("error_type"))
                        .and_then(Value::as_str);
                    let error_message = details
                        .as_ref()
                        .and_then(|d| d.get("error"))
                        .and_then(Value::as_str);
                    let _ = db.finish_run_attempt(
                        &attempt_id,
                        "failed",
                        None,
                        error_type,
                        error_message,
                    );
                    let _ = db.finish_run_task(
                        &task_row_id,
                        "failed",
                        error_type,
                        error_message,
                        details.as_ref(),
                    );
                }
            }
            "succeeded" => {
                if let Ok((attempt_id, task_row_id)) = ensure_task_attempt(
                    db,
                    run_id,
                    task_id,
                    attempt_number,
                    details.as_ref(),
                    &mut attempts,
                    &mut task_rows,
                ) {
                    let _ = db.finish_run_attempt(&attempt_id, "succeeded", Some(0), None, None);
                    let _ =
                        db.finish_run_task(&task_row_id, "succeeded", None, None, details.as_ref());
                }
            }
            "dead_lettered" => {
                let last_attempt_id = attempts
                    .get(&(task_id.to_string(), attempt_number))
                    .map(String::as_str);
                let last_exception = details
                    .as_ref()
                    .and_then(|d| d.get("error"))
                    .and_then(Value::as_str)
                    .or_else(|| {
                        details
                            .as_ref()
                            .and_then(|d| d.get("error_type"))
                            .and_then(Value::as_str)
                    })
                    .unwrap_or("task retry budget exhausted");
                let _ = db.upsert_scheduler_dead_letter(
                    run_id,
                    workflow_id,
                    Some(task_id),
                    last_attempt_id,
                    last_exception,
                );
            }
            "metric" => {
                if let Some(details) = details.as_ref() {
                    if let Some(metric_name) = details.get("metric_name").and_then(Value::as_str) {
                        if let Some(metric_value) =
                            details.get("metric_value").and_then(Value::as_f64)
                        {
                            let metric_unit = details.get("metric_unit").and_then(Value::as_str);
                            let labels = details.get("labels");
                            let _ = db.insert_run_metric(
                                run_id,
                                Some(task_id),
                                metric_name,
                                metric_value,
                                metric_unit,
                                labels,
                            );
                        }
                    }
                }
            }
            "asset_read" | "asset_written" => {
                if let Some(details) = details.as_ref() {
                    if let Some(asset) = details.get("asset") {
                        let asset_kind = asset.get("kind").and_then(Value::as_str);
                        let asset_namespace =
                            asset.get("namespace").and_then(Value::as_str).unwrap_or("");
                        let asset_partition =
                            asset.get("partition").and_then(Value::as_str).unwrap_or("");
                        if let Some(asset_kind) = asset_kind {
                            let attempt_id = ensure_task_attempt(
                                db,
                                run_id,
                                task_id,
                                attempt_number,
                                Some(details),
                                &mut attempts,
                                &mut task_rows,
                            )
                            .ok()
                            .map(|(attempt_id, _)| attempt_id);
                            let action = if status == "asset_read" {
                                "read"
                            } else {
                                "write"
                            };
                            let metadata = details.get("metadata");
                            let freshness_policy = asset.get("freshness_policy");
                            let _ = db.insert_run_asset_with_freshness(
                                run_id,
                                Some(task_id),
                                attempt_id.as_deref(),
                                asset_kind,
                                asset_namespace,
                                asset_partition,
                                action,
                                metadata,
                                freshness_policy,
                            );
                        }
                    }
                }
            }
            "lineage_event" => {
                if let Some(openlineage_event) =
                    details.as_ref().and_then(|d| d.get("openlineage_event"))
                {
                    let attempt_id = ensure_task_attempt(
                        db,
                        run_id,
                        task_id,
                        attempt_number,
                        details.as_ref(),
                        &mut attempts,
                        &mut task_rows,
                    )
                    .ok()
                    .map(|(attempt_id, _)| attempt_id);
                    let _ = db.insert_run_lineage(
                        run_id,
                        Some(task_id),
                        attempt_id.as_deref(),
                        openlineage_event,
                    );
                }
            }
            _ => {}
        }
    }
}

fn ensure_task_attempt(
    db: &Arc<Database>,
    run_id: &str,
    task_id: &str,
    attempt_number: i64,
    details: Option<&Value>,
    attempts: &mut HashMap<(String, i64), String>,
    task_rows: &mut HashMap<(String, i64), String>,
) -> Result<(String, String), String> {
    let key = (task_id.to_string(), attempt_number);
    if let (Some(attempt_id), Some(task_row_id)) = (attempts.get(&key), task_rows.get(&key)) {
        return Ok((attempt_id.clone(), task_row_id.clone()));
    }
    let retry_reason = (attempt_number > 0).then_some("retry");
    let attempt_id = db
        .insert_run_attempt(run_id, task_id, attempt_number, "running", retry_reason)
        .map_err(|e| format!("insert run attempt failed: {}", e))?;
    let task_row_id = db
        .insert_run_task(
            run_id,
            Some(&attempt_id),
            task_id,
            "started",
            attempt_number,
            details,
        )
        .map_err(|e| format!("insert run task failed: {}", e))?;
    attempts.insert(key.clone(), attempt_id.clone());
    task_rows.insert(key, task_row_id.clone());
    Ok((attempt_id, task_row_id))
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

fn capture_pid_identity(pid: u32) -> PidIdentity {
    PidIdentity {
        pid,
        start_time_ticks: process_start_time_ticks(pid),
    }
}

fn pid_identity_is_alive(identity: &PidIdentity) -> bool {
    let alive = unsafe { libc::kill(identity.pid as i32, 0) } == 0;
    if !alive {
        return false;
    }
    match identity.start_time_ticks {
        Some(expected) => process_start_time_ticks(identity.pid) == Some(expected),
        None => true,
    }
}

#[cfg(target_os = "linux")]
fn process_start_time_ticks(pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // Field 2 may contain spaces inside parentheses; starttime is field 22.
    let after_name = stat.rsplit_once(") ")?.1;
    after_name.split_whitespace().nth(19)?.parse().ok()
}

#[cfg(not(target_os = "linux"))]
fn process_start_time_ticks(_pid: u32) -> Option<u64> {
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
#[allow(clippy::too_many_arguments)] // Threads full run context into the background monitor.
fn monitor_background_pid(
    pid_identity: PidIdentity,
    run_id: &str,
    wf_name: &str,
    wf_script: &str,
    chaos_labs_root: &str,
    bg_log_path: Option<&str>,
    log_start_offset: Option<u64>,
    email_enabled: bool,
    completion_email_enabled: bool,
    db: &Arc<Database>,
    python_path: &str,
    workflow_id: &str,
    sample_metadata: ResourceSampleMetadata,
    notify_on_success: bool,
    notify_on_failure: bool,
    app_handle: Option<tauri::AppHandle>,
) {
    log::info!(
        "Monitoring background PID {} for workflow '{}'",
        pid_identity.pid,
        wf_name
    );
    let sampler = spawn_resource_sampler(sample_metadata, pid_identity.pid);

    let mut exited = false;
    for _ in 0..BACKGROUND_MONITOR_MAX_POLLS {
        std::thread::sleep(Duration::from_secs(10));

        if SHUTDOWN.load(Ordering::Relaxed) {
            let _ = db.finish_run_with_status_details(
                run_id,
                None,
                "cancelled",
                "",
                "Background monitor cancelled during scheduler shutdown",
                None,
            );
            stop_resource_sampler(Some(sampler));
            return;
        }

        if !pid_identity_is_alive(&pid_identity) {
            exited = true;
            break;
        }
    }

    stop_resource_sampler(Some(sampler));

    log::info!(
        "Background PID {} for workflow '{}' has {}",
        pid_identity.pid,
        wf_name,
        if exited {
            "exited"
        } else {
            "exceeded monitor bound"
        }
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

    let success = if exited {
        let _ = db.finish_run(run_id, exit_code, &stdout, "", result_url.as_deref());
        exit_code == 0
    } else {
        let stderr = format!(
            "background PID monitor exhausted after {} polls for pid {}",
            BACKGROUND_MONITOR_MAX_POLLS, pid_identity.pid
        );
        let _ = db.finish_run_with_status_details(
            run_id,
            None,
            "stale",
            &stdout,
            &stderr,
            result_url.as_deref(),
        );
        false
    };

    let result = RunResult {
        run_id: run_id.to_string(),
        workflow_name: wf_name.to_string(),
        script_path: wf_script.to_string(),
        success,
        completed: true,
        should_notify: if success {
            notify_on_success
        } else {
            notify_on_failure
        },
        email_on_failure: true,
    };
    if !dispatch_completion_actions(db, app_handle.as_ref(), &result) {
        if result.should_notify {
            if let Some(app_handle) = app_handle.as_ref() {
                send_notification(app_handle, &result);
            }
        }
        if !success && email_enabled {
            send_failure_email(db, chaos_labs_root, &result);
        }
    }

    trigger_on_completion(
        db,
        chaos_labs_root,
        python_path,
        workflow_id,
        run_id,
        success,
        notify_on_success,
        notify_on_failure,
        completion_email_enabled,
    );
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
    if parsed.run_id.as_deref() != Some(run_id) {
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
    if stdout.trim().is_empty() {
        return 1;
    }
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

fn short_id(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

/// On startup, close admitted/running records left by a previous process without
/// inferring success from shared logs. If a persisted PID/start-time proves that
/// a child survived the app, terminate its process group and mark the run stale.
fn recover_orphaned_runs(db: &Database, _chaos_labs_root: &str) {
    let active = match db.get_active_execution_runs() {
        Ok(r) => r,
        Err(e) => {
            log::warn!("Failed to check for orphaned runs: {}", e);
            return;
        }
    };

    if active.is_empty() {
        return;
    }

    log::info!(
        "Found {} active run(s) from a previous scheduler session",
        active.len()
    );

    for run in &active {
        let wf_name = run.workflow_name.as_deref().unwrap_or("unknown");
        let (status, stderr) = if run.status == "admitted" {
            (
                "stale",
                "Scheduler restarted before a worker claimed this admitted run".to_string(),
            )
        } else if let Some(pid) = run.process_pid {
            let live = process_is_alive(pid);
            let start_matches = run
                .process_started_at
                .as_deref()
                .and_then(|expected| {
                    process_start_time_fingerprint(pid as u32).map(|actual| actual == expected)
                })
                .unwrap_or(false);
            if live && start_matches {
                if let Some(pgid) = run.process_pgid {
                    terminate_process_group(pgid, PROCESS_SHUTDOWN_GRACE);
                }
                (
                    "stale",
                    format!(
                        "Scheduler restarted; verified orphan PID {pid} for workflow {} and terminated its process group",
                        run.workflow_id
                    ),
                )
            } else {
                (
                    "stale",
                    format!(
                        "Scheduler restarted; active run had {} process metadata for PID {pid}",
                        if live { "mismatched" } else { "dead" }
                    ),
                )
            }
        } else {
            (
                "stale",
                "Scheduler restarted with no process metadata to reattach safely".to_string(),
            )
        };
        let _ = db.finish_run_with_status_details(&run.id, None, status, "", &stderr, None);
        log::info!(
            "Recovered active run {} ({}) as {}",
            short_id(&run.id),
            wf_name,
            status
        );
    }
}

struct PendingWorkflowGuard {
    pending: Arc<Mutex<HashSet<String>>>,
    key: String,
}

impl PendingWorkflowGuard {
    fn new(pending: Arc<Mutex<HashSet<String>>>, key: String) -> Self {
        Self { pending, key }
    }
}

impl Drop for PendingWorkflowGuard {
    fn drop(&mut self) {
        if let Ok(mut pending) = self.pending.lock() {
            pending.remove(&self.key);
        }
    }
}

type SchedulerJob = Box<dyn FnOnce() + Send + 'static>;

enum WorkerMessage {
    Job(SchedulerJob),
    Shutdown,
}

struct SchedulerWorkerPool {
    tx: mpsc::SyncSender<WorkerMessage>,
    handles: Vec<std::thread::JoinHandle<()>>,
}

impl SchedulerWorkerPool {
    fn new(worker_count: usize, queue_capacity: usize) -> Self {
        let (tx, rx) = mpsc::sync_channel(queue_capacity.max(worker_count));
        let rx = Arc::new(Mutex::new(rx));
        let mut handles = Vec::with_capacity(worker_count);
        for idx in 0..worker_count {
            let rx = Arc::clone(&rx);
            let handle = std::thread::Builder::new()
                .name(format!("scheduler-worker-{idx}"))
                .spawn(move || loop {
                    let message = match rx.lock() {
                        Ok(guard) => guard.recv(),
                        Err(_) => return,
                    };
                    match message {
                        Ok(WorkerMessage::Job(job)) => {
                            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(job)).is_err()
                            {
                                log::error!("scheduler worker {idx} isolated a panicking job");
                            }
                        }
                        Ok(WorkerMessage::Shutdown) | Err(_) => break,
                    }
                })
                .expect("failed to spawn scheduler worker");
            handles.push(handle);
        }
        Self { tx, handles }
    }

    fn submit(&self, job: SchedulerJob) -> bool {
        match self.tx.try_send(WorkerMessage::Job(job)) {
            Ok(()) => true,
            Err(mpsc::TrySendError::Full(_)) => false,
            Err(mpsc::TrySendError::Disconnected(_)) => false,
        }
    }

    fn shutdown(self) {
        for _ in &self.handles {
            let _ = self.tx.send(WorkerMessage::Shutdown);
        }
        for handle in self.handles {
            let _ = handle.join();
        }
    }
}

fn scheduler_worker_count() -> usize {
    std::env::var("CHAOS_SCHEDULER_WORKER_COUNT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|count| *count > 0)
        .unwrap_or(DEFAULT_WORKER_COUNT)
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
        // M6: reclaim any throwaway `chaos-fix/*` worktrees/branches stranded by
        // a crash/kill mid-fix, so a fresh session starts clean and a
        // re-dispatch of the same source run is not blocked by stale state.
        crate::fix_worktree::sweep_orphaned_fix_worktrees(
            &crate::service::SystemProcessRunner,
            &chaos_labs_root,
        );
        // M6: a fix source rerun may have been QUEUED (at capacity / mutex-busy)
        // when the crash hit. Its worktree was just reclaimed above, so cancel
        // the stale queued rerun too — otherwise it would drain later and
        // fail closed against the missing worktree.
        if let Err(e) =
            db.cancel_orphaned_fix_rerun_queued_runs(crate::service::FIX_RERUN_TRIGGER_KIND)
        {
            log::warn!("Failed to cancel orphaned fix-rerun queued runs on startup: {e}");
        }
        // M6: a crash mid-fix also strands the durable single-flight CLAIM (the
        // orchestrator thread that would have rolled it back died with the
        // process). Clear non-terminal LOCAL claims so the source run can be
        // re-dispatched instead of being permanently blocked as a "duplicate".
        match db.clear_orphaned_local_fix_dispatches() {
            Ok(n) if n > 0 => {
                log::info!("Cleared {n} orphaned local fix-agent claim(s) on startup")
            }
            Ok(_) => {}
            Err(e) => log::warn!("Failed to clear orphaned local fix-agent claims on startup: {e}"),
        }
        let worker_count = scheduler_worker_count();
        let worker_pool = SchedulerWorkerPool::new(
            worker_count,
            worker_count.saturating_mul(WORKER_QUEUE_MULTIPLIER).max(1),
        );
        let pending_workflows = Arc::new(Mutex::new(HashSet::<String>::new()));

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("Failed to create scheduler runtime");

        rt.block_on(async {
            // Queue-occupancy sampler cadence + retention. Additive observability
            // that piggybacks on the scheduler tick; FLAGGED for review.
            const QUEUE_SAMPLE_INTERVAL: Duration = Duration::from_secs(60);
            const QUEUE_SAMPLE_RETENTION: &str = "-30 days";
            let mut last_queue_sample: Option<std::time::Instant> = None;
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;

                if SHUTDOWN.load(Ordering::Relaxed) {
                    log::info!("Scheduler shutting down gracefully");
                    break;
                }

                // Periodic queue-occupancy sampling + retention pruning. Appends
                // observability rows only; never affects scheduling behavior.
                let should_sample = match last_queue_sample {
                    Some(t) => t.elapsed() >= QUEUE_SAMPLE_INTERVAL,
                    None => true,
                };
                if should_sample {
                    last_queue_sample = Some(std::time::Instant::now());
                    match db.sample_queue_occupancy() {
                        Ok(count) => {
                            log::debug!("Sampled queue occupancy for {count} queue(s)");
                            if let Err(err) =
                                db.prune_queue_occupancy_samples(QUEUE_SAMPLE_RETENTION)
                            {
                                log::warn!("Failed to prune queue occupancy samples: {err}");
                            }
                        }
                        Err(err) => log::warn!("Queue occupancy sampling failed: {err}"),
                    }
                }

                let (due, notify_success, notify_failure, email_enabled) = {
                    let sched = match scheduler.lock() {
                        Ok(sched) => sched,
                        Err(poisoned) => {
                            log::error!("Scheduler mutex was poisoned; continuing with recovered state");
                            poisoned.into_inner()
                        }
                    };
                    let due = sched.find_due_workflows(&chaos_labs_root);
                    let ns = sched.notify_on_success.load(Ordering::Relaxed);
                    let nf = sched.notify_on_failure.load(Ordering::Relaxed);
                    let ef = sched.should_email_on_failure();
                    (due, ns, nf, ef)
                };

                for wf in due {
                    let pending_key = due_workflow_pending_key(&wf);
                    let should_submit = match pending_workflows.lock() {
                        Ok(mut pending) => pending.insert(pending_key.clone()),
                        Err(_) => false,
                    };
                    if !should_submit {
                        continue;
                    }

                    let db = Arc::clone(&db);
                    let root = chaos_labs_root.clone();
                    let python = python_path.clone();
                    let app = app_handle.clone();
                    let pending = Arc::clone(&pending_workflows);
                    let pending_key_for_guard = pending_key.clone();
                    let pending_key_for_submit = pending_key.clone();
                    let workflow_id = wf.id.clone();
                    let workflow_id_for_log = workflow_id.clone();
                    let job = Box::new(move || {
                        let _pending_guard = PendingWorkflowGuard::new(pending, pending_key_for_guard);
                        match execute_workflow_with_context(
                            &db,
                            &root,
                            &python,
                            &workflow_id,
                            notify_success,
                            notify_failure,
                            email_enabled,
                            wf.trigger_kind.as_deref(),
                            wf.trigger_payload.as_deref(),
                            wf.upstream_run_id.as_deref(),
                            wf.input_json.as_deref(),
                            wf.rerun_of_run_id.as_deref(),
                            wf.queued_run_id.as_deref(),
                            wf.suppress_completion_triggers,
                            Some(app.clone()),
                        ) {
                            Ok(result) => {
                                // M5: a drained run that asked to suppress
                                // completion chains (persisted on the queued
                                // row, v16) must NOT cascade downstream — the
                                // intent is honored here, not just on the inline
                                // dispatch path.
                                if result.completed && !wf.suppress_completion_triggers {
                                    trigger_on_completion(
                                        &db,
                                        &root,
                                        &python,
                                        &workflow_id,
                                        &result.run_id,
                                        result.success,
                                        notify_success,
                                        notify_failure,
                                        email_enabled,
                                    );
                                }
                                if !dispatch_completion_actions(&db, Some(&app), &result) {
                                    if result.should_notify {
                                        send_notification(&app, &result);
                                    }
                                    if email_enabled && !result.success && result.email_on_failure {
                                        send_failure_email(&db, &root, &result);
                                    }
                                }
                            }
                            Err(e) => log::error!("Workflow {} failed: {}", workflow_id, e),
                        }
                    });
                    if !worker_pool.submit(job) {
                        if let Ok(mut pending) = pending_workflows.lock() {
                            pending.remove(&pending_key_for_submit);
                        }
                        log::warn!(
                            "Scheduler worker queue is full; workflow {} will be retried on a later tick",
                            workflow_id_for_log
                        );
                    }
                }
                send_sla_notifications(&app_handle, &db);
            }
        });

        worker_pool.shutdown();
    });
}

/// Bridges the action framework's [`Notifier`](crate::service::Notifier) to
/// Tauri's notification plugin from within the scheduler threads.
struct SchedulerNotifier {
    app: Option<tauri::AppHandle>,
}

impl crate::service::Notifier for SchedulerNotifier {
    fn notify(&self, title: &str, body: &str) {
        if let Some(app) = &self.app {
            use tauri_plugin_notification::NotificationExt;
            if let Err(e) = app.notification().builder().title(title).body(body).show() {
                log::warn!("Failed to send desktop notification: {e}");
            }
        }
    }
}

/// Enqueue a chained workflow requested by an `on_success`/`on_failure`
/// `run_workflow` action, so the normal scheduler loop admits it.
fn enqueue_chained_workflow(
    db: &Arc<Database>,
    workflow_id: &str,
    upstream_run: &Run,
    upstream_workflow: &Workflow,
) {
    match db.get_workflow(workflow_id) {
        Ok(target) => {
            let chain = CompletionChain::from_trigger_payload(
                upstream_run.trigger_payload.as_deref(),
                &upstream_workflow.id,
            );
            let next_chain = match chain.try_advance(&target.id) {
                Ok(next_chain) => next_chain,
                Err(reason) => {
                    log::warn!(
                        "Skipping run_workflow action target {} from {}: {}",
                        target.id,
                        upstream_workflow.id,
                        reason
                    );
                    return;
                }
            };
            let qc = parse_queue_config(target.queue_config.as_deref(), &target.environment);
            let payload =
                run_workflow_action_payload(&upstream_workflow.id, &upstream_run.id, &next_chain);
            if let Err(e) = db.upsert_queued_run_with_context(
                &target.id,
                &qc.queue,
                qc.priority,
                Some("run_workflow_action"),
                Some(&payload),
                Some(&upstream_run.id),
                None,
                None,
                false,
            ) {
                log::warn!("run_workflow action failed to enqueue {workflow_id}: {e}");
            }
        }
        Err(_) => log::warn!("run_workflow action references unknown workflow {workflow_id}"),
    }
}

/// Compute the effective completion actions for a run: the spec's
/// success/failure list, plus (on failure) a default `email` action when the
/// legacy `email_on_failure` flag is set with no explicit email action and email
/// is configured, plus a `desktop_notification` when global prefs request one.
fn select_completion_actions(
    on_success: &[crate::actions::ActionSpec],
    on_failure: &[crate::actions::ActionSpec],
    success: bool,
    should_notify: bool,
    email_on_failure: bool,
    email_configured: bool,
) -> Vec<crate::actions::ActionSpec> {
    use crate::actions::ActionSpec;
    let mut actions = if success {
        on_success.to_vec()
    } else {
        on_failure.to_vec()
    };
    if !success
        && email_on_failure
        && email_configured
        && !actions
            .iter()
            .any(|a| matches!(a, ActionSpec::Email { .. }))
    {
        actions.push(ActionSpec::Email { to: None });
    }
    if should_notify
        && !actions
            .iter()
            .any(|a| matches!(a, ActionSpec::DesktopNotification { .. }))
    {
        actions.push(ActionSpec::DesktopNotification { title: None });
    }
    actions
}

/// Dispatch on-success / on-failure actions for a completed run.
///
/// Returns `true` if the run belongs to a workflow with a spec (actions were
/// evaluated here — the caller must NOT also run the legacy notify/email path).
/// Returns `false` for legacy single-script workflows so the caller keeps their
/// existing notification/email behavior. Legacy `email_on_failure=true` is
/// migrated to a default `on_failure:[email]` for spec workflows.
fn dispatch_completion_actions(
    db: &Arc<Database>,
    app_handle: Option<&tauri::AppHandle>,
    result: &RunResult,
) -> bool {
    dispatch_completion_actions_impl(db, app_handle, result, true)
}

#[cfg(test)]
fn dispatch_completion_actions_sync(
    db: &Arc<Database>,
    app_handle: Option<&tauri::AppHandle>,
    result: &RunResult,
) -> bool {
    dispatch_completion_actions_impl(db, app_handle, result, false)
}

fn dispatch_completion_actions_impl(
    db: &Arc<Database>,
    app_handle: Option<&tauri::AppHandle>,
    result: &RunResult,
    async_dispatch: bool,
) -> bool {
    let Ok(run) = db.get_run(&result.run_id) else {
        return false;
    };
    let Ok(workflow) = db.get_workflow(&run.workflow_id) else {
        return false;
    };
    let Some(spec) = workflow
        .spec_json
        .as_deref()
        .and_then(|j| crate::workflow_spec::WorkflowSpec::from_json(j).ok())
    else {
        return false;
    };

    let email_configured = db
        .resolve_email_config(workflow.email_profile_id.as_deref())
        .map(|c| c.enabled && !c.alert_email.trim().is_empty())
        .unwrap_or(false);
    let actions = select_completion_actions(
        &spec.on_success,
        &spec.on_failure,
        result.success,
        result.should_notify,
        workflow.email_on_failure,
        email_configured,
    );

    if !actions.is_empty() {
        let db = Arc::clone(db);
        let app_handle = app_handle.cloned();
        let success = result.success;
        let dispatch = move || {
            let notifier: Arc<dyn crate::service::Notifier> =
                Arc::new(SchedulerNotifier { app: app_handle });
            let payload = serde_json::json!({
                "workflow_id": workflow.id.clone(),
                "workflow_name": workflow.name.clone(),
                "run_id": run.id.clone(),
                "status": run.status.clone(),
                "exit_code": run.exit_code,
                "stdout": run.stdout.clone(),
                "stderr": run.stderr.clone(),
                "started_at": run.started_at.clone(),
                "finished_at": run.finished_at.clone(),
            });
            let ctx = crate::actions::ActionContext {
                db: Arc::clone(&db),
                notifier,
                workflow_name: workflow.name.clone(),
                run_id: run.id.clone(),
                success,
                result_payload: payload,
                email_profile_id: workflow.email_profile_id.clone(),
            };
            for outcome in crate::actions::dispatch_actions_with_budget(
                &actions,
                &ctx,
                crate::actions::ACTION_DISPATCH_TOTAL_BUDGET,
            ) {
                if !outcome.success {
                    log::warn!(
                        "on-completion action '{}' failed for run {}: {}",
                        outcome.kind,
                        run.id,
                        outcome.message
                    );
                }
            }

            for action in &actions {
                if let crate::actions::ActionSpec::RunWorkflow { workflow_id, .. } = action {
                    enqueue_chained_workflow(&db, workflow_id, &run, &workflow);
                }
            }
        };

        if async_dispatch {
            std::thread::spawn(dispatch);
        } else {
            dispatch();
        }
    }
    true
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

fn send_sla_notifications(app: &tauri::AppHandle, db: &Arc<Database>) {
    use tauri_plugin_notification::NotificationExt;

    let Ok(violations) = db.evaluate_sla_violations() else {
        return;
    };
    let cache = SLA_NOTIFICATION_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut cache) = cache.lock() else {
        return;
    };
    let now = Utc::now().timestamp();
    for violation in violations {
        let key = format!("{}:{}", violation.workflow_id, violation.violation_type);
        if cache
            .get(&key)
            .map(|last| now - *last < 4 * 60 * 60)
            .unwrap_or(false)
        {
            continue;
        }
        cache.insert(key, now);
        if let Err(e) = app
            .notification()
            .builder()
            .title("Scheduler SLA violation")
            .body(&violation.message)
            .show()
        {
            log::warn!("Failed to send SLA notification: {}", e);
        }
    }
}

fn send_failure_email(db: &Database, _workspace_root: &str, result: &RunResult) {
    let run = match db.get_run(&result.run_id) {
        Ok(r) => r,
        Err(e) => {
            log::warn!("Failed to fetch run for email alert: {}", e);
            return;
        }
    };

    // Resolve the workflow's selected email profile (if any); a missing
    // profile falls back to the global email config.
    let profile_id = db
        .get_workflow(&run.workflow_id)
        .ok()
        .and_then(|w| w.email_profile_id);
    let config = match db.resolve_email_config(profile_id.as_deref()) {
        Ok(c) if c.enabled && !c.alert_email.is_empty() => c,
        _ => return,
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

    match crate::commands::send_email_alert(&config, Some(&run_context), "alert") {
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
    let plist_path = format!("{}/{}.plist", plist_dir, SCHEDULER_BUNDLE_ID);

    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
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
        SCHEDULER_BUNDLE_ID, app_path
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
        "{}/Library/LaunchAgents/{}.plist",
        home, SCHEDULER_BUNDLE_ID
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

    /// Regression test for the `cursor_agent` cloud-mode hang: a
    /// `reqwest::blocking::Client` that makes a real request and is then
    /// dropped while running directly inside a tokio multi-thread runtime's
    /// `block_on` (exactly how `execute_typed_operator` is reached from the
    /// REST API's axum handler, see `api::start_api_server`) panics on drop.
    /// `run_possibly_blocking` must route that work through
    /// `tokio::task::block_in_place` to avoid it.
    #[test]
    fn run_possibly_blocking_avoids_reqwest_blocking_drop_panic_on_tokio_worker() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let body = b"ok";
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(resp.as_bytes());
                let _ = stream.write_all(body);
            }
        });
        let make_request_and_drop = move || {
            let client = reqwest::blocking::Client::new();
            for _ in 0..3 {
                let _ = client.get(format!("http://{addr}")).send();
            }
            drop(client);
        };

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();

        // Sanity check: the *unguarded* call reproduces the real-world panic
        // when run directly on a tokio worker (proves this test actually
        // exercises the bug, not just the fix's happy path).
        let unguarded = runtime.block_on(async {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(make_request_and_drop))
        });
        assert!(
            unguarded.is_err(),
            "expected the unguarded blocking client to panic on drop inside a tokio worker \
             (if this starts failing, reqwest/tokio may have changed and the guard may be \
             obsolete or need revisiting)"
        );

        // The fix: routed through `run_possibly_blocking`, no panic.
        let guarded = runtime.block_on(async {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run_possibly_blocking(make_request_and_drop)
            }))
        });
        assert!(
            guarded.is_ok(),
            "run_possibly_blocking should prevent the reqwest::blocking drop panic: {guarded:?}"
        );
    }

    fn structured_test_db() -> (Arc<Database>, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("chaos-sched-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        (Arc::new(Database::new(&dir)), dir)
    }

    fn make_generic_workflow(db: &Arc<Database>, spec_json: &str) -> String {
        let wf = db
            .create_workflow(
                "Structured",
                None,
                "unused-for-step-flow",
                "0 0 * * *",
                false,
                false,
                "UTC",
                "production",
                None,
                None,
                None,
            )
            .unwrap();
        db.set_workflow_spec(&wf.id, "generic", Some(spec_json))
            .unwrap();
        wf.id
    }

    #[test]
    #[cfg(unix)]
    fn worker_pool_isolates_panicking_jobs() {
        let pool = SchedulerWorkerPool::new(1, 2);
        let (tx, rx) = mpsc::channel();
        assert!(pool.submit(Box::new(|| panic!("intentional worker panic"))));
        assert!(pool.submit(Box::new(move || tx.send(()).unwrap())));
        rx.recv_timeout(Duration::from_secs(2))
            .expect("worker should keep accepting jobs after a panic");
        pool.shutdown();
    }

    #[test]
    #[cfg(unix)]
    fn shutdown_kills_in_flight_child_within_grace() {
        let _guard = lock_shutdown_test_state();
        let dir =
            std::env::temp_dir().join(format!("chaos-shutdown-kill-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let child_pid_path = dir.join("child.pid");
        let script = format!(
            "(sleep 30) & echo $! > {}; wait",
            child_pid_path.to_string_lossy()
        );
        let script_for_thread = script.clone();
        let handle = std::thread::spawn(move || {
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg(&script_for_thread);
            run_workflow_command(cmd, None, Duration::from_secs(60))
        });
        std::thread::sleep(Duration::from_millis(150));
        initiate_shutdown();
        let output = handle
            .join()
            .expect("workflow thread")
            .expect("command output");
        assert!(output.cancelled, "shutdown should cancel in-flight command");
        std::thread::sleep(Duration::from_millis(250));
        let child_pid: i64 = std::fs::read_to_string(&child_pid_path)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert!(
            !process_is_alive(child_pid),
            "shutdown should kill descendants in the child process group"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn claim_exit_shutdown_is_idempotent() {
        let _guard = lock_shutdown_test_state();
        assert!(claim_exit_shutdown());
        assert!(!claim_exit_shutdown());
        assert!(!claim_exit_shutdown());
    }

    #[test]
    fn sleep_interruptible_returns_early_on_shutdown() {
        let _guard = lock_shutdown_test_state();
        let handle = std::thread::spawn(|| sleep_interruptible(Duration::from_secs(30)));
        std::thread::sleep(Duration::from_millis(50));
        initiate_shutdown();
        let start = Instant::now();
        handle.join().expect("sleep thread");
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "interruptible sleep should return promptly after SHUTDOWN"
        );
    }

    #[test]
    #[cfg(unix)]
    fn run_command_timeout_kills_process_group() {
        let _guard = lock_shutdown_test_state();
        let dir = std::env::temp_dir().join(format!("chaos-timeout-kill-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let child_pid_path = dir.join("child.pid");
        let script = format!(
            "(sleep 30) & echo $! > {}; wait",
            child_pid_path.to_string_lossy()
        );
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(script);

        let output = run_workflow_command(cmd, None, Duration::from_millis(200)).unwrap();
        assert!(output.timed_out, "command should report a timeout");
        std::thread::sleep(Duration::from_millis(250));
        let child_pid: i64 = std::fs::read_to_string(&child_pid_path)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert!(
            !process_is_alive(child_pid),
            "timeout should kill descendants in the child process group"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    #[cfg(unix)]
    fn run_command_caps_stdout_stderr_and_fd3() {
        let _guard = lock_shutdown_test_state();
        let script = format!(
            "import os,sys; sys.stdout.write('o' * {}); sys.stderr.write('e' * {}); os.write(3, b't' * {})",
            OUTPUT_CAPTURE_LIMIT_BYTES + 1024,
            OUTPUT_CAPTURE_LIMIT_BYTES + 2048,
            TASK_EVENT_CAPTURE_LIMIT_BYTES + 512,
        );
        let mut cmd = Command::new("python3");
        cmd.arg("-c").arg(script);

        let output = run_workflow_command(cmd, None, Duration::from_secs(5)).unwrap();
        assert_eq!(output.stdout.len(), OUTPUT_CAPTURE_LIMIT_BYTES);
        assert_eq!(output.stderr.len(), OUTPUT_CAPTURE_LIMIT_BYTES);
        assert_eq!(output.task_events_raw.len(), TASK_EVENT_CAPTURE_LIMIT_BYTES);
        assert!(output.stdout_truncated);
        assert!(output.stderr_truncated);
        assert!(output.task_events_truncated);
    }

    #[test]
    fn operator_run_terminal_status_maps_poll_exhausted() {
        let exhausted = crate::operators::OperatorOutcome {
            success: false,
            summary: "poll done".into(),
            details: serde_json::json!({"status": "POLL_EXHAUSTED"}),
        };
        assert_eq!(
            operator_run_terminal_status(&exhausted),
            Some("poll_exhausted")
        );

        let plain_failure = crate::operators::OperatorOutcome {
            success: false,
            summary: "boom".into(),
            details: serde_json::json!({"status": "FAILED"}),
        };
        assert_eq!(operator_run_terminal_status(&plain_failure), None);

        let success = crate::operators::OperatorOutcome {
            success: true,
            summary: "ok".into(),
            details: serde_json::json!({}),
        };
        assert_eq!(operator_run_terminal_status(&success), None);
    }

    #[test]
    fn terminal_status_gates_canonicalize_poll_exhausted_stale_and_timed_out() {
        // Regression: the scheduler's terminal-status gates and the KPI roll-ups
        // historically disagreed on `poll_exhausted`/`stale`/`timed_out`, so a
        // run that ended one of those ways could leave a `waits_for`/`depends_on`
        // downstream waiting forever and stay invisible to the dashboards. All
        // three are terminal, non-success statuses and must be treated as such
        // by both gates, which now delegate to the canonical `run_status` set.
        for status in [
            "poll_exhausted",
            "timed_out",
            "stale",
            "failed",
            "cancelled",
            "cascade-skipped",
            "dead_letter",
            "dead_lettered",
        ] {
            assert!(is_terminal_status(status), "{status} must be terminal");
            assert!(
                is_failure_terminal(status),
                "{status} must be a non-success terminal status"
            );
        }
        // Success stays terminal but is NOT a failure.
        for status in ["success", "succeeded"] {
            assert!(is_terminal_status(status), "{status} is terminal");
            assert!(!is_failure_terminal(status), "{status} is not a failure");
        }
        // In-flight statuses are neither terminal nor failures.
        for status in ["running", "admitted"] {
            assert!(!is_terminal_status(status), "{status} is in-flight");
            assert!(!is_failure_terminal(status), "{status} is in-flight");
        }
    }

    #[test]
    fn finish_run_persists_poll_exhausted_run_row_status() {
        let (db, dir) = structured_test_db();
        let wf = db
            .create_workflow(
                "Poll",
                None,
                "scripts/workflows/noop.py",
                "0 0 * * *",
                false,
                true,
                "UTC",
                "production",
                None,
                None,
                None,
            )
            .unwrap();
        let admitted = db
            .admit_run_with_context(
                &wf.id,
                "production-default",
                "production",
                &[],
                Some("manual"),
                None,
                None,
                None,
                None,
                None,
                &[],
                None,
            )
            .unwrap();
        let run_id = match admitted {
            RunAdmission::Admitted(run) => run.id,
            _ => panic!("run should admit"),
        };
        db.finish_run_with_status_details(&run_id, Some(1), "poll_exhausted", "", "poll", None)
            .unwrap();
        let run = db.get_run(&run_id).unwrap();
        assert_eq!(run.status, "poll_exhausted");
        assert_eq!(run.exit_code, Some(1));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn scrub_scheduler_secrets_from_child_removes_denied_keys() {
        // Shares the global test lock: mutates process env + reads std::env::vars().
        let _guard = lock_shutdown_test_state();
        std::env::set_var("CURSOR_API_KEY", "fake-cursor-key-for-test");
        std::env::set_var("SMTP_PASSWORD", "smtp-secret");
        std::env::set_var("CHAOS_SCHEDULER_API_TOKEN", "api-token");
        let mut cmd = Command::new("true");
        cmd.env("PATH", "/usr/bin");
        scrub_scheduler_secrets_from_child(&mut cmd);
        let is_removed = |key: &str| cmd.get_envs().any(|(k, v)| k == key && v.is_none());
        assert!(is_removed("CURSOR_API_KEY"));
        assert!(is_removed("SMTP_PASSWORD"));
        assert!(is_removed("CHAOS_SCHEDULER_API_TOKEN"));
        assert_eq!(
            cmd.get_envs()
                .find(|(k, _)| *k == "PATH")
                .and_then(|(_, v)| v)
                .and_then(|v| v.to_str()),
            Some("/usr/bin")
        );
        std::env::remove_var("CURSOR_API_KEY");
        std::env::remove_var("SMTP_PASSWORD");
        std::env::remove_var("CHAOS_SCHEDULER_API_TOKEN");
    }

    #[test]
    #[cfg(unix)]
    fn child_process_does_not_inherit_scrubbed_scheduler_secrets() {
        let _guard = lock_shutdown_test_state();
        std::env::set_var("CURSOR_API_KEY", "fake-cursor-key-for-test");
        let cmd = build_workflow_command(
            "[ \"${CURSOR_API_KEY:-}\" = \"\" ] && echo MISSING || echo LEAKED",
            "/tmp",
            "python3",
            "run-1",
            "wf-1",
            "production-default",
            "production",
            None,
            "/tmp/db.sqlite",
            None,
        );
        let output = run_workflow_command(cmd, None, Duration::from_secs(5)).unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("MISSING"), "stdout was: {stdout}");
        assert!(std::env::var("PATH").is_ok());
        std::env::remove_var("CURSOR_API_KEY");
    }

    #[test]
    #[cfg(unix)]
    fn orphan_recovery_marks_unclaimed_admitted_runs_stale() {
        let (db, dir) = structured_test_db();
        let wf = db
            .create_workflow(
                "Orphan Candidate",
                None,
                "scripts/workflows/noop.py",
                "0 0 * * *",
                false,
                true,
                "UTC",
                "production",
                None,
                None,
                None,
            )
            .unwrap();
        let admitted = db
            .admit_run_with_context(
                &wf.id,
                "production-default",
                "production",
                &[],
                Some("manual"),
                None,
                None,
                None,
                None,
                None,
                &[],
                None,
            )
            .unwrap();
        let run_id = match admitted {
            RunAdmission::Admitted(run) => run.id,
            _ => panic!("run should admit"),
        };

        recover_orphaned_runs(&db, dir.to_str().unwrap());
        let recovered = db.get_run(&run_id).unwrap();
        assert_eq!(recovered.status, "stale");
        assert!(recovered
            .stderr
            .as_deref()
            .unwrap_or_default()
            .contains("before a worker claimed"));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Admit a run and mark it `running` so orphan recovery evaluates its
    /// process metadata rather than the unclaimed-admitted fast path.
    #[cfg(unix)]
    fn admit_running_orphan_run(db: &Arc<Database>, workflow_id: &str) -> String {
        let admitted = db
            .admit_run_with_context(
                workflow_id,
                "production-default",
                "production",
                &[],
                Some("manual"),
                None,
                None,
                None,
                None,
                None,
                &[],
                None,
            )
            .unwrap();
        let run_id = match admitted {
            RunAdmission::Admitted(run) => run.id,
            _ => panic!("run should admit"),
        };
        db.mark_run_started(&run_id, "worker-orphan-test").unwrap();
        run_id
    }

    #[cfg(unix)]
    fn make_orphan_workflow(db: &Arc<Database>, name: &str) -> Workflow {
        db.create_workflow(
            name,
            None,
            "scripts/workflows/noop.py",
            "0 0 * * *",
            false,
            true,
            "UTC",
            "production",
            None,
            None,
            None,
        )
        .unwrap()
    }

    /// Spawn a child in its OWN session/process-group (mirrors the scheduler's
    /// `setsid` in `run_workflow_command`) so a group-kill can never reach the
    /// test runner. Returns `(child, pid)` where `pgid == pid`.
    #[cfg(unix)]
    fn spawn_own_group_child() -> (std::process::Child, i64) {
        let mut cmd = Command::new("sleep");
        cmd.arg("30");
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = cmd.spawn().expect("spawn sleep child");
        let pid = child.id() as i64;
        (child, pid)
    }

    #[test]
    #[cfg(unix)]
    fn orphan_recovery_terminates_verified_live_orphan() {
        let (db, dir) = structured_test_db();
        let wf = make_orphan_workflow(&db, "Live Orphan");
        let run_id = admit_running_orphan_run(&db, &wf.id);

        let (mut child, pid) = spawn_own_group_child();
        let pgid = pid; // setsid => process-group leader, pgid == pid
        let started = process_start_time_fingerprint(pid as u32);
        assert!(started.is_some(), "ps lstart fingerprint should be present");
        assert!(process_is_alive(pid), "child should be alive pre-recovery");
        db.record_run_process(&run_id, pid, pgid, started.as_deref())
            .unwrap();

        recover_orphaned_runs(&db, dir.to_str().unwrap());

        let recovered = db.get_run(&run_id).unwrap();
        assert_eq!(recovered.status, "stale");
        let stderr = recovered.stderr.as_deref().unwrap_or_default();
        assert!(
            stderr.contains("verified orphan PID")
                && stderr.contains("terminated its process group"),
            "unexpected stderr: {stderr}"
        );
        // Recovery must have signalled the verified orphan dead.
        let status = child.wait().unwrap();
        assert!(
            status.signal().is_some(),
            "verified orphan should have been killed by a signal"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    #[cfg(unix)]
    fn orphan_recovery_marks_dead_pid_stale() {
        let (db, dir) = structured_test_db();
        let wf = make_orphan_workflow(&db, "Dead PID Orphan");
        let run_id = admit_running_orphan_run(&db, &wf.id);

        // Spawn then immediately reap so the recorded PID is no longer alive.
        let mut child = Command::new("true").spawn().expect("spawn true");
        let pid = child.id() as i64;
        child.wait().unwrap();
        assert!(!process_is_alive(pid), "reaped PID should be dead");
        db.record_run_process(&run_id, pid, pid, Some("Fri Jan  1 00:00:00 2021"))
            .unwrap();

        recover_orphaned_runs(&db, dir.to_str().unwrap());

        let recovered = db.get_run(&run_id).unwrap();
        assert_eq!(recovered.status, "stale");
        let stderr = recovered.stderr.as_deref().unwrap_or_default();
        assert!(stderr.contains("dead"), "unexpected stderr: {stderr}");
        assert!(
            !stderr.contains("terminated its process group"),
            "dead PID must not trigger a group kill: {stderr}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    #[cfg(unix)]
    fn orphan_recovery_marks_pid_recycle_mismatch_stale_without_kill() {
        let (db, dir) = structured_test_db();
        let wf = make_orphan_workflow(&db, "Recycled PID Orphan");
        let run_id = admit_running_orphan_run(&db, &wf.id);

        let (mut child, pid) = spawn_own_group_child();
        // Live PID but a start-time fingerprint that can never match a real
        // `ps lstart` => the PID has been recycled to an unrelated process.
        db.record_run_process(&run_id, pid, pid, Some("Thu Jan  1 00:00:00 1970"))
            .unwrap();
        assert!(process_is_alive(pid), "child should be alive pre-recovery");

        recover_orphaned_runs(&db, dir.to_str().unwrap());

        let recovered = db.get_run(&run_id).unwrap();
        assert_eq!(recovered.status, "stale");
        let stderr = recovered.stderr.as_deref().unwrap_or_default();
        assert!(stderr.contains("mismatched"), "unexpected stderr: {stderr}");
        // A recycle mismatch must NOT kill the unrelated live process.
        assert!(
            process_is_alive(pid),
            "recycle-mismatch PID must be left running"
        );

        unsafe {
            libc::kill(pid as i32, libc::SIGKILL);
        }
        let _ = child.wait();
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Typed operators like `cursor_agent` (cloud mode) never call
    /// `record_run_process` — there's no local child process, just HTTP
    /// polling of a remote agent — so a run left `running` by a killed
    /// scheduler has no PID to verify. Orphan recovery already handles this
    /// generically (it operates on any `running` row, not just ones with
    /// process metadata): confirm it marks such a run `stale` with a clear
    /// message rather than leaving it `running` forever.
    #[test]
    #[cfg(unix)]
    fn orphan_recovery_marks_running_run_without_process_metadata_stale() {
        let (db, dir) = structured_test_db();
        let wf = make_orphan_workflow(&db, "No Process Metadata Orphan");
        let run_id = admit_running_orphan_run(&db, &wf.id);
        // No `record_run_process` call: mirrors a typed cloud operator run.

        recover_orphaned_runs(&db, dir.to_str().unwrap());

        let recovered = db.get_run(&run_id).unwrap();
        assert_eq!(recovered.status, "stale");
        let stderr = recovered.stderr.as_deref().unwrap_or_default();
        assert!(
            stderr.contains("no process metadata to reattach safely"),
            "unexpected stderr: {stderr}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Regression test for the "no traceable record if killed mid-poll" gap:
    /// `execute_typed_operator` must insert the `run_tasks` row *before*
    /// calling the operator, and the operator's `on_progress` callback must
    /// persist remote identifiers (agent/run id) before its poll loop starts
    /// — proven here by having the mock's `get_json` (the poll call) assert
    /// the DB already reflects them, i.e. the write happened strictly earlier
    /// than any poll GET, not just at the end of execution.
    #[test]
    #[cfg(unix)]
    fn cursor_agent_on_progress_persists_ids_before_first_poll_get() {
        use crate::operators::{
            CursorAgentOperator, HttpClient, HttpResponse, Operator, OperatorContext,
            SecretResolver,
        };

        struct AssertingMockHttp {
            db: Arc<Database>,
            task_row_id: String,
            run_id: String,
            launch: Value,
            poll: Value,
        }
        impl HttpClient for AssertingMockHttp {
            fn post_json(
                &self,
                _url: &str,
                _headers: &[(String, String)],
                _body: &Value,
            ) -> Result<HttpResponse, String> {
                Ok(HttpResponse {
                    status: 200,
                    body: self.launch.clone(),
                })
            }
            fn get_json(
                &self,
                _url: &str,
                _headers: &[(String, String)],
            ) -> Result<HttpResponse, String> {
                let tasks = self.db.get_run_tasks(&self.run_id).unwrap();
                let task = tasks
                    .iter()
                    .find(|t| t.id == self.task_row_id)
                    .expect("task row must already exist");
                let details = task.details.clone().unwrap_or(Value::Null);
                assert_eq!(
                    details.get("agent_id").and_then(|v| v.as_str()),
                    Some("bc_progress"),
                    "agent_id must be persisted before the first poll GET, got: {details}"
                );
                assert_eq!(
                    details.get("run_id").and_then(|v| v.as_str()),
                    Some("run_progress"),
                    "run_id must be persisted before the first poll GET, got: {details}"
                );
                Ok(HttpResponse {
                    status: 200,
                    body: self.poll.clone(),
                })
            }
        }
        struct MapSecrets(HashMap<String, String>);
        impl SecretResolver for MapSecrets {
            fn get(&self, key: &str) -> Option<String> {
                self.0.get(key).cloned()
            }
        }

        // This test drives the operator poll loop, which checks the
        // process-global SHUTDOWN flag at the top of every iteration. Without
        // this guard, a parallel test that flips SHUTDOWN true (e.g.
        // `shutdown_interrupts_cursor_agent_poll_promptly`) makes this loop
        // break with POLL_EXHAUSTED before the first GET — so the assertions
        // inside `get_json` never run and `outcome.success` is false. The
        // guard both serializes against those tests and resets SHUTDOWN false.
        let _shutdown_guard = lock_shutdown_test_state();

        let (db, dir) = structured_test_db();
        let wf = make_orphan_workflow(&db, "Cursor Progress");
        let run_id = admit_running_orphan_run(&db, &wf.id);
        let task_row_id = db
            .insert_run_task(&run_id, None, "cursor_agent", "running", 0, None)
            .unwrap();

        let progress_db = Arc::clone(&db);
        let progress_task_row_id = task_row_id.clone();
        let on_progress = move |details: &Value| {
            let _ = progress_db.update_run_task_details(&progress_task_row_id, details);
        };
        let runner = crate::service::SystemProcessRunner;
        let http = AssertingMockHttp {
            db: Arc::clone(&db),
            task_row_id: task_row_id.clone(),
            run_id: run_id.clone(),
            launch: serde_json::json!({"agent": {"id": "bc_progress"}, "run": {"id": "run_progress", "status": "RUNNING"}}),
            poll: serde_json::json!({"id": "run_progress", "status": "FINISHED", "result": "done"}),
        };
        let mut secrets_map = HashMap::new();
        secrets_map.insert(
            "cursor_api_key".to_string(),
            "sk-progress-secret".to_string(),
        );
        let secrets = MapSecrets(secrets_map);
        let ctx = OperatorContext {
            runner: &runner,
            http: &http,
            secrets: &secrets,
            workspace_root: "/tmp",
            on_progress: &on_progress,
        };
        let outcome = CursorAgentOperator.execute(
            &ctx,
            &serde_json::json!({
                "mode": "cloud",
                "prompt": "p",
                "repository": "acme/repo",
                "poll_interval_ms": 0,
            }),
        );
        assert!(outcome.success, "{}", outcome.summary);

        // The secret must never have been written into the progress details.
        let tasks = db.get_run_tasks(&run_id).unwrap();
        let task = tasks.iter().find(|t| t.id == task_row_id).unwrap();
        let details_str = task
            .details
            .as_ref()
            .map(|v| v.to_string())
            .unwrap_or_default();
        assert!(!details_str.contains("sk-progress-secret"));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// The insert-before/finish-after `run_tasks` change applies to every
    /// typed operator (not just `cursor_agent`); confirm it still records
    /// exactly one row per run (no duplicate from insert-then-insert-again)
    /// with the final status and error details attached, using `git_pull`
    /// (no network required — it fails fast on a missing `repo_url`).
    #[test]
    #[cfg(unix)]
    fn execute_typed_operator_records_single_task_row_with_final_status_and_error() {
        let (db, dir) = structured_test_db();
        let wf = make_orphan_workflow(&db, "Typed GitPull");
        let run_id = admit_running_orphan_run(&db, &wf.id);
        let workflow = db.get_workflow(&wf.id).unwrap();
        let typed = crate::workflow_spec::TypedSpec {
            operator_type: "git_pull".to_string(),
            config: serde_json::json!({
                "path": format!("/tmp/does-not-exist-{}", uuid::Uuid::new_v4())
            }),
        };

        let (exit_code, _stdout, stderr, terminal) =
            execute_typed_operator(&db, "/tmp", &run_id, &workflow, &typed, None, None);
        assert_ne!(exit_code, 0);
        assert!(terminal.is_none());
        assert!(
            stderr.contains("no repo_url"),
            "unexpected stderr: {stderr}"
        );

        let tasks = db.get_run_tasks(&run_id).unwrap();
        assert_eq!(
            tasks.len(),
            1,
            "exactly one run_task row, not a duplicate from insert-then-insert"
        );
        let task = &tasks[0];
        assert_eq!(task.status, "failed");
        assert_eq!(task.error_type.as_deref(), Some("OperatorError"));
        assert!(task
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("no repo_url"));
        assert!(task.finished_at.is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    // ---- D05 / F10: fix-agent operator-config overlay (seam) -------------

    #[test]
    fn fix_agent_overlay_whitelists_prompt_and_forces_no_auto_pr() {
        // OPTION C (race-free): the fix-agent seam FORCES `auto_create_pr = false`
        // so the cloud agent only PUSHES its `cursor/…` branch and opens NO PR —
        // the scheduler then opens a born-DRAFT PR. Attacker-influenced input
        // cannot flip this: it tries to FORCE the agent to open the PR itself
        // (`auto_create_pr:true` — which would be born NON-draft and could
        // auto-merge), push straight to an existing branch
        // (`workOnCurrentBranch:true`), and redirect the repository — all must be
        // ignored (strict prompt-only whitelist), and `auto_create_pr` is forced
        // false by the backend regardless of stored OR input.
        let stored = serde_json::json!({
            "repository": "https://github.com/owner/repo",
            "prompt": "STORED PROMPT",
            "auto_create_pr": true,
            "api_key_secret": "cursor_api_key"
        });
        let input = r#"{"prompt":"DIAGNOSTIC PROMPT","auto_create_pr":true,"workOnCurrentBranch":true,"repository":"https://github.com/attacker/evil"}"#;

        let effective = fix_agent_config_overlay(
            "cursor_agent",
            Some(crate::service::FIX_AGENT_TRIGGER_KIND),
            Some(input),
            &stored,
        );

        // Prompt is overlaid from input...
        assert_eq!(
            effective.get("prompt").and_then(|v| v.as_str()),
            Some("DIAGNOSTIC PROMPT")
        );
        // ...auto_create_pr is FORCED false despite stored+input asking for true
        // (the agent never opens the PR itself — the scheduler opens a born-draft
        // PR against the pushed branch)...
        assert_eq!(
            effective.get("auto_create_pr").and_then(|v| v.as_bool()),
            Some(false)
        );
        // ...repository stays the STORED value (input's is ignored — whitelist)...
        assert_eq!(
            effective.get("repository").and_then(|v| v.as_str()),
            Some("https://github.com/owner/repo")
        );
        // ...the attacker's `workOnCurrentBranch` never reaches the config (only
        // `prompt` is whitelisted), so the agent pushes to a NEW branch...
        assert!(effective.get("workOnCurrentBranch").is_none());
        // ...and no attacker-supplied field leaked in.
        let serialized = effective.to_string();
        assert!(!serialized.contains("attacker/evil"));
        // api_key_secret comes untouched from stored config.
        assert_eq!(
            effective.get("api_key_secret").and_then(|v| v.as_str()),
            Some("cursor_api_key")
        );
    }

    #[test]
    fn fix_agent_overlay_does_not_hijack_a_cursor_agent_rerun() {
        // M2 regression: a NON-`ui_fix_agent` dispatch (e.g. rerun) carrying
        // `input_json.prompt` must NOT have its stored prompt overlaid.
        let stored = serde_json::json!({
            "repository": "https://github.com/owner/repo",
            "prompt": "STORED PROMPT",
            "auto_create_pr": true
        });
        let input = r#"{"prompt":"INJECTED PROMPT"}"#;

        let effective =
            fix_agent_config_overlay("cursor_agent", Some("ui_rerun"), Some(input), &stored);

        // Returned UNCHANGED: stored prompt + auto_create_pr preserved.
        assert!(matches!(effective, std::borrow::Cow::Borrowed(_)));
        assert_eq!(
            effective.get("prompt").and_then(|v| v.as_str()),
            Some("STORED PROMPT")
        );
        assert_eq!(
            effective.get("auto_create_pr").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn fix_agent_overlay_ignores_non_cursor_agent_operators() {
        // Even with the fix-agent trigger kind, a different operator is untouched.
        let stored = serde_json::json!({ "path": "/repo", "auto_create_pr": true });
        let input = r#"{"prompt":"x"}"#;

        let effective = fix_agent_config_overlay(
            "git_pull",
            Some(crate::service::FIX_AGENT_TRIGGER_KIND),
            Some(input),
            &stored,
        );

        assert!(matches!(effective, std::borrow::Cow::Borrowed(_)));
        assert_eq!(
            effective.get("auto_create_pr").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn fix_agent_overlay_forces_no_auto_pr_without_input_prompt() {
        // A fix-agent dispatch with no usable input prompt still FORCES
        // `auto_create_pr = false` (the agent opens no PR; the scheduler opens
        // the born-draft PR) even when the stored config enabled it; the stored
        // prompt is left in place.
        let stored = serde_json::json!({
            "repository": "https://github.com/o/r",
            "prompt": "STORED",
            "auto_create_pr": true
        });

        let effective = fix_agent_config_overlay(
            "cursor_agent",
            Some(crate::service::FIX_AGENT_TRIGGER_KIND),
            None,
            &stored,
        );

        assert!(matches!(effective, std::borrow::Cow::Owned(_)));
        assert_eq!(
            effective.get("prompt").and_then(|v| v.as_str()),
            Some("STORED")
        );
        assert_eq!(
            effective.get("auto_create_pr").and_then(|v| v.as_bool()),
            Some(false)
        );
    }

    #[test]
    #[cfg(unix)]
    fn cloud_fix_draft_hardening_option_c_routes_primary_fallback_and_gate() {
        // D05 PR2e Option C seam: gated on the fix trigger kind + a `cursor_agent`
        // operator (the same M2 gate as the config overlay). Verify the routing:
        // (1) a non-fix trigger is a NO-OP (never runs gh, never annotates);
        // (2) PRIMARY — a fix dispatch with a pushed `cursor/…` branch + no
        //     pr_url has the SCHEDULER open a born-draft PR against the TARGET repo
        //     (`gh pr create -R <repo> --draft …`), records `opened_draft`, and
        //     BACKFILLS the top-level pr_url (Finding 1 + Finding 3);
        // (3) FALLBACK — a fix dispatch that UNEXPECTEDLY carries a pr_url (a
        //     future Cursor change ignored auto_create_pr=false) probes + converts
        //     the existing PR and records `converted_to_draft`;
        // (4) a fix dispatch that pushed nothing + opened no PR is a no-op;
        // (5) RECONCILE (Finding 2) — PRIMARY create FAILS because a PR already
        //     exists for the branch; the seam probes (`gh pr list`) + converts the
        //     orphaned non-draft PR and records `recovered_existing_converted_to_draft`.
        use crate::service::ProcessRunner;
        use std::os::unix::process::ExitStatusExt;
        use std::process::{ExitStatus, Output};
        use std::sync::Mutex;

        struct RecordingRunner {
            create_code: i32,
            create_stdout: String,
            list_stdout: String,
            view_stdout: String,
            calls: Mutex<Vec<Vec<String>>>,
        }
        impl RecordingRunner {
            fn new(
                create_code: i32,
                create_stdout: &str,
                list_stdout: &str,
                view_stdout: &str,
            ) -> Self {
                RecordingRunner {
                    create_code,
                    create_stdout: create_stdout.into(),
                    list_stdout: list_stdout.into(),
                    view_stdout: view_stdout.into(),
                    calls: Mutex::new(vec![]),
                }
            }
            fn argvs(&self) -> Vec<Vec<String>> {
                self.calls.lock().unwrap().clone()
            }
        }
        impl ProcessRunner for RecordingRunner {
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
                // `create` echoes its (optional) URL + scripted exit code; `list`
                // echoes the scripted probe JSON; `view … --jq .isDraft` the draft
                // state; `ready` succeeds with empty stdout.
                let (code, stdout) = if args.iter().any(|a| a == "create") {
                    (self.create_code, self.create_stdout.clone())
                } else if args.iter().any(|a| a == "list") {
                    (0, self.list_stdout.clone())
                } else if args.iter().any(|a| a == "view") {
                    (0, self.view_stdout.clone())
                } else {
                    (0, String::new())
                };
                Ok(Output {
                    status: ExitStatus::from_raw((code & 0xff) << 8),
                    stdout: stdout.into_bytes(),
                    stderr: Vec::new(),
                })
            }
        }

        // (1) A NON-fix trigger (even with a pr_url) must NOT run gh nor annotate.
        let runner = RecordingRunner::new(0, "", "", "false");
        let mut outcome = crate::operators::OperatorOutcome {
            success: true,
            summary: "s".into(),
            details: serde_json::json!({ "pr_url": "https://github.com/o/r/pull/7" }),
        };
        apply_cloud_fix_draft_hardening(
            &runner,
            Some("/tmp"),
            "cursor_agent",
            Some("ui_rerun"),
            "Fix WF",
            "run-1",
            &mut outcome,
        );
        assert!(
            runner.argvs().is_empty(),
            "a non-fix trigger must not run gh"
        );
        assert!(outcome.details.get("draft_hardening").is_none());

        // (2) PRIMARY: pushed cursor/ branch, no pr_url, a surfaced repo => the
        // scheduler opens the born-draft PR itself against `-R <repo>`, records
        // opened_draft, and backfills the top-level pr_url from create's stdout.
        let runner = RecordingRunner::new(0, "https://github.com/acme/app/pull/42\n", "", "false");
        let mut outcome = crate::operators::OperatorOutcome {
            success: true,
            summary: "s".into(),
            details: serde_json::json!({
                "pushed_branch": "cursor/fix-xyz", "pr_url": null, "repo": "acme/app"
            }),
        };
        apply_cloud_fix_draft_hardening(
            &runner,
            Some("/tmp"),
            "cursor_agent",
            Some(crate::service::FIX_AGENT_TRIGGER_KIND),
            "Fix WF",
            "run-2",
            &mut outcome,
        );
        let argvs = runner.argvs();
        assert_eq!(argvs.len(), 1, "the scheduler runs exactly one gh command");
        assert_eq!(
            &argvs[0][..9],
            &["gh", "pr", "create", "-R", "acme/app", "--draft", "--base", "main", "--head"],
            "the scheduler opens a born --draft PR against the TARGET repo (-R)"
        );
        assert_eq!(argvs[0][9], "cursor/fix-xyz", "…--head <pushed branch>");
        assert!(
            !argvs[0].iter().any(|a| a == "--auto" || a == "merge"),
            "the scheduler never arms auto-merge / merges"
        );
        assert_eq!(
            outcome.details["draft_hardening"]["cloud_pr_draft"],
            serde_json::json!("opened_draft")
        );
        assert_eq!(
            outcome.details["draft_hardening"]["branch"],
            serde_json::json!("cursor/fix-xyz")
        );
        assert_eq!(
            outcome.details["pr_url"],
            serde_json::json!("https://github.com/acme/app/pull/42"),
            "Finding 3: the opened PR's URL is backfilled to the top-level pr_url"
        );

        // (3) FALLBACK: an unexpected pr_url => probe + convert the EXISTING PR.
        let runner = RecordingRunner::new(0, "", "", "false");
        let mut outcome = crate::operators::OperatorOutcome {
            success: true,
            summary: "s".into(),
            details: serde_json::json!({ "pr_url": "https://github.com/o/r/pull/7" }),
        };
        apply_cloud_fix_draft_hardening(
            &runner,
            Some("/tmp"),
            "cursor_agent",
            Some(crate::service::FIX_AGENT_TRIGGER_KIND),
            "Fix WF",
            "run-3",
            &mut outcome,
        );
        let argvs = runner.argvs();
        assert!(
            argvs.iter().any(|a| a.iter().any(|x| x == "view")),
            "the fallback probes the existing PR draft state"
        );
        assert!(
            argvs.iter().all(|a| !a.iter().any(|x| x == "create")),
            "the fallback never opens a NEW PR (the PR already exists)"
        );
        assert_eq!(
            outcome.details["draft_hardening"]["cloud_pr_draft"],
            serde_json::json!("converted_to_draft")
        );

        // (4) Nothing pushed + no PR (poll-exhausted) => no-op.
        let runner = RecordingRunner::new(0, "", "", "false");
        let mut outcome = crate::operators::OperatorOutcome {
            success: false,
            summary: "s".into(),
            details: serde_json::json!({ "status": "POLL_EXHAUSTED" }),
        };
        apply_cloud_fix_draft_hardening(
            &runner,
            Some("/tmp"),
            "cursor_agent",
            Some(crate::service::FIX_AGENT_TRIGGER_KIND),
            "Fix WF",
            "run-4",
            &mut outcome,
        );
        assert!(
            runner.argvs().is_empty(),
            "no branch + no pr_url => nothing to do"
        );
        assert!(outcome.details.get("draft_hardening").is_none());

        // (5) RECONCILE (Finding 2): PRIMARY create FAILS (a PR already exists for
        // the branch — Cursor opened one despite auto_create_pr=false, no prUrl).
        // The seam probes `gh pr list`, finds the NON-draft PR, converts it, and
        // records a `recovered_existing_converted_to_draft` + backfills pr_url.
        let runner = RecordingRunner::new(
            1, // create fails ("a pull request already exists")
            "",
            r#"[{"number":7,"isDraft":false,"url":"https://github.com/acme/app/pull/7"}]"#,
            "false", // the found PR is a non-draft => convert
        );
        let mut outcome = crate::operators::OperatorOutcome {
            success: true,
            summary: "s".into(),
            details: serde_json::json!({
                "pushed_branch": "cursor/fix-xyz", "pr_url": null, "repo": "acme/app"
            }),
        };
        apply_cloud_fix_draft_hardening(
            &runner,
            Some("/tmp"),
            "cursor_agent",
            Some(crate::service::FIX_AGENT_TRIGGER_KIND),
            "Fix WF",
            "run-5",
            &mut outcome,
        );
        let argvs = runner.argvs();
        assert!(
            argvs.iter().any(|a| a.iter().any(|x| x == "create")),
            "the primary create was attempted (and failed)"
        );
        assert!(
            argvs.iter().any(|a| a.iter().any(|x| x == "list")),
            "on failure the seam probes for an existing PR (gh pr list)"
        );
        assert!(
            argvs.iter().any(|a| a.iter().any(|x| x == "ready")),
            "the found non-draft orphan is converted to a draft (gh pr ready --undo)"
        );
        assert_eq!(
            outcome.details["draft_hardening"]["cloud_pr_draft"],
            serde_json::json!("recovered_existing_converted_to_draft")
        );
        assert_eq!(
            outcome.details["pr_url"],
            serde_json::json!("https://github.com/acme/app/pull/7"),
            "the reconciled PR's URL is backfilled to the top-level pr_url"
        );
    }

    #[test]
    #[cfg(unix)]
    fn live_generic_step_flow_records_tasks_and_attempts() {
        let (db, dir) = structured_test_db();
        // Two steps: build (ok) -> test (ok), test depends on build.
        let spec = r#"{
            "kind":"generic",
            "generic":{"steps":[
                {"id":"build","command":"true","depends_on":[]},
                {"id":"test","command":"true","depends_on":["build"]}
            ]}
        }"#;
        let wf_id = make_generic_workflow(&db, spec);
        let result = execute_workflow_with_context(
            &db,
            "/tmp",
            "python3",
            &wf_id,
            false,
            false,
            false,
            Some("manual"),
            None,
            None,
            None,
            None,
            None,
            false,
            None,
        )
        .unwrap();
        assert!(result.completed);
        assert!(result.success, "all steps succeed => run succeeds");

        let tasks = db.get_run_tasks(&result.run_id).unwrap();
        let ids: std::collections::HashSet<String> =
            tasks.iter().map(|t| t.task_id.clone()).collect();
        assert!(ids.contains("build") && ids.contains("test"));
        assert!(tasks.iter().all(|t| t.status == "success"));
        assert!(!db.get_run_attempts(&result.run_id).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    #[cfg(unix)]
    fn live_generic_step_flow_fails_fast_and_skips_dependent() {
        let (db, dir) = structured_test_db();
        let spec = r#"{
            "kind":"generic",
            "generic":{"steps":[
                {"id":"a","command":"false","depends_on":[]},
                {"id":"b","command":"true","depends_on":["a"]}
            ]}
        }"#;
        let wf_id = make_generic_workflow(&db, spec);
        let result = execute_workflow_with_context(
            &db,
            "/tmp",
            "python3",
            &wf_id,
            false,
            false,
            false,
            Some("manual"),
            None,
            None,
            None,
            None,
            None,
            false,
            None,
        )
        .unwrap();
        assert!(result.completed);
        assert!(!result.success, "failed step => run fails");
        let tasks = db.get_run_tasks(&result.run_id).unwrap();
        let b = tasks.iter().find(|t| t.task_id == "b").unwrap();
        assert_eq!(b.status, "skipped");
        let a = tasks.iter().find(|t| t.task_id == "a").unwrap();
        assert_eq!(a.status, "failed");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn legacy_email_on_failure_injects_default_email_action() {
        use crate::actions::ActionSpec;
        // Failure + email_on_failure + configured + no explicit email => inject.
        let actions = select_completion_actions(&[], &[], false, false, true, true);
        assert!(actions
            .iter()
            .any(|a| matches!(a, ActionSpec::Email { .. })));

        // Not injected when email isn't configured.
        let actions = select_completion_actions(&[], &[], false, false, true, false);
        assert!(actions.is_empty());

        // Not duplicated when an explicit email already exists.
        let existing = vec![ActionSpec::Email {
            to: Some("x@y.com".into()),
        }];
        let actions = select_completion_actions(&[], &existing, false, false, true, true);
        assert_eq!(
            actions
                .iter()
                .filter(|a| matches!(a, ActionSpec::Email { .. }))
                .count(),
            1
        );

        // Success path never injects a failure email.
        let actions = select_completion_actions(&[], &[], true, false, true, true);
        assert!(actions.is_empty());
    }

    #[test]
    fn should_notify_injects_desktop_notification() {
        use crate::actions::ActionSpec;
        let actions = select_completion_actions(&[], &[], true, true, false, false);
        assert!(actions
            .iter()
            .any(|a| matches!(a, ActionSpec::DesktopNotification { .. })));
    }

    #[test]
    #[cfg(unix)]
    fn dispatch_completion_actions_returns_false_for_legacy_true_for_spec() {
        let (db, dir) = structured_test_db();
        // Legacy single-script workflow (no spec).
        let legacy = db
            .create_workflow(
                "Legacy",
                None,
                "scripts/x.py",
                "0 0 * * *",
                false,
                false,
                "UTC",
                "production",
                None,
                None,
                None,
            )
            .unwrap();
        let run = db
            .create_run_with_context(&legacy.id, Some("manual"), None, None, None, None)
            .unwrap();
        db.finish_run(&run.id, 0, "", "", None).unwrap();
        let result = RunResult {
            run_id: run.id.clone(),
            workflow_name: legacy.name.clone(),
            script_path: legacy.script_path.clone(),
            success: true,
            completed: true,
            should_notify: false,
            email_on_failure: false,
        };
        assert!(!dispatch_completion_actions_sync(&db, None, &result));

        // Spec workflow => handled here (returns true).
        let spec = r#"{"kind":"generic","generic":{"steps":[{"id":"s","command":"true"}]},"on_success":[{"type":"desktop_notification"}]}"#;
        let wf_id = make_generic_workflow(&db, spec);
        let run2 = db
            .create_run_with_context(&wf_id, Some("manual"), None, None, None, None)
            .unwrap();
        db.finish_run(&run2.id, 0, "", "", None).unwrap();
        let result2 = RunResult {
            run_id: run2.id.clone(),
            workflow_name: "Structured".into(),
            script_path: "unused-for-step-flow".into(),
            success: true,
            completed: true,
            should_notify: false,
            email_on_failure: false,
        };
        assert!(dispatch_completion_actions_sync(&db, None, &result2));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    #[cfg(unix)]
    fn run_workflow_action_enqueues_chain_target() {
        let (db, dir) = structured_test_db();
        let target = db
            .create_workflow(
                "Target",
                None,
                "scripts/t.py",
                "0 0 * * *",
                false,
                false,
                "UTC",
                "production",
                None,
                None,
                None,
            )
            .unwrap();
        let spec = format!(
            r#"{{"kind":"generic","generic":{{"steps":[{{"id":"s","command":"true"}}]}},"on_success":[{{"type":"run_workflow","workflow_id":"{}"}}]}}"#,
            target.id
        );
        let wf_id = make_generic_workflow(&db, &spec);
        let run = db
            .create_run_with_context(&wf_id, Some("manual"), None, None, None, None)
            .unwrap();
        db.finish_run(&run.id, 0, "", "", None).unwrap();
        let result = RunResult {
            run_id: run.id.clone(),
            workflow_name: "Structured".into(),
            script_path: "unused".into(),
            success: true,
            completed: true,
            should_notify: false,
            email_on_failure: false,
        };
        assert!(dispatch_completion_actions_sync(&db, None, &result));
        // The chain target should now be queued with chain metadata.
        let queued = db.list_queued_runs(50).unwrap();
        let chained = queued.iter().find(|q| q.workflow_id == target.id).unwrap();
        let payload: Value =
            serde_json::from_str(chained.trigger_payload.as_deref().unwrap()).unwrap();
        assert_eq!(payload["_chain"]["depth"], serde_json::json!(1));
        assert_eq!(
            payload["_chain"]["visited_workflow_ids"],
            serde_json::json!([wf_id, target.id])
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn completion_chain_rejects_cycles_and_depth_overflow() {
        let root = CompletionChain::root("wf-a");
        assert!(root.try_advance("wf-a").unwrap_err().contains("cycle"));

        let full = CompletionChain {
            visited_workflow_ids: (0..=COMPLETION_CHAIN_MAX_DEPTH)
                .map(|idx| format!("wf-{idx}"))
                .collect(),
        };
        assert_eq!(full.depth(), COMPLETION_CHAIN_MAX_DEPTH);
        assert!(full.try_advance("wf-overflow").unwrap_err().contains("max"));
    }

    #[test]
    #[cfg(unix)]
    fn run_workflow_action_skips_self_cycle() {
        let (db, dir) = structured_test_db();
        let wf = db
            .create_workflow(
                "Self Cycle",
                None,
                "scripts/self.py",
                "0 0 * * *",
                false,
                false,
                "UTC",
                "production",
                None,
                None,
                None,
            )
            .unwrap();
        let spec = format!(
            r#"{{"kind":"generic","generic":{{"steps":[{{"id":"s","command":"true"}}]}},"on_success":[{{"type":"run_workflow","workflow_id":"{}"}}]}}"#,
            wf.id
        );
        db.set_workflow_spec(&wf.id, "generic", Some(&spec))
            .unwrap();
        let run = db
            .create_run_with_context(&wf.id, Some("manual"), None, None, None, None)
            .unwrap();
        db.finish_run(&run.id, 0, "", "", None).unwrap();
        let result = RunResult {
            run_id: run.id.clone(),
            workflow_name: wf.name.clone(),
            script_path: wf.script_path.clone(),
            success: true,
            completed: true,
            should_notify: false,
            email_on_failure: false,
        };

        assert!(dispatch_completion_actions_sync(&db, None, &result));

        let queued = db.list_queued_runs(50).unwrap();
        assert!(!queued.iter().any(|q| q.workflow_id == wf.id));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    #[cfg(unix)]
    fn live_typed_operator_unknown_type_fails_run() {
        let (db, dir) = structured_test_db();
        let wf = db
            .create_workflow(
                "Typed",
                None,
                "unused",
                "0 0 * * *",
                false,
                false,
                "UTC",
                "production",
                None,
                None,
                None,
            )
            .unwrap();
        let spec = r#"{"kind":"typed","typed":{"operator_type":"does_not_exist","config":{}}}"#;
        db.set_workflow_spec(&wf.id, "typed", Some(spec)).unwrap();
        let result = execute_workflow_with_context(
            &db,
            "/tmp",
            "python3",
            &wf.id,
            false,
            false,
            false,
            Some("manual"),
            None,
            None,
            None,
            None,
            None,
            false,
            None,
        )
        .unwrap();
        assert!(result.completed && !result.success);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn build_workflow_command_dual_emits_new_and_legacy_env() {
        let cmd = build_workflow_command(
            "scripts/workflows/noop.py",
            "/tmp/workspace",
            "python3",
            "run-1",
            "wf-1",
            "production-default",
            "production",
            Some("owner"),
            "/tmp/scheduler.db",
            Some(r#"{"k":"v"}"#),
        );
        let envs: std::collections::HashMap<String, String> = cmd
            .get_envs()
            .filter_map(|(k, v)| {
                Some((
                    k.to_string_lossy().to_string(),
                    v?.to_string_lossy().to_string(),
                ))
            })
            .collect();
        // New canonical names.
        assert_eq!(
            envs.get("CHAOS_SCHEDULER_RUN_ID").map(String::as_str),
            Some("run-1")
        );
        assert_eq!(
            envs.get("CHAOS_SCHEDULER_ENVIRONMENT").map(String::as_str),
            Some("production")
        );
        assert_eq!(
            envs.get("CHAOS_SCHEDULER_WORKSPACE_ROOT")
                .map(String::as_str),
            Some("/tmp/workspace")
        );
        // Legacy names still dual-emitted.
        assert_eq!(
            envs.get("CHAOS_LABS_SCHEDULER_RUN_ID").map(String::as_str),
            Some("run-1")
        );
        assert_eq!(
            envs.get("CHAOS_LABS_SCHEDULER_CORPUS").map(String::as_str),
            Some("production")
        );
        assert_eq!(
            envs.get("CHAOS_LABS_ROOT").map(String::as_str),
            Some("/tmp/workspace")
        );
    }

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
        // New York is behind UTC, so 9:00 AM local maps to a later UTC
        // time-of-day (13:00 EDT / 14:00 EST) than UTC 9:00 (09:00). Compare the
        // UTC hour-of-day rather than the absolute instants: the two "next
        // Monday 9am" occurrences can otherwise land on different calendar weeks
        // depending on the current wall clock, which is irrelevant to the
        // timezone offset this test asserts.
        assert_eq!(utc_next.hour(), 9, "UTC 9:00 AM should map to 09:00 UTC");
        assert!(
            ny_next.hour() > utc_next.hour(),
            "New York 9:00 AM should map to a later UTC hour-of-day than UTC 9:00 AM"
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
    fn infer_exit_code_treats_empty_output_as_failure() {
        assert_eq!(infer_exit_code_from_current_output(" \n\t"), 1);
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

    #[test]
    fn run_scoped_status_requires_matching_run_id_field() {
        let dir = std::env::temp_dir().join(format!("chaos-run-status-{}", uuid::Uuid::new_v4()));
        let status_dir = dir.join("run-status");
        std::fs::create_dir_all(&status_dir).unwrap();
        std::fs::write(status_dir.join("run-a.json"), r#"{"exit_code":0}"#).unwrap();

        let parsed = read_run_scoped_exit_status_from_dir(dir.to_str().unwrap(), "run-a");
        let _ = std::fs::remove_dir_all(dir);

        assert!(parsed.is_none());
    }

    #[test]
    fn trigger_config_accepts_wrapped_trigger_list() {
        let triggers = parse_trigger_config(Some(
            r#"{"triggers":[{"kind":"file_arrival","path":"data/inbox/*.json"}]}"#,
        ));

        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers[0]["kind"], "file_arrival");
    }

    #[test]
    fn queue_config_defaults_to_corpus_queue() {
        let config = parse_queue_config(None, "production");

        assert_eq!(config.queue, "production-default");
        assert_eq!(config.priority, 0);
        assert!(config.depends_on.is_empty());
        assert!(config.waits_for.is_empty());
    }

    #[test]
    fn scheduler_skips_due_workflows_when_cap_lattice_invalid() {
        let dir = std::env::temp_dir().join(format!("chaos-cap-lattice-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Arc::new(Database::new(&dir));
        db.create_workflow(
            "Due Workflow",
            None,
            "scripts/workflows/noop.py",
            "* * * * *",
            false,
            true,
            "UTC",
            "production",
            None,
            None,
            Some(r#"{"queue":"production-default"}"#),
        )
        .unwrap();
        let conn = rusqlite::Connection::open(dir.join("scheduler.db")).unwrap();
        conn.execute(
            "UPDATE scheduler_config SET value = '2' WHERE key = 'global_parallelism_cap'",
            [],
        )
        .unwrap();

        let scheduler = WorkflowScheduler::new(db.clone());

        assert!(scheduler
            .find_due_workflows(dir.to_str().unwrap())
            .is_empty());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn queue_config_parses_dependency_fields() {
        let config = parse_queue_config(
            Some(
                r#"{"depends_on":["capture"],"waits_for":["gmail"],"queue":"source-heavy","priority":7}"#,
            ),
            "production",
        );

        assert_eq!(config.depends_on, vec!["capture"]);
        assert_eq!(config.waits_for, vec!["gmail"]);
        assert_eq!(config.queue, "source-heavy");
        assert_eq!(config.priority, 7);
    }

    #[test]
    fn task_event_lines_require_transition_fields() {
        let raw = r#"
{"schema_version":"scheduler.task_event.v1","ts":"2026-05-09T23:00:00Z","task_id":"discover","status":"started"}
{"schema_version":"scheduler.task_event.v1","task_id":"bad","status":"started"}
not json
"#;

        let events = valid_task_event_lines(raw);

        assert_eq!(events.len(), 1);
        assert!(events[0].contains("\"task_id\":\"discover\""));
    }

    #[test]
    fn stdout_appends_valid_task_event_lines() {
        let stdout = stdout_with_task_events(
            "Log: /tmp/run.log\n".to_string(),
            r#"{"ts":"2026-05-09T23:00:00Z","task_id":"summarize","status":"succeeded"}"#,
        );

        assert!(stdout.contains("TASK_EVENT_JSON:"));
        assert!(stdout.contains("\"task_id\":\"summarize\""));
    }

    #[test]
    fn task_events_persist_assets_and_lineage() {
        let dir = std::env::temp_dir().join(format!("chaos-task-assets-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Arc::new(Database::new(&dir));
        let workflow = db
            .create_workflow(
                "Asset Workflow",
                None,
                "scripts/workflows/noop.py",
                "0 0 * * *",
                false,
                true,
                "UTC",
                "production",
                None,
                None,
                Some(r#"{"queue":"production-default"}"#),
            )
            .unwrap();
        let run = db
            .create_run_with_context(&workflow.id, Some("manual"), None, None, None, None)
            .unwrap();
        let raw = r#"
{"schema_version":"scheduler.task_event.v1","ts":"2026-05-09T23:00:00Z","task_id":"discover","status":"started","attempt":0}
{"schema_version":"scheduler.task_event.v1","ts":"2026-05-09T23:00:01Z","task_id":"discover","status":"asset_written","attempt":0,"details":{"asset":{"kind":"source","namespace":"slack","partition":"C123"},"metadata":{"count":2}}}
{"schema_version":"scheduler.task_event.v1","ts":"2026-05-09T23:00:02Z","task_id":"discover","status":"lineage_event","attempt":0,"details":{"openlineage_event":{"eventType":"COMPLETE"}}}
"#;

        persist_task_events(&db, &run.id, &workflow.id, raw);

        let conn = rusqlite::Connection::open(dir.join("scheduler.db")).unwrap();
        let asset_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM run_assets WHERE run_id = ?1",
                [&run.id],
                |row| row.get(0),
            )
            .unwrap();
        let lineage_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM run_lineage WHERE run_id = ?1",
                [&run.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(asset_count, 1);
        assert_eq!(lineage_count, 1);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn asset_update_trigger_detects_new_matching_writes() {
        let dir =
            std::env::temp_dir().join(format!("chaos-asset-trigger-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Arc::new(Database::new(&dir));
        let upstream = db
            .create_workflow(
                "Capture",
                None,
                "scripts/workflows/capture.py",
                "0 0 * * *",
                false,
                true,
                "UTC",
                "production",
                None,
                None,
                Some(r#"{"queue":"production-default"}"#),
            )
            .unwrap();
        let downstream = db
            .create_workflow(
                "Refresh",
                None,
                "scripts/workflows/refresh.py",
                "0 0 * * *",
                false,
                true,
                "UTC",
                "production",
                None,
                Some(r#"{"triggers":[{"kind":"asset_update","asset":{"kind":"source","namespace":"slack","partition":"C123"}}]}"#),
                Some(r#"{"queue":"production-default"}"#),
            )
            .unwrap();
        let run = db
            .create_run_with_context(&upstream.id, Some("manual"), None, None, None, None)
            .unwrap();
        db.insert_run_asset(
            &run.id,
            Some("discover"),
            None,
            "source",
            "slack",
            "C123",
            "write",
            None,
        )
        .unwrap();

        let scheduler = WorkflowScheduler::new(db.clone());
        let due = scheduler.find_due_workflows(dir.to_str().unwrap());

        let downstream_candidate = due
            .iter()
            .find(|candidate| candidate.id == downstream.id)
            .unwrap();
        assert_eq!(
            downstream_candidate.trigger_kind.as_deref(),
            Some("asset_update")
        );

        let downstream_run = db
            .create_run_with_context(&downstream.id, Some("manual"), None, None, None, None)
            .unwrap();
        db.insert_run_asset(
            &downstream_run.id,
            Some("refresh"),
            None,
            "source",
            "slack",
            "C123",
            "write",
            None,
        )
        .unwrap();
        let after_self_write = scheduler.find_due_workflows(dir.to_str().unwrap());
        assert!(!after_self_write
            .iter()
            .any(|candidate| candidate.id == downstream.id));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn resource_sampler_tree_selection_can_detect_missing_root_pid() {
        let processes = vec![ProcessStat {
            pid: 200,
            ppid: 100,
            cpu_percent: 1.0,
            rss_kb: 10,
            vms_kb: 20,
        }];

        let selected = select_process_tree(100, &processes);

        assert_eq!(selected, std::collections::HashSet::from([100, 200]));
        assert!(!processes.iter().any(|process| process.pid == 100));
    }

    #[test]
    fn mutex_keys_include_pair_groups() {
        let config = parse_queue_config(
            Some(r#"{"excludes":["refresh"],"tags":["heavy_io"],"queue":"source-heavy"}"#),
            "production",
        );

        let keys = mutex_keys("capture", &config);

        assert_eq!(keys, vec!["exclude:capture::refresh".to_string()]);
    }

    #[test]
    fn non_cron_dispatch_returns_queued_for_expected_dependency_wait() {
        let dir =
            std::env::temp_dir().join(format!("chaos-scheduler-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Arc::new(Database::new(&dir));
        let workflow = db
            .create_workflow(
                "Backfill Workflow",
                None,
                "scripts/workflows/noop.py",
                "0 0 * * *",
                false,
                true,
                "UTC",
                "production",
                Some("scheduler"),
                None,
                Some(r#"{"queue":"production-default","depends_on":["upstream"]}"#),
            )
            .unwrap();

        let outcome = dispatch_non_cron_workflow(
            &db,
            dir.to_str().unwrap(),
            "python3",
            &workflow.id,
            NonCronDispatchOptions {
                notify_on_success: false,
                notify_on_failure: false,
                email_on_failure_enabled: false,
                trigger_kind: "backfill",
                trigger_payload: Some(r#"{"logical_date":"2026-05-01T00:00:00Z"}"#),
                upstream_run_id: None,
                input_json: Some(r#"{"backfill":{"logical_date":"2026-05-01T00:00:00Z"}}"#),
                rerun_of_run_id: None,
                suppress_completion_triggers: true,
                dedupe: true,
                app_handle: None,
            },
        )
        .unwrap();

        assert_eq!(outcome.status, "queued");
        assert!(outcome.queued_run_id.is_some());
        assert!(outcome.reason.unwrap().contains("depends_on upstream"));
        let rows = db.list_queued_runs(10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].trigger_kind.as_deref(), Some("backfill"));

        let _ = std::fs::remove_dir_all(dir);
    }

    /// M5 (D05): the queued-drain job decides whether to fire on-completion
    /// chains via `DueWorkflow.suppress_completion_triggers`, which it reads from
    /// the persisted queued row (`find_due_workflows`). This proves the
    /// suppression intent survives the enqueue -> drain boundary: a queued
    /// `ui_fix_rerun` with the bit set is surfaced to the drain job as `true`, so
    /// `if result.completed && !wf.suppress_completion_triggers` skips the
    /// cascade. Before schema v16 the bit was dropped on the queued path (no
    /// column / no `DueWorkflow` field), so a drained fix rerun cascaded
    /// downstream side effects.
    #[test]
    fn find_due_workflows_carries_suppress_completion_triggers_to_drain() {
        let (db, dir) = structured_test_db();
        let wf = db
            .create_workflow(
                "Suppressing Rerun",
                None,
                "true",
                "0 0 * * *",
                false,
                false,
                "UTC",
                "production",
                None,
                None,
                Some(r#"{"queue":"production-default"}"#),
            )
            .unwrap();

        // Seed a queued run carrying the suppression intent, exactly as the
        // dispatch/queued path would when a fix rerun is admission-queued. No
        // unmet dependency, so it becomes due on the next scan.
        db.upsert_queued_run_with_context(
            &wf.id,
            "production-default",
            5,
            Some("ui_fix_rerun"),
            None,
            None,
            None,
            None,
            true,
        )
        .unwrap();

        let scheduler = WorkflowScheduler::new(db.clone());
        let due = scheduler.find_due_workflows(dir.to_str().unwrap());
        let candidate = due
            .iter()
            .find(|d| d.id == wf.id)
            .expect("queued run with a met dependency should be due");
        assert!(
            candidate.suppress_completion_triggers,
            "suppression intent must reach the drain job so a fix rerun does not cascade"
        );
        assert_eq!(candidate.trigger_kind.as_deref(), Some("ui_fix_rerun"));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn fix_rerun_executes_in_the_derived_worktree_not_the_primary_tree() {
        // M2: a fix source rerun (reserved trigger kind) must execute inside the
        // fix's dedicated throwaway worktree, NEVER the shared primary checkout.
        let (db, dir) = structured_test_db();
        // A command workflow that records its execution cwd via a marker file.
        let wf = db
            .create_workflow(
                "Fix Source",
                None,
                "MARKER=1 && pwd > pwd_marker.txt",
                "0 0 * * *",
                false,
                false,
                "UTC",
                "production",
                None,
                None,
                Some(r#"{"queue":"production-default"}"#),
            )
            .unwrap();

        let source_run_id = format!("src-{}", uuid::Uuid::new_v4());
        // The orchestrator creates this worktree; here we just need the directory
        // to exist so the derive resolves and the command has a cwd.
        let worktree = crate::fix_worktree::fix_worktree_path_for(&source_run_id);
        std::fs::create_dir_all(&worktree).unwrap();

        let result = execute_workflow_with_context(
            &db,
            dir.to_str().unwrap(), // primary checkout
            "python3",
            &wf.id,
            false,
            false,
            false,
            Some(crate::service::FIX_RERUN_TRIGGER_KIND),
            None,
            None,
            None,
            Some(&source_run_id), // rerun_of => derives the worktree cwd
            None,
            true, // suppress_completion_triggers (M5)
            None,
        )
        .expect("fix rerun executes");

        assert!(result.success, "the source command should exit 0");
        assert!(
            worktree.join("pwd_marker.txt").exists(),
            "fix rerun must execute in the derived worktree"
        );
        assert!(
            !dir.join("pwd_marker.txt").exists(),
            "fix rerun must NOT execute in the primary checkout"
        );

        let _ = std::fs::remove_dir_all(&worktree);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn fix_rerun_fails_closed_when_its_worktree_is_absent() {
        // M2 safety: a fix rerun whose worktree is gone (e.g. a post-crash stale
        // queued rerun the startup sweep already reclaimed) must REFUSE, never
        // silently run agent-edited code against the primary checkout.
        let (db, dir) = structured_test_db();
        let wf = db
            .create_workflow(
                "Fix Source",
                None,
                "MARKER=1 && pwd > pwd_marker.txt",
                "0 0 * * *",
                false,
                false,
                "UTC",
                "production",
                None,
                None,
                Some(r#"{"queue":"production-default"}"#),
            )
            .unwrap();

        let source_run_id = format!("src-{}", uuid::Uuid::new_v4());
        let worktree = crate::fix_worktree::fix_worktree_path_for(&source_run_id);
        assert!(!worktree.exists(), "worktree deliberately absent");

        let result = execute_workflow_with_context(
            &db,
            dir.to_str().unwrap(),
            "python3",
            &wf.id,
            false,
            false,
            false,
            Some(crate::service::FIX_RERUN_TRIGGER_KIND),
            None,
            None,
            None,
            Some(&source_run_id),
            None,
            true,
            None,
        );
        match result {
            Err(e) => assert!(
                e.contains("worktree is missing"),
                "expected a fail-closed refusal, got: {e}"
            ),
            Ok(_) => panic!("must refuse when the fix-rerun worktree is missing"),
        }
        assert!(
            !dir.join("pwd_marker.txt").exists(),
            "must never fall back to the primary checkout"
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn subworkflow_requests_create_child_relationships_through_admission() {
        let dir = std::env::temp_dir().join(format!("chaos-child-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Arc::new(Database::new(&dir));
        let parent = db
            .create_workflow(
                "Parent Workflow",
                None,
                "scripts/workflows/noop.py",
                "0 0 * * *",
                false,
                true,
                "UTC",
                "production",
                Some("scheduler"),
                None,
                Some(r#"{"queue":"production-default"}"#),
            )
            .unwrap();
        let child = db
            .create_workflow(
                "Child Workflow",
                None,
                "scripts/workflows/noop.py",
                "0 0 * * *",
                false,
                true,
                "UTC",
                "production",
                Some("scheduler"),
                None,
                Some(r#"{"queue":"production-default","depends_on":["missing-upstream"]}"#),
            )
            .unwrap();
        let parent_run = db
            .create_run_with_context(&parent.id, Some("manual"), None, None, None, None)
            .unwrap();
        let raw = format!(
            r#"{{"schema_version":"scheduler.task_event.v1","ts":"2026-05-19T00:00:00Z","task_id":"fanout","status":"subworkflow_requested","details":{{"workflow_id":"{}","inputs":{{"partition":"p1"}},"wait":true,"correlation_id":"c1"}}}}"#,
            child.id
        );

        let summary = dispatch_child_workflow_requests(
            &db,
            dir.to_str().unwrap(),
            "python3",
            &parent_run.id,
            &parent.id,
            &raw,
            None,
        );

        assert_eq!(summary.failure_count, 0);
        let relationships = db.list_run_relationships(&parent_run.id).unwrap();
        assert_eq!(relationships.len(), 1);
        assert_eq!(relationships[0].child_workflow_id, child.id);
        assert_eq!(relationships[0].status, "queued");
        assert!(relationships[0].queued_run_id.is_some());
        assert_eq!(relationships[0].task_id.as_deref(), Some("fanout"));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn subworkflow_requests_reject_immediate_parent_child_loops() {
        let dir =
            std::env::temp_dir().join(format!("chaos-child-loop-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Arc::new(Database::new(&dir));
        let parent = db
            .create_workflow(
                "Parent Workflow",
                None,
                "scripts/workflows/noop.py",
                "0 0 * * *",
                false,
                true,
                "UTC",
                "production",
                Some("scheduler"),
                None,
                Some(r#"{"queue":"production-default"}"#),
            )
            .unwrap();
        let parent_run = db
            .create_run_with_context(&parent.id, Some("manual"), None, None, None, None)
            .unwrap();
        let raw = format!(
            r#"{{"schema_version":"scheduler.task_event.v1","ts":"2026-05-19T00:00:00Z","task_id":"fanout","status":"subworkflow_requested","details":{{"workflow_id":"{}","inputs":{{}},"wait":true}}}}"#,
            parent.id
        );

        let summary = dispatch_child_workflow_requests(
            &db,
            dir.to_str().unwrap(),
            "python3",
            &parent_run.id,
            &parent.id,
            &raw,
            None,
        );

        assert_eq!(summary.failure_count, 1);
        let relationships = db.list_run_relationships(&parent_run.id).unwrap();
        assert_eq!(relationships[0].status, "rejected");
        assert!(relationships[0].reason.as_deref().unwrap().contains("loop"));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn file_sensor_fingerprint_resolves_simple_glob() {
        let dir = std::env::temp_dir().join(format!("chaos-file-trigger-{}", uuid::Uuid::new_v4()));
        let inbox = dir.join("data").join("inbox");
        std::fs::create_dir_all(&inbox).unwrap();
        std::fs::write(inbox.join("sample.json"), br#"{"ok":true}"#).unwrap();

        let fingerprint = fingerprint_file_sensor(
            "data/inbox/*.json",
            "content_hash_changed",
            dir.to_str().unwrap(),
        );
        let _ = std::fs::remove_dir_all(dir);

        assert!(fingerprint.is_some());
        let (path, value) = fingerprint.unwrap();
        assert!(path.ends_with("sample.json"));
        assert!(value.starts_with("hash:"));
    }
}
