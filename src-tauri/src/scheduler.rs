use crate::db::{Database, WorkflowResourceSample};
use chrono::Utc;
use chrono_tz::Tz;
use cron::Schedule;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{hash_map::DefaultHasher, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::io::FromRawFd;
#[cfg(unix)]
use std::os::unix::process::CommandExt;

pub static SHUTDOWN: AtomicBool = AtomicBool::new(false);
static SLA_NOTIFICATION_CACHE: OnceLock<Mutex<HashMap<String, i64>>> = OnceLock::new();

const RESOURCE_SAMPLE_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Clone)]
struct ResourceSampleMetadata {
    db: Arc<Database>,
    run_id: String,
    workflow_id: String,
    queue_name: Option<String>,
    corpus: String,
}

struct ResourceSamplerHandle {
    stop_tx: mpsc::Sender<()>,
    join: std::thread::JoinHandle<()>,
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

        for workflow in workflows {
            if !workflow.enabled {
                continue;
            }

            let queue_config =
                parse_queue_config(workflow.queue_config.as_deref(), &workflow.corpus);
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
                let since = now - chrono::Duration::days(2);
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
                    let _ = self.db.upsert_queued_run(
                        &workflow.id,
                        &queue_config.queue,
                        queue_config.priority,
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
                let _ = self.db.upsert_queued_run(
                    &workflow.id,
                    &queue_config.queue,
                    queue_config.priority,
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
        for upstream in &queue_config.depends_on {
            match self.latest_status(upstream) {
                Some(status) if status == "success" => {}
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
            match self.latest_status(upstream) {
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

    fn has_queue_capacity(&self, queue_config: &QueueConfig) -> Result<bool, String> {
        let capacity = self
            .db
            .queue_capacity(&queue_config.queue, &queue_config.corpus)
            .map_err(|e| e.to_string())?;
        let running = self
            .running_count_for_queue(&queue_config.queue, &queue_config.corpus)
            .map_err(|e| e.to_string())?;
        Ok(running < capacity)
    }

    fn running_count_for_queue(&self, queue_name: &str, corpus: &str) -> Result<i64, String> {
        running_count_for_queue(&self.db, queue_name, corpus)
    }

    fn latest_status(&self, workflow_id: &str) -> Option<String> {
        match self.db.latest_run_status(workflow_id) {
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
        let _ = self
            .db
            .set_trigger_state(workflow_id, &trigger_id, &fingerprint, changed);
        if changed {
            Some(serde_json::json!({
                "trigger_id": trigger_id,
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
        let Some((matched_path, fingerprint)) =
            fingerprint_file_sensor(path, mode, chaos_labs_root)
        else {
            return None;
        };
        let prior = self
            .db
            .get_trigger_fingerprint(workflow_id, &trigger_id)
            .ok()
            .flatten();
        let changed = prior.as_deref() != Some(fingerprint.as_str());
        let _ = self
            .db
            .set_trigger_state(workflow_id, &trigger_id, &fingerprint, changed);
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

pub struct DueWorkflow {
    pub id: String,
    pub trigger_kind: Option<String>,
    pub trigger_payload: Option<String>,
    pub queue_name: String,
    pub priority: i64,
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
    corpus: String,
}

enum DependencyDecision {
    Ready,
    Waiting(String),
    CascadeSkip(String),
}

fn parse_queue_config(queue_config: Option<&str>, corpus: &str) -> QueueConfig {
    let default_queue = format!("{}-default", corpus);
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
            corpus: corpus.to_string(),
        });
    if parsed.queue.trim().is_empty() {
        parsed.queue = default_queue;
    }
    parsed.corpus = corpus.to_string();
    parsed
}

fn mutex_keys(workflow_id: &str, queue_config: &QueueConfig) -> Vec<String> {
    let mut keys = Vec::new();
    for other in &queue_config.excludes {
        let mut pair = [workflow_id.to_string(), other.to_string()];
        pair.sort();
        keys.push(format!("exclude:{}::{}", pair[0], pair[1]));
    }
    for tag in &queue_config.tags {
        keys.push(format!(
            "tag:{}:{}:{}",
            queue_config.corpus, queue_config.queue, tag
        ));
    }
    keys.sort();
    keys.dedup();
    keys
}

fn running_count_for_queue(
    db: &Arc<Database>,
    queue_name: &str,
    corpus: &str,
) -> Result<i64, String> {
    let running = db.get_running_runs().map_err(|e| e.to_string())?;
    let mut count = 0;
    for run in running {
        let workflow = db
            .get_workflow(&run.workflow_id)
            .map_err(|e| e.to_string())?;
        let config = parse_queue_config(workflow.queue_config.as_deref(), &workflow.corpus);
        if config.queue == queue_name && config.corpus == corpus {
            count += 1;
        }
    }
    Ok(count)
}

fn is_terminal_status(status: &str) -> bool {
    matches!(
        status,
        "success" | "failed" | "cancelled" | "cascade-skipped"
    )
}

fn is_failure_terminal(status: &str) -> bool {
    matches!(status, "failed" | "cancelled" | "cascade-skipped")
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
) -> Result<RunResult, String> {
    let workflow = db
        .get_workflow(workflow_id)
        .map_err(|e| format!("Failed to get workflow: {}", e))?;
    let queue_config = parse_queue_config(workflow.queue_config.as_deref(), &workflow.corpus);
    let capacity = db
        .queue_capacity(&queue_config.queue, &queue_config.corpus)
        .map_err(|e| format!("Failed to read queue capacity: {}", e))?;
    let running = running_count_for_queue(db, &queue_config.queue, &queue_config.corpus)?;
    if running >= capacity {
        let _ = db.upsert_queued_run(&workflow.id, &queue_config.queue, queue_config.priority);
        return Err(format!(
            "Queue {} is at capacity ({}/{})",
            queue_config.queue, running, capacity
        ));
    }
    let mutex_keys = mutex_keys(&workflow.id, &queue_config);

    let run = db
        .create_run_with_context(
            &workflow.id,
            trigger_kind,
            trigger_payload,
            upstream_run_id,
            input_json,
            rerun_of_run_id,
        )
        .map_err(|e| format!("Failed to create run record: {}", e))?;
    let _ = db.mark_queued_run_admitted(&workflow.id, &run.id);

    let acquired = db
        .acquire_mutex_locks(&workflow.id, &run.id, &mutex_keys)
        .map_err(|e| format!("Failed to acquire mutex locks: {}", e))?;
    if !acquired {
        let reason = format!(
            "Workflow {} could not acquire mutex locks in queue {}",
            workflow.id, queue_config.queue
        );
        let _ = db.finish_run_with_status(&run.id, "cancelled", "", &reason);
        return Err(reason);
    }

    let sample_metadata = ResourceSampleMetadata {
        db: Arc::clone(db),
        run_id: run.id.clone(),
        workflow_id: workflow.id.clone(),
        queue_name: Some(queue_config.queue.clone()),
        corpus: queue_config.corpus.clone(),
    };

    let output = run_workflow_command(
        build_workflow_command(
            &workflow.script_path,
            chaos_labs_root,
            python_path,
            &run.id,
            &workflow.id,
            &queue_config.queue,
            &queue_config.corpus,
            workflow.domain.as_deref(),
            db.path(),
            input_json,
        ),
        Some(sample_metadata.clone()),
    );

    match output {
        Ok((output, task_events_raw)) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = output.status.code().unwrap_or(-1);
            let stdout = stdout_with_task_events(stdout, &task_events_raw);
            persist_task_events(db, &run.id, &workflow.id, &task_events_raw);

            let result_url = extract_result_url(&stdout);

            let bg_pid = extract_background_pid(&stdout, chaos_labs_root);

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
                        &py,
                        &workflow_id,
                        sample_metadata,
                        notify_success,
                        notify_failure,
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

            db.finish_run(&run.id, exit_code, &stdout, &stderr, result_url.as_deref())
                .map_err(|e| format!("Failed to update run: {}", e))?;

            let success = exit_code == 0;
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
            let _ = db.finish_run(&run.id, -1, "", &format!("Failed to execute: {}", e), None);
            Err(format!("Failed to execute workflow: {}", e))
        }
    }
}

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
        let payload = serde_json::json!({
            "upstream_workflow_id": upstream_workflow_id,
            "upstream_run_id": upstream_run_id,
            "status": status,
        })
        .to_string();
        if let Err(e) = execute_workflow_with_context(
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
        ) {
            log::error!(
                "Completion trigger failed for downstream workflow {}: {}",
                workflow.id,
                e
            );
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
fn build_workflow_command(
    script_path: &str,
    chaos_labs_root: &str,
    python_path: &str,
    run_id: &str,
    workflow_id: &str,
    queue_name: &str,
    corpus: &str,
    domain: Option<&str>,
    scheduler_db_path: &str,
    input_json: Option<&str>,
) -> Command {
    let is_shell_cmd = script_path.contains('=') || script_path.contains("/bin/python");

    if is_shell_cmd {
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg(script_path)
            .current_dir(chaos_labs_root)
            .env("CHAOS_LABS_ROOT", chaos_labs_root)
            .env("CHAOS_LABS_SCHEDULER_RUN_ID", run_id)
            .env("CHAOS_LABS_SCHEDULER_WORKFLOW_ID", workflow_id)
            .env("CHAOS_LABS_SCHEDULER_QUEUE", queue_name)
            .env("CHAOS_LABS_SCHEDULER_CORPUS", corpus)
            .env("CHAOS_LABS_SCHEDULER_DB_PATH", scheduler_db_path);
        if let Some(domain) = domain {
            cmd.env("CHAOS_LABS_SCHEDULER_DOMAIN", domain);
        }
        if let Some(input) = input_json {
            cmd.env("CHAOS_LABS_WORKFLOW_INPUT_JSON", input);
        }
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
            .env("CHAOS_LABS_SCHEDULER_RUN_ID", run_id)
            .env("CHAOS_LABS_SCHEDULER_WORKFLOW_ID", workflow_id)
            .env("CHAOS_LABS_SCHEDULER_QUEUE", queue_name)
            .env("CHAOS_LABS_SCHEDULER_CORPUS", corpus)
            .env("CHAOS_LABS_SCHEDULER_DB_PATH", scheduler_db_path);
        if let Some(domain) = domain {
            cmd.env("CHAOS_LABS_SCHEDULER_DOMAIN", domain);
        }
        if let Some(input) = input_json {
            cmd.env("CHAOS_LABS_WORKFLOW_INPUT_JSON", input);
        }
        cmd
    }
}

#[cfg(unix)]
fn run_workflow_command(
    mut cmd: Command,
    sample_metadata: Option<ResourceSampleMetadata>,
) -> std::io::Result<(Output, String)> {
    let mut fds = [0; 2];
    let pipe_result = unsafe { libc::pipe(fds.as_mut_ptr()) };
    if pipe_result == -1 {
        return Err(std::io::Error::last_os_error());
    }
    let read_fd = fds[0];
    let write_fd = fds[1];

    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("CHAOS_LABS_TASK_CHANNEL_FD", "3");
    unsafe {
        cmd.pre_exec(move || {
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
    let child = match child {
        Ok(child) => child,
        Err(e) => {
            unsafe {
                libc::close(read_fd);
            }
            return Err(e);
        }
    };

    let sampler = sample_metadata.map(|metadata| spawn_resource_sampler(metadata, child.id()));
    let task_reader = std::thread::spawn(move || {
        let mut file = unsafe { std::fs::File::from_raw_fd(read_fd) };
        let mut task_events = String::new();
        let _ = file.read_to_string(&mut task_events);
        task_events
    });
    let output_result = child.wait_with_output();
    stop_resource_sampler(sampler);
    let output = output_result?;
    let task_events = task_reader.join().unwrap_or_default();
    Ok((output, task_events))
}

#[cfg(not(unix))]
fn run_workflow_command(
    mut cmd: Command,
    _sample_metadata: Option<ResourceSampleMetadata>,
) -> std::io::Result<(Output, String)> {
    cmd.output().map(|output| (output, String::new()))
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
        corpus: metadata.corpus.clone(),
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
                        let asset_namespace = asset
                            .get("namespace")
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        let asset_partition = asset
                            .get("partition")
                            .and_then(Value::as_str)
                            .unwrap_or("");
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
                            let action = if status == "asset_read" { "read" } else { "write" };
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
                if let Some(openlineage_event) = details
                    .as_ref()
                    .and_then(|d| d.get("openlineage_event"))
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
    db: &Arc<Database>,
    python_path: &str,
    workflow_id: &str,
    sample_metadata: ResourceSampleMetadata,
    notify_on_success: bool,
    notify_on_failure: bool,
) {
    log::info!(
        "Monitoring background PID {} for workflow '{}'",
        pid,
        wf_name
    );
    let sampler = spawn_resource_sampler(sample_metadata, pid);

    loop {
        std::thread::sleep(Duration::from_secs(10));

        let alive = unsafe { libc::kill(pid as i32, 0) } == 0;
        if !alive {
            break;
        }
    }

    stop_resource_sampler(Some(sampler));

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
            completed: true,
            should_notify: true,
            email_on_failure: true,
        };
        send_failure_email(db, chaos_labs_root, &result);
    }

    if exit_code == 0 {
        trigger_on_completion(
            db,
            chaos_labs_root,
            python_path,
            workflow_id,
            run_id,
            true,
            notify_on_success,
            notify_on_failure,
            email_enabled,
        );
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
                    let due = sched.find_due_workflows(&chaos_labs_root);
                    let ns = sched.notify_on_success.load(Ordering::Relaxed);
                    let nf = sched.notify_on_failure.load(Ordering::Relaxed);
                    let ef = sched.should_email_on_failure();
                    (due, ns, nf, ef)
                };
                // Lock released — all subsequent work is lock-free

                // Phase 2 (unlocked): execute workflows
                let mut results = vec![];
                for wf in &due {
                    match execute_workflow_with_context(
                        &db,
                        &chaos_labs_root,
                        &python_path,
                        &wf.id,
                        notify_success,
                        notify_failure,
                        email_enabled,
                        wf.trigger_kind.as_deref(),
                        wf.trigger_payload.as_deref(),
                        None,
                        None,
                        None,
                    ) {
                        Ok(result) => {
                            if result.completed {
                                trigger_on_completion(
                                    &db,
                                    &chaos_labs_root,
                                    &python_path,
                                    &wf.id,
                                    &result.run_id,
                                    result.success,
                                    notify_success,
                                    notify_failure,
                                    email_enabled,
                                );
                            }
                            results.push(result)
                        }
                        Err(e) => log::error!("Workflow {} failed: {}", wf.id, e),
                    }
                }

                for result in &results {
                    if result.should_notify {
                        send_notification(&app_handle, result);
                    }
                }
                send_sla_notifications(&app_handle, &db);

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
        let config = parse_queue_config(None, "source");

        assert_eq!(config.queue, "source-default");
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
            "source",
            None,
            None,
            Some(r#"{"queue":"source-default"}"#),
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
            "source",
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
                "source",
                None,
                None,
                Some(r#"{"queue":"source-default"}"#),
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
            .query_row("SELECT COUNT(*) FROM run_assets WHERE run_id = ?1", [&run.id], |row| {
                row.get(0)
            })
            .unwrap();
        let lineage_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM run_lineage WHERE run_id = ?1", [&run.id], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(asset_count, 1);
        assert_eq!(lineage_count, 1);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn asset_update_trigger_detects_new_matching_writes() {
        let dir = std::env::temp_dir().join(format!("chaos-asset-trigger-{}", uuid::Uuid::new_v4()));
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
                "source",
                None,
                None,
                Some(r#"{"queue":"source-default"}"#),
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
                "source",
                None,
                Some(r#"{"triggers":[{"kind":"asset_update","asset":{"kind":"source","namespace":"slack","partition":"C123"}}]}"#),
                Some(r#"{"queue":"source-default"}"#),
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

        let downstream_candidate = due.iter().find(|candidate| candidate.id == downstream.id).unwrap();
        assert_eq!(downstream_candidate.trigger_kind.as_deref(), Some("asset_update"));

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
        assert!(!after_self_write.iter().any(|candidate| candidate.id == downstream.id));

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
    fn mutex_keys_include_pair_and_tag_groups() {
        let config = parse_queue_config(
            Some(r#"{"excludes":["refresh"],"tags":["heavy_io"],"queue":"source-heavy"}"#),
            "source",
        );

        let keys = mutex_keys("capture", &config);

        assert_eq!(
            keys,
            vec![
                "exclude:capture::refresh".to_string(),
                "tag:source:source-heavy:heavy_io".to_string()
            ]
        );
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
