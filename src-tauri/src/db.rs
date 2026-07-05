use rusqlite::{params, types::Type, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::path::Path;

fn default_utc() -> String {
    "UTC".to_string()
}

fn default_source_corpus() -> String {
    "source".to_string()
}

fn default_kind() -> String {
    "generic".to_string()
}

/// Normalize a mission-control environment filter. Environments are
/// user-managed, so any non-empty value is accepted verbatim as the partition
/// to filter on; empty or "all" means no environment filter.
fn normalize_mission_corpus_filter(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("all") {
        "all".to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalize_mission_domain_filter(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("all") {
        "all".to_string()
    } else if trimmed.eq_ignore_ascii_case("unowned") || trimmed.eq_ignore_ascii_case("__unowned__")
    {
        "__unowned__".to_string()
    } else {
        trimmed.to_string()
    }
}

fn owner_label(domain: Option<String>) -> String {
    match domain.map(|value| value.trim().to_string()) {
        Some(value) if !value.is_empty() => value,
        _ => "Unowned".to_string(),
    }
}

fn redact_audit_path(path: &str) -> String {
    path.split('?')
        .next()
        .unwrap_or(path)
        .chars()
        .filter(|c| !c.is_control())
        .take(512)
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub script_path: String,
    pub cron_schedule: String,
    pub enabled: bool,
    pub async_mode: bool,
    pub email_on_failure: bool,
    #[serde(default = "default_source_corpus")]
    pub corpus: String,
    /// First-class environment (partition/queue-scope/filter). Additive over
    /// `corpus`, which is retained as a shadow for one migration cycle; both
    /// carry the same value today.
    #[serde(default = "default_source_corpus")]
    pub environment: String,
    /// Governance flag: whether this workflow's definition is owned by an
    /// external source of truth and therefore read-only in the UI/API.
    /// Decoupled from `corpus`; backfilled from `corpus == 'source'`.
    #[serde(default)]
    pub managed_externally: bool,
    /// Execution model: `generic` (step-flow) or `typed` (operator).
    #[serde(default = "default_kind")]
    pub kind: String,
    /// Serialized [`crate::workflow_spec::WorkflowSpec`] (null for legacy
    /// single-script workflows).
    #[serde(default)]
    pub spec_json: Option<String>,
    pub domain: Option<String>,
    #[serde(default = "default_utc")]
    pub timezone: String,
    pub trigger_config: Option<String>,
    pub queue_config: Option<String>,
    pub last_run_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub id: String,
    pub workflow_id: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub exit_code: Option<i32>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub result_url: Option<String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_analysis: Option<serde_json::Value>,
    pub trigger_kind: Option<String>,
    pub trigger_payload: Option<String>,
    pub upstream_run_id: Option<String>,
    pub input_json: Option<String>,
    pub rerun_of_run_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RunExecutionRecord {
    pub id: String,
    pub workflow_id: String,
    pub workflow_name: Option<String>,
    pub status: String,
    pub process_pid: Option<i64>,
    pub process_pgid: Option<i64>,
    pub process_started_at: Option<String>,
}

#[allow(clippy::large_enum_variant)]
pub enum RunAdmission {
    Admitted(Run),
    /// Queue, global, or per-tag capacity was exhausted when re-checked inside
    /// the admission transaction; the caller should enqueue the work.
    AtCapacity,
    MutexBusy,
    QueuedRunUnavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerStatus {
    pub active_workflows: usize,
    pub running_count: usize,
    pub next_runs: Vec<NextRun>,
    pub recent_runs: Vec<Run>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueInfo {
    pub name: String,
    pub environment: String,
    pub capacity: i64,
    pub tag_cap: Option<i64>,
    pub max_queued: Option<i64>,
    pub active_count: i64,
    pub queued_count: i64,
    pub global_parallelism_cap: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedRun {
    pub id: String,
    pub run_id: Option<String>,
    pub workflow_id: String,
    pub workflow_name: Option<String>,
    pub queue_name: String,
    pub environment: String,
    pub priority: i64,
    pub status: String,
    pub queued_at: String,
    pub admitted_at: Option<String>,
    pub finished_at: Option<String>,
    pub trigger_kind: Option<String>,
    pub trigger_payload: Option<String>,
    pub upstream_run_id: Option<String>,
    pub input_json: Option<String>,
    pub rerun_of_run_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerDeadLetter {
    pub id: String,
    pub run_id: String,
    pub workflow_id: String,
    pub workflow_name: Option<String>,
    pub task_id: Option<String>,
    pub last_attempt_id: Option<String>,
    pub last_failure_at: String,
    pub last_exception: String,
    pub acknowledged_at: Option<String>,
    pub acknowledged_reason: Option<String>,
    pub acknowledged_by: Option<String>,
    pub recovery_run_id: Option<String>,
    pub run_status: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRelationship {
    pub id: String,
    pub parent_run_id: String,
    pub child_run_id: Option<String>,
    pub queued_run_id: Option<String>,
    pub child_workflow_id: String,
    pub child_workflow_name: Option<String>,
    pub relationship: String,
    pub task_id: Option<String>,
    pub wait: bool,
    pub status: String,
    pub reason: Option<String>,
    pub details: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPreview {
    pub cutoff: String,
    pub candidate_runs: i64,
    pub preserved_dead_letter_runs: i64,
    pub dry_run: bool,
    pub deleted_runs: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct RunAttempt {
    pub id: String,
    pub run_id: String,
    pub task_id: String,
    pub attempt_number: i64,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub exit_code: Option<i32>,
    pub retry_reason: Option<String>,
    pub error_type: Option<String>,
    pub error_message: Option<String>,
    pub trigger_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct RunTask {
    pub id: String,
    pub run_id: String,
    pub attempt_id: Option<String>,
    pub task_id: String,
    pub status: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub attempt_number: i64,
    pub parent_task_id: Option<String>,
    pub error_type: Option<String>,
    pub error_message: Option<String>,
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct RunMetric {
    pub id: String,
    pub run_id: String,
    pub task_id: Option<String>,
    pub metric_name: String,
    pub metric_value: f64,
    pub metric_unit: Option<String>,
    pub emitted_at: String,
    pub labels: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowHistoryBucket {
    pub day: String,
    pub total: i64,
    pub failed: i64,
    pub succeeded: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaViolation {
    pub workflow_id: String,
    pub workflow_name: String,
    pub violation_type: String,
    pub message: String,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct RunIoValue {
    pub id: String,
    pub run_id: String,
    pub task_id: Option<String>,
    pub key: String,
    pub value: serde_json::Value,
    pub schema_version: String,
    pub recorded_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct SchedulerAsset {
    pub asset_id: String,
    pub asset_kind: String,
    pub asset_namespace: String,
    pub asset_partition: String,
    pub last_action: Option<String>,
    pub last_written_at: Option<String>,
    pub last_writer_run_id: Option<String>,
    pub freshness_policy: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct RunAsset {
    pub id: String,
    pub run_id: String,
    pub task_id: Option<String>,
    pub attempt_id: Option<String>,
    pub asset_id: Option<String>,
    pub asset_kind: String,
    pub asset_namespace: String,
    pub asset_partition: String,
    pub action: String,
    pub emitted_at: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct RunLineage {
    pub id: String,
    pub run_id: String,
    pub task_id: Option<String>,
    pub attempt_id: Option<String>,
    pub openlineage_event: serde_json::Value,
    pub emitted_at: String,
    pub exported_at: Option<String>,
    pub export_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetUpdateRecord {
    pub asset_id: Option<String>,
    pub asset_kind: String,
    pub asset_namespace: String,
    pub asset_partition: String,
    pub run_id: String,
    pub workflow_id: String,
    pub task_id: Option<String>,
    pub emitted_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct QueueEvent {
    pub id: String,
    pub queue_name: String,
    pub corpus: String,
    pub workflow_id: Option<String>,
    pub run_id: Option<String>,
    pub event_type: String,
    pub reason: Option<String>,
    pub emitted_at: String,
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct WorkflowResourceSample {
    pub id: String,
    pub run_id: Option<String>,
    pub workflow_id: String,
    pub queue_name: Option<String>,
    pub environment: String,
    pub pid: Option<i64>,
    pub sampled_at: String,
    pub cpu_percent: Option<f64>,
    pub memory_rss_bytes: Option<i64>,
    pub memory_vms_bytes: Option<i64>,
    pub swap_bytes: Option<i64>,
    pub labels: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct WorkflowTokenUsage {
    pub id: String,
    pub run_id: Option<String>,
    pub workflow_id: String,
    pub task_id: Option<String>,
    pub provider: String,
    pub model: Option<String>,
    pub token_kind: String,
    pub token_count: i64,
    pub emitted_at: String,
    pub labels: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTokenUsageRollup {
    pub time_bucket: Option<String>,
    pub workflow_id: Option<String>,
    pub corpus: Option<String>,
    #[serde(default)]
    pub environment: Option<String>,
    pub domain: Option<String>,
    pub queue_name: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub token_kind: Option<String>,
    pub total_tokens: i64,
    pub call_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainOption {
    pub value: String,
    pub label: String,
    pub workflow_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionControlPreferences {
    pub default_landing: String,
    pub corpus_filter: String,
    pub domain_filter: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionControlHeader {
    pub active_workflows: i64,
    pub running_count: i64,
    pub queued_count: i64,
    pub recent_failures: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionControlSlaSummary {
    pub violations_count: i64,
    pub success_rate_24h: Option<f64>,
    pub median_wait_seconds: Option<i64>,
    pub long_running_count: i64,
    pub blocked_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionControlNeedsAttentionItem {
    pub id: String,
    pub severity: String,
    pub title: String,
    pub detail: String,
    pub workflow_id: Option<String>,
    pub workflow_name: Option<String>,
    pub run_id: Option<String>,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionControlActivityItem {
    pub id: String,
    pub workflow_id: String,
    pub workflow_name: String,
    pub corpus: String,
    #[serde(default)]
    pub environment: String,
    pub domain: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub run_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionControlFreshnessItem {
    pub asset_id: String,
    pub asset_kind: String,
    pub asset_namespace: String,
    pub asset_partition: String,
    pub last_action: Option<String>,
    pub last_written_at: Option<String>,
    pub workflow_id: Option<String>,
    pub workflow_name: Option<String>,
    pub corpus: Option<String>,
    #[serde(default)]
    pub environment: Option<String>,
    pub domain: String,
    pub attribution: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionControlWorkflowTelemetry {
    pub workflow_id: String,
    pub workflow_name: String,
    pub corpus: String,
    #[serde(default)]
    pub environment: String,
    pub domain: String,
    pub max_cpu_percent: Option<f64>,
    pub max_memory_rss_bytes: Option<i64>,
    pub sample_count: i64,
    pub total_tokens: i64,
    pub token_call_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionControlUpcomingRun {
    pub workflow_id: String,
    pub workflow_name: String,
    pub corpus: String,
    #[serde(default)]
    pub environment: String,
    pub domain: String,
    pub trigger_kind: String,
    pub trigger_label: String,
    pub next_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionControlPanelAvailability {
    pub panel: String,
    pub source_tables: Vec<String>,
    pub command: String,
    pub filter_behavior: String,
    pub empty_state: String,
    pub degraded_state: String,
    pub click_through_target: Option<String>,
    pub persistence_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionControlSnapshot {
    pub preferences: MissionControlPreferences,
    pub domains: Vec<DomainOption>,
    pub header: MissionControlHeader,
    pub sla: MissionControlSlaSummary,
    pub needs_attention: Vec<MissionControlNeedsAttentionItem>,
    pub needs_attention_total: i64,
    pub needs_attention_truncated: bool,
    pub live_activity: Vec<MissionControlActivityItem>,
    pub upcoming_runs: Vec<MissionControlUpcomingRun>,
    pub freshness_ledger: Vec<MissionControlFreshnessItem>,
    pub recent_runs: Vec<Run>,
    pub workflow_telemetry: Vec<MissionControlWorkflowTelemetry>,
    pub availability: Vec<MissionControlPanelAvailability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextRun {
    pub workflow_id: String,
    pub workflow_name: String,
    pub corpus: String,
    #[serde(default)]
    pub environment: String,
    pub next_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailConfig {
    pub enabled: bool,
    pub alert_email: String,
    pub smtp_host: String,
    pub smtp_port: i32,
    pub smtp_user: String,
    pub smtp_password: String,
    pub from_address: String,
    pub from_name: String,
}

impl Default for EmailConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            alert_email: String::new(),
            smtp_host: String::from("smtp.gmail.com"),
            smtp_port: 587,
            smtp_user: String::new(),
            smtp_password: String::new(),
            from_address: String::new(),
            from_name: String::from(crate::branding::EMAIL_FROM_NAME),
        }
    }
}

/// A user-managed execution environment: the first-class replacement for the
/// overloaded `corpus`. Partitions queues/telemetry and can carry a default
/// working directory and queue caps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Environment {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub working_dir: Option<String>,
    pub default_queue_capacity: Option<i64>,
    pub default_tag_cap: Option<i64>,
    pub default_max_queued: Option<i64>,
    #[serde(default)]
    pub managed_externally: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// API key metadata surfaced to the UI (never includes the hash/salt/secret).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyInfo {
    pub id: String,
    pub name: Option<String>,
    pub scopes: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub revoked: bool,
}

#[derive(Debug, Clone)]
pub struct IdempotencyRecord {
    pub run_id: Option<String>,
    pub queued_run_id: Option<String>,
    pub request_fingerprint: Option<String>,
}

pub enum IdempotencyReservation {
    Reserved,
    Existing(IdempotencyRecord),
}

/// The schema version this binary understands. Bump this (and add a numbered
/// migration in [`Database::run_migrations`]) whenever a schema change lands.
/// Persisted in the DB via `PRAGMA user_version`; a DB reporting a higher
/// version than this constant is refused (downgrade guard) so an older binary
/// never silently corrupts a newer file.
pub const CURRENT_SCHEMA_VERSION: i64 = 8;

pub struct Database {
    path: String,
}

impl Database {
    pub fn new(app_data_dir: &Path) -> Self {
        let db_path = app_data_dir.join("scheduler.db");
        let db = Database {
            path: db_path.to_string_lossy().to_string(),
        };
        db.init().expect("Failed to initialize database");
        db
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    fn conn(&self) -> rusqlite::Result<Connection> {
        let conn = Connection::open(&self.path)?;
        // WAL improves read/write concurrency (needed once the HTTP API writes
        // concurrently with the polling engine); busy_timeout avoids spurious
        // SQLITE_BUSY under contention. journal_mode is persistent per-db but
        // re-asserting it on each connection is cheap and idempotent.
        conn.busy_timeout(std::time::Duration::from_millis(5_000))?;
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(conn)
    }

    fn init(&self) -> rusqlite::Result<()> {
        let conn = self.conn()?;

        // Downgrade / open guard: refuse to touch a DB written by a newer schema.
        let existing_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if existing_version > CURRENT_SCHEMA_VERSION {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_ERROR),
                Some(format!(
                    "scheduler.db schema version {existing_version} is newer than this build supports ({CURRENT_SCHEMA_VERSION}); update the application"
                )),
            ));
        }

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS workflows (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                script_path TEXT NOT NULL,
                cron_schedule TEXT NOT NULL,
                enabled INTEGER DEFAULT 1,
                async_mode INTEGER DEFAULT 0,
                corpus TEXT NOT NULL DEFAULT 'source',
                domain TEXT,
                trigger_config TEXT,
                queue_config TEXT,
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS runs (
                id TEXT PRIMARY KEY,
                workflow_id TEXT NOT NULL REFERENCES workflows(id),
                started_at TEXT NOT NULL,
                finished_at TEXT,
                exit_code INTEGER,
                stdout TEXT,
                stderr TEXT,
                result_url TEXT,
                trigger_kind TEXT,
                trigger_payload TEXT,
                upstream_run_id TEXT,
                input_json TEXT,
                rerun_of_run_id TEXT,
                status TEXT DEFAULT 'running',
                execution_worker_id TEXT,
                process_pid INTEGER,
                process_pgid INTEGER,
                process_started_at TEXT,
                stdout_truncated INTEGER NOT NULL DEFAULT 0,
                stderr_truncated INTEGER NOT NULL DEFAULT 0,
                task_events_truncated INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS workflow_trigger_state (
                workflow_id TEXT NOT NULL,
                trigger_id TEXT NOT NULL,
                fingerprint TEXT,
                observed_at TEXT NOT NULL,
                fired_at TEXT,
                PRIMARY KEY (workflow_id, trigger_id)
            );
            CREATE TABLE IF NOT EXISTS scheduler_config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS queues (
                name TEXT NOT NULL,
                corpus TEXT NOT NULL,
                capacity INTEGER NOT NULL DEFAULT 1,
                tag_cap INTEGER,
                max_queued INTEGER,
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT DEFAULT (datetime('now')),
                PRIMARY KEY (name, corpus)
            );
            CREATE TABLE IF NOT EXISTS queued_runs (
                id TEXT PRIMARY KEY,
                run_id TEXT REFERENCES runs(id) ON DELETE SET NULL,
                workflow_id TEXT NOT NULL REFERENCES workflows(id),
                queue_name TEXT NOT NULL,
                priority INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'queued',
                queued_at TEXT NOT NULL,
                admitted_at TEXT,
                finished_at TEXT,
                trigger_kind TEXT,
                trigger_payload TEXT,
                upstream_run_id TEXT,
                input_json TEXT,
                rerun_of_run_id TEXT
            );
            CREATE TABLE IF NOT EXISTS workflow_mutex_locks (
                mutex_key TEXT PRIMARY KEY,
                workflow_id TEXT NOT NULL REFERENCES workflows(id),
                run_id TEXT REFERENCES runs(id) ON DELETE CASCADE,
                acquired_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_runs_workflow ON runs(workflow_id);",
        )?;
        // Safe migration: add last_run_at if it doesn't exist
        let has_col: bool = conn
            .prepare("SELECT last_run_at FROM workflows LIMIT 0")
            .is_ok();
        if !has_col {
            conn.execute_batch("ALTER TABLE workflows ADD COLUMN last_run_at TEXT;")?;
        }
        conn.execute_batch(
            "
            CREATE INDEX IF NOT EXISTS idx_runs_started ON runs(started_at DESC);",
        )?;
        let _ =
            conn.execute_batch("ALTER TABLE workflows ADD COLUMN async_mode INTEGER DEFAULT 0;");
        let _ = conn.execute_batch("ALTER TABLE runs ADD COLUMN error_analysis TEXT;");
        let _ = conn
            .execute_batch("ALTER TABLE workflows ADD COLUMN email_on_failure INTEGER DEFAULT 1;");
        let _ = conn.execute_batch(
            "ALTER TABLE workflows ADD COLUMN corpus TEXT NOT NULL DEFAULT 'source';",
        );
        let _ = conn.execute_batch("ALTER TABLE workflows ADD COLUMN trigger_config TEXT;");
        let _ = conn.execute_batch("ALTER TABLE workflows ADD COLUMN queue_config TEXT;");
        let _ = conn.execute_batch("ALTER TABLE workflows ADD COLUMN domain TEXT;");
        let _ = conn.execute_batch("ALTER TABLE workflows ADD COLUMN timezone TEXT DEFAULT 'UTC';");
        let _ = conn.execute_batch("ALTER TABLE runs ADD COLUMN trigger_kind TEXT;");
        let _ = conn.execute_batch("ALTER TABLE runs ADD COLUMN trigger_payload TEXT;");
        let _ = conn.execute_batch("ALTER TABLE runs ADD COLUMN upstream_run_id TEXT;");
        let _ = conn.execute_batch("ALTER TABLE runs ADD COLUMN input_json TEXT;");
        let _ = conn.execute_batch("ALTER TABLE runs ADD COLUMN rerun_of_run_id TEXT;");
        let _ = conn.execute_batch("ALTER TABLE queued_runs ADD COLUMN trigger_kind TEXT;");
        let _ = conn.execute_batch("ALTER TABLE queued_runs ADD COLUMN trigger_payload TEXT;");
        let _ = conn.execute_batch("ALTER TABLE queued_runs ADD COLUMN upstream_run_id TEXT;");
        let _ = conn.execute_batch("ALTER TABLE queued_runs ADD COLUMN input_json TEXT;");
        let _ = conn.execute_batch("ALTER TABLE queued_runs ADD COLUMN rerun_of_run_id TEXT;");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS email_config (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                enabled INTEGER DEFAULT 0,
                alert_email TEXT DEFAULT '',
                smtp_host TEXT DEFAULT 'smtp.gmail.com',
                smtp_port INTEGER DEFAULT 587,
                smtp_user TEXT DEFAULT '',
                smtp_password TEXT DEFAULT '',
                from_address TEXT DEFAULT '',
                from_name TEXT DEFAULT 'Chaos Scheduler'
            );
            INSERT OR IGNORE INTO email_config (id) VALUES (1);",
        )?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS run_attempts (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
                task_id TEXT NOT NULL,
                attempt_number INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL,
                started_at TEXT NOT NULL,
                finished_at TEXT,
                exit_code INTEGER,
                retry_reason TEXT,
                error_type TEXT,
                error_message TEXT,
                trigger_kind TEXT,
                created_at TEXT DEFAULT (datetime('now')),
                UNIQUE(run_id, task_id, attempt_number)
            );
            CREATE TABLE IF NOT EXISTS run_tasks (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
                attempt_id TEXT REFERENCES run_attempts(id) ON DELETE SET NULL,
                task_id TEXT NOT NULL,
                status TEXT NOT NULL,
                started_at TEXT,
                finished_at TEXT,
                attempt_number INTEGER NOT NULL DEFAULT 0,
                parent_task_id TEXT,
                error_type TEXT,
                error_message TEXT,
                details_json TEXT,
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT DEFAULT (datetime('now')),
                UNIQUE(run_id, task_id, attempt_number)
            );
            CREATE TABLE IF NOT EXISTS run_metrics (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
                task_id TEXT,
                metric_name TEXT NOT NULL,
                metric_value REAL NOT NULL,
                metric_unit TEXT,
                emitted_at TEXT NOT NULL,
                labels_json TEXT
            );
            CREATE TABLE IF NOT EXISTS run_inputs (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
                task_id TEXT,
                key TEXT NOT NULL,
                value_json TEXT NOT NULL,
                schema_version TEXT NOT NULL DEFAULT '1.0.0',
                recorded_at TEXT NOT NULL,
                UNIQUE(run_id, task_id, key)
            );
            CREATE TABLE IF NOT EXISTS run_outputs (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
                task_id TEXT,
                key TEXT NOT NULL,
                value_json TEXT NOT NULL,
                schema_version TEXT NOT NULL DEFAULT '1.0.0',
                recorded_at TEXT NOT NULL,
                UNIQUE(run_id, task_id, key)
            );
            CREATE TABLE IF NOT EXISTS scheduler_assets (
                asset_id TEXT PRIMARY KEY,
                asset_kind TEXT NOT NULL,
                asset_namespace TEXT NOT NULL,
                asset_partition TEXT NOT NULL DEFAULT '',
                last_action TEXT,
                last_written_at TEXT,
                last_writer_run_id TEXT REFERENCES runs(id) ON DELETE SET NULL,
                freshness_policy_json TEXT,
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT DEFAULT (datetime('now')),
                UNIQUE(asset_kind, asset_namespace, asset_partition)
            );
            CREATE TABLE IF NOT EXISTS run_assets (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
                task_id TEXT,
                attempt_id TEXT REFERENCES run_attempts(id) ON DELETE SET NULL,
                asset_id TEXT REFERENCES scheduler_assets(asset_id) ON DELETE SET NULL,
                asset_kind TEXT NOT NULL,
                asset_namespace TEXT NOT NULL,
                asset_partition TEXT NOT NULL DEFAULT '',
                action TEXT NOT NULL CHECK (action IN ('read', 'write')),
                emitted_at TEXT NOT NULL,
                metadata_json TEXT
            );
            CREATE TABLE IF NOT EXISTS run_lineage (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
                task_id TEXT,
                attempt_id TEXT REFERENCES run_attempts(id) ON DELETE SET NULL,
                openlineage_event_json TEXT NOT NULL,
                emitted_at TEXT NOT NULL,
                exported_at TEXT,
                export_status TEXT
            );
            CREATE TABLE IF NOT EXISTS run_relationships (
                id TEXT PRIMARY KEY,
                parent_run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
                child_run_id TEXT REFERENCES runs(id) ON DELETE SET NULL,
                queued_run_id TEXT REFERENCES queued_runs(id) ON DELETE SET NULL,
                child_workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
                relationship TEXT NOT NULL,
                task_id TEXT,
                wait INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL,
                reason TEXT,
                details_json TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS scheduler_idempotency_keys (
                key TEXT PRIMARY KEY,
                run_id TEXT REFERENCES runs(id) ON DELETE SET NULL,
                queued_run_id TEXT REFERENCES queued_runs(id) ON DELETE SET NULL,
                workflow_id TEXT REFERENCES workflows(id) ON DELETE SET NULL,
                request_fingerprint TEXT,
                status TEXT NOT NULL DEFAULT 'reserved',
                task_id TEXT,
                attempt_id TEXT REFERENCES run_attempts(id) ON DELETE SET NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS scheduler_checkpoints (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
                task_id TEXT NOT NULL,
                attempt_id TEXT REFERENCES run_attempts(id) ON DELETE SET NULL,
                checkpoint_key TEXT NOT NULL,
                state_blob BLOB NOT NULL,
                state_size_bytes INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                UNIQUE(run_id, task_id, checkpoint_key)
            );
            CREATE TABLE IF NOT EXISTS scheduler_dead_letters (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL UNIQUE REFERENCES runs(id) ON DELETE CASCADE,
                workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
                task_id TEXT,
                last_attempt_id TEXT REFERENCES run_attempts(id) ON DELETE SET NULL,
                last_failure_at TEXT NOT NULL,
                last_exception TEXT NOT NULL,
                acknowledged_at TEXT,
                acknowledged_reason TEXT,
                acknowledged_by TEXT,
                recovery_run_id TEXT REFERENCES runs(id) ON DELETE SET NULL,
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS queue_events (
                id TEXT PRIMARY KEY,
                queue_name TEXT NOT NULL,
                corpus TEXT NOT NULL,
                workflow_id TEXT REFERENCES workflows(id) ON DELETE SET NULL,
                run_id TEXT REFERENCES runs(id) ON DELETE SET NULL,
                event_type TEXT NOT NULL,
                reason TEXT,
                emitted_at TEXT NOT NULL,
                details_json TEXT
            );
            CREATE TABLE IF NOT EXISTS workflow_resource_samples (
                id TEXT PRIMARY KEY,
                run_id TEXT REFERENCES runs(id) ON DELETE CASCADE,
                workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
                queue_name TEXT,
                corpus TEXT NOT NULL,
                pid INTEGER,
                sampled_at TEXT NOT NULL,
                cpu_percent REAL,
                memory_rss_bytes INTEGER,
                memory_vms_bytes INTEGER,
                swap_bytes INTEGER,
                labels_json TEXT
            );
            CREATE TABLE IF NOT EXISTS workflow_token_usage (
                id TEXT PRIMARY KEY,
                run_id TEXT REFERENCES runs(id) ON DELETE CASCADE,
                workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
                task_id TEXT,
                provider TEXT NOT NULL,
                model TEXT,
                token_kind TEXT NOT NULL,
                token_count INTEGER NOT NULL,
                emitted_at TEXT NOT NULL,
                labels_json TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_runs_workflow_started ON runs(workflow_id, started_at DESC);
            CREATE INDEX IF NOT EXISTS idx_queued_runs_queue_status ON queued_runs(queue_name, status, priority DESC, queued_at ASC);
            CREATE INDEX IF NOT EXISTS idx_queued_runs_workflow_status ON queued_runs(workflow_id, status);
            CREATE INDEX IF NOT EXISTS idx_workflow_mutex_locks_workflow ON workflow_mutex_locks(workflow_id);
            CREATE INDEX IF NOT EXISTS idx_run_attempts_run_task ON run_attempts(run_id, task_id, attempt_number);
            CREATE INDEX IF NOT EXISTS idx_run_tasks_run_task ON run_tasks(run_id, task_id);
            CREATE INDEX IF NOT EXISTS idx_run_tasks_status ON run_tasks(status, started_at);
            CREATE INDEX IF NOT EXISTS idx_run_metrics_run ON run_metrics(run_id);
            CREATE INDEX IF NOT EXISTS idx_run_metrics_name_time ON run_metrics(metric_name, emitted_at);
            CREATE INDEX IF NOT EXISTS idx_run_inputs_run ON run_inputs(run_id);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_run_inputs_unique_key ON run_inputs(run_id, COALESCE(task_id, ''), key);
            CREATE INDEX IF NOT EXISTS idx_run_outputs_run ON run_outputs(run_id);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_run_outputs_unique_key ON run_outputs(run_id, COALESCE(task_id, ''), key);
            CREATE INDEX IF NOT EXISTS idx_scheduler_assets_identity ON scheduler_assets(asset_kind, asset_namespace, asset_partition);
            CREATE INDEX IF NOT EXISTS idx_run_assets_run ON run_assets(run_id);
            CREATE INDEX IF NOT EXISTS idx_run_assets_identity_time ON run_assets(asset_kind, asset_namespace, asset_partition, emitted_at);
            CREATE INDEX IF NOT EXISTS idx_run_lineage_run ON run_lineage(run_id);
            CREATE INDEX IF NOT EXISTS idx_run_lineage_time ON run_lineage(emitted_at);
            CREATE INDEX IF NOT EXISTS idx_run_relationships_parent ON run_relationships(parent_run_id);
            CREATE INDEX IF NOT EXISTS idx_run_relationships_child ON run_relationships(child_run_id);
            CREATE INDEX IF NOT EXISTS idx_idempotency_run_task ON scheduler_idempotency_keys(run_id, task_id);
            CREATE INDEX IF NOT EXISTS idx_checkpoints_run_task ON scheduler_checkpoints(run_id, task_id);
            CREATE INDEX IF NOT EXISTS idx_dead_letters_workflow ON scheduler_dead_letters(workflow_id, last_failure_at);
            CREATE INDEX IF NOT EXISTS idx_queue_events_run ON queue_events(run_id);
            CREATE INDEX IF NOT EXISTS idx_resource_samples_workflow_time ON workflow_resource_samples(workflow_id, sampled_at);
            CREATE INDEX IF NOT EXISTS idx_resource_samples_run ON workflow_resource_samples(run_id);
            CREATE INDEX IF NOT EXISTS idx_token_usage_workflow_time ON workflow_token_usage(workflow_id, emitted_at);
            CREATE INDEX IF NOT EXISTS idx_token_usage_run ON workflow_token_usage(run_id);",
        )?;
        let _ = conn.execute_batch(
            "ALTER TABLE scheduler_dead_letters ADD COLUMN acknowledged_reason TEXT;",
        );
        let _ = conn
            .execute_batch("ALTER TABLE scheduler_dead_letters ADD COLUMN acknowledged_by TEXT;");
        let _ = conn.execute_batch("ALTER TABLE scheduler_dead_letters ADD COLUMN recovery_run_id TEXT REFERENCES runs(id) ON DELETE SET NULL;");
        conn.execute_batch(
            "INSERT OR IGNORE INTO scheduler_config (key, value) VALUES ('global_parallelism_cap', '4');
             INSERT OR IGNORE INTO scheduler_config (key, value) VALUES ('notify_on_failure', 'true');
             INSERT OR IGNORE INTO scheduler_config (key, value) VALUES ('notify_on_success', 'false');",
        )?;

        // Apply versioned, transactional migrations (with a pre-migration backup)
        // on top of the idempotent base schema established above.
        self.run_migrations(&conn, existing_version)?;

        // Seed default queues AFTER migrations so this always runs against the
        // final `environment`-keyed queues shape (v5+), never the legacy corpus
        // shape. Idempotent.
        conn.execute_batch(
            "INSERT OR IGNORE INTO queues (name, environment, capacity) VALUES ('source-default', 'source', 4);
             INSERT OR IGNORE INTO queues (name, environment, capacity) VALUES ('instance-default', 'instance', 2);",
        )?;
        Ok(())
    }

    /// Ordered list of schema migrations. Each entry is `(target_version, apply)`
    /// where `apply` runs inside its own transaction and is only executed when the
    /// DB's current `user_version` is below `target_version`. Never renumber or
    /// mutate a shipped migration — only append. The base schema (v1) is created
    /// idempotently in [`Database::init`], so this list starts at v2.
    #[allow(clippy::type_complexity)]
    fn migrations() -> Vec<(i64, fn(&Connection) -> rusqlite::Result<()>)> {
        vec![
            (2, Self::migrate_v2_environments),
            (3, Self::migrate_v3_workflow_spec),
            (4, Self::migrate_v4_api_keys),
            (5, Self::migrate_v5_queue_environment),
            (6, Self::migrate_v6_run_retention_fk_actions),
            (7, Self::migrate_v7_idempotency_contract),
            (8, Self::migrate_v8_execution_metadata),
        ]
    }

    /// v8: persist worker/process ownership metadata for timeout, shutdown, and
    /// restart recovery without guessing from shared logs.
    fn migrate_v8_execution_metadata(conn: &Connection) -> rusqlite::Result<()> {
        let _ = conn.execute_batch("ALTER TABLE runs ADD COLUMN execution_worker_id TEXT;");
        let _ = conn.execute_batch("ALTER TABLE runs ADD COLUMN process_pid INTEGER;");
        let _ = conn.execute_batch("ALTER TABLE runs ADD COLUMN process_pgid INTEGER;");
        let _ = conn.execute_batch("ALTER TABLE runs ADD COLUMN process_started_at TEXT;");
        let _ = conn.execute_batch(
            "ALTER TABLE runs ADD COLUMN stdout_truncated INTEGER NOT NULL DEFAULT 0;",
        );
        let _ = conn.execute_batch(
            "ALTER TABLE runs ADD COLUMN stderr_truncated INTEGER NOT NULL DEFAULT 0;",
        );
        let _ = conn.execute_batch(
            "ALTER TABLE runs ADD COLUMN task_events_truncated INTEGER NOT NULL DEFAULT 0;",
        );
        Ok(())
    }

    /// v6: make retention-safe `runs` foreign-key behavior explicit for queue
    /// and mutex rows that predate the full run-child cascade matrix.
    fn migrate_v6_run_retention_fk_actions(conn: &Connection) -> rusqlite::Result<()> {
        conn.execute_batch(
            "CREATE TEMP TABLE run_relationships_queued_run_refs_v6 AS
                SELECT id, queued_run_id
                FROM run_relationships
                WHERE queued_run_id IS NOT NULL;

            CREATE TABLE queued_runs_v6 (
                id TEXT PRIMARY KEY,
                run_id TEXT REFERENCES runs(id) ON DELETE SET NULL,
                workflow_id TEXT NOT NULL REFERENCES workflows(id),
                queue_name TEXT NOT NULL,
                priority INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'queued',
                queued_at TEXT NOT NULL,
                admitted_at TEXT,
                finished_at TEXT,
                trigger_kind TEXT,
                trigger_payload TEXT,
                upstream_run_id TEXT,
                input_json TEXT,
                rerun_of_run_id TEXT
            );
            INSERT INTO queued_runs_v6
                (id, run_id, workflow_id, queue_name, priority, status, queued_at,
                 admitted_at, finished_at, trigger_kind, trigger_payload,
                 upstream_run_id, input_json, rerun_of_run_id)
                SELECT id,
                       CASE WHEN run_id IS NULL OR EXISTS (SELECT 1 FROM runs r WHERE r.id = queued_runs.run_id)
                            THEN run_id ELSE NULL END,
                       workflow_id, queue_name, priority, status, queued_at,
                       admitted_at, finished_at, trigger_kind, trigger_payload,
                       upstream_run_id, input_json, rerun_of_run_id
                FROM queued_runs;
            DROP TABLE queued_runs;
            ALTER TABLE queued_runs_v6 RENAME TO queued_runs;
            CREATE INDEX IF NOT EXISTS idx_queued_runs_queue_status ON queued_runs(queue_name, status, priority DESC, queued_at ASC);
            CREATE INDEX IF NOT EXISTS idx_queued_runs_workflow_status ON queued_runs(workflow_id, status);

            UPDATE run_relationships
               SET queued_run_id = (
                   SELECT queued_run_id
                   FROM run_relationships_queued_run_refs_v6 refs
                   WHERE refs.id = run_relationships.id
               )
             WHERE id IN (SELECT id FROM run_relationships_queued_run_refs_v6)
               AND EXISTS (
                   SELECT 1
                   FROM queued_runs q
                   WHERE q.id = (
                       SELECT queued_run_id
                       FROM run_relationships_queued_run_refs_v6 refs
                       WHERE refs.id = run_relationships.id
                   )
               );
            DROP TABLE run_relationships_queued_run_refs_v6;

            CREATE TABLE workflow_mutex_locks_v6 (
                mutex_key TEXT PRIMARY KEY,
                workflow_id TEXT NOT NULL REFERENCES workflows(id),
                run_id TEXT REFERENCES runs(id) ON DELETE CASCADE,
                acquired_at TEXT NOT NULL
            );
            INSERT INTO workflow_mutex_locks_v6 (mutex_key, workflow_id, run_id, acquired_at)
                SELECT mutex_key,
                       workflow_id,
                       CASE WHEN run_id IS NULL OR EXISTS (SELECT 1 FROM runs r WHERE r.id = workflow_mutex_locks.run_id)
                            THEN run_id ELSE NULL END,
                       acquired_at
                FROM workflow_mutex_locks;
            DROP TABLE workflow_mutex_locks;
            ALTER TABLE workflow_mutex_locks_v6 RENAME TO workflow_mutex_locks;
            CREATE INDEX IF NOT EXISTS idx_workflow_mutex_locks_workflow ON workflow_mutex_locks(workflow_id);",
        )?;
        Ok(())
    }

    /// v7: extend idempotency records so queued dispatches can replay the
    /// original queued_run_id and mismatched key reuse can be rejected.
    fn migrate_v7_idempotency_contract(conn: &Connection) -> rusqlite::Result<()> {
        let _ = conn.execute_batch(
            "ALTER TABLE scheduler_idempotency_keys ADD COLUMN queued_run_id TEXT REFERENCES queued_runs(id) ON DELETE SET NULL;",
        );
        let _ = conn.execute_batch(
            "ALTER TABLE scheduler_idempotency_keys ADD COLUMN workflow_id TEXT REFERENCES workflows(id) ON DELETE SET NULL;",
        );
        let _ = conn.execute_batch(
            "ALTER TABLE scheduler_idempotency_keys ADD COLUMN request_fingerprint TEXT;",
        );
        let _ = conn.execute_batch(
            "ALTER TABLE scheduler_idempotency_keys ADD COLUMN status TEXT NOT NULL DEFAULT 'reserved';",
        );
        let _ = conn
            .execute_batch("ALTER TABLE scheduler_idempotency_keys ADD COLUMN updated_at TEXT;");
        conn.execute_batch(
            "UPDATE scheduler_idempotency_keys
                SET workflow_id = (SELECT workflow_id FROM runs r WHERE r.id = scheduler_idempotency_keys.run_id)
              WHERE workflow_id IS NULL AND run_id IS NOT NULL;
             UPDATE scheduler_idempotency_keys
                SET status = CASE
                    WHEN queued_run_id IS NOT NULL THEN 'queued'
                    WHEN run_id IS NOT NULL THEN 'admitted'
                    ELSE COALESCE(NULLIF(status, ''), 'reserved')
                END,
                    updated_at = COALESCE(updated_at, created_at, datetime('now'))
              WHERE updated_at IS NULL OR updated_at = '';
             CREATE INDEX IF NOT EXISTS idx_idempotency_queued_run ON scheduler_idempotency_keys(queued_run_id);
             CREATE INDEX IF NOT EXISTS idx_idempotency_workflow ON scheduler_idempotency_keys(workflow_id);",
        )?;
        Ok(())
    }

    /// v5: promote `environment` to the authoritative partition key across the
    /// queue/telemetry tables via a deliberate table rebuild (not a best-effort
    /// ALTER). The `queues` composite primary key becomes `(name, environment)`;
    /// `queue_events` and `workflow_resource_samples` replace their `corpus`
    /// column with `environment`. Data is copied `corpus -> environment`.
    fn migrate_v5_queue_environment(conn: &Connection) -> rusqlite::Result<()> {
        // --- queues: rebuild PK (name, corpus) -> (name, environment) ---
        conn.execute_batch(
            "CREATE TABLE queues_v5 (
                name TEXT NOT NULL,
                environment TEXT NOT NULL,
                capacity INTEGER NOT NULL DEFAULT 1,
                tag_cap INTEGER,
                max_queued INTEGER,
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT DEFAULT (datetime('now')),
                PRIMARY KEY (name, environment)
            );
            INSERT OR IGNORE INTO queues_v5 (name, environment, capacity, tag_cap, max_queued, created_at, updated_at)
                SELECT name,
                       COALESCE(NULLIF(environment, ''), corpus),
                       capacity, tag_cap, max_queued,
                       COALESCE(created_at, datetime('now')),
                       COALESCE(updated_at, datetime('now'))
                FROM queues;
            DROP TABLE queues;
            ALTER TABLE queues_v5 RENAME TO queues;",
        )?;

        // --- queue_events: corpus -> environment ---
        conn.execute_batch(
            "CREATE TABLE queue_events_v5 (
                id TEXT PRIMARY KEY,
                queue_name TEXT NOT NULL,
                environment TEXT NOT NULL,
                workflow_id TEXT REFERENCES workflows(id) ON DELETE SET NULL,
                run_id TEXT REFERENCES runs(id) ON DELETE SET NULL,
                event_type TEXT NOT NULL,
                reason TEXT,
                emitted_at TEXT NOT NULL,
                details_json TEXT
            );
            INSERT INTO queue_events_v5 (id, queue_name, environment, workflow_id, run_id, event_type, reason, emitted_at, details_json)
                SELECT id, queue_name, corpus, workflow_id, run_id, event_type, reason, emitted_at, details_json
                FROM queue_events;
            DROP TABLE queue_events;
            ALTER TABLE queue_events_v5 RENAME TO queue_events;
            CREATE INDEX IF NOT EXISTS idx_queue_events_queue_time ON queue_events(queue_name, environment, emitted_at);
            CREATE INDEX IF NOT EXISTS idx_queue_events_run ON queue_events(run_id);",
        )?;

        // --- workflow_resource_samples: corpus -> environment ---
        conn.execute_batch(
            "CREATE TABLE workflow_resource_samples_v5 (
                id TEXT PRIMARY KEY,
                run_id TEXT REFERENCES runs(id) ON DELETE CASCADE,
                workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
                queue_name TEXT,
                environment TEXT NOT NULL,
                pid INTEGER,
                sampled_at TEXT NOT NULL,
                cpu_percent REAL,
                memory_rss_bytes INTEGER,
                memory_vms_bytes INTEGER,
                swap_bytes INTEGER,
                labels_json TEXT
            );
            INSERT INTO workflow_resource_samples_v5 (id, run_id, workflow_id, queue_name, environment, pid, sampled_at, cpu_percent, memory_rss_bytes, memory_vms_bytes, swap_bytes, labels_json)
                SELECT id, run_id, workflow_id, queue_name, corpus, pid, sampled_at, cpu_percent, memory_rss_bytes, memory_vms_bytes, swap_bytes, labels_json
                FROM workflow_resource_samples;
            DROP TABLE workflow_resource_samples;
            ALTER TABLE workflow_resource_samples_v5 RENAME TO workflow_resource_samples;
            CREATE INDEX IF NOT EXISTS idx_resource_samples_workflow_time ON workflow_resource_samples(workflow_id, sampled_at);
            CREATE INDEX IF NOT EXISTS idx_resource_samples_run ON workflow_resource_samples(run_id);",
        )?;

        Ok(())
    }

    /// v4: HTTP API key store (salted hashes only — never the plaintext) and an
    /// audit log of authenticated API requests.
    fn migrate_v4_api_keys(conn: &Connection) -> rusqlite::Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS api_keys (
                id TEXT PRIMARY KEY,
                name TEXT,
                key_hash TEXT NOT NULL,
                salt TEXT NOT NULL,
                scopes TEXT NOT NULL DEFAULT 'read',
                created_at TEXT DEFAULT (datetime('now')),
                last_used_at TEXT,
                revoked INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS api_audit_log (
                id TEXT PRIMARY KEY,
                key_id TEXT,
                method TEXT NOT NULL,
                path TEXT NOT NULL,
                status INTEGER NOT NULL,
                remote TEXT,
                at TEXT DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_api_audit_time ON api_audit_log(at);",
        )?;
        Ok(())
    }

    /// v3: add the workflow execution model (`kind` + `spec_json`) and the
    /// action dead-letter table for exhausted webhook deliveries.
    fn migrate_v3_workflow_spec(conn: &Connection) -> rusqlite::Result<()> {
        let _ = conn.execute_batch(
            "ALTER TABLE workflows ADD COLUMN kind TEXT NOT NULL DEFAULT 'generic';",
        );
        let _ = conn.execute_batch("ALTER TABLE workflows ADD COLUMN spec_json TEXT;");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS action_dead_letters (
                id TEXT PRIMARY KEY,
                run_id TEXT,
                action_kind TEXT NOT NULL,
                target TEXT,
                last_error TEXT NOT NULL,
                created_at TEXT DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_action_dead_letters_run ON action_dead_letters(run_id);",
        )?;
        Ok(())
    }

    /// v2: introduce first-class **environments** and split the overloaded
    /// `corpus` into a partition (`environment`) + a governance flag
    /// (`managed_externally`). Additive: `corpus` is preserved as a shadow so
    /// existing queue/telemetry code keeps working during the transition.
    fn migrate_v2_environments(conn: &Connection) -> rusqlite::Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS environments (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                description TEXT,
                working_dir TEXT,
                default_queue_capacity INTEGER,
                default_tag_cap INTEGER,
                default_max_queued INTEGER,
                managed_externally INTEGER NOT NULL DEFAULT 0,
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT DEFAULT (datetime('now'))
            );",
        )?;

        // Additive columns on workflows (ignore if a prior partial run added them).
        let _ = conn.execute_batch("ALTER TABLE workflows ADD COLUMN environment TEXT;");
        let _ = conn.execute_batch(
            "ALTER TABLE workflows ADD COLUMN managed_externally INTEGER NOT NULL DEFAULT 0;",
        );
        // Additive shadow column on queues (composite-PK rebuild to key on
        // environment is a deliberate, separate migration — see plan; this keeps
        // corpus authoritative for now while surfacing environment).
        let _ = conn.execute_batch("ALTER TABLE queues ADD COLUMN environment TEXT;");

        // Backfill from the legacy corpus value.
        conn.execute_batch(
            "UPDATE workflows SET environment = corpus WHERE environment IS NULL OR environment = '';
             UPDATE workflows SET managed_externally = CASE WHEN corpus = 'source' THEN 1 ELSE 0 END;
             UPDATE queues SET environment = corpus WHERE environment IS NULL OR environment = '';",
        )?;

        // Seed the two continuity environments plus any distinct corpus values
        // already present in the data. `source` is externally-managed by default.
        conn.execute_batch(
            "INSERT OR IGNORE INTO environments (id, name, managed_externally)
                VALUES ('source', 'source', 1), ('instance', 'instance', 0);
             INSERT OR IGNORE INTO environments (id, name)
                SELECT DISTINCT corpus, corpus FROM workflows
                WHERE corpus IS NOT NULL AND corpus <> ''
                  AND corpus NOT IN (SELECT name FROM environments);
             INSERT OR IGNORE INTO environments (id, name)
                SELECT DISTINCT corpus, corpus FROM queues
                WHERE corpus IS NOT NULL AND corpus <> ''
                  AND corpus NOT IN (SELECT name FROM environments);",
        )?;
        Ok(())
    }

    /// Copy the live DB (including WAL contents) to a timestamped sidecar file
    /// before mutating migrations run, so a failed/partial upgrade is recoverable.
    fn backup_before_migration(&self, from_version: i64) -> rusqlite::Result<()> {
        let backup_path = format!(
            "{}.pre-migrate-v{}-{}.bak",
            self.path,
            from_version,
            chrono::Utc::now().format("%Y%m%dT%H%M%S")
        );
        // VACUUM INTO produces a consistent standalone copy that folds in WAL.
        let conn = self.conn()?;
        match conn.execute("VACUUM INTO ?1", params![backup_path]) {
            Ok(_) => {
                log::info!("Pre-migration backup written to {backup_path}");
                Self::prune_migration_backups(&self.path);
                Ok(())
            }
            Err(err) => {
                log::error!("Pre-migration backup failed: {err}");
                Err(err)
            }
        }
    }

    /// Retain only the most recent `MIGRATION_BACKUP_KEEP` pre-migration
    /// sidecars for this DB so repeated upgrades cannot accumulate unbounded
    /// `.bak` copies. Best-effort: prune failures are logged, never fatal.
    fn prune_migration_backups(path: &str) {
        const MIGRATION_BACKUP_KEEP: usize = 3;
        let p = std::path::Path::new(path);
        let dir = match p.parent() {
            Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
            _ => std::path::PathBuf::from("."),
        };
        let Some(file_name) = p.file_name().and_then(|n| n.to_str()) else {
            return;
        };
        let prefix = format!("{file_name}.pre-migrate-");
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };
        let mut sidecars: Vec<(std::time::SystemTime, std::path::PathBuf)> = entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name();
                let name = name.to_str()?;
                if name.starts_with(&prefix) && name.ends_with(".bak") {
                    let mtime = entry.metadata().ok()?.modified().ok()?;
                    Some((mtime, entry.path()))
                } else {
                    None
                }
            })
            .collect();
        // Newest first; delete everything past the retention window.
        sidecars.sort_by(|a, b| b.0.cmp(&a.0));
        for (_, stale) in sidecars.into_iter().skip(MIGRATION_BACKUP_KEEP) {
            if let Err(err) = std::fs::remove_file(&stale) {
                log::warn!("Failed to prune old migration backup {stale:?}: {err}");
            }
        }
    }

    /// Run any pending migrations. Each migration is applied atomically and the
    /// `user_version` is advanced only on success; a failure rolls back and
    /// aborts so the DB is never left partially migrated.
    fn run_migrations(&self, conn: &Connection, from_version: i64) -> rusqlite::Result<()> {
        let migrations = Self::migrations();
        let pending: Vec<_> = migrations
            .into_iter()
            .filter(|(target, _)| *target > from_version && *target <= CURRENT_SCHEMA_VERSION)
            .collect();

        if pending.is_empty() {
            // Baseline / already-current: just record the version stamp (legacy
            // DBs report user_version=0 even though their schema is current).
            if from_version < CURRENT_SCHEMA_VERSION {
                conn.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)?;
            }
            return Ok(());
        }

        // A real data-affecting upgrade is about to run — back up first.
        self.backup_before_migration(from_version)?;

        for (target, apply) in pending {
            conn.execute_batch("BEGIN")?;
            match apply(conn) {
                Ok(()) => {
                    conn.pragma_update(None, "user_version", target)?;
                    conn.execute_batch("COMMIT")?;
                    log::info!("Applied schema migration to v{target}");
                }
                Err(err) => {
                    let _ = conn.execute_batch("ROLLBACK");
                    log::error!("Schema migration to v{target} failed, rolled back: {err}");
                    return Err(err);
                }
            }
        }
        conn.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)?;
        Ok(())
    }

    pub fn list_workflows(&self) -> rusqlite::Result<Vec<Workflow>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, description, script_path, cron_schedule, enabled, async_mode, last_run_at, created_at, updated_at, email_on_failure, timezone, corpus, domain, trigger_config, queue_config, COALESCE(NULLIF(environment, ''), corpus), COALESCE(managed_externally, CASE WHEN corpus = 'source' THEN 1 ELSE 0 END), COALESCE(kind, 'generic'), spec_json FROM workflows ORDER BY corpus, name"
        )?;
        let rows = stmt.query_map([], Self::row_to_workflow)?;
        rows.collect()
    }

    /// Shared projection decoder for the standard workflow column list used by
    /// `list_workflows`, `get_workflow`, and `list_workflows_filtered`.
    fn row_to_workflow(row: &rusqlite::Row<'_>) -> rusqlite::Result<Workflow> {
        let corpus = row
            .get::<_, String>(12)
            .unwrap_or_else(|_| "source".to_string());
        Ok(Workflow {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            script_path: row.get(3)?,
            cron_schedule: row.get(4)?,
            enabled: row.get::<_, i32>(5)? != 0,
            async_mode: row.get::<_, i32>(6).unwrap_or(0) != 0,
            last_run_at: row.get(7)?,
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
            email_on_failure: row.get::<_, i32>(10).unwrap_or(1) != 0,
            timezone: row
                .get::<_, String>(11)
                .unwrap_or_else(|_| "UTC".to_string()),
            environment: row.get::<_, String>(16).unwrap_or_else(|_| corpus.clone()),
            managed_externally: row.get::<_, i32>(17).unwrap_or(0) != 0,
            kind: row
                .get::<_, String>(18)
                .unwrap_or_else(|_| "generic".to_string()),
            spec_json: row.get(19).unwrap_or(None),
            corpus,
            domain: row.get(13).unwrap_or(None),
            trigger_config: row.get(14).unwrap_or(None),
            queue_config: row.get(15).unwrap_or(None),
        })
    }

    pub fn get_workflow(&self, id: &str) -> rusqlite::Result<Workflow> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT id, name, description, script_path, cron_schedule, enabled, async_mode, last_run_at, created_at, updated_at, email_on_failure, timezone, corpus, domain, trigger_config, queue_config, COALESCE(NULLIF(environment, ''), corpus), COALESCE(managed_externally, CASE WHEN corpus = 'source' THEN 1 ELSE 0 END), COALESCE(kind, 'generic'), spec_json FROM workflows WHERE id = ?1",
            params![id],
            Self::row_to_workflow,
        )
    }

    pub fn set_last_run_at(&self, id: &str, time: &str) -> rusqlite::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE workflows SET last_run_at = ?2 WHERE id = ?1",
            params![id, time],
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)] // Column-per-arg persistence writer.
    pub fn create_workflow(
        &self,
        name: &str,
        description: Option<&str>,
        script_path: &str,
        cron_schedule: &str,
        async_mode: bool,
        email_on_failure: bool,
        timezone: &str,
        corpus: &str,
        domain: Option<&str>,
        trigger_config: Option<&str>,
        queue_config: Option<&str>,
    ) -> rusqlite::Result<Workflow> {
        let id = uuid::Uuid::new_v4().to_string();
        let conn = self.conn()?;
        let managed = if corpus == "source" { 1 } else { 0 };
        conn.execute(
            "INSERT INTO workflows (id, name, description, script_path, cron_schedule, async_mode, email_on_failure, timezone, corpus, environment, managed_externally, domain, trigger_config, queue_config) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![id, name, description, script_path, cron_schedule, async_mode as i32, email_on_failure as i32, timezone, corpus, corpus, managed, domain, trigger_config, queue_config],
        )?;
        self.get_workflow(&id)
    }

    #[allow(clippy::too_many_arguments)] // Column-per-arg persistence writer.
    pub fn update_workflow(
        &self,
        id: &str,
        name: &str,
        description: Option<&str>,
        script_path: &str,
        cron_schedule: &str,
        enabled: bool,
        async_mode: bool,
        email_on_failure: bool,
        timezone: &str,
        corpus: &str,
        domain: Option<&str>,
        trigger_config: Option<&str>,
        queue_config: Option<&str>,
    ) -> rusqlite::Result<Workflow> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE workflows SET name = ?2, description = ?3, script_path = ?4, cron_schedule = ?5, enabled = ?6, async_mode = ?7, email_on_failure = ?8, timezone = ?9, corpus = ?10, environment = ?10, domain = ?11, trigger_config = ?12, queue_config = ?13, updated_at = datetime('now') WHERE id = ?1",
            params![id, name, description, script_path, cron_schedule, enabled as i32, async_mode as i32, email_on_failure as i32, timezone, corpus, domain, trigger_config, queue_config],
        )?;
        self.get_workflow(id)
    }

    /// Set a workflow's authoritative `environment` (partition). Used when a
    /// UI/API caller targets an environment distinct from the legacy corpus.
    pub fn set_workflow_environment(&self, id: &str, environment: &str) -> rusqlite::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE workflows SET environment = ?2, updated_at = datetime('now') WHERE id = ?1",
            params![id, environment],
        )?;
        Ok(())
    }

    /// Explicitly set the governance flag, decoupled from `corpus`. Used by the
    /// API registration path to mark a workflow externally-managed regardless of
    /// its environment.
    pub fn set_workflow_managed_externally(&self, id: &str, managed: bool) -> rusqlite::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE workflows SET managed_externally = ?2, updated_at = datetime('now') WHERE id = ?1",
            params![id, managed as i32],
        )?;
        Ok(())
    }

    /// Persist a workflow's execution model + validated spec blob.
    pub fn set_workflow_spec(
        &self,
        id: &str,
        kind: &str,
        spec_json: Option<&str>,
    ) -> rusqlite::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE workflows SET kind = ?2, spec_json = ?3, updated_at = datetime('now') WHERE id = ?1",
            params![id, kind, spec_json],
        )?;
        Ok(())
    }

    /// Record an exhausted action delivery (e.g. a webhook that failed all
    /// retries) for later inspection/triage.
    pub fn record_action_dead_letter(
        &self,
        run_id: &str,
        action_kind: &str,
        target: &str,
        last_error: &str,
    ) -> rusqlite::Result<()> {
        let conn = self.conn()?;
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO action_dead_letters (id, run_id, action_kind, target, last_error) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, run_id, action_kind, target, last_error],
        )?;
        Ok(())
    }

    /// List API key metadata (never the hash/salt/secret).
    pub fn list_api_keys(&self) -> rusqlite::Result<Vec<ApiKeyInfo>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, scopes, created_at, last_used_at, revoked FROM api_keys ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ApiKeyInfo {
                id: row.get(0)?,
                name: row.get(1)?,
                scopes: row.get(2)?,
                created_at: row.get(3)?,
                last_used_at: row.get(4)?,
                revoked: row.get::<_, i32>(5)? != 0,
            })
        })?;
        rows.collect()
    }

    /// Revoke an API key (soft-delete). Returns rows affected.
    pub fn revoke_api_key(&self, id: &str) -> rusqlite::Result<usize> {
        let conn = self.conn()?;
        conn.execute("UPDATE api_keys SET revoked = 1 WHERE id = ?1", params![id])
    }

    /// Insert a pre-hashed API key record. Returns the generated key id.
    pub fn insert_api_key(
        &self,
        name: Option<&str>,
        key_hash: &str,
        salt: &str,
        scopes: &str,
    ) -> rusqlite::Result<String> {
        let id = format!("key_{}", uuid::Uuid::new_v4().simple());
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO api_keys (id, name, key_hash, salt, scopes) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, name, key_hash, salt, scopes],
        )?;
        Ok(id)
    }

    /// Fetch the salt/hash/scopes for a key id (only if not revoked).
    pub fn get_api_key(&self, id: &str) -> rusqlite::Result<Option<(String, String, String)>> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT key_hash, salt, scopes FROM api_keys WHERE id = ?1 AND revoked = 0",
            params![id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
    }

    pub fn touch_api_key(&self, id: &str) -> rusqlite::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE api_keys SET last_used_at = datetime('now') WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn record_api_audit(
        &self,
        key_id: Option<&str>,
        method: &str,
        path: &str,
        status: u16,
        remote: Option<&str>,
    ) -> rusqlite::Result<()> {
        let conn = self.conn()?;
        let id = uuid::Uuid::new_v4().to_string();
        let path = redact_audit_path(path);
        conn.execute(
            "INSERT INTO api_audit_log (id, key_id, method, path, status, remote) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, key_id, method, path, status as i64, remote],
        )?;
        Ok(())
    }

    pub fn get_idempotency_record(&self, key: &str) -> rusqlite::Result<Option<IdempotencyRecord>> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT run_id, queued_run_id, request_fingerprint
             FROM scheduler_idempotency_keys WHERE key = ?1",
            params![key],
            |row| {
                Ok(IdempotencyRecord {
                    run_id: row.get(0)?,
                    queued_run_id: row.get(1)?,
                    request_fingerprint: row.get(2)?,
                })
            },
        )
        .optional()
    }

    pub fn reserve_idempotency_key(
        &self,
        key: &str,
        workflow_id: &str,
        request_fingerprint: &str,
    ) -> rusqlite::Result<IdempotencyReservation> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn()?;
        let inserted = conn.execute(
            "INSERT OR IGNORE INTO scheduler_idempotency_keys
                (key, workflow_id, request_fingerprint, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'reserved', ?4, ?4)",
            params![key, workflow_id, request_fingerprint, now],
        )?;
        if inserted == 1 {
            Ok(IdempotencyReservation::Reserved)
        } else {
            Ok(IdempotencyReservation::Existing(
                self.get_idempotency_record(key)?
                    .expect("idempotency key exists"),
            ))
        }
    }

    pub fn complete_idempotency_key(
        &self,
        key: &str,
        run_id: Option<&str>,
        queued_run_id: Option<&str>,
        status: &str,
    ) -> rusqlite::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn()?;
        conn.execute(
            "UPDATE scheduler_idempotency_keys
                SET run_id = ?2, queued_run_id = ?3, status = ?4, updated_at = ?5
              WHERE key = ?1",
            params![key, run_id, queued_run_id, status, now],
        )?;
        Ok(())
    }

    pub fn delete_idempotency_reservation(
        &self,
        key: &str,
        request_fingerprint: &str,
    ) -> rusqlite::Result<usize> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM scheduler_idempotency_keys
              WHERE key = ?1
                AND request_fingerprint = ?2
                AND run_id IS NULL
                AND queued_run_id IS NULL
                AND status = 'reserved'",
            params![key, request_fingerprint],
        )
    }

    fn row_to_environment(row: &rusqlite::Row<'_>) -> rusqlite::Result<Environment> {
        Ok(Environment {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            working_dir: row.get(3)?,
            default_queue_capacity: row.get(4)?,
            default_tag_cap: row.get(5)?,
            default_max_queued: row.get(6)?,
            managed_externally: row.get::<_, i32>(7).unwrap_or(0) != 0,
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
        })
    }

    const ENVIRONMENT_COLUMNS: &'static str =
        "id, name, description, working_dir, default_queue_capacity, default_tag_cap, default_max_queued, managed_externally, created_at, updated_at";

    pub fn list_environments(&self) -> rusqlite::Result<Vec<Environment>> {
        let conn = self.conn()?;
        let sql = format!(
            "SELECT {} FROM environments ORDER BY name",
            Self::ENVIRONMENT_COLUMNS
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], Self::row_to_environment)?;
        rows.collect()
    }

    pub fn get_environment(&self, id: &str) -> rusqlite::Result<Environment> {
        let conn = self.conn()?;
        let sql = format!(
            "SELECT {} FROM environments WHERE id = ?1",
            Self::ENVIRONMENT_COLUMNS
        );
        conn.query_row(&sql, params![id], Self::row_to_environment)
    }

    pub fn get_environment_by_name(&self, name: &str) -> rusqlite::Result<Option<Environment>> {
        let conn = self.conn()?;
        let sql = format!(
            "SELECT {} FROM environments WHERE name = ?1",
            Self::ENVIRONMENT_COLUMNS
        );
        conn.query_row(&sql, params![name], Self::row_to_environment)
            .optional()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_environment(
        &self,
        name: &str,
        description: Option<&str>,
        working_dir: Option<&str>,
        default_queue_capacity: Option<i64>,
        default_tag_cap: Option<i64>,
        default_max_queued: Option<i64>,
        managed_externally: bool,
    ) -> rusqlite::Result<Environment> {
        let id = uuid::Uuid::new_v4().to_string();
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO environments (id, name, description, working_dir, default_queue_capacity, default_tag_cap, default_max_queued, managed_externally) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, name, description, working_dir, default_queue_capacity, default_tag_cap, default_max_queued, managed_externally as i32],
        )?;
        self.get_environment(&id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_environment(
        &self,
        id: &str,
        name: &str,
        description: Option<&str>,
        working_dir: Option<&str>,
        default_queue_capacity: Option<i64>,
        default_tag_cap: Option<i64>,
        default_max_queued: Option<i64>,
    ) -> rusqlite::Result<Environment> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE environments SET name = ?2, description = ?3, working_dir = ?4, default_queue_capacity = ?5, default_tag_cap = ?6, default_max_queued = ?7, updated_at = datetime('now') WHERE id = ?1",
            params![id, name, description, working_dir, default_queue_capacity, default_tag_cap, default_max_queued],
        )?;
        self.get_environment(id)
    }

    /// Number of workflows currently assigned to an environment (by name).
    pub fn count_workflows_in_environment(&self, name: &str) -> rusqlite::Result<i64> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT COUNT(*) FROM workflows WHERE COALESCE(NULLIF(environment, ''), corpus) = ?1",
            params![name],
            |row| row.get(0),
        )
    }

    pub fn delete_environment(&self, id: &str) -> rusqlite::Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM environments WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn delete_workflow(&self, id: &str) -> rusqlite::Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM workflow_mutex_locks WHERE workflow_id = ?1 OR run_id IN (SELECT id FROM runs WHERE workflow_id = ?1)",
            params![id],
        )?;
        tx.execute(
            "DELETE FROM queued_runs WHERE workflow_id = ?1 OR run_id IN (SELECT id FROM runs WHERE workflow_id = ?1)",
            params![id],
        )?;
        tx.execute(
            "DELETE FROM workflow_trigger_state WHERE workflow_id = ?1",
            params![id],
        )?;
        tx.execute("DELETE FROM runs WHERE workflow_id = ?1", params![id])?;
        tx.execute("DELETE FROM workflows WHERE id = ?1", params![id])?;
        tx.commit()?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn create_run_with_context(
        &self,
        workflow_id: &str,
        trigger_kind: Option<&str>,
        trigger_payload: Option<&str>,
        upstream_run_id: Option<&str>,
        input_json: Option<&str>,
        rerun_of_run_id: Option<&str>,
    ) -> rusqlite::Result<Run> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO runs (id, workflow_id, started_at, status, trigger_kind, trigger_payload, upstream_run_id, input_json, rerun_of_run_id) VALUES (?1, ?2, ?3, 'running', ?4, ?5, ?6, ?7, ?8)",
            params![id, workflow_id, now, trigger_kind, trigger_payload, upstream_run_id, input_json, rerun_of_run_id],
        )?;
        self.get_run(&id)
    }

    #[allow(clippy::too_many_arguments)] // Column-per-arg persistence writer.
    pub fn create_terminal_run_with_context(
        &self,
        workflow_id: &str,
        status: &str,
        trigger_kind: Option<&str>,
        trigger_payload: Option<&str>,
        upstream_run_id: Option<&str>,
        input_json: Option<&str>,
        rerun_of_run_id: Option<&str>,
    ) -> rusqlite::Result<Run> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO runs (id, workflow_id, started_at, finished_at, status, trigger_kind, trigger_payload, upstream_run_id, input_json, rerun_of_run_id) VALUES (?1, ?2, ?3, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![id, workflow_id, now, status, trigger_kind, trigger_payload, upstream_run_id, input_json, rerun_of_run_id],
        )?;
        self.get_run(&id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn admit_run_with_context(
        &self,
        workflow_id: &str,
        queue_name: &str,
        environment: &str,
        tags: &[String],
        trigger_kind: Option<&str>,
        trigger_payload: Option<&str>,
        upstream_run_id: Option<&str>,
        input_json: Option<&str>,
        rerun_of_run_id: Option<&str>,
        queued_run_id: Option<&str>,
        mutex_keys: &[String],
        trigger_state: Option<(&str, &str)>,
    ) -> rusqlite::Result<RunAdmission> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let mut conn = self.conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

        // Capacity re-check under the held write lock. The scheduler also
        // pre-checks capacity, but that read races the admission; re-counting
        // inside this IMMEDIATE transaction is what actually enforces the
        // queue/global/tag caps against concurrent admitters. Each running
        // run's queue identity and tags are derived exactly as the queue
        // metrics path does (canonical environment COALESCE +
        // `queue_identity_from_config`).
        let mut running: Vec<(String, String, Vec<String>)> = Vec::new();
        {
            let mut stmt = tx.prepare(
                "SELECT COALESCE(NULLIF(w.environment, ''), w.corpus, 'source') AS env, w.queue_config
                 FROM runs r JOIN workflows w ON w.id = r.workflow_id
                 WHERE r.status IN ('admitted', 'running')",
            )?;
            let rows = stmt.query_map([], |row| {
                let env: String = row.get(0)?;
                let queue_config: Option<String> = row.get(1)?;
                Ok((env, queue_config))
            })?;
            for row in rows {
                let (env, queue_config) = row?;
                let (queue, env) = queue_identity_from_config(queue_config.as_deref(), &env);
                let row_tags = queue_tags_from_config(queue_config.as_deref());
                running.push((queue, env, row_tags));
            }
        }
        let total_running = running.len() as i64;
        let queue_running = running
            .iter()
            .filter(|(queue, env, _)| queue == queue_name && env == environment)
            .count() as i64;

        let capacity = tx
            .query_row(
                "SELECT capacity FROM queues WHERE name = ?1 AND environment = ?2",
                params![queue_name, environment],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .unwrap_or(1)
            .max(1);
        if queue_running >= capacity {
            tx.rollback()?;
            return Ok(RunAdmission::AtCapacity);
        }

        let global_cap: i64 = {
            let raw: String = tx.query_row(
                "SELECT value FROM scheduler_config WHERE key = 'global_parallelism_cap'",
                [],
                |row| row.get(0),
            )?;
            raw.parse::<i64>().map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(e))
            })?
        };
        if total_running >= global_cap {
            tx.rollback()?;
            return Ok(RunAdmission::AtCapacity);
        }

        let tag_cap: Option<i64> = tx
            .query_row(
                "SELECT tag_cap FROM queues WHERE name = ?1 AND environment = ?2",
                params![queue_name, environment],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        if let Some(tag_cap) = tag_cap {
            for tag in tags {
                let tag_running = running
                    .iter()
                    .filter(|(queue, env, row_tags)| {
                        queue == queue_name
                            && env == environment
                            && row_tags.iter().any(|candidate| candidate == tag)
                    })
                    .count() as i64;
                if tag_running >= tag_cap {
                    tx.rollback()?;
                    return Ok(RunAdmission::AtCapacity);
                }
            }
        }

        for key in mutex_keys {
            let exists: i64 = tx.query_row(
                "SELECT COUNT(*) FROM workflow_mutex_locks WHERE mutex_key = ?1",
                params![key],
                |row| row.get(0),
            )?;
            if exists > 0 {
                tx.rollback()?;
                return Ok(RunAdmission::MutexBusy);
            }
        }

        tx.execute(
            "INSERT INTO runs (id, workflow_id, started_at, status, trigger_kind, trigger_payload, upstream_run_id, input_json, rerun_of_run_id)
             VALUES (?1, ?2, ?3, 'admitted', ?4, ?5, ?6, ?7, ?8)",
            params![
                id,
                workflow_id,
                now,
                trigger_kind,
                trigger_payload,
                upstream_run_id,
                input_json,
                rerun_of_run_id
            ],
        )?;

        if let Some(queued_run_id) = queued_run_id {
            let updated = tx.execute(
                "UPDATE queued_runs SET run_id = ?2, status = 'admitted', admitted_at = ?3 WHERE id = ?1 AND status = 'queued'",
                params![queued_run_id, id, now],
            )?;
            if updated == 0 {
                tx.rollback()?;
                return Ok(RunAdmission::QueuedRunUnavailable);
            }
        }

        for key in mutex_keys {
            tx.execute(
                "INSERT INTO workflow_mutex_locks (mutex_key, workflow_id, run_id, acquired_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![key, workflow_id, id, now],
            )?;
        }

        // Record the trigger fingerprint as fired in the same transaction so a
        // crash between run creation and trigger-state persistence cannot refire
        // the same file-arrival / asset-update trigger.
        if let Some((trigger_id, fingerprint)) = trigger_state {
            tx.execute(
                "INSERT INTO workflow_trigger_state (workflow_id, trigger_id, fingerprint, observed_at, fired_at)
                 VALUES (?1, ?2, ?3, ?4, ?4)
                 ON CONFLICT(workflow_id, trigger_id) DO UPDATE SET
                   fingerprint = excluded.fingerprint,
                   observed_at = excluded.observed_at,
                   fired_at = COALESCE(excluded.fired_at, workflow_trigger_state.fired_at)",
                params![workflow_id, trigger_id, fingerprint, now],
            )?;
        }

        tx.commit()?;
        self.get_run(&id).map(RunAdmission::Admitted)
    }

    pub fn mark_run_started(&self, id: &str, worker_id: &str) -> rusqlite::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE runs SET status = 'running', execution_worker_id = ?2 WHERE id = ?1 AND status IN ('admitted', 'running')",
            params![id, worker_id],
        )?;
        Ok(())
    }

    pub fn record_run_process(
        &self,
        id: &str,
        pid: i64,
        pgid: i64,
        process_started_at: Option<&str>,
    ) -> rusqlite::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE runs SET process_pid = ?2, process_pgid = ?3, process_started_at = ?4 WHERE id = ?1",
            params![id, pid, pgid, process_started_at],
        )?;
        Ok(())
    }

    pub fn finish_run(
        &self,
        id: &str,
        exit_code: i32,
        stdout: &str,
        stderr: &str,
        result_url: Option<&str>,
    ) -> rusqlite::Result<()> {
        let status = if exit_code == 0 { "success" } else { "failed" };
        self.finish_run_with_status_details(id, Some(exit_code), status, stdout, stderr, result_url)
    }

    pub fn finish_run_with_status_details(
        &self,
        id: &str,
        exit_code: Option<i32>,
        status: &str,
        stdout: &str,
        stderr: &str,
        result_url: Option<&str>,
    ) -> rusqlite::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE runs SET finished_at = ?2, exit_code = ?3, stdout = ?4, stderr = ?5, result_url = ?6, status = ?7 WHERE id = ?1",
            params![id, now, exit_code, stdout, stderr, result_url, status],
        )?;
        tx.execute(
            "UPDATE queued_runs SET status = ?2, finished_at = ?3 WHERE run_id = ?1",
            params![id, status, now],
        )?;
        tx.execute(
            "DELETE FROM workflow_mutex_locks WHERE run_id = ?1",
            params![id],
        )?;
        tx.commit()?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn finish_run_with_status(
        &self,
        id: &str,
        status: &str,
        stdout: &str,
        stderr: &str,
    ) -> rusqlite::Result<()> {
        self.finish_run_with_status_details(id, None, status, stdout, stderr, None)
    }
}

// Session 5 lands schema substrate before later sessions wire runtime callers.
#[allow(dead_code)]
impl Database {
    pub fn insert_run_attempt(
        &self,
        run_id: &str,
        task_id: &str,
        attempt_number: i64,
        status: &str,
        retry_reason: Option<&str>,
    ) -> rusqlite::Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO run_attempts (id, run_id, task_id, attempt_number, status, started_at, retry_reason)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, run_id, task_id, attempt_number, status, now, retry_reason],
        )?;
        Ok(id)
    }

    pub fn finish_run_attempt(
        &self,
        attempt_id: &str,
        status: &str,
        exit_code: Option<i32>,
        error_type: Option<&str>,
        error_message: Option<&str>,
    ) -> rusqlite::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn()?;
        conn.execute(
            "UPDATE run_attempts
             SET status = ?2, finished_at = ?3, exit_code = ?4, error_type = ?5, error_message = ?6
             WHERE id = ?1",
            params![
                attempt_id,
                status,
                now,
                exit_code,
                error_type,
                error_message
            ],
        )?;
        Ok(())
    }

    pub fn insert_run_task(
        &self,
        run_id: &str,
        attempt_id: Option<&str>,
        task_id: &str,
        status: &str,
        attempt_number: i64,
        details: Option<&serde_json::Value>,
    ) -> rusqlite::Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let details_json = json_to_string(details)?;
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO run_tasks (id, run_id, attempt_id, task_id, status, started_at, attempt_number, details_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, run_id, attempt_id, task_id, status, now, attempt_number, details_json],
        )?;
        Ok(id)
    }

    pub fn finish_run_task(
        &self,
        task_row_id: &str,
        status: &str,
        error_type: Option<&str>,
        error_message: Option<&str>,
        details: Option<&serde_json::Value>,
    ) -> rusqlite::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let details_json = json_to_string(details)?;
        let conn = self.conn()?;
        conn.execute(
            "UPDATE run_tasks
             SET status = ?2, finished_at = ?3, error_type = ?4, error_message = ?5, details_json = COALESCE(?6, details_json), updated_at = datetime('now')
             WHERE id = ?1",
            params![task_row_id, status, now, error_type, error_message, details_json],
        )?;
        Ok(())
    }

    pub fn insert_run_metric(
        &self,
        run_id: &str,
        task_id: Option<&str>,
        metric_name: &str,
        metric_value: f64,
        metric_unit: Option<&str>,
        labels: Option<&serde_json::Value>,
    ) -> rusqlite::Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let emitted_at = chrono::Utc::now().to_rfc3339();
        let labels_json = json_to_string(labels)?;
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO run_metrics (id, run_id, task_id, metric_name, metric_value, metric_unit, emitted_at, labels_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, run_id, task_id, metric_name, metric_value, metric_unit, emitted_at, labels_json],
        )?;
        Ok(id)
    }

    pub fn insert_run_input(
        &self,
        run_id: &str,
        task_id: Option<&str>,
        key: &str,
        value: &serde_json::Value,
        schema_version: &str,
    ) -> rusqlite::Result<String> {
        self.insert_run_io_value("run_inputs", run_id, task_id, key, value, schema_version)
    }

    pub fn insert_run_output(
        &self,
        run_id: &str,
        task_id: Option<&str>,
        key: &str,
        value: &serde_json::Value,
        schema_version: &str,
    ) -> rusqlite::Result<String> {
        self.insert_run_io_value("run_outputs", run_id, task_id, key, value, schema_version)
    }

    fn insert_run_io_value(
        &self,
        table: &str,
        run_id: &str,
        task_id: Option<&str>,
        key: &str,
        value: &serde_json::Value,
        schema_version: &str,
    ) -> rusqlite::Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let recorded_at = chrono::Utc::now().to_rfc3339();
        let value_json = serde_json::to_string(value)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let sql = format!(
            "INSERT INTO {} (id, run_id, task_id, key, value_json, schema_version, recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            table
        );
        let conn = self.conn()?;
        conn.execute(
            &sql,
            params![
                id,
                run_id,
                task_id,
                key,
                value_json,
                schema_version,
                recorded_at
            ],
        )?;
        Ok(id)
    }

    pub fn upsert_scheduler_asset(
        &self,
        asset_kind: &str,
        asset_namespace: &str,
        asset_partition: &str,
        last_action: Option<&str>,
        last_writer_run_id: Option<&str>,
        freshness_policy: Option<&serde_json::Value>,
    ) -> rusqlite::Result<String> {
        let asset_id = scheduler_asset_id(asset_kind, asset_namespace, asset_partition);
        let now = chrono::Utc::now().to_rfc3339();
        let freshness_policy_json = json_to_string(freshness_policy)?;
        let last_written_at = last_action
            .filter(|action| *action == "write")
            .map(|_| now.clone());
        let last_writer_run_id = if last_written_at.is_some() {
            last_writer_run_id
        } else {
            None
        };
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO scheduler_assets (
                asset_id, asset_kind, asset_namespace, asset_partition, last_action, last_written_at, last_writer_run_id, freshness_policy_json, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))
             ON CONFLICT(asset_kind, asset_namespace, asset_partition) DO UPDATE SET
                last_action = COALESCE(excluded.last_action, scheduler_assets.last_action),
                last_written_at = COALESCE(excluded.last_written_at, scheduler_assets.last_written_at),
                last_writer_run_id = CASE
                    WHEN excluded.last_written_at IS NOT NULL THEN excluded.last_writer_run_id
                    ELSE scheduler_assets.last_writer_run_id
                END,
                freshness_policy_json = COALESCE(excluded.freshness_policy_json, scheduler_assets.freshness_policy_json),
                updated_at = datetime('now')",
            params![asset_id, asset_kind, asset_namespace, asset_partition, last_action, last_written_at, last_writer_run_id, freshness_policy_json],
        )?;
        Ok(asset_id)
    }

    #[allow(clippy::too_many_arguments)] // Column-per-arg persistence writer.
    pub fn insert_run_asset(
        &self,
        run_id: &str,
        task_id: Option<&str>,
        attempt_id: Option<&str>,
        asset_kind: &str,
        asset_namespace: &str,
        asset_partition: &str,
        action: &str,
        metadata: Option<&serde_json::Value>,
    ) -> rusqlite::Result<String> {
        self.insert_run_asset_with_freshness(
            run_id,
            task_id,
            attempt_id,
            asset_kind,
            asset_namespace,
            asset_partition,
            action,
            metadata,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)] // Column-per-arg persistence writer.
    pub fn insert_run_asset_with_freshness(
        &self,
        run_id: &str,
        task_id: Option<&str>,
        attempt_id: Option<&str>,
        asset_kind: &str,
        asset_namespace: &str,
        asset_partition: &str,
        action: &str,
        metadata: Option<&serde_json::Value>,
        freshness_policy: Option<&serde_json::Value>,
    ) -> rusqlite::Result<String> {
        if !matches!(action, "read" | "write") {
            return Err(rusqlite::Error::InvalidParameterName(
                "asset action must be read or write".to_string(),
            ));
        }
        let asset_id = self.upsert_scheduler_asset(
            asset_kind,
            asset_namespace,
            asset_partition,
            Some(action),
            Some(run_id),
            freshness_policy,
        )?;
        let id = uuid::Uuid::new_v4().to_string();
        let emitted_at = chrono::Utc::now().to_rfc3339();
        let metadata_json = json_to_string(metadata)?;
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO run_assets (
                id, run_id, task_id, attempt_id, asset_id, asset_kind, asset_namespace, asset_partition, action, emitted_at, metadata_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![id, run_id, task_id, attempt_id, asset_id, asset_kind, asset_namespace, asset_partition, action, emitted_at, metadata_json],
        )?;
        Ok(id)
    }

    pub fn insert_run_lineage(
        &self,
        run_id: &str,
        task_id: Option<&str>,
        attempt_id: Option<&str>,
        openlineage_event: &serde_json::Value,
    ) -> rusqlite::Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let emitted_at = chrono::Utc::now().to_rfc3339();
        let openlineage_event_json = serde_json::to_string(openlineage_event)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO run_lineage (id, run_id, task_id, attempt_id, openlineage_event_json, emitted_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, run_id, task_id, attempt_id, openlineage_event_json, emitted_at],
        )?;
        Ok(id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn insert_run_relationship(
        &self,
        parent_run_id: &str,
        child_run_id: Option<&str>,
        queued_run_id: Option<&str>,
        child_workflow_id: &str,
        relationship: &str,
        task_id: Option<&str>,
        wait: bool,
        status: &str,
        reason: Option<&str>,
        details: Option<&serde_json::Value>,
    ) -> rusqlite::Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let details_json = json_to_string(details)?;
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO run_relationships (
                id, parent_run_id, child_run_id, queued_run_id, child_workflow_id, relationship,
                task_id, wait, status, reason, details_json, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
            params![
                id,
                parent_run_id,
                child_run_id,
                queued_run_id,
                child_workflow_id,
                relationship,
                task_id,
                wait,
                status,
                reason,
                details_json,
                now,
            ],
        )?;
        Ok(id)
    }

    pub fn list_run_relationships(&self, run_id: &str) -> rusqlite::Result<Vec<RunRelationship>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT rr.id, rr.parent_run_id, rr.child_run_id, rr.queued_run_id,
                    rr.child_workflow_id, w.name, rr.relationship, rr.task_id, rr.wait,
                    rr.status, rr.reason, rr.details_json, rr.created_at, rr.updated_at
             FROM run_relationships rr
             LEFT JOIN workflows w ON w.id = rr.child_workflow_id
             WHERE rr.parent_run_id = ?1 OR rr.child_run_id = ?1
             ORDER BY rr.created_at ASC",
        )?;
        let rows = stmt.query_map(params![run_id], |row| {
            let details_json: Option<String> = row.get(11)?;
            Ok(RunRelationship {
                id: row.get(0)?,
                parent_run_id: row.get(1)?,
                child_run_id: row.get(2)?,
                queued_run_id: row.get(3)?,
                child_workflow_id: row.get(4)?,
                child_workflow_name: row.get(5)?,
                relationship: row.get(6)?,
                task_id: row.get(7)?,
                wait: row.get::<_, i64>(8)? != 0,
                status: row.get(9)?,
                reason: row.get(10)?,
                details: parse_json_opt(details_json),
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            })
        })?;
        rows.collect()
    }

    pub fn latest_asset_write_matching(
        &self,
        asset_kind: &str,
        asset_namespace: Option<&str>,
        asset_partition: Option<&str>,
        _exclude_workflow_id: Option<&str>,
    ) -> rusqlite::Result<Option<AssetUpdateRecord>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT ra.asset_id, ra.asset_kind, ra.asset_namespace, ra.asset_partition,
                    ra.run_id, r.workflow_id, ra.task_id, ra.emitted_at
             FROM run_assets ra
             JOIN runs r ON r.id = ra.run_id
             WHERE ra.action = 'write'
               AND ra.asset_kind = ?1
               AND (?2 IS NULL OR ra.asset_namespace = ?2)
               AND (?3 IS NULL OR ra.asset_partition = ?3)
             ORDER BY ra.emitted_at DESC
             LIMIT 1",
        )?;
        let mut rows = stmt.query(params![asset_kind, asset_namespace, asset_partition])?;
        if let Some(row) = rows.next()? {
            Ok(Some(AssetUpdateRecord {
                asset_id: row.get(0).unwrap_or(None),
                asset_kind: row.get(1).unwrap_or_default(),
                asset_namespace: row.get(2).unwrap_or_default(),
                asset_partition: row.get(3).unwrap_or_default(),
                run_id: row.get(4).unwrap_or_default(),
                workflow_id: row.get(5).unwrap_or_default(),
                task_id: row.get(6).unwrap_or(None),
                emitted_at: row.get(7).unwrap_or_default(),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn query_stale_assets(
        &self,
        max_age_seconds: i64,
        asset_kind: Option<&str>,
    ) -> rusqlite::Result<Vec<SchedulerAsset>> {
        let modifier = format!("-{} seconds", max_age_seconds);
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT asset_id, asset_kind, asset_namespace, asset_partition, last_action,
                    last_written_at, last_writer_run_id, freshness_policy_json
             FROM scheduler_assets
             WHERE (?1 IS NULL OR asset_kind = ?1)
               AND (last_written_at IS NULL OR datetime(last_written_at) <= datetime('now', ?2))
             ORDER BY COALESCE(last_written_at, '') ASC, asset_kind ASC, asset_namespace ASC",
        )?;
        let rows = stmt.query_map(params![asset_kind, modifier], |row| {
            Ok(scheduler_asset_from_row(row))
        })?;
        rows.collect()
    }

    pub fn insert_idempotency_key(
        &self,
        key: &str,
        run_id: Option<&str>,
        task_id: Option<&str>,
        attempt_id: Option<&str>,
    ) -> rusqlite::Result<bool> {
        let created_at = chrono::Utc::now().to_rfc3339();
        let conn = self.conn()?;
        let inserted = conn.execute(
            "INSERT OR IGNORE INTO scheduler_idempotency_keys
                (key, run_id, workflow_id, status, task_id, attempt_id, created_at, updated_at)
             VALUES (?1, ?2, (SELECT workflow_id FROM runs WHERE id = ?2),
                     CASE WHEN ?2 IS NULL THEN 'reserved' ELSE 'admitted' END,
                     ?3, ?4, ?5, ?5)",
            params![key, run_id, task_id, attempt_id, created_at],
        )?;
        Ok(inserted == 1)
    }

    pub fn insert_scheduler_checkpoint(
        &self,
        run_id: &str,
        task_id: &str,
        attempt_id: Option<&str>,
        checkpoint_key: &str,
        state_blob: &[u8],
    ) -> rusqlite::Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let created_at = chrono::Utc::now().to_rfc3339();
        let state_size_bytes = state_blob.len() as i64;
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO scheduler_checkpoints (
                id, run_id, task_id, attempt_id, checkpoint_key, state_blob, state_size_bytes, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(run_id, task_id, checkpoint_key) DO UPDATE SET
                attempt_id = excluded.attempt_id,
                state_blob = excluded.state_blob,
                state_size_bytes = excluded.state_size_bytes,
                created_at = excluded.created_at",
            params![id, run_id, task_id, attempt_id, checkpoint_key, state_blob, state_size_bytes, created_at],
        )?;
        conn.query_row(
            "SELECT id FROM scheduler_checkpoints WHERE run_id = ?1 AND task_id = ?2 AND checkpoint_key = ?3",
            params![run_id, task_id, checkpoint_key],
            |row| row.get(0),
        )
    }

    pub fn upsert_scheduler_dead_letter(
        &self,
        run_id: &str,
        workflow_id: &str,
        task_id: Option<&str>,
        last_attempt_id: Option<&str>,
        last_exception: &str,
    ) -> rusqlite::Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let last_failure_at = chrono::Utc::now().to_rfc3339();
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO scheduler_dead_letters (
                id, run_id, workflow_id, task_id, last_attempt_id, last_failure_at, last_exception, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))
             ON CONFLICT(run_id) DO UPDATE SET
                task_id = excluded.task_id,
                last_attempt_id = excluded.last_attempt_id,
                last_failure_at = excluded.last_failure_at,
                last_exception = excluded.last_exception,
                acknowledged_at = NULL,
                updated_at = datetime('now')",
            params![id, run_id, workflow_id, task_id, last_attempt_id, last_failure_at, last_exception],
        )?;
        conn.query_row(
            "SELECT id FROM scheduler_dead_letters WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )
    }

    fn scheduler_dead_letter_from_row(
        row: &rusqlite::Row<'_>,
    ) -> rusqlite::Result<SchedulerDeadLetter> {
        Ok(SchedulerDeadLetter {
            id: row.get(0)?,
            run_id: row.get(1)?,
            workflow_id: row.get(2)?,
            workflow_name: row.get(3)?,
            task_id: row.get(4)?,
            last_attempt_id: row.get(5)?,
            last_failure_at: row.get(6)?,
            last_exception: row.get(7)?,
            acknowledged_at: row.get(8)?,
            acknowledged_reason: row.get(9)?,
            acknowledged_by: row.get(10)?,
            recovery_run_id: row.get(11)?,
            run_status: row.get(12)?,
            created_at: row.get(13)?,
            updated_at: row.get(14)?,
        })
    }

    pub fn list_scheduler_dead_letters(
        &self,
        include_acknowledged: bool,
        limit: i64,
    ) -> rusqlite::Result<Vec<SchedulerDeadLetter>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT d.id, d.run_id, d.workflow_id, w.name, d.task_id, d.last_attempt_id,
                    d.last_failure_at, d.last_exception, d.acknowledged_at,
                    d.acknowledged_reason, d.acknowledged_by, d.recovery_run_id,
                    r.status, d.created_at, d.updated_at
             FROM scheduler_dead_letters d
             LEFT JOIN workflows w ON w.id = d.workflow_id
             LEFT JOIN runs r ON r.id = d.run_id
             WHERE (?1 OR d.acknowledged_at IS NULL)
             ORDER BY d.acknowledged_at IS NOT NULL, d.last_failure_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![include_acknowledged, limit], |row| {
            Self::scheduler_dead_letter_from_row(row)
        })?;
        rows.collect()
    }

    pub fn get_scheduler_dead_letter(&self, id: &str) -> rusqlite::Result<SchedulerDeadLetter> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT d.id, d.run_id, d.workflow_id, w.name, d.task_id, d.last_attempt_id,
                    d.last_failure_at, d.last_exception, d.acknowledged_at,
                    d.acknowledged_reason, d.acknowledged_by, d.recovery_run_id,
                    r.status, d.created_at, d.updated_at
             FROM scheduler_dead_letters d
             LEFT JOIN workflows w ON w.id = d.workflow_id
             LEFT JOIN runs r ON r.id = d.run_id
             WHERE d.id = ?1",
            params![id],
            Self::scheduler_dead_letter_from_row,
        )
    }

    pub fn acknowledge_scheduler_dead_letter(
        &self,
        id: &str,
        reason: &str,
        operator: Option<&str>,
    ) -> rusqlite::Result<usize> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE scheduler_dead_letters
             SET acknowledged_at = ?2,
                 acknowledged_reason = ?3,
                 acknowledged_by = ?4,
                 updated_at = datetime('now')
             WHERE id = ?1 AND acknowledged_at IS NULL",
            params![id, chrono::Utc::now().to_rfc3339(), reason, operator],
        )
    }

    pub fn link_dead_letter_recovery(
        &self,
        id: &str,
        recovery_run_id: &str,
    ) -> rusqlite::Result<usize> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE scheduler_dead_letters
             SET recovery_run_id = ?2, updated_at = datetime('now')
             WHERE id = ?1",
            params![id, recovery_run_id],
        )
    }

    pub fn set_workflow_enabled(&self, id: &str, enabled: bool) -> rusqlite::Result<usize> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE workflows SET enabled = ?2, updated_at = datetime('now') WHERE id = ?1",
            params![id, enabled],
        )
    }

    #[allow(clippy::too_many_arguments)] // Column-per-arg persistence writer.
    pub fn insert_queue_event(
        &self,
        queue_name: &str,
        environment: &str,
        workflow_id: Option<&str>,
        run_id: Option<&str>,
        event_type: &str,
        reason: Option<&str>,
        details: Option<&serde_json::Value>,
    ) -> rusqlite::Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let emitted_at = chrono::Utc::now().to_rfc3339();
        let details_json = json_to_string(details)?;
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO queue_events (id, queue_name, environment, workflow_id, run_id, event_type, reason, emitted_at, details_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![id, queue_name, environment, workflow_id, run_id, event_type, reason, emitted_at, details_json],
        )?;
        Ok(id)
    }

    pub fn insert_workflow_resource_sample(
        &self,
        sample: &WorkflowResourceSample,
    ) -> rusqlite::Result<String> {
        let id = if sample.id.is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            sample.id.clone()
        };
        let labels_json = json_to_string(sample.labels.as_ref())?;
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO workflow_resource_samples (
                id, run_id, workflow_id, queue_name, environment, pid, sampled_at, cpu_percent, memory_rss_bytes, memory_vms_bytes, swap_bytes, labels_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                id,
                sample.run_id.as_deref(),
                &sample.workflow_id,
                sample.queue_name.as_deref(),
                &sample.environment,
                sample.pid,
                &sample.sampled_at,
                sample.cpu_percent,
                sample.memory_rss_bytes,
                sample.memory_vms_bytes,
                sample.swap_bytes,
                labels_json
            ],
        )?;
        Ok(id)
    }

    pub fn insert_workflow_token_usage(
        &self,
        usage: &WorkflowTokenUsage,
    ) -> rusqlite::Result<String> {
        let id = if usage.id.is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            usage.id.clone()
        };
        let labels_json = json_to_string(usage.labels.as_ref())?;
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO workflow_token_usage (
                id, run_id, workflow_id, task_id, provider, model, token_kind, token_count, emitted_at, labels_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                id,
                usage.run_id.as_deref(),
                &usage.workflow_id,
                usage.task_id.as_deref(),
                &usage.provider,
                usage.model.as_deref(),
                &usage.token_kind,
                usage.token_count,
                &usage.emitted_at,
                labels_json
            ],
        )?;
        Ok(id)
    }
}

impl Database {
    pub fn get_run(&self, id: &str) -> rusqlite::Result<Run> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT r.id, r.workflow_id, r.started_at, r.finished_at, r.exit_code, r.stdout, r.stderr, r.result_url, r.status, w.name, r.error_analysis, r.trigger_kind, r.trigger_payload, r.upstream_run_id, r.input_json, r.rerun_of_run_id
             FROM runs r LEFT JOIN workflows w ON r.workflow_id = w.id WHERE r.id = ?1",
            params![id],
            |row| Ok(run_from_row(row)),
        )
    }

    pub fn get_run_history(&self, workflow_id: &str, limit: i64) -> rusqlite::Result<Vec<Run>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT r.id, r.workflow_id, r.started_at, r.finished_at, r.exit_code, r.stdout, r.stderr, r.result_url, r.status, w.name, r.error_analysis, r.trigger_kind, r.trigger_payload, r.upstream_run_id, r.input_json, r.rerun_of_run_id
             FROM runs r LEFT JOIN workflows w ON r.workflow_id = w.id WHERE r.workflow_id = ?1 ORDER BY r.started_at DESC LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![workflow_id, limit], |row| Ok(run_from_row(row)))?;
        rows.collect()
    }

    pub fn get_recent_runs(&self, limit: i64) -> rusqlite::Result<Vec<Run>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT r.id, r.workflow_id, r.started_at, r.finished_at, r.exit_code, r.stdout, r.stderr, r.result_url, r.status, w.name, r.error_analysis, r.trigger_kind, r.trigger_payload, r.upstream_run_id, r.input_json, r.rerun_of_run_id
             FROM runs r LEFT JOIN workflows w ON r.workflow_id = w.id ORDER BY r.started_at DESC LIMIT ?1"
        )?;
        let rows = stmt.query_map(params![limit], |row| Ok(run_from_row(row)))?;
        rows.collect()
    }

    pub fn get_global_run_history(
        &self,
        status_filter: Option<&str>,
        trigger_kind: Option<&str>,
        corpus_filter: Option<&str>,
        domain_filter: Option<&str>,
        limit: i64,
    ) -> rusqlite::Result<Vec<Run>> {
        let status_filter = status_filter.unwrap_or("all");
        let trigger_kind = trigger_kind.unwrap_or("all");
        let corpus_filter = corpus_filter.unwrap_or("all");
        let domain_filter = domain_filter.unwrap_or("all");
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT r.id, r.workflow_id, r.started_at, r.finished_at, r.exit_code, r.stdout, r.stderr, r.result_url, r.status, w.name, r.error_analysis, r.trigger_kind, r.trigger_payload, r.upstream_run_id, r.input_json, r.rerun_of_run_id
             FROM runs r
             LEFT JOIN workflows w ON r.workflow_id = w.id
             WHERE (?1 = 'all' OR r.status = ?1)
               AND (?2 = 'all' OR COALESCE(r.trigger_kind, 'cron') = ?2)
               AND (?3 = 'all' OR COALESCE(NULLIF(w.environment, ''), w.corpus) = ?3)
               AND (?4 = 'all'
                 OR (?4 = '__unowned__' AND (w.domain IS NULL OR TRIM(w.domain) = ''))
                 OR TRIM(w.domain) = ?4)
             ORDER BY r.started_at DESC
             LIMIT ?5",
        )?;
        let rows = stmt.query_map(
            params![
                status_filter,
                trigger_kind,
                corpus_filter,
                domain_filter,
                limit
            ],
            |row| Ok(run_from_row(row)),
        )?;
        rows.collect()
    }

    pub fn retention_preview(
        &self,
        older_than_days: i64,
        dry_run: bool,
    ) -> rusqlite::Result<RetentionPreview> {
        let days = older_than_days.max(1);
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339();
        let conn = self.conn()?;
        let preserved_dead_letter_runs: i64 = conn.query_row(
            "SELECT COUNT(*)
             FROM runs r
             JOIN scheduler_dead_letters d ON d.run_id = r.id
             WHERE datetime(COALESCE(r.finished_at, r.started_at)) < datetime(?1)",
            params![cutoff],
            |row| row.get(0),
        )?;
        let candidate_runs: i64 = conn.query_row(
            "SELECT COUNT(*)
             FROM runs r
             WHERE datetime(COALESCE(r.finished_at, r.started_at)) < datetime(?1)
               AND NOT EXISTS (SELECT 1 FROM scheduler_dead_letters d WHERE d.run_id = r.id)",
            params![cutoff],
            |row| row.get(0),
        )?;
        Ok(RetentionPreview {
            cutoff,
            candidate_runs,
            preserved_dead_letter_runs,
            dry_run,
            deleted_runs: 0,
        })
    }

    pub fn cleanup_retention(
        &self,
        older_than_days: i64,
        dry_run: bool,
    ) -> rusqlite::Result<RetentionPreview> {
        let mut preview = self.retention_preview(older_than_days, dry_run)?;
        if dry_run {
            return Ok(preview);
        }
        let conn = self.conn()?;
        let deleted = conn.execute(
            "DELETE FROM runs
             WHERE datetime(COALESCE(finished_at, started_at)) < datetime(?1)
               AND NOT EXISTS (SELECT 1 FROM scheduler_dead_letters d WHERE d.run_id = runs.id)",
            params![preview.cutoff],
        )?;
        preview.deleted_runs = deleted as i64;
        preview.dry_run = false;
        let details = serde_json::json!({
            "cutoff": preview.cutoff,
            "deleted_runs": preview.deleted_runs,
            "preserved_dead_letter_runs": preview.preserved_dead_letter_runs,
        });
        let _ = self.insert_queue_event(
            "retention",
            "scheduler",
            None,
            None,
            "retention_cleanup",
            Some("manual retention cleanup"),
            Some(&details),
        );
        Ok(preview)
    }

    pub fn list_workflows_filtered(
        &self,
        corpus_filter: &str,
        domain_filter: &str,
    ) -> rusqlite::Result<Vec<Workflow>> {
        let corpus_filter = normalize_mission_corpus_filter(corpus_filter);
        let domain_filter = normalize_mission_domain_filter(domain_filter);
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, description, script_path, cron_schedule, enabled, async_mode, last_run_at, created_at, updated_at, email_on_failure, timezone, corpus, domain, trigger_config, queue_config, COALESCE(NULLIF(environment, ''), corpus), COALESCE(managed_externally, CASE WHEN corpus = 'source' THEN 1 ELSE 0 END), COALESCE(kind, 'generic'), spec_json
             FROM workflows w
             WHERE (?1 = 'all' OR COALESCE(NULLIF(w.environment, ''), w.corpus) = ?1)
               AND (?2 = 'all'
                 OR (?2 = '__unowned__' AND (w.domain IS NULL OR TRIM(w.domain) = ''))
                 OR TRIM(w.domain) = ?2)
             ORDER BY w.corpus, COALESCE(NULLIF(TRIM(w.domain), ''), 'Unowned'), w.name",
        )?;
        let rows = stmt.query_map(params![corpus_filter, domain_filter], Self::row_to_workflow)?;
        rows.collect()
    }

    pub fn mission_control_domains(
        &self,
        corpus_filter: &str,
    ) -> rusqlite::Result<Vec<DomainOption>> {
        let corpus_filter = normalize_mission_corpus_filter(corpus_filter);
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT COALESCE(NULLIF(TRIM(domain), ''), 'Unowned') AS owner, COUNT(*)
             FROM workflows
             WHERE (?1 = 'all' OR COALESCE(NULLIF(environment, ''), corpus) = ?1)
             GROUP BY owner
             ORDER BY CASE owner WHEN 'Unowned' THEN 1 ELSE 0 END, owner",
        )?;
        let rows = stmt.query_map(params![corpus_filter], |row| {
            let label: String = row.get(0)?;
            Ok(DomainOption {
                value: if label == "Unowned" {
                    "__unowned__".to_string()
                } else {
                    label.clone()
                },
                label,
                workflow_count: row.get(1)?,
            })
        })?;
        let mut out = vec![DomainOption {
            value: "all".to_string(),
            label: "All".to_string(),
            workflow_count: self.list_workflows_filtered(&corpus_filter, "all")?.len() as i64,
        }];
        out.extend(rows.collect::<rusqlite::Result<Vec<_>>>()?);
        Ok(out)
    }

    pub fn mission_control_header(
        &self,
        corpus_filter: &str,
        domain_filter: &str,
    ) -> rusqlite::Result<MissionControlHeader> {
        let corpus_filter = normalize_mission_corpus_filter(corpus_filter);
        let domain_filter = normalize_mission_domain_filter(domain_filter);
        let conn = self.conn()?;
        conn.query_row(
            "SELECT
                COUNT(DISTINCT CASE WHEN w.enabled = 1 THEN w.id END) AS active_workflows,
                SUM(CASE WHEN r.status IN ('admitted', 'running') THEN 1 ELSE 0 END) AS running_count,
                (SELECT COUNT(*)
                   FROM queued_runs q JOIN workflows qw ON qw.id = q.workflow_id
                  WHERE q.status IN ('queued', 'admitted')
                    AND (?1 = 'all' OR COALESCE(NULLIF(qw.environment, ''), qw.corpus) = ?1)
                    AND (?2 = 'all'
                      OR (?2 = '__unowned__' AND (qw.domain IS NULL OR TRIM(qw.domain) = ''))
                      OR TRIM(qw.domain) = ?2)) AS queued_count,
                SUM(CASE WHEN r.status IN ('failed', 'cancelled', 'cascade-skipped', 'dead_letter', 'dead_lettered')
                          AND datetime(COALESCE(r.finished_at, r.started_at)) >= datetime('now', '-1 day')
                         THEN 1 ELSE 0 END) AS recent_failures
             FROM workflows w
             LEFT JOIN runs r ON r.workflow_id = w.id
             WHERE (?1 = 'all' OR COALESCE(NULLIF(w.environment, ''), w.corpus) = ?1)
               AND (?2 = 'all'
                 OR (?2 = '__unowned__' AND (w.domain IS NULL OR TRIM(w.domain) = ''))
                 OR TRIM(w.domain) = ?2)",
            params![corpus_filter, domain_filter],
            |row| {
                Ok(MissionControlHeader {
                    active_workflows: row.get::<_, Option<i64>>(0)?.unwrap_or(0),
                    running_count: row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                    queued_count: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                    recent_failures: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
                })
            },
        )
    }

    pub fn mission_control_sla_summary(
        &self,
        corpus_filter: &str,
        domain_filter: &str,
        violations_count: i64,
    ) -> rusqlite::Result<MissionControlSlaSummary> {
        let corpus_filter = normalize_mission_corpus_filter(corpus_filter);
        let domain_filter = normalize_mission_domain_filter(domain_filter);
        let conn = self.conn()?;
        let (total, succeeded): (i64, i64) = conn.query_row(
            "SELECT COUNT(*),
                    SUM(CASE WHEN r.status IN ('success', 'succeeded') THEN 1 ELSE 0 END)
             FROM runs r
             JOIN workflows w ON w.id = r.workflow_id
             WHERE datetime(COALESCE(r.finished_at, r.started_at)) >= datetime('now', '-1 day')
               AND r.status IN ('success', 'succeeded', 'failed', 'cancelled', 'cascade-skipped', 'dead_letter', 'dead_lettered')
               AND (?1 = 'all' OR COALESCE(NULLIF(w.environment, ''), w.corpus) = ?1)
               AND (?2 = 'all'
                 OR (?2 = '__unowned__' AND (w.domain IS NULL OR TRIM(w.domain) = ''))
                 OR TRIM(w.domain) = ?2)",
            params![corpus_filter, domain_filter],
            |row| Ok((row.get(0)?, row.get::<_, Option<i64>>(1)?.unwrap_or(0))),
        )?;
        let median_wait_seconds = conn
            .query_row(
                "SELECT CAST((julianday(admitted_at) - julianday(queued_at)) * 86400 AS INTEGER)
                 FROM queued_runs q
                 JOIN workflows w ON w.id = q.workflow_id
                 WHERE q.admitted_at IS NOT NULL
                   AND datetime(q.queued_at) >= datetime('now', '-1 day')
                   AND (?1 = 'all' OR COALESCE(NULLIF(w.environment, ''), w.corpus) = ?1)
                   AND (?2 = 'all'
                     OR (?2 = '__unowned__' AND (w.domain IS NULL OR TRIM(w.domain) = ''))
                     OR TRIM(w.domain) = ?2)
                 ORDER BY (julianday(admitted_at) - julianday(queued_at))
                 LIMIT 1 OFFSET (
                   SELECT COUNT(*) / 2
                   FROM queued_runs mq
                   JOIN workflows mw ON mw.id = mq.workflow_id
                   WHERE mq.admitted_at IS NOT NULL
                     AND datetime(mq.queued_at) >= datetime('now', '-1 day')
                     AND (?1 = 'all' OR COALESCE(NULLIF(mw.environment, ''), mw.corpus) = ?1)
                     AND (?2 = 'all'
                       OR (?2 = '__unowned__' AND (mw.domain IS NULL OR TRIM(mw.domain) = ''))
                       OR TRIM(mw.domain) = ?2)
                 )",
                params![corpus_filter, domain_filter],
                |row| row.get(0),
            )
            .optional()?;
        let blocked_count: i64 = conn.query_row(
            "SELECT COUNT(*)
             FROM queued_runs q JOIN workflows w ON w.id = q.workflow_id
             WHERE q.status = 'queued'
               AND (?1 = 'all' OR COALESCE(NULLIF(w.environment, ''), w.corpus) = ?1)
               AND (?2 = 'all'
                 OR (?2 = '__unowned__' AND (w.domain IS NULL OR TRIM(w.domain) = ''))
                 OR TRIM(w.domain) = ?2)",
            params![corpus_filter, domain_filter],
            |row| row.get(0),
        )?;
        Ok(MissionControlSlaSummary {
            violations_count,
            success_rate_24h: if total > 0 {
                Some(succeeded as f64 / total as f64)
            } else {
                None
            },
            median_wait_seconds,
            long_running_count: violations_count,
            blocked_count,
        })
    }

    pub fn mission_control_recent_runs(
        &self,
        corpus_filter: &str,
        domain_filter: &str,
        limit: i64,
    ) -> rusqlite::Result<Vec<Run>> {
        let corpus_filter = normalize_mission_corpus_filter(corpus_filter);
        let domain_filter = normalize_mission_domain_filter(domain_filter);
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT r.id, r.workflow_id, r.started_at, r.finished_at, r.exit_code, r.stdout, r.stderr, r.result_url, r.status, w.name, r.error_analysis, r.trigger_kind, r.trigger_payload, r.upstream_run_id, r.input_json, r.rerun_of_run_id
             FROM runs r
             JOIN workflows w ON w.id = r.workflow_id
             WHERE (?1 = 'all' OR COALESCE(NULLIF(w.environment, ''), w.corpus) = ?1)
               AND (?2 = 'all'
                 OR (?2 = '__unowned__' AND (w.domain IS NULL OR TRIM(w.domain) = ''))
                 OR TRIM(w.domain) = ?2)
             ORDER BY r.started_at DESC
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![corpus_filter, domain_filter, limit], |row| {
            Ok(run_from_row(row))
        })?;
        rows.collect()
    }

    pub fn mission_control_failed_runs(
        &self,
        corpus_filter: &str,
        domain_filter: &str,
        limit: i64,
    ) -> rusqlite::Result<Vec<Run>> {
        let corpus_filter = normalize_mission_corpus_filter(corpus_filter);
        let domain_filter = normalize_mission_domain_filter(domain_filter);
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "WITH ranked_terminal AS (
                SELECT r.*,
                       ROW_NUMBER() OVER (
                         PARTITION BY r.workflow_id
                         ORDER BY COALESCE(r.finished_at, r.started_at) DESC, r.rowid DESC
                       ) AS terminal_rank
                FROM runs r
                WHERE r.status IN ('success', 'succeeded', 'failed', 'cancelled', 'cascade-skipped', 'dead_letter', 'dead_lettered')
             ),
             latest_terminal AS (
                SELECT * FROM ranked_terminal WHERE terminal_rank = 1
             )
             SELECT r.id, r.workflow_id, r.started_at, r.finished_at, r.exit_code, r.stdout, r.stderr, r.result_url, r.status, w.name, r.error_analysis, r.trigger_kind, r.trigger_payload, r.upstream_run_id, r.input_json, r.rerun_of_run_id
             FROM latest_terminal r
             JOIN workflows w ON w.id = r.workflow_id
             WHERE r.status IN ('failed', 'cancelled', 'cascade-skipped', 'dead_letter', 'dead_lettered')
               AND (?1 = 'all' OR COALESCE(NULLIF(w.environment, ''), w.corpus) = ?1)
               AND (?2 = 'all'
                 OR (?2 = '__unowned__' AND (w.domain IS NULL OR TRIM(w.domain) = ''))
                 OR TRIM(w.domain) = ?2)
             ORDER BY COALESCE(r.finished_at, r.started_at) DESC
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![corpus_filter, domain_filter, limit], |row| {
            Ok(run_from_row(row))
        })?;
        rows.collect()
    }

    pub fn mission_control_failed_run_count(
        &self,
        corpus_filter: &str,
        domain_filter: &str,
    ) -> rusqlite::Result<i64> {
        let corpus_filter = normalize_mission_corpus_filter(corpus_filter);
        let domain_filter = normalize_mission_domain_filter(domain_filter);
        let conn = self.conn()?;
        conn.query_row(
            "WITH ranked_terminal AS (
                SELECT r.*,
                       ROW_NUMBER() OVER (
                         PARTITION BY r.workflow_id
                         ORDER BY COALESCE(r.finished_at, r.started_at) DESC, r.rowid DESC
                       ) AS terminal_rank
                FROM runs r
                WHERE r.status IN ('success', 'succeeded', 'failed', 'cancelled', 'cascade-skipped', 'dead_letter', 'dead_lettered')
             ),
             latest_terminal AS (
                SELECT * FROM ranked_terminal WHERE terminal_rank = 1
             )
             SELECT COUNT(*)
             FROM latest_terminal r
             JOIN workflows w ON w.id = r.workflow_id
             WHERE r.status IN ('failed', 'cancelled', 'cascade-skipped', 'dead_letter', 'dead_lettered')
               AND (?1 = 'all' OR COALESCE(NULLIF(w.environment, ''), w.corpus) = ?1)
               AND (?2 = 'all'
                 OR (?2 = '__unowned__' AND (w.domain IS NULL OR TRIM(w.domain) = ''))
                 OR TRIM(w.domain) = ?2)",
            params![corpus_filter, domain_filter],
            |row| row.get(0),
        )
    }

    pub fn mission_control_live_activity(
        &self,
        corpus_filter: &str,
        domain_filter: &str,
        limit: i64,
    ) -> rusqlite::Result<Vec<MissionControlActivityItem>> {
        let corpus_filter = normalize_mission_corpus_filter(corpus_filter);
        let domain_filter = normalize_mission_domain_filter(domain_filter);
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT r.id, r.workflow_id, w.name, w.corpus, w.domain, r.status, r.started_at, r.finished_at
             FROM runs r
             JOIN workflows w ON w.id = r.workflow_id
             WHERE (?1 = 'all' OR COALESCE(NULLIF(w.environment, ''), w.corpus) = ?1)
               AND (?2 = 'all'
                 OR (?2 = '__unowned__' AND (w.domain IS NULL OR TRIM(w.domain) = ''))
                 OR TRIM(w.domain) = ?2)
             ORDER BY CASE r.status WHEN 'running' THEN 0 ELSE 1 END, r.started_at DESC
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![corpus_filter, domain_filter, limit], |row| {
            let run_id: String = row.get(0)?;
            let corpus: String = row.get(3)?;
            Ok(MissionControlActivityItem {
                id: run_id.clone(),
                workflow_id: row.get(1)?,
                workflow_name: row.get(2)?,
                environment: corpus.clone(),
                corpus,
                domain: owner_label(row.get(4)?),
                status: row.get(5)?,
                started_at: row.get(6)?,
                finished_at: row.get(7)?,
                run_id,
            })
        })?;
        rows.collect()
    }

    pub fn mission_control_freshness_ledger(
        &self,
        corpus_filter: &str,
        domain_filter: &str,
        max_age_seconds: i64,
        limit: i64,
    ) -> rusqlite::Result<Vec<MissionControlFreshnessItem>> {
        let corpus_filter = normalize_mission_corpus_filter(corpus_filter);
        let domain_filter = normalize_mission_domain_filter(domain_filter);
        let modifier = format!("-{} seconds", max_age_seconds.max(0));
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT a.asset_id, a.asset_kind, a.asset_namespace, a.asset_partition, a.last_action,
                    a.last_written_at, r.workflow_id, w.name, w.corpus, w.domain
             FROM scheduler_assets a
             LEFT JOIN runs r ON r.id = a.last_writer_run_id
             LEFT JOIN workflows w ON w.id = r.workflow_id
             WHERE (a.last_written_at IS NULL OR datetime(a.last_written_at) <= datetime('now', ?3))
               AND (
                 (w.id IS NOT NULL
                   AND (?1 = 'all' OR COALESCE(NULLIF(w.environment, ''), w.corpus) = ?1)
                   AND (?2 = 'all'
                     OR (?2 = '__unowned__' AND (w.domain IS NULL OR TRIM(w.domain) = ''))
                     OR TRIM(w.domain) = ?2))
                 OR (w.id IS NULL AND ?1 = 'all' AND ?2 = 'all')
               )
             ORDER BY COALESCE(a.last_written_at, '') ASC, a.asset_kind ASC, a.asset_namespace ASC
             LIMIT ?4",
        )?;
        let rows = stmt.query_map(
            params![corpus_filter, domain_filter, modifier, limit],
            |row| {
                let workflow_id: Option<String> = row.get(6)?;
                Ok(MissionControlFreshnessItem {
                    asset_id: row.get(0)?,
                    asset_kind: row.get(1)?,
                    asset_namespace: row.get(2)?,
                    asset_partition: row.get(3)?,
                    last_action: row.get(4)?,
                    last_written_at: row.get(5)?,
                    workflow_id: workflow_id.clone(),
                    workflow_name: row.get(7)?,
                    environment: row.get(8)?,
                    corpus: row.get(8)?,
                    domain: owner_label(row.get(9)?),
                    attribution: if workflow_id.is_some() {
                        "last_writer_run".to_string()
                    } else {
                        "unattributed_all_only".to_string()
                    },
                })
            },
        )?;
        rows.collect()
    }

    pub fn mission_control_workflow_telemetry(
        &self,
        corpus_filter: &str,
        domain_filter: &str,
        window_modifier: &str,
        limit: i64,
    ) -> rusqlite::Result<Vec<MissionControlWorkflowTelemetry>> {
        let corpus_filter = normalize_mission_corpus_filter(corpus_filter);
        let domain_filter = normalize_mission_domain_filter(domain_filter);
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "WITH visible_workflows AS (
                SELECT w.id, w.name, w.corpus, w.domain
                FROM workflows w
                WHERE w.enabled = 1
                  AND (?1 = 'all' OR COALESCE(NULLIF(w.environment, ''), w.corpus) = ?1)
                  AND (?2 = 'all'
                    OR (?2 = '__unowned__' AND (w.domain IS NULL OR TRIM(w.domain) = ''))
                    OR TRIM(w.domain) = ?2)
                ORDER BY w.corpus, COALESCE(NULLIF(TRIM(w.domain), ''), 'Unowned'), w.name
                LIMIT ?4
             ),
             resource_rollup AS (
                SELECT s.workflow_id,
                       MAX(s.cpu_percent) AS max_cpu_percent,
                       MAX(s.memory_rss_bytes) AS max_memory_rss_bytes,
                       COUNT(s.id) AS sample_count
                FROM workflow_resource_samples s
                JOIN visible_workflows vw ON vw.id = s.workflow_id
                WHERE datetime(s.sampled_at) >= datetime('now', ?3)
                GROUP BY s.workflow_id
             ),
             token_rollup AS (
                SELECT t.workflow_id,
                       SUM(t.token_count) AS total_tokens,
                       COUNT(DISTINCT COALESCE(json_extract(t.labels_json, '$.call_id'), t.id)) AS token_call_count
                FROM workflow_token_usage t
                JOIN visible_workflows vw ON vw.id = t.workflow_id
                WHERE datetime(t.emitted_at) >= datetime('now', ?3)
                GROUP BY t.workflow_id
             )
             SELECT vw.id, vw.name, vw.corpus, vw.domain,
                    r.max_cpu_percent,
                    r.max_memory_rss_bytes,
                    COALESCE(r.sample_count, 0),
                    COALESCE(t.total_tokens, 0),
                    COALESCE(t.token_call_count, 0)
             FROM visible_workflows vw
             LEFT JOIN resource_rollup r ON r.workflow_id = vw.id
             LEFT JOIN token_rollup t ON t.workflow_id = vw.id
             ORDER BY vw.corpus, COALESCE(NULLIF(TRIM(vw.domain), ''), 'Unowned'), vw.name",
        )?;
        let rows = stmt.query_map(
            params![corpus_filter, domain_filter, window_modifier, limit],
            |row| {
                let corpus: String = row.get(2)?;
                Ok(MissionControlWorkflowTelemetry {
                    workflow_id: row.get(0)?,
                    workflow_name: row.get(1)?,
                    environment: corpus.clone(),
                    corpus,
                    domain: owner_label(row.get(3)?),
                    max_cpu_percent: row.get(4)?,
                    max_memory_rss_bytes: row.get(5)?,
                    sample_count: row.get(6)?,
                    total_tokens: row.get(7)?,
                    token_call_count: row.get(8)?,
                })
            },
        )?;
        rows.collect()
    }

    pub fn get_run_tasks(&self, run_id: &str) -> rusqlite::Result<Vec<RunTask>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, run_id, attempt_id, task_id, status, started_at, finished_at, attempt_number, parent_task_id, error_type, error_message, details_json
             FROM run_tasks WHERE run_id = ?1 ORDER BY started_at ASC, task_id ASC, attempt_number ASC",
        )?;
        let rows = stmt.query_map(params![run_id], |row| Ok(run_task_from_row(row)))?;
        rows.collect()
    }

    pub fn get_run_attempts(&self, run_id: &str) -> rusqlite::Result<Vec<RunAttempt>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, run_id, task_id, attempt_number, status, started_at, finished_at, exit_code, retry_reason, error_type, error_message, trigger_kind
             FROM run_attempts WHERE run_id = ?1 ORDER BY started_at ASC, task_id ASC, attempt_number ASC",
        )?;
        let rows = stmt.query_map(params![run_id], |row| Ok(run_attempt_from_row(row)))?;
        rows.collect()
    }

    pub fn get_run_metrics(&self, run_id: &str) -> rusqlite::Result<Vec<RunMetric>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, run_id, task_id, metric_name, metric_value, metric_unit, emitted_at, labels_json
             FROM run_metrics WHERE run_id = ?1 ORDER BY emitted_at ASC, metric_name ASC",
        )?;
        let rows = stmt.query_map(params![run_id], |row| Ok(run_metric_from_row(row)))?;
        rows.collect()
    }

    pub fn query_workflow_resource_samples(
        &self,
        workflow_id: &str,
        window_modifier: &str,
    ) -> rusqlite::Result<Vec<WorkflowResourceSample>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, run_id, workflow_id, queue_name, environment, pid, sampled_at, cpu_percent, memory_rss_bytes, memory_vms_bytes, swap_bytes, labels_json
             FROM workflow_resource_samples
             WHERE workflow_id = ?1 AND datetime(sampled_at) >= datetime('now', ?2)
             ORDER BY sampled_at ASC",
        )?;
        let rows = stmt.query_map(params![workflow_id, window_modifier], |row| {
            Ok(workflow_resource_sample_from_row(row))
        })?;
        rows.collect()
    }

    pub fn query_token_usage_rollup(
        &self,
        group_by: &[String],
        window_modifier: &str,
        time_bucket: &str,
    ) -> rusqlite::Result<Vec<WorkflowTokenUsageRollup>> {
        let selected: std::collections::HashSet<String> = group_by
            .iter()
            .map(|item| normalize_rollup_dimension(item))
            .collect();
        let time_expr = if selected.contains("time_bucket") {
            token_time_bucket_expr(time_bucket)
        } else {
            "NULL".to_string()
        };
        let workflow_expr = if selected.contains("workflow_id") {
            "workflow_id".to_string()
        } else {
            "NULL".to_string()
        };
        let corpus_expr = if selected.contains("corpus") {
            "json_extract(labels_json, '$.corpus')".to_string()
        } else {
            "NULL".to_string()
        };
        let domain_expr = if selected.contains("domain") {
            "json_extract(labels_json, '$.domain')".to_string()
        } else {
            "NULL".to_string()
        };
        let queue_expr = if selected.contains("queue_name") {
            "COALESCE(json_extract(labels_json, '$.queue_name'), json_extract(labels_json, '$.queue'))".to_string()
        } else {
            "NULL".to_string()
        };
        let provider_expr = if selected.contains("provider") {
            "provider".to_string()
        } else {
            "NULL".to_string()
        };
        let model_expr = if selected.contains("model") {
            "model".to_string()
        } else {
            "NULL".to_string()
        };
        let token_kind_expr = if selected.contains("token_kind") {
            "token_kind".to_string()
        } else {
            "NULL".to_string()
        };

        let sql = format!(
            "SELECT {time_expr} AS time_bucket,
                    {workflow_expr} AS workflow_id,
                    {corpus_expr} AS corpus,
                    {domain_expr} AS domain,
                    {queue_expr} AS queue_name,
                    {provider_expr} AS provider,
                    {model_expr} AS model,
                    {token_kind_expr} AS token_kind,
                    SUM(token_count) AS total_tokens,
                    COUNT(DISTINCT COALESCE(json_extract(labels_json, '$.call_id'), id)) AS call_count
             FROM workflow_token_usage
             WHERE datetime(emitted_at) >= datetime('now', ?1)
             GROUP BY 1, 2, 3, 4, 5, 6, 7, 8
             ORDER BY time_bucket ASC, workflow_id ASC, provider ASC, model ASC, token_kind ASC"
        );
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![window_modifier], |row| {
            Ok(WorkflowTokenUsageRollup {
                time_bucket: row.get(0)?,
                workflow_id: row.get(1)?,
                environment: row.get(2)?,
                corpus: row.get(2)?,
                domain: row.get(3)?,
                queue_name: row.get(4)?,
                provider: row.get(5)?,
                model: row.get(6)?,
                token_kind: row.get(7)?,
                total_tokens: row.get::<_, Option<i64>>(8)?.unwrap_or(0),
                call_count: row.get(9)?,
            })
        })?;
        rows.collect()
    }

    pub fn workflow_history_buckets(
        &self,
        workflow_id: &str,
        days: i64,
    ) -> rusqlite::Result<Vec<WorkflowHistoryBucket>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT substr(started_at, 1, 10) AS day,
                    COUNT(*) AS total,
                    SUM(CASE WHEN status IN ('failed', 'cancelled', 'cascade-skipped', 'dead_letter', 'dead_lettered') THEN 1 ELSE 0 END) AS failed,
                    SUM(CASE WHEN status IN ('success', 'succeeded') THEN 1 ELSE 0 END) AS succeeded
             FROM runs
             WHERE workflow_id = ?1 AND datetime(started_at) >= datetime('now', ?2)
             GROUP BY day
             ORDER BY day ASC",
        )?;
        let window = format!("-{} days", days.max(1));
        let rows = stmt.query_map(params![workflow_id, window], |row| {
            Ok(WorkflowHistoryBucket {
                day: row.get(0)?,
                total: row.get(1)?,
                failed: row.get(2)?,
                succeeded: row.get(3)?,
            })
        })?;
        rows.collect()
    }

    pub fn prometheus_metrics(&self) -> rusqlite::Result<String> {
        let conn = self.conn()?;
        let mut out = String::new();
        out.push_str("# HELP scheduler_workflow_runs_total Workflow runs by workflow and status\n");
        out.push_str("# TYPE scheduler_workflow_runs_total counter\n");
        let mut stmt = conn.prepare(
            "SELECT workflow_id, status, COUNT(*) FROM runs GROUP BY workflow_id, status",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;
        for row in rows {
            let (workflow_id, status, count) = row?;
            out.push_str(&format!(
                "scheduler_workflow_runs_total{{workflow_id=\"{}\",status=\"{}\"}} {}\n",
                metric_label(&workflow_id),
                metric_label(&status),
                count
            ));
        }

        out.push_str("# HELP scheduler_task_runs_total Task runs by workflow, task, and status\n");
        out.push_str("# TYPE scheduler_task_runs_total counter\n");
        let mut stmt = conn.prepare(
            "SELECT r.workflow_id, t.task_id, t.status, COUNT(*)
             FROM run_tasks t JOIN runs r ON r.id = t.run_id
             GROUP BY r.workflow_id, t.task_id, t.status",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;
        for row in rows {
            let (workflow_id, task_id, status, count) = row?;
            out.push_str(&format!(
                "scheduler_task_runs_total{{workflow_id=\"{}\",task_id=\"{}\",status=\"{}\"}} {}\n",
                metric_label(&workflow_id),
                metric_label(&task_id),
                metric_label(&status),
                count
            ));
        }

        out.push_str("# HELP scheduler_dead_letter_runs Dead-letter rows by workflow\n");
        out.push_str("# TYPE scheduler_dead_letter_runs gauge\n");
        let mut stmt = conn.prepare(
            "SELECT workflow_id, COUNT(*) FROM scheduler_dead_letters WHERE acknowledged_at IS NULL GROUP BY workflow_id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        for row in rows {
            let (workflow_id, count) = row?;
            out.push_str(&format!(
                "scheduler_dead_letter_runs{{workflow_id=\"{}\"}} {}\n",
                metric_label(&workflow_id),
                count
            ));
        }
        Ok(out)
    }

    pub fn evaluate_sla_violations(&self) -> rusqlite::Result<Vec<SlaViolation>> {
        let workflows = self.list_workflows()?;
        let mut violations = Vec::new();
        for workflow in workflows.into_iter().filter(|w| w.enabled) {
            let Some(queue_config) = workflow.queue_config.as_deref() else {
                continue;
            };
            let Ok(config) = serde_json::from_str::<serde_json::Value>(queue_config) else {
                continue;
            };
            let Some(sla) = config.get("sla") else {
                continue;
            };
            if let Some(max_runtime) = sla.get("max_runtime_seconds").and_then(|v| v.as_i64()) {
                for run in self
                    .get_running_runs()?
                    .into_iter()
                    .filter(|r| r.workflow_id == workflow.id)
                {
                    if run_age_seconds(&run.started_at) > max_runtime {
                        violations.push(SlaViolation {
                            workflow_id: workflow.id.clone(),
                            workflow_name: workflow.name.clone(),
                            violation_type: "max_runtime_seconds".to_string(),
                            message: format!(
                                "{} has been running longer than {}s",
                                workflow.name, max_runtime
                            ),
                            severity: "warning".to_string(),
                        });
                    }
                }
            }
            if let Some(min_rate) = sla.get("min_success_rate_24h").and_then(|v| v.as_f64()) {
                let conn = self.conn()?;
                let (total, succeeded): (i64, i64) = conn.query_row(
                    "SELECT COUNT(*), SUM(CASE WHEN status IN ('success', 'succeeded') THEN 1 ELSE 0 END)
                     FROM runs
                     WHERE workflow_id = ?1
                       AND datetime(started_at) >= datetime('now', '-1 day')
                       AND status IN ('success', 'succeeded', 'failed', 'cancelled', 'cascade-skipped', 'dead_letter', 'dead_lettered')",
                    params![workflow.id],
                    |row| Ok((row.get(0)?, row.get::<_, Option<i64>>(1)?.unwrap_or(0))),
                )?;
                if total > 0 {
                    let rate = succeeded as f64 / total as f64;
                    if rate < min_rate {
                        violations.push(SlaViolation {
                            workflow_id: workflow.id.clone(),
                            workflow_name: workflow.name.clone(),
                            violation_type: "min_success_rate_24h".to_string(),
                            message: format!(
                                "{} 24h success rate {:.0}% is below {:.0}%",
                                workflow.name,
                                rate * 100.0,
                                min_rate * 100.0
                            ),
                            severity: "warning".to_string(),
                        });
                    }
                }
            }
        }
        Ok(violations)
    }

    pub fn set_error_analysis(&self, run_id: &str, analysis_json: &str) -> rusqlite::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE runs SET error_analysis = ?2 WHERE id = ?1",
            params![run_id, analysis_json],
        )?;
        Ok(())
    }

    pub fn get_trigger_fingerprint(
        &self,
        workflow_id: &str,
        trigger_id: &str,
    ) -> rusqlite::Result<Option<String>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT fingerprint FROM workflow_trigger_state WHERE workflow_id = ?1 AND trigger_id = ?2",
        )?;
        let mut rows = stmt.query(params![workflow_id, trigger_id])?;
        if let Some(row) = rows.next()? {
            Ok(row.get(0)?)
        } else {
            Ok(None)
        }
    }

    pub fn set_trigger_state(
        &self,
        workflow_id: &str,
        trigger_id: &str,
        fingerprint: &str,
        fired: bool,
    ) -> rusqlite::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let fired_at: Option<&str> = if fired { Some(&now) } else { None };
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO workflow_trigger_state (workflow_id, trigger_id, fingerprint, observed_at, fired_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(workflow_id, trigger_id) DO UPDATE SET
               fingerprint = excluded.fingerprint,
               observed_at = excluded.observed_at,
               fired_at = COALESCE(excluded.fired_at, workflow_trigger_state.fired_at)",
            params![workflow_id, trigger_id, fingerprint, now, fired_at],
        )?;
        Ok(())
    }

    pub fn get_running_count(&self) -> rusqlite::Result<usize> {
        let conn = self.conn()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM runs WHERE status IN ('admitted', 'running')",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    pub fn latest_run_status(&self, workflow_id: &str) -> rusqlite::Result<Option<String>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT status FROM runs WHERE workflow_id = ?1 ORDER BY started_at DESC LIMIT 1",
        )?;
        let mut rows = stmt.query(params![workflow_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn queue_capacity(&self, queue_name: &str, environment: &str) -> rusqlite::Result<i64> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT capacity FROM queues WHERE name = ?1 AND environment = ?2")?;
        let mut rows = stmt.query(params![queue_name, environment])?;
        if let Some(row) = rows.next()? {
            let capacity: i64 = row.get(0)?;
            Ok(capacity.max(1))
        } else {
            Ok(1)
        }
    }

    pub fn queue_tag_cap(
        &self,
        queue_name: &str,
        environment: &str,
    ) -> rusqlite::Result<Option<i64>> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT tag_cap FROM queues WHERE name = ?1 AND environment = ?2",
            params![queue_name, environment],
            |row| row.get(0),
        )
        .optional()
        .map(|value| value.flatten())
    }

    pub fn global_parallelism_cap(&self) -> rusqlite::Result<i64> {
        let conn = self.conn()?;
        let raw: String = conn.query_row(
            "SELECT value FROM scheduler_config WHERE key = 'global_parallelism_cap'",
            [],
            |row| row.get(0),
        )?;
        raw.parse::<i64>()
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(e)))
    }

    fn get_bool_config(&self, key: &str, default: bool) -> rusqlite::Result<bool> {
        let conn = self.conn()?;
        let raw: Option<String> = conn
            .query_row(
                "SELECT value FROM scheduler_config WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(
            match raw
                .as_deref()
                .map(str::trim)
                .map(str::to_ascii_lowercase)
                .as_deref()
            {
                Some("true") | Some("1") | Some("yes") => true,
                Some("false") | Some("0") | Some("no") => false,
                _ => default,
            },
        )
    }

    fn set_bool_config(&self, key: &str, value: bool) -> rusqlite::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO scheduler_config (key, value, updated_at) VALUES (?1, ?2, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, if value { "true" } else { "false" }],
        )?;
        Ok(())
    }

    fn get_string_config(&self, key: &str) -> rusqlite::Result<Option<String>> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT value FROM scheduler_config WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
    }

    /// Public accessor for a scheduler_config string value (used by the HTTP API
    /// to read, e.g., the inbound webhook secret).
    pub fn get_scheduler_config(&self, key: &str) -> rusqlite::Result<Option<String>> {
        self.get_string_config(key)
    }

    pub fn get_notification_prefs(&self) -> rusqlite::Result<(bool, bool)> {
        Ok((
            self.get_bool_config("notify_on_failure", true)?,
            self.get_bool_config("notify_on_success", false)?,
        ))
    }

    pub fn set_notification_prefs(
        &self,
        notify_on_failure: bool,
        notify_on_success: bool,
    ) -> rusqlite::Result<()> {
        self.set_bool_config("notify_on_failure", notify_on_failure)?;
        self.set_bool_config("notify_on_success", notify_on_success)?;
        Ok(())
    }

    pub fn get_mission_control_preferences(&self) -> rusqlite::Result<MissionControlPreferences> {
        let default_landing = match self
            .get_string_config("mission_control.default_landing")?
            .unwrap_or_else(|| "mission_control".to_string())
            .trim()
        {
            "dashboard" => "dashboard".to_string(),
            _ => "mission_control".to_string(),
        };
        let corpus_filter = normalize_mission_corpus_filter(
            &self
                .get_string_config("mission_control.corpus_filter")?
                .unwrap_or_else(|| "all".to_string()),
        );
        let domain_filter = normalize_mission_domain_filter(
            &self
                .get_string_config("mission_control.domain_filter")?
                .unwrap_or_else(|| "all".to_string()),
        );
        Ok(MissionControlPreferences {
            default_landing,
            corpus_filter,
            domain_filter,
        })
    }

    pub fn set_mission_control_preferences(
        &self,
        default_landing: &str,
        corpus_filter: &str,
        domain_filter: &str,
    ) -> rusqlite::Result<MissionControlPreferences> {
        let default_landing = match default_landing.trim() {
            "dashboard" => "dashboard".to_string(),
            _ => "mission_control".to_string(),
        };
        let corpus_filter = normalize_mission_corpus_filter(corpus_filter);
        let domain_filter = normalize_mission_domain_filter(domain_filter);
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        for (key, value) in [
            ("mission_control.default_landing", default_landing.as_str()),
            ("mission_control.corpus_filter", corpus_filter.as_str()),
            ("mission_control.domain_filter", domain_filter.as_str()),
        ] {
            tx.execute(
                "INSERT INTO scheduler_config (key, value, updated_at) VALUES (?1, ?2, datetime('now'))
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                params![key, value],
            )?;
        }
        tx.commit()?;
        self.get_mission_control_preferences()
    }

    pub fn validate_queue_cap_lattice(&self) -> rusqlite::Result<Vec<String>> {
        let conn = self.conn()?;
        let global_cap = self.global_parallelism_cap()?;
        let mut errors = Vec::new();
        if global_cap < 1 {
            errors.push("global_parallelism_cap must be >= 1".to_string());
        }
        let mut stmt = conn.prepare(
            "SELECT name, environment, capacity, tag_cap FROM queues ORDER BY environment, name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<i64>>(3)?,
            ))
        })?;
        for row in rows {
            let (name, environment, capacity, tag_cap) = row?;
            let label = format!("{}/{}", environment, name);
            if capacity < 1 {
                errors.push(format!("queue {} capacity must be >= 1", label));
            }
            if capacity > global_cap {
                errors.push(format!(
                    "queue {} capacity {} exceeds global cap {}",
                    label, capacity, global_cap
                ));
            }
            if let Some(tag_cap) = tag_cap {
                if tag_cap < 1 {
                    errors.push(format!("queue {} tag_cap must be >= 1", label));
                }
                if tag_cap > capacity {
                    errors.push(format!(
                        "queue {} tag_cap {} exceeds queue capacity {}",
                        label, tag_cap, capacity
                    ));
                }
            }
        }
        Ok(errors)
    }

    pub fn list_queues(&self) -> rusqlite::Result<Vec<QueueInfo>> {
        let conn = self.conn()?;
        let global_cap = self.global_parallelism_cap()?;
        let mut stmt = conn.prepare(
            "SELECT name, environment, capacity, tag_cap, max_queued, updated_at FROM queues ORDER BY environment, name",
        )?;
        let rows = stmt.query_map([], |row| {
            let name: String = row.get(0)?;
            let environment: String = row.get(1)?;
            Ok(QueueInfo {
                active_count: self.running_count_for_queue(&name, &environment)?,
                queued_count: self.queued_count_for_queue(&name, &environment)?,
                name,
                environment,
                capacity: row.get(2)?,
                tag_cap: row.get(3)?,
                max_queued: row.get(4)?,
                global_parallelism_cap: global_cap,
                updated_at: row.get(5)?,
            })
        })?;
        rows.collect()
    }

    pub fn upsert_queue(
        &self,
        name: &str,
        environment: &str,
        capacity: i64,
        tag_cap: Option<i64>,
        max_queued: Option<i64>,
    ) -> rusqlite::Result<QueueInfo> {
        validate_queue_values(
            name,
            environment,
            capacity,
            tag_cap,
            max_queued,
            self.global_parallelism_cap()?,
        )?;
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO queues (name, environment, capacity, tag_cap, max_queued, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))
             ON CONFLICT(name, environment) DO UPDATE SET
               capacity = excluded.capacity,
               tag_cap = excluded.tag_cap,
               max_queued = excluded.max_queued,
               updated_at = datetime('now')",
            params![name, environment, capacity, tag_cap, max_queued],
        )?;
        self.get_queue(name, environment)
    }

    pub fn get_queue(&self, name: &str, environment: &str) -> rusqlite::Result<QueueInfo> {
        let global_cap = self.global_parallelism_cap()?;
        let conn = self.conn()?;
        conn.query_row(
            "SELECT name, environment, capacity, tag_cap, max_queued, updated_at FROM queues WHERE name = ?1 AND environment = ?2",
            params![name, environment],
            |row| {
                Ok(QueueInfo {
                    name: row.get(0)?,
                    environment: row.get(1)?,
                    capacity: row.get(2)?,
                    tag_cap: row.get(3)?,
                    max_queued: row.get(4)?,
                    active_count: self.running_count_for_queue(name, environment)?,
                    queued_count: self.queued_count_for_queue(name, environment)?,
                    global_parallelism_cap: global_cap,
                    updated_at: row.get(5)?,
                })
            },
        )
    }

    pub fn list_queued_runs(&self, limit: i64) -> rusqlite::Result<Vec<QueuedRun>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT q.id, q.run_id, q.workflow_id, w.name, q.queue_name, COALESCE(NULLIF(w.environment, ''), w.corpus), q.priority, q.status, q.queued_at, q.admitted_at, q.finished_at,
                    q.trigger_kind, q.trigger_payload, q.upstream_run_id, q.input_json, q.rerun_of_run_id
             FROM queued_runs q
             LEFT JOIN workflows w ON q.workflow_id = w.id
             ORDER BY
               CASE q.status WHEN 'queued' THEN 0 WHEN 'admitted' THEN 1 ELSE 2 END,
               q.priority DESC,
               q.queued_at ASC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(QueuedRun {
                id: row.get(0)?,
                run_id: row.get(1)?,
                workflow_id: row.get(2)?,
                workflow_name: row.get(3)?,
                queue_name: row.get(4)?,
                environment: row
                    .get::<_, Option<String>>(5)?
                    .unwrap_or_else(|| "source".to_string()),
                priority: row.get(6)?,
                status: row.get(7)?,
                queued_at: row.get(8)?,
                admitted_at: row.get(9)?,
                finished_at: row.get(10)?,
                trigger_kind: row.get(11)?,
                trigger_payload: row.get(12)?,
                upstream_run_id: row.get(13)?,
                input_json: row.get(14)?,
                rerun_of_run_id: row.get(15)?,
            })
        })?;
        rows.collect()
    }

    pub fn find_run_by_dispatch_context(
        &self,
        workflow_id: &str,
        trigger_kind: Option<&str>,
        trigger_payload: Option<&str>,
        input_json: Option<&str>,
        rerun_of_run_id: Option<&str>,
    ) -> rusqlite::Result<Option<Run>> {
        let conn = self.conn()?;
        let id = conn
            .query_row(
                "SELECT id FROM runs
                 WHERE workflow_id = ?1
                   AND COALESCE(trigger_kind, '') = COALESCE(?2, '')
                   AND COALESCE(trigger_payload, '') = COALESCE(?3, '')
                   AND COALESCE(input_json, '') = COALESCE(?4, '')
                   AND COALESCE(rerun_of_run_id, '') = COALESCE(?5, '')
                 ORDER BY started_at DESC LIMIT 1",
                params![
                    workflow_id,
                    trigger_kind,
                    trigger_payload,
                    input_json,
                    rerun_of_run_id
                ],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        match id {
            Some(id) => self.get_run(&id).map(Some),
            None => Ok(None),
        }
    }

    pub fn find_queued_run_by_dispatch_context(
        &self,
        workflow_id: &str,
        trigger_kind: Option<&str>,
        trigger_payload: Option<&str>,
        input_json: Option<&str>,
        rerun_of_run_id: Option<&str>,
    ) -> rusqlite::Result<Option<QueuedRun>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT q.id, q.run_id, q.workflow_id, w.name, q.queue_name, COALESCE(NULLIF(w.environment, ''), w.corpus), q.priority,
                    q.status, q.queued_at, q.admitted_at, q.finished_at, q.trigger_kind,
                    q.trigger_payload, q.upstream_run_id, q.input_json, q.rerun_of_run_id
             FROM queued_runs q
             LEFT JOIN workflows w ON q.workflow_id = w.id
             WHERE q.workflow_id = ?1
               AND q.status = 'queued'
               AND COALESCE(q.trigger_kind, '') = COALESCE(?2, '')
               AND COALESCE(q.trigger_payload, '') = COALESCE(?3, '')
               AND COALESCE(q.input_json, '') = COALESCE(?4, '')
               AND COALESCE(q.rerun_of_run_id, '') = COALESCE(?5, '')
             ORDER BY q.queued_at ASC LIMIT 1",
        )?;
        stmt.query_row(
            params![
                workflow_id,
                trigger_kind,
                trigger_payload,
                input_json,
                rerun_of_run_id
            ],
            |row| {
                Ok(QueuedRun {
                    id: row.get(0)?,
                    run_id: row.get(1)?,
                    workflow_id: row.get(2)?,
                    workflow_name: row.get(3)?,
                    queue_name: row.get(4)?,
                    environment: row
                        .get::<_, Option<String>>(5)?
                        .unwrap_or_else(|| "source".to_string()),
                    priority: row.get(6)?,
                    status: row.get(7)?,
                    queued_at: row.get(8)?,
                    admitted_at: row.get(9)?,
                    finished_at: row.get(10)?,
                    trigger_kind: row.get(11)?,
                    trigger_payload: row.get(12)?,
                    upstream_run_id: row.get(13)?,
                    input_json: row.get(14)?,
                    rerun_of_run_id: row.get(15)?,
                })
            },
        )
        .optional()
    }

    #[allow(dead_code)]
    pub fn upsert_queued_run(
        &self,
        workflow_id: &str,
        queue_name: &str,
        priority: i64,
    ) -> rusqlite::Result<String> {
        self.upsert_queued_run_with_context(
            workflow_id,
            queue_name,
            priority,
            None,
            None,
            None,
            None,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn upsert_queued_run_with_context(
        &self,
        workflow_id: &str,
        queue_name: &str,
        priority: i64,
        trigger_kind: Option<&str>,
        trigger_payload: Option<&str>,
        upstream_run_id: Option<&str>,
        input_json: Option<&str>,
        rerun_of_run_id: Option<&str>,
    ) -> rusqlite::Result<String> {
        let conn = self.conn()?;
        let existing: rusqlite::Result<String> = conn.query_row(
            "SELECT id FROM queued_runs
             WHERE workflow_id = ?1 AND status = 'queued'
               AND COALESCE(trigger_kind, '') = COALESCE(?2, '')
               AND COALESCE(trigger_payload, '') = COALESCE(?3, '')
               AND COALESCE(upstream_run_id, '') = COALESCE(?4, '')
               AND COALESCE(input_json, '') = COALESCE(?5, '')
               AND COALESCE(rerun_of_run_id, '') = COALESCE(?6, '')
             ORDER BY queued_at ASC LIMIT 1",
            params![
                workflow_id,
                trigger_kind,
                trigger_payload,
                upstream_run_id,
                input_json,
                rerun_of_run_id
            ],
            |row| row.get(0),
        );
        if let Ok(id) = existing {
            conn.execute(
                "UPDATE queued_runs
                 SET queue_name = ?2, priority = ?3, trigger_kind = ?4, trigger_payload = ?5,
                     upstream_run_id = ?6, input_json = ?7, rerun_of_run_id = ?8
                 WHERE id = ?1",
                params![
                    id,
                    queue_name,
                    priority,
                    trigger_kind,
                    trigger_payload,
                    upstream_run_id,
                    input_json,
                    rerun_of_run_id
                ],
            )?;
            return Ok(id);
        }

        let workflow = self.get_workflow(workflow_id)?;
        let max_queued: Option<i64> = conn
            .query_row(
                "SELECT max_queued FROM queues WHERE name = ?1 AND environment = ?2",
                params![queue_name, workflow.environment],
                |row| row.get(0),
            )
            .unwrap_or(None);
        if let Some(max_queued) = max_queued {
            if self.queued_count_for_queue(queue_name, &workflow.environment)? >= max_queued {
                return Err(rusqlite::Error::InvalidParameterName(format!(
                    "queue {} max queued threshold {} reached",
                    queue_name, max_queued
                )));
            }
        }

        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO queued_runs
                (id, workflow_id, queue_name, priority, status, queued_at,
                 trigger_kind, trigger_payload, upstream_run_id, input_json, rerun_of_run_id)
             VALUES (?1, ?2, ?3, ?4, 'queued', ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                id,
                workflow_id,
                queue_name,
                priority,
                now,
                trigger_kind,
                trigger_payload,
                upstream_run_id,
                input_json,
                rerun_of_run_id
            ],
        )?;
        Ok(id)
    }

    #[allow(dead_code)]
    pub fn mark_queued_run_admitted(
        &self,
        workflow_id: &str,
        run_id: &str,
    ) -> rusqlite::Result<usize> {
        let conn = self.conn()?;
        let id: rusqlite::Result<String> = conn.query_row(
            "SELECT id FROM queued_runs WHERE workflow_id = ?1 AND status = 'queued' ORDER BY priority DESC, queued_at ASC LIMIT 1",
            params![workflow_id],
            |row| row.get(0),
        );
        let Ok(id) = id else {
            return Ok(0);
        };
        self.mark_queued_run_admitted_by_id(&id, run_id)
    }

    pub fn mark_queued_run_admitted_by_id(
        &self,
        queued_run_id: &str,
        run_id: &str,
    ) -> rusqlite::Result<usize> {
        let conn = self.conn()?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE queued_runs SET run_id = ?2, status = 'admitted', admitted_at = ?3 WHERE id = ?1 AND status = 'queued'",
            params![queued_run_id, run_id, now],
        )
    }

    pub fn cancel_queued_run(&self, id: &str) -> rusqlite::Result<usize> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn()?;
        conn.execute(
            "UPDATE queued_runs SET status = 'cancelled', finished_at = ?2 WHERE id = ?1 AND status = 'queued'",
            params![id, now],
        )
    }

    pub fn mark_queued_run_terminal_by_id(
        &self,
        queued_run_id: &str,
        run_id: &str,
        status: &str,
    ) -> rusqlite::Result<usize> {
        let conn = self.conn()?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE queued_runs SET run_id = ?2, status = ?3, admitted_at = COALESCE(admitted_at, ?4), finished_at = ?4 WHERE id = ?1 AND status = 'queued'",
            params![queued_run_id, run_id, status, now],
        )
    }

    fn running_count_for_queue(
        &self,
        queue_name: &str,
        environment: &str,
    ) -> rusqlite::Result<i64> {
        let mut count = 0;
        for run in self.get_running_runs()? {
            let workflow = self.get_workflow(&run.workflow_id)?;
            let (run_queue, run_environment) =
                queue_identity_from_config(workflow.queue_config.as_deref(), &workflow.environment);
            if run_queue == queue_name && run_environment == environment {
                count += 1;
            }
        }
        Ok(count)
    }

    fn queued_count_for_queue(&self, queue_name: &str, environment: &str) -> rusqlite::Result<i64> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT COUNT(*)
             FROM queued_runs q
             LEFT JOIN workflows w ON q.workflow_id = w.id
             WHERE q.queue_name = ?1 AND COALESCE(NULLIF(w.environment, ''), w.corpus, 'source') = ?2 AND q.status = 'queued'",
            params![queue_name, environment],
            |row| row.get(0),
        )
    }

    #[allow(dead_code)]
    pub fn acquire_mutex_locks(
        &self,
        workflow_id: &str,
        run_id: &str,
        mutex_keys: &[String],
    ) -> rusqlite::Result<bool> {
        if mutex_keys.is_empty() {
            return Ok(true);
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for key in mutex_keys {
            let exists: i64 = tx.query_row(
                "SELECT COUNT(*) FROM workflow_mutex_locks WHERE mutex_key = ?1",
                params![key],
                |row| row.get(0),
            )?;
            if exists > 0 {
                tx.rollback()?;
                return Ok(false);
            }
        }
        let now = chrono::Utc::now().to_rfc3339();
        for key in mutex_keys {
            tx.execute(
                "INSERT INTO workflow_mutex_locks (mutex_key, workflow_id, run_id, acquired_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![key, workflow_id, run_id, now],
            )?;
        }
        tx.commit()?;
        Ok(true)
    }

    #[allow(dead_code)]
    pub fn release_mutex_locks(&self, run_id: &str) -> rusqlite::Result<usize> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM workflow_mutex_locks WHERE run_id = ?1",
            params![run_id],
        )
    }

    pub fn get_running_runs(&self) -> rusqlite::Result<Vec<Run>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT r.id, r.workflow_id, r.started_at, r.finished_at, r.exit_code, r.stdout, r.stderr, r.result_url, r.status, w.name, r.error_analysis, r.trigger_kind, r.trigger_payload, r.upstream_run_id, r.input_json, r.rerun_of_run_id
             FROM runs r LEFT JOIN workflows w ON r.workflow_id = w.id WHERE r.status IN ('admitted', 'running') ORDER BY r.started_at DESC"
        )?;
        let rows = stmt.query_map([], |row| Ok(run_from_row(row)))?;
        rows.collect()
    }

    pub fn get_active_execution_runs(&self) -> rusqlite::Result<Vec<RunExecutionRecord>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT r.id, r.workflow_id, w.name, r.status, r.process_pid, r.process_pgid, r.process_started_at
             FROM runs r LEFT JOIN workflows w ON r.workflow_id = w.id
             WHERE r.status IN ('admitted', 'running')
             ORDER BY r.started_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(RunExecutionRecord {
                id: row.get(0)?,
                workflow_id: row.get(1)?,
                workflow_name: row.get(2)?,
                status: row.get(3)?,
                process_pid: row.get(4)?,
                process_pgid: row.get(5)?,
                process_started_at: row.get(6)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_email_config(&self) -> rusqlite::Result<EmailConfig> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT enabled, alert_email, smtp_host, smtp_port, smtp_user, smtp_password, from_address, from_name FROM email_config WHERE id = 1",
            [],
            |row| {
                Ok(EmailConfig {
                    enabled: row.get::<_, i32>(0)? != 0,
                    alert_email: row.get(1)?,
                    smtp_host: row.get(2)?,
                    smtp_port: row.get(3)?,
                    smtp_user: row.get(4)?,
                    smtp_password: row.get(5)?,
                    from_address: row.get(6)?,
                    from_name: row.get(7)?,
                })
            },
        )
    }

    pub fn set_email_config(&self, config: &EmailConfig) -> rusqlite::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE email_config SET enabled = ?1, alert_email = ?2, smtp_host = ?3, smtp_port = ?4, smtp_user = ?5, smtp_password = ?6, from_address = ?7, from_name = ?8 WHERE id = 1",
            params![
                config.enabled as i32,
                config.alert_email,
                config.smtp_host,
                config.smtp_port,
                config.smtp_user,
                config.smtp_password,
                config.from_address,
                config.from_name,
            ],
        )?;
        Ok(())
    }
}

fn json_to_string(value: Option<&serde_json::Value>) -> rusqlite::Result<Option<String>> {
    value
        .map(serde_json::to_string)
        .transpose()
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
}

fn parse_json_opt(value: Option<String>) -> Option<serde_json::Value> {
    value.as_deref().and_then(|s| serde_json::from_str(s).ok())
}

fn workflow_resource_sample_from_row(row: &rusqlite::Row) -> WorkflowResourceSample {
    let labels_str: Option<String> = row.get(11).unwrap_or(None);
    WorkflowResourceSample {
        id: row.get(0).unwrap_or_default(),
        run_id: row.get(1).unwrap_or(None),
        workflow_id: row.get(2).unwrap_or_default(),
        queue_name: row.get(3).unwrap_or(None),
        environment: row.get(4).unwrap_or_default(),
        pid: row.get(5).unwrap_or(None),
        sampled_at: row.get(6).unwrap_or_default(),
        cpu_percent: row.get(7).unwrap_or(None),
        memory_rss_bytes: row.get(8).unwrap_or(None),
        memory_vms_bytes: row.get(9).unwrap_or(None),
        swap_bytes: row.get(10).unwrap_or(None),
        labels: labels_str
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
    }
}

fn normalize_rollup_dimension(item: &str) -> String {
    match item.trim().to_ascii_lowercase().as_str() {
        "time" | "bucket" | "time_bucket" => "time_bucket".to_string(),
        "workflow" | "workflow_id" => "workflow_id".to_string(),
        "queue" | "queue_name" => "queue_name".to_string(),
        "kind" | "token" | "token_kind" => "token_kind".to_string(),
        other => other.to_string(),
    }
}

fn token_time_bucket_expr(time_bucket: &str) -> String {
    match time_bucket {
        "minute" => "substr(emitted_at, 1, 16)".to_string(),
        "day" => "substr(emitted_at, 1, 10)".to_string(),
        _ => "substr(emitted_at, 1, 13)".to_string(),
    }
}

fn scheduler_asset_id(asset_kind: &str, asset_namespace: &str, asset_partition: &str) -> String {
    format!("{}:{}:{}", asset_kind, asset_namespace, asset_partition)
}

fn scheduler_asset_from_row(row: &rusqlite::Row) -> SchedulerAsset {
    let freshness_str: Option<String> = row.get(7).unwrap_or(None);
    SchedulerAsset {
        asset_id: row.get(0).unwrap_or_default(),
        asset_kind: row.get(1).unwrap_or_default(),
        asset_namespace: row.get(2).unwrap_or_default(),
        asset_partition: row.get(3).unwrap_or_default(),
        last_action: row.get(4).unwrap_or(None),
        last_written_at: row.get(5).unwrap_or(None),
        last_writer_run_id: row.get(6).unwrap_or(None),
        freshness_policy: freshness_str
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
    }
}

fn run_from_row(row: &rusqlite::Row) -> Run {
    let stdout: Option<String> = row.get(5).unwrap_or(None);
    let summary = stdout.as_deref().and_then(extract_summary);
    let analysis_str: Option<String> = row.get(10).unwrap_or(None);
    let error_analysis = analysis_str
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok());
    Run {
        id: row.get(0).unwrap_or_default(),
        workflow_id: row.get(1).unwrap_or_default(),
        started_at: row.get(2).unwrap_or_default(),
        finished_at: row.get(3).unwrap_or(None),
        exit_code: row.get(4).unwrap_or(None),
        stdout,
        stderr: row.get(6).unwrap_or(None),
        result_url: row.get(7).unwrap_or(None),
        status: row.get(8).unwrap_or_default(),
        workflow_name: row.get(9).unwrap_or(None),
        summary,
        error_analysis,
        trigger_kind: row.get(11).unwrap_or(None),
        trigger_payload: row.get(12).unwrap_or(None),
        upstream_run_id: row.get(13).unwrap_or(None),
        input_json: row.get(14).unwrap_or(None),
        rerun_of_run_id: row.get(15).unwrap_or(None),
    }
}

fn run_task_from_row(row: &rusqlite::Row) -> RunTask {
    let details_str: Option<String> = row.get(11).unwrap_or(None);
    RunTask {
        id: row.get(0).unwrap_or_default(),
        run_id: row.get(1).unwrap_or_default(),
        attempt_id: row.get(2).unwrap_or(None),
        task_id: row.get(3).unwrap_or_default(),
        status: row.get(4).unwrap_or_default(),
        started_at: row.get(5).unwrap_or(None),
        finished_at: row.get(6).unwrap_or(None),
        attempt_number: row.get(7).unwrap_or_default(),
        parent_task_id: row.get(8).unwrap_or(None),
        error_type: row.get(9).unwrap_or(None),
        error_message: row.get(10).unwrap_or(None),
        details: details_str
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
    }
}

fn run_attempt_from_row(row: &rusqlite::Row) -> RunAttempt {
    RunAttempt {
        id: row.get(0).unwrap_or_default(),
        run_id: row.get(1).unwrap_or_default(),
        task_id: row.get(2).unwrap_or_default(),
        attempt_number: row.get(3).unwrap_or_default(),
        status: row.get(4).unwrap_or_default(),
        started_at: row.get(5).unwrap_or_default(),
        finished_at: row.get(6).unwrap_or(None),
        exit_code: row.get(7).unwrap_or(None),
        retry_reason: row.get(8).unwrap_or(None),
        error_type: row.get(9).unwrap_or(None),
        error_message: row.get(10).unwrap_or(None),
        trigger_kind: row.get(11).unwrap_or(None),
    }
}

fn run_metric_from_row(row: &rusqlite::Row) -> RunMetric {
    let labels_str: Option<String> = row.get(7).unwrap_or(None);
    RunMetric {
        id: row.get(0).unwrap_or_default(),
        run_id: row.get(1).unwrap_or_default(),
        task_id: row.get(2).unwrap_or(None),
        metric_name: row.get(3).unwrap_or_default(),
        metric_value: row.get(4).unwrap_or_default(),
        metric_unit: row.get(5).unwrap_or(None),
        emitted_at: row.get(6).unwrap_or_default(),
        labels: labels_str
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
    }
}

fn metric_label(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn run_age_seconds(started_at: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(started_at)
        .map(|started| (chrono::Utc::now() - started.with_timezone(&chrono::Utc)).num_seconds())
        .unwrap_or(0)
}

/// Extract the latest SUMMARY_JSON:{...} line from workflow stdout.
/// Current async runs store a run-scoped log slice, but latest-wins keeps the
/// UI correct if a workflow emits multiple summaries inside one run.
pub fn extract_summary(stdout: &str) -> Option<serde_json::Value> {
    for line in stdout.lines().rev() {
        let trimmed = line.trim();
        if let Some(json_str) = trimmed.strip_prefix("SUMMARY_JSON:") {
            if let Ok(val) = serde_json::from_str(json_str.trim()) {
                return Some(val);
            }
        }
    }
    None
}

fn validate_queue_values(
    name: &str,
    environment: &str,
    capacity: i64,
    tag_cap: Option<i64>,
    max_queued: Option<i64>,
    global_cap: i64,
) -> rusqlite::Result<()> {
    if name.trim().is_empty() {
        return Err(rusqlite::Error::InvalidParameterName(
            "queue name must not be empty".to_string(),
        ));
    }
    // Environments are user-managed; any non-empty environment name is valid.
    if environment.trim().is_empty() {
        return Err(rusqlite::Error::InvalidParameterName(
            "queue environment must not be empty".to_string(),
        ));
    }
    if capacity < 1 {
        return Err(rusqlite::Error::InvalidParameterName(
            "queue capacity must be >= 1".to_string(),
        ));
    }
    if capacity > global_cap {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "queue capacity {} exceeds global cap {}",
            capacity, global_cap
        )));
    }
    if let Some(tag_cap) = tag_cap {
        if tag_cap < 1 {
            return Err(rusqlite::Error::InvalidParameterName(
                "tag cap must be >= 1".to_string(),
            ));
        }
        if tag_cap > capacity {
            return Err(rusqlite::Error::InvalidParameterName(format!(
                "tag cap {} exceeds queue capacity {}",
                tag_cap, capacity
            )));
        }
    }
    if let Some(max_queued) = max_queued {
        if max_queued < 0 {
            return Err(rusqlite::Error::InvalidParameterName(
                "max queued must be >= 0".to_string(),
            ));
        }
    }
    Ok(())
}

fn queue_identity_from_config(queue_config: Option<&str>, environment: &str) -> (String, String) {
    let default_queue = format!("{}-default", environment);
    let queue = queue_config
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .and_then(|value| {
            value
                .get("queue")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .filter(|queue| !queue.trim().is_empty())
        .unwrap_or(default_queue);
    (queue, environment.to_string())
}

/// Extract the `tags` array from a workflow `queue_config` JSON blob. Mirrors
/// the tag parsing that `scheduler::parse_queue_config` performs so tag-cap
/// accounting inside `admit_run_with_context` matches the scheduler's view.
fn queue_tags_from_config(queue_config: Option<&str>) -> Vec<String> {
    queue_config
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .and_then(|value| {
            value
                .get("tags")
                .and_then(|tags| tags.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|tag| tag.as_str().map(str::to_string))
                        .collect()
                })
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fk_delete_action(conn: &Connection, table: &str, from_col: &str) -> String {
        let mut stmt = conn
            .prepare(&format!("PRAGMA foreign_key_list({table})"))
            .unwrap();
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(3)?, row.get::<_, String>(6)?))
            })
            .unwrap();
        for row in rows {
            let (column, on_delete) = row.unwrap();
            if column == from_col {
                return on_delete;
            }
        }
        panic!("missing foreign key for {table}.{from_col}");
    }

    #[test]
    fn api_audit_redacts_query_strings() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::new(&dir);
        db.record_api_audit(
            Some("key-1"),
            "GET",
            "/api/v1/runs/run-1?token=secret&password=hidden",
            200,
            Some("127.0.0.1:9618"),
        )
        .unwrap();
        let conn = db.conn().unwrap();
        let path: String = conn
            .query_row("SELECT path FROM api_audit_log", [], |row| row.get(0))
            .unwrap();
        assert_eq!(path, "/api/v1/runs/run-1");
        assert!(!path.contains("secret"));
        assert!(!path.contains("password"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn fresh_db_stamps_current_schema_version_and_enables_wal() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::new(&dir);
        let conn = db.conn().unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, CURRENT_SCHEMA_VERSION);
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode.to_ascii_lowercase(), "wal");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn legacy_db_with_user_version_zero_is_stamped_to_current() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("scheduler.db");
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE workflows (
                    id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT,
                    script_path TEXT NOT NULL, cron_schedule TEXT NOT NULL,
                    enabled INTEGER DEFAULT 1, corpus TEXT NOT NULL DEFAULT 'source',
                    created_at TEXT, updated_at TEXT
                );
                INSERT INTO workflows (id, name, script_path, cron_schedule)
                VALUES ('wf-legacy', 'Legacy', 'scripts/noop.py', '0 0 * * *');",
            )
            .unwrap();
            let v: i64 = conn
                .query_row("PRAGMA user_version", [], |r| r.get(0))
                .unwrap();
            assert_eq!(v, 0, "legacy DB starts with user_version 0");
        }
        let db = Database::new(&dir);
        let conn = db.conn().unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, CURRENT_SCHEMA_VERSION);
        // Existing data preserved.
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM workflows WHERE id = 'wf-legacy'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn migration_fixture_v7_to_v8_adds_execution_metadata_and_stamps_version() {
        // Tripwire: refresh this fixture (seed schema + asserted columns) the
        // next time a migration lands so the N-1 -> N path stays covered.
        assert_eq!(
            CURRENT_SCHEMA_VERSION, 8,
            "add a v(N-1)->v(N) fixture when a new migration ships"
        );

        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("scheduler.db");
        let db = Database {
            path: db_path.to_string_lossy().to_string(),
        };

        // Seed the pre-v8 shape of `runs` (no execution/truncation metadata) and
        // stamp the DB at v(N-1) so exactly the v8 migration is pending. We drive
        // `run_migrations` directly to isolate the N-1 -> N transition rather than
        // rebuilding the entire intermediate schema through `init`.
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE runs (
                    id TEXT PRIMARY KEY,
                    workflow_id TEXT NOT NULL,
                    started_at TEXT NOT NULL,
                    finished_at TEXT,
                    exit_code INTEGER,
                    stdout TEXT,
                    stderr TEXT,
                    result_url TEXT,
                    trigger_kind TEXT,
                    trigger_payload TEXT,
                    upstream_run_id TEXT,
                    input_json TEXT,
                    rerun_of_run_id TEXT,
                    status TEXT DEFAULT 'running'
                );
                INSERT INTO runs (id, workflow_id, started_at, status)
                    VALUES ('run-mig', 'wf-mig', '2026-01-01T00:00:00Z', 'running');
                PRAGMA user_version = 7;",
            )
            .unwrap();

            let cols = runs_columns(&conn);
            assert!(
                !cols.contains("execution_worker_id") && !cols.contains("process_pid"),
                "pre-migration fixture must not already carry v8 columns"
            );
            let v: i64 = conn
                .query_row("PRAGMA user_version", [], |r| r.get(0))
                .unwrap();
            assert_eq!(v, CURRENT_SCHEMA_VERSION - 1);
        }

        // Apply exactly the pending v(N-1) -> v(N) migration.
        let conn = db.conn().unwrap();
        db.run_migrations(&conn, CURRENT_SCHEMA_VERSION - 1)
            .unwrap();

        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, CURRENT_SCHEMA_VERSION);

        let cols = runs_columns(&conn);
        for expected in [
            "execution_worker_id",
            "process_pid",
            "process_pgid",
            "process_started_at",
            "stdout_truncated",
            "stderr_truncated",
            "task_events_truncated",
        ] {
            assert!(
                cols.contains(expected),
                "v8 migration should add `{expected}` to runs"
            );
        }

        // Existing pre-migration row survives the in-place upgrade.
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM runs WHERE id = 'run-mig'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);

        let _ = std::fs::remove_dir_all(dir);
    }

    fn runs_columns(conn: &Connection) -> std::collections::HashSet<String> {
        let mut stmt = conn.prepare("PRAGMA table_info(runs)").unwrap();
        let cols = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        cols
    }

    #[test]
    fn migration_backup_prune_keeps_last_three() {
        let dir = std::env::temp_dir().join(format!("chaos-db-bakprune-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("scheduler.db");
        let db_path_str = db_path.to_string_lossy().to_string();

        // Five pre-migration sidecars for THIS db, created oldest -> newest.
        let mut created = vec![];
        for i in 1..=5 {
            let bak = dir.join(format!(
                "scheduler.db.pre-migrate-v{i}-2026010{i}T000000.bak"
            ));
            std::fs::write(&bak, b"backup").unwrap();
            created.push(bak);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        // A sidecar belonging to a DIFFERENT db must never be pruned.
        let unrelated = dir.join("other.db.pre-migrate-v1-20260101T000000.bak");
        std::fs::write(&unrelated, b"other").unwrap();

        Database::prune_migration_backups(&db_path_str);

        let remaining: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.starts_with("scheduler.db.pre-migrate-") && n.ends_with(".bak"))
            .collect();
        assert_eq!(
            remaining.len(),
            3,
            "only the 3 newest sidecars survive: {remaining:?}"
        );
        // Oldest two pruned; newest three retained.
        assert!(!created[0].exists() && !created[1].exists());
        assert!(created[2].exists() && created[3].exists() && created[4].exists());
        // Unrelated db's sidecar is left intact.
        assert!(unrelated.exists());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn migration_v2_backfills_environment_and_governance() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("scheduler.db");
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE workflows (
                    id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT,
                    script_path TEXT NOT NULL, cron_schedule TEXT NOT NULL,
                    enabled INTEGER DEFAULT 1, corpus TEXT NOT NULL DEFAULT 'source',
                    created_at TEXT, updated_at TEXT
                );
                CREATE TABLE queues (
                    name TEXT NOT NULL, corpus TEXT NOT NULL, capacity INTEGER NOT NULL DEFAULT 1,
                    tag_cap INTEGER, max_queued INTEGER,
                    created_at TEXT DEFAULT (datetime('now')), updated_at TEXT DEFAULT (datetime('now')),
                    PRIMARY KEY (name, corpus)
                );
                INSERT INTO workflows (id, name, script_path, cron_schedule, corpus, created_at, updated_at)
                    VALUES ('wf-src', 'Src', 's.py', '0 0 * * *', 'source', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
                INSERT INTO workflows (id, name, script_path, cron_schedule, corpus, created_at, updated_at)
                    VALUES ('wf-inst', 'Inst', 'i.py', '0 0 * * *', 'instance', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
                INSERT INTO queues (name, corpus, capacity) VALUES ('source-default', 'source', 4);",
            )
            .unwrap();
        }
        let db = Database::new(&dir);
        let conn = db.conn().unwrap();

        // Schema advanced to the current version (v2 backfill runs en route).
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, CURRENT_SCHEMA_VERSION);

        // environment backfilled from corpus; managed_externally derived.
        let src = db.get_workflow("wf-src").unwrap();
        assert_eq!(src.environment, "source");
        assert!(src.managed_externally);
        let inst = db.get_workflow("wf-inst").unwrap();
        assert_eq!(inst.environment, "instance");
        assert!(!inst.managed_externally);

        // environments table seeded with continuity environments.
        let envs = db.list_environments().unwrap();
        let names: std::collections::HashSet<String> =
            envs.iter().map(|e| e.name.clone()).collect();
        assert!(names.contains("source"));
        assert!(names.contains("instance"));
        let source_env = envs.iter().find(|e| e.name == "source").unwrap();
        assert!(source_env.managed_externally);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn migration_v5_rebuilds_queues_keyed_on_environment() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("scheduler.db");
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE workflows (
                    id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT,
                    script_path TEXT NOT NULL, cron_schedule TEXT NOT NULL,
                    enabled INTEGER DEFAULT 1, corpus TEXT NOT NULL DEFAULT 'source',
                    created_at TEXT, updated_at TEXT
                );
                CREATE TABLE queues (
                    name TEXT NOT NULL, corpus TEXT NOT NULL, capacity INTEGER NOT NULL DEFAULT 1,
                    tag_cap INTEGER, max_queued INTEGER,
                    created_at TEXT DEFAULT (datetime('now')), updated_at TEXT DEFAULT (datetime('now')),
                    PRIMARY KEY (name, corpus)
                );
                CREATE TABLE queue_events (
                    id TEXT PRIMARY KEY, queue_name TEXT NOT NULL, corpus TEXT NOT NULL,
                    workflow_id TEXT, run_id TEXT, event_type TEXT NOT NULL, reason TEXT,
                    emitted_at TEXT NOT NULL, details_json TEXT
                );
                CREATE TABLE workflow_resource_samples (
                    id TEXT PRIMARY KEY, run_id TEXT, workflow_id TEXT NOT NULL, queue_name TEXT,
                    corpus TEXT NOT NULL, pid INTEGER, sampled_at TEXT NOT NULL, cpu_percent REAL,
                    memory_rss_bytes INTEGER, memory_vms_bytes INTEGER, swap_bytes INTEGER, labels_json TEXT
                );
                INSERT INTO queues (name, corpus, capacity, tag_cap) VALUES ('prod-q', 'prod', 7, 3);
                INSERT INTO queue_events (id, queue_name, corpus, event_type, emitted_at)
                    VALUES ('qe-1', 'prod-q', 'prod', 'admitted', '2026-01-01T00:00:00Z');",
            )
            .unwrap();
        }
        let db = Database::new(&dir);
        let conn = db.conn().unwrap();

        // queues now keyed on `environment` (no `corpus` column).
        let queue_cols: std::collections::HashSet<String> = conn
            .prepare("PRAGMA table_info(queues)")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert!(queue_cols.contains("environment"));
        assert!(!queue_cols.contains("corpus"), "corpus dropped from queues");

        // PK is (name, environment).
        let pk_cols: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info('queues') WHERE pk > 0 ORDER BY pk")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(pk_cols, vec!["name".to_string(), "environment".to_string()]);

        // Data copied corpus -> environment, and capacity reads by environment.
        assert_eq!(db.queue_capacity("prod-q", "prod").unwrap(), 7);
        assert_eq!(db.queue_tag_cap("prod-q", "prod").unwrap(), Some(3));

        // queue_events + resource_samples rebuilt onto `environment`.
        for table in ["queue_events", "workflow_resource_samples"] {
            let cols: std::collections::HashSet<String> = conn
                .prepare(&format!("PRAGMA table_info({table})"))
                .unwrap()
                .query_map([], |r| r.get::<_, String>(1))
                .unwrap()
                .map(Result::unwrap)
                .collect();
            assert!(cols.contains("environment"), "{table} has environment");
            assert!(!cols.contains("corpus"), "{table} dropped corpus");
        }
        let ev_env: String = conn
            .query_row(
                "SELECT environment FROM queue_events WHERE id = 'qe-1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ev_env, "prod");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn migration_v6_rebuilds_run_fk_actions_for_retention_children() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("scheduler.db");
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "PRAGMA foreign_keys = ON;
                CREATE TABLE workflows (
                    id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT,
                    script_path TEXT NOT NULL, cron_schedule TEXT NOT NULL,
                    enabled INTEGER DEFAULT 1, corpus TEXT NOT NULL DEFAULT 'source',
                    created_at TEXT, updated_at TEXT
                );
                CREATE TABLE runs (
                    id TEXT PRIMARY KEY, workflow_id TEXT NOT NULL REFERENCES workflows(id),
                    started_at TEXT NOT NULL, finished_at TEXT, exit_code INTEGER,
                    stdout TEXT, stderr TEXT, result_url TEXT, status TEXT DEFAULT 'running'
                );
                CREATE TABLE queued_runs (
                    id TEXT PRIMARY KEY, run_id TEXT REFERENCES runs(id),
                    workflow_id TEXT NOT NULL REFERENCES workflows(id), queue_name TEXT NOT NULL,
                    priority INTEGER NOT NULL DEFAULT 0, status TEXT NOT NULL DEFAULT 'queued',
                    queued_at TEXT NOT NULL, admitted_at TEXT, finished_at TEXT
                );
                CREATE TABLE workflow_mutex_locks (
                    mutex_key TEXT PRIMARY KEY, workflow_id TEXT NOT NULL REFERENCES workflows(id),
                    run_id TEXT REFERENCES runs(id), acquired_at TEXT NOT NULL
                );
                CREATE TABLE queues (
                    name TEXT NOT NULL, environment TEXT NOT NULL,
                    capacity INTEGER NOT NULL DEFAULT 1, tag_cap INTEGER, max_queued INTEGER,
                    created_at TEXT DEFAULT (datetime('now')), updated_at TEXT DEFAULT (datetime('now')),
                    PRIMARY KEY (name, environment)
                );
                CREATE TABLE run_relationships (
                    id TEXT PRIMARY KEY,
                    parent_run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
                    child_run_id TEXT REFERENCES runs(id) ON DELETE SET NULL,
                    queued_run_id TEXT REFERENCES queued_runs(id) ON DELETE SET NULL,
                    child_workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
                    relationship TEXT NOT NULL, task_id TEXT, wait INTEGER NOT NULL DEFAULT 0,
                    status TEXT NOT NULL, reason TEXT, details_json TEXT,
                    created_at TEXT NOT NULL, updated_at TEXT NOT NULL
                );
                INSERT INTO workflows (id, name, script_path, cron_schedule, created_at, updated_at)
                    VALUES ('wf-retention', 'Retention', 's.py', '0 0 * * *', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
                INSERT INTO runs (id, workflow_id, started_at, status) VALUES
                    ('run-delete', 'wf-retention', '2026-01-01T00:00:00Z', 'success'),
                    ('run-parent', 'wf-retention', '2026-01-01T00:00:00Z', 'success');
                INSERT INTO queued_runs (id, run_id, workflow_id, queue_name, status, queued_at)
                    VALUES ('queue-delete', 'run-delete', 'wf-retention', 'source-default', 'success', '2026-01-01T00:00:00Z');
                INSERT INTO workflow_mutex_locks (mutex_key, workflow_id, run_id, acquired_at)
                    VALUES ('mutex-delete', 'wf-retention', 'run-delete', '2026-01-01T00:00:00Z');
                INSERT INTO run_relationships (id, parent_run_id, queued_run_id, child_workflow_id, relationship, wait, status, created_at, updated_at)
                    VALUES ('rel-queue', 'run-parent', 'queue-delete', 'wf-retention', 'run_workflow', 0, 'queued', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
                PRAGMA user_version = 5;",
            )
            .unwrap();
        }

        let db = Database::new(&dir);
        let conn = db.conn().unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, CURRENT_SCHEMA_VERSION);
        assert_eq!(fk_delete_action(&conn, "queued_runs", "run_id"), "SET NULL");
        assert_eq!(
            fk_delete_action(&conn, "workflow_mutex_locks", "run_id"),
            "CASCADE"
        );

        let relationship_ref: Option<String> = conn
            .query_row(
                "SELECT queued_run_id FROM run_relationships WHERE id = 'rel-queue'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(relationship_ref.as_deref(), Some("queue-delete"));

        conn.execute("DELETE FROM runs WHERE id = 'run-delete'", [])
            .unwrap();
        let queue_run_id: Option<String> = conn
            .query_row(
                "SELECT run_id FROM queued_runs WHERE id = 'queue-delete'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(queue_run_id.is_none());
        let lock_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM workflow_mutex_locks WHERE mutex_key = 'mutex-delete'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(lock_count, 0);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn migration_v7_backfills_and_records_queued_idempotency() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("scheduler.db");
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "PRAGMA foreign_keys = ON;
                CREATE TABLE workflows (
                    id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT,
                    script_path TEXT NOT NULL, cron_schedule TEXT NOT NULL,
                    enabled INTEGER DEFAULT 1, corpus TEXT NOT NULL DEFAULT 'source',
                    created_at TEXT, updated_at TEXT
                );
                CREATE TABLE runs (
                    id TEXT PRIMARY KEY, workflow_id TEXT NOT NULL REFERENCES workflows(id),
                    started_at TEXT NOT NULL, finished_at TEXT, exit_code INTEGER,
                    stdout TEXT, stderr TEXT, result_url TEXT, status TEXT DEFAULT 'running'
                );
                CREATE TABLE queued_runs (
                    id TEXT PRIMARY KEY, run_id TEXT REFERENCES runs(id) ON DELETE SET NULL,
                    workflow_id TEXT NOT NULL REFERENCES workflows(id), queue_name TEXT NOT NULL,
                    priority INTEGER NOT NULL DEFAULT 0, status TEXT NOT NULL DEFAULT 'queued',
                    queued_at TEXT NOT NULL, admitted_at TEXT, finished_at TEXT,
                    trigger_kind TEXT, trigger_payload TEXT, upstream_run_id TEXT,
                    input_json TEXT, rerun_of_run_id TEXT
                );
                CREATE TABLE queues (
                    name TEXT NOT NULL, environment TEXT NOT NULL,
                    capacity INTEGER NOT NULL DEFAULT 1, tag_cap INTEGER, max_queued INTEGER,
                    created_at TEXT DEFAULT (datetime('now')), updated_at TEXT DEFAULT (datetime('now')),
                    PRIMARY KEY (name, environment)
                );
                CREATE TABLE workflow_mutex_locks (
                    mutex_key TEXT PRIMARY KEY, workflow_id TEXT NOT NULL REFERENCES workflows(id),
                    run_id TEXT REFERENCES runs(id) ON DELETE CASCADE, acquired_at TEXT NOT NULL
                );
                CREATE TABLE scheduler_idempotency_keys (
                    key TEXT PRIMARY KEY,
                    run_id TEXT REFERENCES runs(id) ON DELETE SET NULL,
                    task_id TEXT,
                    attempt_id TEXT,
                    created_at TEXT NOT NULL
                );
                INSERT INTO workflows (id, name, script_path, cron_schedule, created_at, updated_at)
                    VALUES ('wf-idem', 'Idem', 's.py', '0 0 * * *', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
                INSERT INTO runs (id, workflow_id, started_at, status)
                    VALUES ('run-idem', 'wf-idem', '2026-01-01T00:00:00Z', 'success');
                INSERT INTO queued_runs (id, workflow_id, queue_name, status, queued_at)
                    VALUES ('queue-idem', 'wf-idem', 'source-default', 'queued', '2026-01-01T00:00:00Z');
                INSERT INTO scheduler_idempotency_keys (key, run_id, created_at)
                    VALUES ('legacy-key', 'run-idem', '2026-01-01T00:00:00Z');
                PRAGMA user_version = 6;",
            )
            .unwrap();
        }

        let db = Database::new(&dir);
        let conn = db.conn().unwrap();
        let cols: std::collections::HashSet<String> = conn
            .prepare("PRAGMA table_info(scheduler_idempotency_keys)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        for col in [
            "queued_run_id",
            "workflow_id",
            "request_fingerprint",
            "status",
            "updated_at",
        ] {
            assert!(cols.contains(col), "missing idempotency column {col}");
        }
        let legacy: (Option<String>, String) = conn
            .query_row(
                "SELECT workflow_id, status FROM scheduler_idempotency_keys WHERE key = 'legacy-key'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            legacy,
            (Some("wf-idem".to_string()), "admitted".to_string())
        );

        assert!(matches!(
            db.reserve_idempotency_key("queued-key", "wf-idem", "fp-queued")
                .unwrap(),
            IdempotencyReservation::Reserved
        ));
        db.complete_idempotency_key("queued-key", None, Some("queue-idem"), "queued")
            .unwrap();
        let queued = db.get_idempotency_record("queued-key").unwrap().unwrap();
        assert_eq!(queued.queued_run_id.as_deref(), Some("queue-idem"));
        assert_eq!(queued.request_fingerprint.as_deref(), Some("fp-queued"));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn environment_crud_and_delete_guard() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::new(&dir);

        let env = db
            .create_environment(
                "staging",
                Some("Staging env"),
                Some("/tmp/staging"),
                Some(2),
                None,
                None,
                false,
            )
            .unwrap();
        assert_eq!(env.name, "staging");
        assert_eq!(db.count_workflows_in_environment("staging").unwrap(), 0);

        // A workflow assigned to the environment should be counted.
        db.create_workflow(
            "wf",
            None,
            "s.py",
            "0 0 * * *",
            false,
            true,
            "UTC",
            "staging",
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(db.count_workflows_in_environment("staging").unwrap(), 1);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn downgrade_guard_refuses_newer_schema_version() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("scheduler.db");
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION + 5)
                .unwrap();
        }
        let db = Database {
            path: db_path.to_string_lossy().to_string(),
        };
        let result = db.init();
        assert!(result.is_err(), "opening a newer-schema DB must fail");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn extract_summary_returns_latest_valid_summary() {
        let stdout = "\
SUMMARY_JSON:{\"title\":\"stale\"}
noise
SUMMARY_JSON:{\"title\":\"current\"}
";
        let summary = extract_summary(stdout).expect("summary should parse");
        assert_eq!(summary["title"], "current");
    }

    #[test]
    fn queue_capacity_defaults_are_seeded() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::new(&dir);

        assert_eq!(db.global_parallelism_cap().unwrap(), 4);
        assert_eq!(db.queue_capacity("source-default", "source").unwrap(), 4);
        assert_eq!(
            db.queue_capacity("instance-default", "instance").unwrap(),
            2
        );
        assert_eq!(db.queue_capacity("missing", "source").unwrap(), 1);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn cap_lattice_validation_rejects_invalid_caps() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::new(&dir);
        let conn = db.conn().unwrap();
        conn.execute(
            "UPDATE scheduler_config SET value = '3' WHERE key = 'global_parallelism_cap'",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO queues (name, environment, capacity, tag_cap) VALUES ('too-big', 'source', 5, 6)",
            [],
        )
        .unwrap();

        let errors = db.validate_queue_cap_lattice().unwrap();

        assert!(errors.iter().any(|e| e.contains("exceeds global cap")));
        assert!(errors
            .iter()
            .any(|e| e.contains("tag_cap 6 exceeds queue capacity 5")));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn mutex_locks_are_acquired_and_released_by_run() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::new(&dir);
        let workflow = db
            .create_workflow(
                "Workflow",
                None,
                "scripts/workflows/noop.py",
                "0 0 * * *",
                false,
                true,
                "UTC",
                "source",
                None,
                None,
                None,
            )
            .unwrap();
        let run = db
            .create_run_with_context(&workflow.id, Some("manual"), None, None, None, None)
            .unwrap();
        let other_run = db
            .create_run_with_context(&workflow.id, Some("manual"), None, None, None, None)
            .unwrap();
        let keys = vec!["tag:source:source-default:heavy_io".to_string()];

        assert!(db
            .acquire_mutex_locks(&workflow.id, &run.id, &keys)
            .unwrap());
        assert!(!db
            .acquire_mutex_locks(&workflow.id, &other_run.id, &keys)
            .unwrap());
        assert_eq!(db.release_mutex_locks(&run.id).unwrap(), 1);
        assert!(db
            .acquire_mutex_locks(&workflow.id, &other_run.id, &keys)
            .unwrap());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn admit_run_claims_queue_and_mutex_atomically() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::new(&dir);
        let workflow = db
            .create_workflow(
                "Admission Workflow",
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
        let keys = vec!["exclude:admission".to_string()];
        let queued_id = db
            .upsert_queued_run_with_context(
                &workflow.id,
                "source-default",
                0,
                Some("manual"),
                None,
                None,
                None,
                None,
            )
            .unwrap();

        let run = match db
            .admit_run_with_context(
                &workflow.id,
                "source-default",
                "source",
                &[],
                Some("manual"),
                None,
                None,
                None,
                None,
                Some(&queued_id),
                &keys,
                None,
            )
            .unwrap()
        {
            RunAdmission::Admitted(run) => run,
            _ => panic!("expected admission to succeed"),
        };
        let conn = db.conn().unwrap();
        let queued: (String, Option<String>) = conn
            .query_row(
                "SELECT status, run_id FROM queued_runs WHERE id = ?1",
                params![queued_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(queued, ("admitted".to_string(), Some(run.id.clone())));
        let lock_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM workflow_mutex_locks WHERE run_id = ?1 AND mutex_key = ?2",
                params![run.id, keys[0]],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(lock_count, 1);

        let blocked_queue = db
            .upsert_queued_run_with_context(
                &workflow.id,
                "source-default",
                0,
                Some("manual"),
                Some("blocked"),
                None,
                None,
                None,
            )
            .unwrap();
        assert!(matches!(
            db.admit_run_with_context(
                &workflow.id,
                "source-default",
                "source",
                &[],
                Some("manual"),
                Some("blocked"),
                None,
                None,
                None,
                Some(&blocked_queue),
                &keys,
                None,
            )
            .unwrap(),
            RunAdmission::MutexBusy
        ));
        let blocked: (String, Option<String>) = conn
            .query_row(
                "SELECT status, run_id FROM queued_runs WHERE id = ?1",
                params![blocked_queue],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(blocked, ("queued".to_string(), None));

        let _ = std::fs::remove_dir_all(dir);
    }

    fn admission_capacity_test_workflow(db: &Database, queue_config: Option<&str>) -> Workflow {
        db.create_workflow(
            "Capacity Admission Workflow",
            None,
            "scripts/workflows/noop.py",
            "0 0 * * *",
            false,
            true,
            "UTC",
            "source",
            None,
            None,
            queue_config,
        )
        .unwrap()
    }

    #[test]
    fn admit_run_with_context_enforces_capacity_under_concurrency() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = std::sync::Arc::new(Database::new(&dir));
        // Capacity 1: only one run may occupy this queue at a time, so the
        // in-transaction capacity re-check must reject every racing admitter
        // but one.
        db.upsert_queue("source-default", "source", 1, None, None)
            .unwrap();
        let workflow = admission_capacity_test_workflow(&db, Some(r#"{"queue":"source-default"}"#));

        let threads = 8;
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(threads));
        let mut handles = Vec::new();
        for _ in 0..threads {
            let db = std::sync::Arc::clone(&db);
            let barrier = std::sync::Arc::clone(&barrier);
            let workflow_id = workflow.id.clone();
            handles.push(std::thread::spawn(move || {
                barrier.wait();
                match db
                    .admit_run_with_context(
                        &workflow_id,
                        "source-default",
                        "source",
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
                    .unwrap()
                {
                    RunAdmission::Admitted(_) => "admitted",
                    RunAdmission::AtCapacity => "at_capacity",
                    RunAdmission::MutexBusy => "mutex",
                    RunAdmission::QueuedRunUnavailable => "queued_gone",
                }
            }));
        }
        let mut admitted = 0;
        let mut at_capacity = 0;
        for handle in handles {
            match handle.join().unwrap() {
                "admitted" => admitted += 1,
                "at_capacity" => at_capacity += 1,
                other => panic!("unexpected admission outcome: {other}"),
            }
        }
        assert_eq!(admitted, 1, "exactly one run may hold the single slot");
        assert_eq!(at_capacity, threads - 1);
        assert_eq!(db.get_running_count().unwrap(), 1);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn admit_run_with_context_records_trigger_state_in_transaction() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::new(&dir);
        let workflow = admission_capacity_test_workflow(&db, Some(r#"{"queue":"source-default"}"#));

        let outcome = db
            .admit_run_with_context(
                &workflow.id,
                "source-default",
                "source",
                &[],
                Some("file_arrival"),
                Some(r#"{"trigger_id":"inbox","fingerprint":"abc"}"#),
                None,
                None,
                None,
                None,
                &[],
                Some(("inbox", "abc")),
            )
            .unwrap();
        assert!(matches!(outcome, RunAdmission::Admitted(_)));
        assert_eq!(
            db.get_trigger_fingerprint(&workflow.id, "inbox").unwrap(),
            Some("abc".to_string())
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn queue_admin_rejects_cap_lattice_violations() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::new(&dir);

        assert!(db
            .upsert_queue("too-large", "source", 9, Some(1), None)
            .is_err());
        assert!(db
            .upsert_queue("bad-tag", "source", 2, Some(3), None)
            .is_err());
        assert!(db
            .upsert_queue("source-heavy", "source", 2, Some(1), Some(10))
            .is_ok());

        let queue = db.get_queue("source-heavy", "source").unwrap();
        assert_eq!(queue.capacity, 2);
        assert_eq!(queue.tag_cap, Some(1));
        assert_eq!(queue.max_queued, Some(10));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn workflow_domain_round_trips_through_write_helpers() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::new(&dir);

        let workflow = db
            .create_workflow(
                "Owned Workflow",
                None,
                "scripts/workflows/noop.py",
                "0 0 * * *",
                false,
                true,
                "UTC",
                "source",
                Some("scheduler"),
                None,
                None,
            )
            .unwrap();
        assert_eq!(workflow.domain.as_deref(), Some("scheduler"));

        let updated = db
            .update_workflow(
                &workflow.id,
                "Owned Workflow",
                None,
                "scripts/workflows/noop.py",
                "0 1 * * *",
                true,
                false,
                true,
                "UTC",
                "source",
                Some("agent-ecosystem"),
                None,
                None,
            )
            .unwrap();
        assert_eq!(updated.domain.as_deref(), Some("agent-ecosystem"));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn mission_control_preferences_fall_back_from_invalid_values() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::new(&dir);
        let conn = db.conn().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO scheduler_config (key, value) VALUES ('mission_control.default_landing', 'surprise')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO scheduler_config (key, value) VALUES ('mission_control.corpus_filter', 'both')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO scheduler_config (key, value) VALUES ('mission_control.domain_filter', '  ')",
            [],
        )
        .unwrap();

        let prefs = db.get_mission_control_preferences().unwrap();
        assert_eq!(prefs.default_landing, "mission_control");
        // Environments are user-managed: an arbitrary environment filter value
        // is now preserved verbatim (no longer collapsed to "all").
        assert_eq!(prefs.corpus_filter, "both");
        assert_eq!(prefs.domain_filter, "all");

        let prefs = db
            .set_mission_control_preferences("dashboard", "source", "Unowned")
            .unwrap();
        assert_eq!(prefs.default_landing, "dashboard");
        assert_eq!(prefs.corpus_filter, "source");
        assert_eq!(prefs.domain_filter, "__unowned__");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn mission_control_read_models_filter_before_limits_and_account_success_statuses() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::new(&dir);
        let source = db
            .create_workflow(
                "Source Workflow",
                None,
                "scripts/workflows/noop.py",
                "0 0 * * *",
                false,
                true,
                "UTC",
                "source",
                Some("scheduler"),
                None,
                None,
            )
            .unwrap();
        let legacy_source = db
            .create_workflow(
                "Legacy Success Workflow",
                None,
                "scripts/workflows/noop.py",
                "0 1 * * *",
                false,
                true,
                "UTC",
                "source",
                Some("scheduler"),
                None,
                None,
            )
            .unwrap();
        let instance = db
            .create_workflow(
                "Instance Workflow",
                None,
                "scripts/workflows/noop.py",
                "0 2 * * *",
                false,
                true,
                "UTC",
                "instance",
                Some("card"),
                None,
                None,
            )
            .unwrap();
        let unowned = db
            .create_workflow(
                "Unowned Workflow",
                None,
                "scripts/workflows/noop.py",
                "0 3 * * *",
                false,
                true,
                "UTC",
                "source",
                Some("   "),
                None,
                None,
            )
            .unwrap();

        let source_run = db
            .create_terminal_run_with_context(&source.id, "success", None, None, None, None, None)
            .unwrap();
        db.create_terminal_run_with_context(&source.id, "success", None, None, None, None, None)
            .unwrap();
        db.create_terminal_run_with_context(
            &legacy_source.id,
            "succeeded",
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let instance_run = db
            .create_terminal_run_with_context(&instance.id, "failed", None, None, None, None, None)
            .unwrap();
        db.upsert_queued_run(&instance.id, "instance-default", 1)
            .unwrap();

        let source_header = db.mission_control_header("source", "scheduler").unwrap();
        assert_eq!(source_header.active_workflows, 2);
        assert_eq!(source_header.queued_count, 0);
        assert_eq!(source_header.recent_failures, 0);

        let instance_header = db.mission_control_header("instance", "all").unwrap();
        assert_eq!(instance_header.queued_count, 1);
        assert_eq!(instance_header.recent_failures, 1);

        let source_runs = db
            .mission_control_recent_runs("source", "scheduler", 1)
            .unwrap();
        assert_eq!(source_runs.len(), 1);
        assert_ne!(source_runs[0].workflow_id, instance.id);

        let sla = db
            .mission_control_sla_summary("source", "scheduler", 0)
            .unwrap();
        assert_eq!(sla.success_rate_24h, Some(1.0));

        let bucket = db.workflow_history_buckets(&legacy_source.id, 1).unwrap();
        assert_eq!(bucket[0].succeeded, 1);

        let unowned_rows = db.list_workflows_filtered("all", "__unowned__").unwrap();
        assert_eq!(unowned_rows[0].id, unowned.id);

        let domains = db.mission_control_domains("all").unwrap();
        assert!(domains.iter().any(|domain| domain.value == "__unowned__"));
        let source_domains = db.mission_control_domains("source").unwrap();
        assert!(source_domains
            .iter()
            .any(|domain| domain.value == "scheduler"));
        assert!(!source_domains.iter().any(|domain| domain.value == "card"));

        db.upsert_scheduler_asset(
            "source",
            "kg",
            "projects-attributed",
            Some("write"),
            Some(&source_run.id),
            None,
        )
        .unwrap();
        db.upsert_scheduler_asset(
            "source",
            "kg",
            "projects-rewritten",
            Some("write"),
            Some(&source_run.id),
            None,
        )
        .unwrap();
        db.upsert_scheduler_asset(
            "source",
            "kg",
            "projects-rewritten",
            Some("write"),
            None,
            None,
        )
        .unwrap();
        db.upsert_scheduler_asset("source", "manual", "unknown", Some("write"), None, None)
            .unwrap();
        let source_assets = db
            .mission_control_freshness_ledger("source", "scheduler", 0, 10)
            .unwrap();
        assert_eq!(source_assets.len(), 1);
        assert_eq!(source_assets[0].attribution, "last_writer_run");
        assert_eq!(source_assets[0].asset_partition, "projects-attributed");
        let all_assets = db
            .mission_control_freshness_ledger("all", "all", 0, 10)
            .unwrap();
        assert!(all_assets
            .iter()
            .any(|asset| asset.attribution == "unattributed_all_only"));
        let rewritten_asset = all_assets
            .iter()
            .find(|asset| asset.asset_partition == "projects-rewritten")
            .unwrap();
        assert_eq!(rewritten_asset.attribution, "unattributed_all_only");
        assert!(rewritten_asset.workflow_id.is_none());
        assert!(rewritten_asset.corpus.is_none());

        assert_eq!(instance_run.status, "failed");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn mission_control_failed_runs_count_current_terminal_failures_before_limit() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::new(&dir);

        let recovered = db
            .create_workflow(
                "Recovered Workflow",
                None,
                "scripts/workflows/noop.py",
                "0 0 * * *",
                false,
                true,
                "UTC",
                "source",
                Some("scheduler"),
                None,
                None,
            )
            .unwrap();
        db.create_terminal_run_with_context(&recovered.id, "failed", None, None, None, None, None)
            .unwrap();
        db.create_terminal_run_with_context(&recovered.id, "success", None, None, None, None, None)
            .unwrap();

        let mut old_started_failure_id = None;
        for idx in 0..6 {
            let workflow = db
                .create_workflow(
                    &format!("Failing Workflow {}", idx),
                    None,
                    "scripts/workflows/noop.py",
                    "0 0 * * *",
                    false,
                    true,
                    "UTC",
                    "source",
                    Some("scheduler"),
                    None,
                    None,
                )
                .unwrap();
            let run = db
                .create_terminal_run_with_context(
                    &workflow.id,
                    "failed",
                    None,
                    None,
                    None,
                    None,
                    None,
                )
                .unwrap();
            if idx == 0 {
                old_started_failure_id = Some(run.id);
            }
        }

        let now = chrono::Utc::now().to_rfc3339();
        db.conn()
            .unwrap()
            .execute(
                "UPDATE runs SET started_at = ?2, finished_at = ?3 WHERE id = ?1",
                params![
                    old_started_failure_id.unwrap(),
                    "2000-01-01T00:00:00+00:00",
                    now
                ],
            )
            .unwrap();

        let failed_count = db
            .mission_control_failed_run_count("source", "scheduler")
            .unwrap();
        assert_eq!(failed_count, 6);

        let displayed = db
            .mission_control_failed_runs("source", "scheduler", 4)
            .unwrap();
        assert_eq!(displayed.len(), 4);
        assert!(!displayed.iter().any(|run| run.workflow_id == recovered.id));

        let header = db.mission_control_header("source", "scheduler").unwrap();
        assert_eq!(header.recent_failures, 7);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn mission_control_telemetry_batches_visible_workflows() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::new(&dir);
        let source = db
            .create_workflow(
                "Source Telemetry",
                None,
                "scripts/workflows/noop.py",
                "0 0 * * *",
                false,
                true,
                "UTC",
                "source",
                Some("scheduler"),
                None,
                None,
            )
            .unwrap();
        let instance = db
            .create_workflow(
                "Instance Telemetry",
                None,
                "scripts/workflows/noop.py",
                "0 0 * * *",
                false,
                true,
                "UTC",
                "instance",
                Some("scheduler"),
                None,
                None,
            )
            .unwrap();
        let second_source = db
            .create_workflow(
                "Z Source Outside Limit",
                None,
                "scripts/workflows/noop.py",
                "0 0 * * *",
                false,
                true,
                "UTC",
                "source",
                Some("scheduler"),
                None,
                None,
            )
            .unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        db.insert_workflow_resource_sample(&WorkflowResourceSample {
            id: String::new(),
            run_id: None,
            workflow_id: source.id.clone(),
            queue_name: Some("source-default".to_string()),
            environment: "source".to_string(),
            pid: None,
            sampled_at: now.clone(),
            cpu_percent: Some(42.0),
            memory_rss_bytes: Some(256 * 1024 * 1024),
            memory_vms_bytes: None,
            swap_bytes: None,
            labels: None,
        })
        .unwrap();
        db.insert_workflow_resource_sample(&WorkflowResourceSample {
            id: String::new(),
            run_id: None,
            workflow_id: instance.id.clone(),
            queue_name: Some("instance-default".to_string()),
            environment: "instance".to_string(),
            pid: None,
            sampled_at: now.clone(),
            cpu_percent: Some(99.0),
            memory_rss_bytes: Some(512 * 1024 * 1024),
            memory_vms_bytes: None,
            swap_bytes: None,
            labels: None,
        })
        .unwrap();
        db.insert_workflow_token_usage(&WorkflowTokenUsage {
            id: String::new(),
            run_id: None,
            workflow_id: source.id.clone(),
            task_id: None,
            provider: "anthropic".to_string(),
            model: Some("claude".to_string()),
            token_kind: "input".to_string(),
            token_count: 123,
            emitted_at: now,
            labels: Some(serde_json::json!({"call_id": "call-source"})),
        })
        .unwrap();
        db.insert_workflow_token_usage(&WorkflowTokenUsage {
            id: String::new(),
            run_id: None,
            workflow_id: second_source.id.clone(),
            task_id: None,
            provider: "anthropic".to_string(),
            model: Some("claude".to_string()),
            token_kind: "input".to_string(),
            token_count: 999,
            emitted_at: chrono::Utc::now().to_rfc3339(),
            labels: Some(serde_json::json!({"call_id": "call-outside-limit"})),
        })
        .unwrap();

        let rows = db
            .mission_control_workflow_telemetry("source", "scheduler", "-24 hours", 1)
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].workflow_id, source.id);
        assert_eq!(rows[0].max_cpu_percent, Some(42.0));
        assert_eq!(rows[0].total_tokens, 123);
        assert_eq!(rows[0].token_call_count, 1);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn queued_runs_can_be_listed_admitted_and_cancelled() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::new(&dir);
        let workflow = db
            .create_workflow(
                "Queued Workflow",
                None,
                "scripts/workflows/noop.py",
                "0 0 * * *",
                false,
                true,
                "UTC",
                "source",
                None,
                None,
                Some(r#"{"queue":"source-default","priority":5}"#),
            )
            .unwrap();

        let queued_id = db
            .upsert_queued_run(&workflow.id, "source-default", 5)
            .unwrap();
        assert_eq!(db.list_queued_runs(10).unwrap().len(), 1);

        let run = db
            .create_run_with_context(&workflow.id, Some("manual"), None, None, None, None)
            .unwrap();
        assert_eq!(
            db.mark_queued_run_admitted(&workflow.id, &run.id).unwrap(),
            1
        );
        let rows = db.list_queued_runs(10).unwrap();
        assert_eq!(rows[0].status, "admitted");
        assert_eq!(rows[0].run_id.as_deref(), Some(run.id.as_str()));

        let queued_id_2 = db
            .upsert_queued_run(&workflow.id, "source-default", 4)
            .unwrap();
        assert_ne!(queued_id, queued_id_2);
        assert_eq!(db.cancel_queued_run(&queued_id_2).unwrap(), 1);
        let rows = db.list_queued_runs(10).unwrap();
        assert!(rows.iter().any(|row| row.status == "cancelled"));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn queued_runs_preserve_trigger_context_until_admission() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::new(&dir);
        let workflow = db
            .create_workflow(
                "Queued Trigger Workflow",
                None,
                "scripts/workflows/noop.py",
                "0 0 * * *",
                false,
                true,
                "UTC",
                "source",
                None,
                None,
                Some(r#"{"queue":"source-default","priority":5}"#),
            )
            .unwrap();

        let queued_id = db
            .upsert_queued_run_with_context(
                &workflow.id,
                "source-default",
                5,
                Some("file_arrival"),
                Some(r#"{"trigger_id":"inbox","fingerprint":"abc"}"#),
                Some("upstream-run"),
                Some(r#"{"manual":true}"#),
                Some("source-run"),
            )
            .unwrap();
        let rows = db.list_queued_runs(10).unwrap();
        let queued = rows.iter().find(|row| row.id == queued_id).unwrap();
        assert_eq!(queued.trigger_kind.as_deref(), Some("file_arrival"));
        assert_eq!(
            queued.trigger_payload.as_deref(),
            Some(r#"{"trigger_id":"inbox","fingerprint":"abc"}"#),
        );
        assert_eq!(queued.upstream_run_id.as_deref(), Some("upstream-run"));
        assert_eq!(queued.input_json.as_deref(), Some(r#"{"manual":true}"#));
        assert_eq!(queued.rerun_of_run_id.as_deref(), Some("source-run"));

        let run = db
            .create_run_with_context(
                &workflow.id,
                queued.trigger_kind.as_deref(),
                queued.trigger_payload.as_deref(),
                queued.upstream_run_id.as_deref(),
                queued.input_json.as_deref(),
                queued.rerun_of_run_id.as_deref(),
            )
            .unwrap();
        assert_eq!(
            db.mark_queued_run_admitted_by_id(&queued_id, &run.id)
                .unwrap(),
            1
        );
        let rows = db.list_queued_runs(10).unwrap();
        let admitted = rows.iter().find(|row| row.id == queued_id).unwrap();
        assert_eq!(admitted.status, "admitted");
        assert_eq!(admitted.run_id.as_deref(), Some(run.id.as_str()));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn dispatch_context_lookup_deduplicates_backfill_slots() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::new(&dir);
        let workflow = db
            .create_workflow(
                "Backfill Workflow",
                None,
                "scripts/workflows/noop.py",
                "0 0 * * *",
                false,
                true,
                "UTC",
                "source",
                None,
                None,
                Some(r#"{"queue":"source-default","priority":5}"#),
            )
            .unwrap();
        let payload = r#"{"logical_date":"2026-05-01T00:00:00Z","chain_suppressed":true}"#;
        let input = r#"{"backfill":{"logical_date":"2026-05-01T00:00:00Z"}}"#;

        assert!(db
            .find_run_by_dispatch_context(
                &workflow.id,
                Some("backfill"),
                Some(payload),
                Some(input),
                None
            )
            .unwrap()
            .is_none());
        let run = db
            .create_run_with_context(
                &workflow.id,
                Some("backfill"),
                Some(payload),
                None,
                Some(input),
                None,
            )
            .unwrap();
        assert_eq!(
            db.find_run_by_dispatch_context(
                &workflow.id,
                Some("backfill"),
                Some(payload),
                Some(input),
                None
            )
            .unwrap()
            .unwrap()
            .id,
            run.id
        );

        let queued_id = db
            .upsert_queued_run_with_context(
                &workflow.id,
                "source-default",
                5,
                Some("backfill"),
                Some(r#"{"logical_date":"2026-05-02T00:00:00Z","chain_suppressed":true}"#),
                None,
                Some(r#"{"backfill":{"logical_date":"2026-05-02T00:00:00Z"}}"#),
                None,
            )
            .unwrap();
        assert_eq!(
            db.find_queued_run_by_dispatch_context(
                &workflow.id,
                Some("backfill"),
                Some(r#"{"logical_date":"2026-05-02T00:00:00Z","chain_suppressed":true}"#),
                Some(r#"{"backfill":{"logical_date":"2026-05-02T00:00:00Z"}}"#),
                None
            )
            .unwrap()
            .unwrap()
            .id,
            queued_id
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn dead_letters_can_be_acknowledged_and_linked_to_recovery() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::new(&dir);
        let workflow = db
            .create_workflow(
                "Dead Letter Workflow",
                None,
                "scripts/workflows/noop.py",
                "0 0 * * *",
                false,
                false,
                "UTC",
                "source",
                None,
                None,
                None,
            )
            .unwrap();
        let run = db
            .create_terminal_run_with_context(
                &workflow.id,
                "dead_lettered",
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
        let dead_letter_id = db
            .upsert_scheduler_dead_letter(&run.id, &workflow.id, Some("extract"), None, "boom")
            .unwrap();

        let rows = db.list_scheduler_dead_letters(false, 10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].task_id.as_deref(), Some("extract"));
        assert_eq!(db.set_workflow_enabled(&workflow.id, true).unwrap(), 1);
        assert_eq!(
            db.acknowledge_scheduler_dead_letter(&dead_letter_id, "handled", Some("test"))
                .unwrap(),
            1
        );
        assert!(db
            .list_scheduler_dead_letters(false, 10)
            .unwrap()
            .is_empty());
        let recovery = db
            .create_run_with_context(
                &workflow.id,
                Some("dead_letter_recovery"),
                None,
                Some(&run.id),
                None,
                Some(&run.id),
            )
            .unwrap();
        assert_eq!(
            db.link_dead_letter_recovery(&dead_letter_id, &recovery.id)
                .unwrap(),
            1
        );
        let row = db.get_scheduler_dead_letter(&dead_letter_id).unwrap();
        assert_eq!(row.acknowledged_reason.as_deref(), Some("handled"));
        assert_eq!(row.acknowledged_by.as_deref(), Some("test"));
        assert_eq!(row.recovery_run_id.as_deref(), Some(recovery.id.as_str()));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn retention_cleanup_preserves_dead_letter_runs() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::new(&dir);
        let workflow = db
            .create_workflow(
                "Retention Workflow",
                None,
                "scripts/workflows/noop.py",
                "0 0 * * *",
                false,
                false,
                "UTC",
                "source",
                Some("scheduler"),
                None,
                Some(r#"{"queue":"source-default"}"#),
            )
            .unwrap();
        let deletable = db
            .create_terminal_run_with_context(&workflow.id, "success", None, None, None, None, None)
            .unwrap();
        let preserved = db
            .create_terminal_run_with_context(
                &workflow.id,
                "dead_lettered",
                Some("backfill"),
                None,
                None,
                None,
                None,
            )
            .unwrap();
        let conn = db.conn().unwrap();
        conn.execute(
            "UPDATE runs SET started_at = datetime('now', '-120 days'), finished_at = datetime('now', '-120 days') WHERE id IN (?1, ?2)",
            params![deletable.id, preserved.id],
        )
        .unwrap();
        db.upsert_scheduler_dead_letter(&preserved.id, &workflow.id, None, None, "boom")
            .unwrap();
        let deletable_queue_id = db
            .upsert_queued_run_with_context(
                &workflow.id,
                "source-default",
                0,
                Some("retention-test"),
                Some("delete"),
                None,
                None,
                None,
            )
            .unwrap();
        db.mark_queued_run_terminal_by_id(&deletable_queue_id, &deletable.id, "success")
            .unwrap();
        let preserved_queue_id = db
            .upsert_queued_run_with_context(
                &workflow.id,
                "source-default",
                0,
                Some("retention-test"),
                Some("preserve"),
                None,
                None,
                None,
            )
            .unwrap();
        db.mark_queued_run_terminal_by_id(&preserved_queue_id, &preserved.id, "dead_lettered")
            .unwrap();
        conn.execute(
            "INSERT INTO workflow_mutex_locks (mutex_key, workflow_id, run_id, acquired_at)
             VALUES ('retention-delete', ?1, ?2, datetime('now')),
                    ('retention-preserve', ?1, ?3, datetime('now'))",
            params![workflow.id, deletable.id, preserved.id],
        )
        .unwrap();

        let dry = db.cleanup_retention(90, true).unwrap();
        assert_eq!(dry.candidate_runs, 1);
        assert_eq!(dry.preserved_dead_letter_runs, 1);
        assert_eq!(dry.deleted_runs, 0);

        let applied = db.cleanup_retention(90, false).unwrap();
        assert_eq!(applied.deleted_runs, 1);
        assert!(db.get_run(&deletable.id).is_err());
        assert!(db.get_run(&preserved.id).is_ok());

        // Retention's runs-FK deletion matrix:
        // queued_runs.run_id is retained as queue history with SET NULL;
        // workflow_mutex_locks rows are deleted with the expired run;
        // dead-lettered runs and their dependent rows are preserved.
        let deleted_queue_run_id: Option<String> = conn
            .query_row(
                "SELECT run_id FROM queued_runs WHERE id = ?1",
                params![deletable_queue_id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(deleted_queue_run_id.is_none());
        let preserved_queue_run_id: Option<String> = conn
            .query_row(
                "SELECT run_id FROM queued_runs WHERE id = ?1",
                params![preserved_queue_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            preserved_queue_run_id.as_deref(),
            Some(preserved.id.as_str())
        );
        let deleted_lock_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM workflow_mutex_locks WHERE mutex_key = 'retention-delete'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(deleted_lock_count, 0);
        let preserved_lock_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM workflow_mutex_locks WHERE mutex_key = 'retention-preserve'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(preserved_lock_count, 1);

        let filtered = db
            .get_global_run_history(
                Some("dead_lettered"),
                Some("backfill"),
                Some("source"),
                Some("scheduler"),
                10,
            )
            .unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, preserved.id);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn notification_preferences_persist_in_scheduler_config() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::new(&dir);

        assert_eq!(db.get_notification_prefs().unwrap(), (true, false));
        db.set_notification_prefs(false, true).unwrap();
        assert_eq!(db.get_notification_prefs().unwrap(), (false, true));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn session_5_schema_tables_and_indexes_are_created() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::new(&dir);
        let conn = db.conn().unwrap();

        let tables: std::collections::HashSet<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        for table in [
            "run_attempts",
            "run_tasks",
            "run_metrics",
            "run_inputs",
            "run_outputs",
            "scheduler_assets",
            "run_assets",
            "run_lineage",
            "scheduler_idempotency_keys",
            "scheduler_checkpoints",
            "scheduler_dead_letters",
            "queue_events",
            "workflow_resource_samples",
            "workflow_token_usage",
        ] {
            assert!(tables.contains(table), "missing Session 5 table {table}");
        }

        let workflow_columns: std::collections::HashSet<String> = conn
            .prepare("PRAGMA table_info(workflows)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert!(workflow_columns.contains("domain"));

        let indexes: std::collections::HashSet<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'index'")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        for index in [
            "idx_runs_workflow_started",
            "idx_run_tasks_run_task",
            "idx_run_attempts_run_task",
            "idx_run_inputs_unique_key",
            "idx_run_outputs_unique_key",
            "idx_run_assets_identity_time",
            "idx_queue_events_queue_time",
            "idx_queue_events_run",
            "idx_resource_samples_workflow_time",
            "idx_token_usage_workflow_time",
        ] {
            assert!(indexes.contains(index), "missing Session 5 index {index}");
        }

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn session_5_schema_migrates_existing_scheduler_db_without_data_loss() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("scheduler.db");
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE workflows (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    description TEXT,
                    script_path TEXT NOT NULL,
                    cron_schedule TEXT NOT NULL,
                    enabled INTEGER DEFAULT 1,
                    async_mode INTEGER DEFAULT 0,
                    corpus TEXT NOT NULL DEFAULT 'source',
                    trigger_config TEXT,
                    queue_config TEXT,
                    created_at TEXT DEFAULT (datetime('now')),
                    updated_at TEXT DEFAULT (datetime('now')),
                    last_run_at TEXT,
                    email_on_failure INTEGER DEFAULT 1,
                    timezone TEXT DEFAULT 'UTC'
                );
                CREATE TABLE runs (
                    id TEXT PRIMARY KEY,
                    workflow_id TEXT NOT NULL REFERENCES workflows(id),
                    started_at TEXT NOT NULL,
                    finished_at TEXT,
                    exit_code INTEGER,
                    stdout TEXT,
                    stderr TEXT,
                    result_url TEXT,
                    trigger_kind TEXT,
                    trigger_payload TEXT,
                    upstream_run_id TEXT,
                    input_json TEXT,
                    rerun_of_run_id TEXT,
                    status TEXT DEFAULT 'running',
                    error_analysis TEXT
                );
                INSERT INTO workflows (id, name, script_path, cron_schedule)
                VALUES ('wf-1', 'Existing Workflow', 'scripts/workflows/noop.py', '0 0 * * *');
                INSERT INTO runs (id, workflow_id, started_at, status)
                VALUES ('run-1', 'wf-1', '2026-05-09T00:00:00Z', 'success');",
            )
            .unwrap();
        }

        let db = Database::new(&dir);
        let conn = db.conn().unwrap();
        let run_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM runs WHERE id = 'run-1'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(run_count, 1);
        let has_run_tasks: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'run_tasks'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(has_run_tasks, 1);
        let has_domain: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('workflows') WHERE name = 'domain'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(has_domain, 1);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn session_5_insert_helpers_record_history_assets_and_instrumentation() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::new(&dir);
        let workflow = db
            .create_workflow(
                "History Workflow",
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

        let attempt_id = db
            .insert_run_attempt(&run.id, "discover", 0, "running", None)
            .unwrap();
        let task_row_id = db
            .insert_run_task(
                &run.id,
                Some(&attempt_id),
                "discover",
                "started",
                0,
                Some(&serde_json::json!({"phase": "start"})),
            )
            .unwrap();
        db.finish_run_task(
            &task_row_id,
            "succeeded",
            None,
            None,
            Some(&serde_json::json!({"phase": "done"})),
        )
        .unwrap();
        db.finish_run_attempt(&attempt_id, "succeeded", Some(0), None, None)
            .unwrap();
        db.insert_run_metric(
            &run.id,
            Some("discover"),
            "items_seen",
            3.0,
            Some("count"),
            Some(&serde_json::json!({"source": "test"})),
        )
        .unwrap();
        db.insert_run_input(
            &run.id,
            None,
            "request",
            &serde_json::json!({"kind": "fixture"}),
            "1.0.0",
        )
        .unwrap();
        assert!(db
            .insert_run_input(
                &run.id,
                None,
                "request",
                &serde_json::json!({"kind": "duplicate"}),
                "1.0.0",
            )
            .is_err());
        db.insert_run_output(
            &run.id,
            None,
            "result",
            &serde_json::json!({"ok": true}),
            "1.0.0",
        )
        .unwrap();
        db.insert_run_asset(
            &run.id,
            Some("discover"),
            Some(&attempt_id),
            "source",
            "slack",
            "channel:C123",
            "write",
            Some(&serde_json::json!({"count": 3})),
        )
        .unwrap();
        let read_run = db
            .create_run_with_context(&workflow.id, Some("manual"), None, None, None, None)
            .unwrap();
        db.insert_run_asset(
            &read_run.id,
            Some("discover"),
            Some(&attempt_id),
            "source",
            "slack",
            "channel:C123",
            "read",
            None,
        )
        .unwrap();
        db.insert_run_lineage(
            &run.id,
            Some("discover"),
            Some(&attempt_id),
            &serde_json::json!({"eventType": "COMPLETE"}),
        )
        .unwrap();
        let latest_asset = db
            .latest_asset_write_matching("source", Some("slack"), Some("channel:C123"), None)
            .unwrap()
            .unwrap();
        assert_eq!(latest_asset.run_id, run.id);
        assert_eq!(latest_asset.workflow_id, workflow.id);
        db.insert_run_asset_with_freshness(
            &run.id,
            Some("discover"),
            Some(&attempt_id),
            "source",
            "notion",
            "page:1",
            "write",
            None,
            Some(&serde_json::json!({"max_age_seconds": 86400})),
        )
        .unwrap();
        let conn = db.conn().unwrap();
        let freshness_policy: Option<String> = conn
            .query_row(
                "SELECT freshness_policy_json FROM scheduler_assets WHERE asset_namespace = 'notion'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(freshness_policy.unwrap().contains("max_age_seconds"));
        let stale_assets = db.query_stale_assets(0, Some("source")).unwrap();
        assert!(stale_assets
            .iter()
            .any(|asset| asset.asset_namespace == "slack"));
        assert!(db
            .insert_idempotency_key(
                "wf-1:run-1:discover:item-1",
                Some(&run.id),
                Some("discover"),
                Some(&attempt_id)
            )
            .unwrap());
        assert!(!db
            .insert_idempotency_key(
                "wf-1:run-1:discover:item-1",
                Some(&run.id),
                Some("discover"),
                Some(&attempt_id)
            )
            .unwrap());
        let checkpoint_id = db
            .insert_scheduler_checkpoint(&run.id, "discover", Some(&attempt_id), "latest", b"state")
            .unwrap();
        let checkpoint_id_2 = db
            .insert_scheduler_checkpoint(
                &run.id,
                "discover",
                Some(&attempt_id),
                "latest",
                b"new-state",
            )
            .unwrap();
        assert_eq!(checkpoint_id, checkpoint_id_2);
        let dead_letter_id = db
            .upsert_scheduler_dead_letter(
                &run.id,
                &workflow.id,
                Some("discover"),
                Some(&attempt_id),
                "boom",
            )
            .unwrap();
        let dead_letter_id_2 = db
            .upsert_scheduler_dead_letter(
                &run.id,
                &workflow.id,
                Some("discover"),
                Some(&attempt_id),
                "boom again",
            )
            .unwrap();
        assert_eq!(dead_letter_id, dead_letter_id_2);
        db.insert_queue_event(
            "source-default",
            "source",
            Some(&workflow.id),
            Some(&run.id),
            "admitted",
            None,
            Some(&serde_json::json!({"priority": 0})),
        )
        .unwrap();
        db.insert_workflow_resource_sample(&WorkflowResourceSample {
            id: String::new(),
            run_id: Some(run.id.clone()),
            workflow_id: workflow.id.clone(),
            queue_name: Some("source-default".to_string()),
            environment: "source".to_string(),
            pid: Some(123),
            sampled_at: chrono::Utc::now().to_rfc3339(),
            cpu_percent: Some(1.25),
            memory_rss_bytes: Some(1024),
            memory_vms_bytes: Some(2048),
            swap_bytes: Some(0),
            labels: Some(serde_json::json!({"host": "local"})),
        })
        .unwrap();
        db.insert_workflow_token_usage(&WorkflowTokenUsage {
            id: String::new(),
            run_id: Some(run.id.clone()),
            workflow_id: workflow.id.clone(),
            task_id: Some("discover".to_string()),
            provider: "anthropic".to_string(),
            model: Some("claude".to_string()),
            token_kind: "input".to_string(),
            token_count: 42,
            emitted_at: chrono::Utc::now().to_rfc3339(),
            labels: None,
        })
        .unwrap();

        let conn = db.conn().unwrap();
        for (table, expected_count) in [
            ("run_attempts", 1),
            ("run_tasks", 1),
            ("run_metrics", 1),
            ("run_inputs", 1),
            ("run_outputs", 1),
            ("scheduler_assets", 2),
            ("run_assets", 3),
            ("run_lineage", 1),
            ("scheduler_idempotency_keys", 1),
            ("scheduler_checkpoints", 1),
            ("scheduler_dead_letters", 1),
            ("queue_events", 1),
            ("workflow_resource_samples", 1),
            ("workflow_token_usage", 1),
        ] {
            let count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {}", table), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(count, expected_count, "unexpected row count in {table}");
        }
        let asset_state: (String, String) = conn
            .query_row(
                "SELECT last_action, last_writer_run_id FROM scheduler_assets WHERE asset_kind = 'source' AND asset_namespace = 'slack'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(asset_state.0, "read");
        assert_eq!(asset_state.1, run.id);
        let checkpoint_blob: Vec<u8> = conn
            .query_row(
                "SELECT state_blob FROM scheduler_checkpoints WHERE id = ?1",
                params![checkpoint_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(checkpoint_blob, b"new-state");
        let dead_letter_exception: String = conn
            .query_row(
                "SELECT last_exception FROM scheduler_dead_letters WHERE id = ?1",
                params![dead_letter_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(dead_letter_exception, "boom again");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn resource_sample_query_filters_by_workflow_and_window() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::new(&dir);
        let workflow = db
            .create_workflow(
                "Rollup Workflow",
                None,
                "scripts/workflows/noop.py",
                "0 0 * * *",
                false,
                true,
                "UTC",
                "source",
                Some("scheduler"),
                None,
                Some(r#"{"queue":"source-default"}"#),
            )
            .unwrap();
        let other = db
            .create_workflow(
                "Other Workflow",
                None,
                "scripts/workflows/noop.py",
                "0 0 * * *",
                false,
                true,
                "UTC",
                "source",
                Some("scheduler"),
                None,
                Some(r#"{"queue":"source-default"}"#),
            )
            .unwrap();
        let run = db
            .create_run_with_context(&workflow.id, Some("manual"), None, None, None, None)
            .unwrap();
        for (workflow_id, sampled_at, rss) in [
            (&workflow.id, chrono::Utc::now().to_rfc3339(), 1024),
            (&workflow.id, "2000-01-01T00:00:00Z".to_string(), 2048),
            (&other.id, chrono::Utc::now().to_rfc3339(), 4096),
        ] {
            db.insert_workflow_resource_sample(&WorkflowResourceSample {
                id: String::new(),
                run_id: Some(run.id.clone()),
                workflow_id: workflow_id.clone(),
                queue_name: Some("source-default".to_string()),
                environment: "source".to_string(),
                pid: Some(123),
                sampled_at,
                cpu_percent: Some(1.0),
                memory_rss_bytes: Some(rss),
                memory_vms_bytes: Some(rss * 2),
                swap_bytes: None,
                labels: None,
            })
            .unwrap();
        }

        let samples = db
            .query_workflow_resource_samples(&workflow.id, "-1 days")
            .unwrap();

        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].workflow_id, workflow.id);
        assert_eq!(samples[0].memory_rss_bytes, Some(1024));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn token_usage_rollup_counts_distinct_calls_not_token_rows() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::new(&dir);
        let workflow = db
            .create_workflow(
                "Token Workflow",
                None,
                "scripts/workflows/noop.py",
                "0 0 * * *",
                false,
                true,
                "UTC",
                "source",
                Some("scheduler"),
                None,
                Some(r#"{"queue":"source-default"}"#),
            )
            .unwrap();
        let run = db
            .create_run_with_context(&workflow.id, Some("manual"), None, None, None, None)
            .unwrap();
        for (token_kind, token_count, call_id) in [
            ("input", 10, "call-1"),
            ("output", 5, "call-1"),
            ("input", 3, "call-2"),
        ] {
            db.insert_workflow_token_usage(&WorkflowTokenUsage {
                id: String::new(),
                run_id: Some(run.id.clone()),
                workflow_id: workflow.id.clone(),
                task_id: Some("summarize".to_string()),
                provider: "anthropic".to_string(),
                model: Some("claude".to_string()),
                token_kind: token_kind.to_string(),
                token_count,
                emitted_at: chrono::Utc::now().to_rfc3339(),
                labels: Some(serde_json::json!({
                    "corpus": "source",
                    "domain": "scheduler",
                    "queue_name": "source-default",
                    "call_id": call_id,
                })),
            })
            .unwrap();
        }

        let rollups = db
            .query_token_usage_rollup(
                &[
                    "workflow_id".to_string(),
                    "corpus".to_string(),
                    "domain".to_string(),
                    "queue_name".to_string(),
                    "provider".to_string(),
                    "model".to_string(),
                ],
                "-1 days",
                "hour",
            )
            .unwrap();

        assert_eq!(rollups.len(), 1);
        assert_eq!(
            rollups[0].workflow_id.as_deref(),
            Some(workflow.id.as_str())
        );
        assert_eq!(rollups[0].corpus.as_deref(), Some("source"));
        assert_eq!(rollups[0].domain.as_deref(), Some("scheduler"));
        assert_eq!(rollups[0].queue_name.as_deref(), Some("source-default"));
        assert_eq!(rollups[0].total_tokens, 18);
        assert_eq!(rollups[0].call_count, 2);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn session_5_detail_rows_follow_run_and_workflow_delete_semantics() {
        let dir = std::env::temp_dir().join(format!("chaos-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::new(&dir);
        let workflow = db
            .create_workflow(
                "Delete Workflow",
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
        db.upsert_queued_run(&workflow.id, "source-default", 0)
            .unwrap();
        db.mark_queued_run_admitted(&workflow.id, &run.id).unwrap();
        let mutex_keys = vec!["tag:source:source-default:cleanup".to_string()];
        assert!(db
            .acquire_mutex_locks(&workflow.id, &run.id, &mutex_keys)
            .unwrap());
        let attempt_id = db
            .insert_run_attempt(&run.id, "cleanup", 0, "running", None)
            .unwrap();
        db.insert_run_task(&run.id, Some(&attempt_id), "cleanup", "started", 0, None)
            .unwrap();
        db.insert_run_metric(&run.id, Some("cleanup"), "rows", 1.0, None, None)
            .unwrap();
        db.insert_run_input(
            &run.id,
            Some("cleanup"),
            "request",
            &serde_json::json!({"delete": true}),
            "1.0.0",
        )
        .unwrap();
        db.insert_run_output(
            &run.id,
            Some("cleanup"),
            "result",
            &serde_json::json!({"ok": true}),
            "1.0.0",
        )
        .unwrap();
        db.insert_run_asset(
            &run.id,
            Some("cleanup"),
            Some(&attempt_id),
            "source",
            "test",
            "partition",
            "write",
            None,
        )
        .unwrap();
        db.insert_run_lineage(
            &run.id,
            Some("cleanup"),
            Some(&attempt_id),
            &serde_json::json!({}),
        )
        .unwrap();
        db.insert_idempotency_key(
            "delete-key",
            Some(&run.id),
            Some("cleanup"),
            Some(&attempt_id),
        )
        .unwrap();
        db.insert_scheduler_checkpoint(&run.id, "cleanup", Some(&attempt_id), "latest", b"state")
            .unwrap();
        db.upsert_scheduler_dead_letter(
            &run.id,
            &workflow.id,
            Some("cleanup"),
            Some(&attempt_id),
            "boom",
        )
        .unwrap();
        db.insert_queue_event(
            "source-default",
            "source",
            Some(&workflow.id),
            Some(&run.id),
            "admitted",
            None,
            None,
        )
        .unwrap();
        db.insert_workflow_resource_sample(&WorkflowResourceSample {
            id: String::new(),
            run_id: Some(run.id.clone()),
            workflow_id: workflow.id.clone(),
            queue_name: Some("source-default".to_string()),
            environment: "source".to_string(),
            pid: Some(123),
            sampled_at: chrono::Utc::now().to_rfc3339(),
            cpu_percent: Some(1.0),
            memory_rss_bytes: Some(1),
            memory_vms_bytes: Some(1),
            swap_bytes: Some(0),
            labels: None,
        })
        .unwrap();
        db.insert_workflow_token_usage(&WorkflowTokenUsage {
            id: String::new(),
            run_id: Some(run.id.clone()),
            workflow_id: workflow.id.clone(),
            task_id: Some("cleanup".to_string()),
            provider: "anthropic".to_string(),
            model: None,
            token_kind: "input".to_string(),
            token_count: 1,
            emitted_at: chrono::Utc::now().to_rfc3339(),
            labels: None,
        })
        .unwrap();

        db.delete_workflow(&workflow.id).unwrap();
        let conn = db.conn().unwrap();
        for table in [
            "runs",
            "run_attempts",
            "run_tasks",
            "run_metrics",
            "run_inputs",
            "run_outputs",
            "run_assets",
            "run_lineage",
            "scheduler_checkpoints",
            "scheduler_dead_letters",
            "workflow_resource_samples",
            "workflow_token_usage",
            "queued_runs",
            "workflow_mutex_locks",
        ] {
            let count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {}", table), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(count, 0, "expected {table} to cascade-delete");
        }
        let idempotency_run_id: Option<String> = conn
            .query_row(
                "SELECT run_id FROM scheduler_idempotency_keys WHERE key = 'delete-key'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(idempotency_run_id, None);
        let asset_writer_run_id: Option<String> = conn
            .query_row(
                "SELECT last_writer_run_id FROM scheduler_assets WHERE asset_kind = 'source'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(asset_writer_run_id, None);
        let queue_refs: (Option<String>, Option<String>) = conn
            .query_row(
                "SELECT workflow_id, run_id FROM queue_events WHERE event_type = 'admitted'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(queue_refs, (None, None));

        let _ = std::fs::remove_dir_all(dir);
    }
}
