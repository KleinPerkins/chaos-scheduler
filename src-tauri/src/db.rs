use rusqlite::{params, types::Type, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;

fn default_utc() -> String {
    "UTC".to_string()
}

fn default_source_corpus() -> String {
    "source".to_string()
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
    pub corpus: String,
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
    pub corpus: String,
    pub priority: i64,
    pub status: String,
    pub queued_at: String,
    pub admitted_at: Option<String>,
    pub finished_at: Option<String>,
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
    pub corpus: String,
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
pub struct NextRun {
    pub workflow_id: String,
    pub workflow_name: String,
    pub corpus: String,
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
            from_name: String::from("Chaos Labs Scheduler"),
        }
    }
}

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

    fn conn(&self) -> rusqlite::Result<Connection> {
        let conn = Connection::open(&self.path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(conn)
    }

    fn init(&self) -> rusqlite::Result<()> {
        let conn = self.conn()?;
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
                status TEXT DEFAULT 'running'
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
                run_id TEXT REFERENCES runs(id),
                workflow_id TEXT NOT NULL REFERENCES workflows(id),
                queue_name TEXT NOT NULL,
                priority INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'queued',
                queued_at TEXT NOT NULL,
                admitted_at TEXT,
                finished_at TEXT
            );
            CREATE TABLE IF NOT EXISTS workflow_mutex_locks (
                mutex_key TEXT PRIMARY KEY,
                workflow_id TEXT NOT NULL REFERENCES workflows(id),
                run_id TEXT REFERENCES runs(id),
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
                from_name TEXT DEFAULT 'Chaos Labs Scheduler'
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
            CREATE TABLE IF NOT EXISTS scheduler_idempotency_keys (
                key TEXT PRIMARY KEY,
                run_id TEXT REFERENCES runs(id) ON DELETE SET NULL,
                task_id TEXT,
                attempt_id TEXT REFERENCES run_attempts(id) ON DELETE SET NULL,
                created_at TEXT NOT NULL
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
            CREATE INDEX IF NOT EXISTS idx_idempotency_run_task ON scheduler_idempotency_keys(run_id, task_id);
            CREATE INDEX IF NOT EXISTS idx_checkpoints_run_task ON scheduler_checkpoints(run_id, task_id);
            CREATE INDEX IF NOT EXISTS idx_dead_letters_workflow ON scheduler_dead_letters(workflow_id, last_failure_at);
            CREATE INDEX IF NOT EXISTS idx_queue_events_queue_time ON queue_events(queue_name, corpus, emitted_at);
            CREATE INDEX IF NOT EXISTS idx_resource_samples_workflow_time ON workflow_resource_samples(workflow_id, sampled_at);
            CREATE INDEX IF NOT EXISTS idx_resource_samples_run ON workflow_resource_samples(run_id);
            CREATE INDEX IF NOT EXISTS idx_token_usage_workflow_time ON workflow_token_usage(workflow_id, emitted_at);
            CREATE INDEX IF NOT EXISTS idx_token_usage_run ON workflow_token_usage(run_id);",
        )?;
        conn.execute_batch(
            "INSERT OR IGNORE INTO scheduler_config (key, value) VALUES ('global_parallelism_cap', '4');
             INSERT OR IGNORE INTO queues (name, corpus, capacity) VALUES ('source-default', 'source', 4);
             INSERT OR IGNORE INTO queues (name, corpus, capacity) VALUES ('instance-default', 'instance', 2);",
        )?;
        Ok(())
    }

    pub fn list_workflows(&self) -> rusqlite::Result<Vec<Workflow>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, description, script_path, cron_schedule, enabled, async_mode, last_run_at, created_at, updated_at, email_on_failure, timezone, corpus, domain, trigger_config, queue_config FROM workflows ORDER BY corpus, name"
        )?;
        let rows = stmt.query_map([], |row| {
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
                corpus: row
                    .get::<_, String>(12)
                    .unwrap_or_else(|_| "source".to_string()),
                domain: row.get(13).unwrap_or(None),
                trigger_config: row.get(14).unwrap_or(None),
                queue_config: row.get(15).unwrap_or(None),
            })
        })?;
        rows.collect()
    }

    pub fn get_workflow(&self, id: &str) -> rusqlite::Result<Workflow> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT id, name, description, script_path, cron_schedule, enabled, async_mode, last_run_at, created_at, updated_at, email_on_failure, timezone, corpus, domain, trigger_config, queue_config FROM workflows WHERE id = ?1",
            params![id],
            |row| {
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
                    timezone: row.get::<_, String>(11).unwrap_or_else(|_| "UTC".to_string()),
                    corpus: row.get::<_, String>(12).unwrap_or_else(|_| "source".to_string()),
                    domain: row.get(13).unwrap_or(None),
                    trigger_config: row.get(14).unwrap_or(None),
                    queue_config: row.get(15).unwrap_or(None),
                })
            },
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
        trigger_config: Option<&str>,
        queue_config: Option<&str>,
    ) -> rusqlite::Result<Workflow> {
        let id = uuid::Uuid::new_v4().to_string();
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO workflows (id, name, description, script_path, cron_schedule, async_mode, email_on_failure, timezone, corpus, trigger_config, queue_config) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![id, name, description, script_path, cron_schedule, async_mode as i32, email_on_failure as i32, timezone, corpus, trigger_config, queue_config],
        )?;
        self.get_workflow(&id)
    }

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
        trigger_config: Option<&str>,
        queue_config: Option<&str>,
    ) -> rusqlite::Result<Workflow> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE workflows SET name = ?2, description = ?3, script_path = ?4, cron_schedule = ?5, enabled = ?6, async_mode = ?7, email_on_failure = ?8, timezone = ?9, corpus = ?10, trigger_config = ?11, queue_config = ?12, updated_at = datetime('now') WHERE id = ?1",
            params![id, name, description, script_path, cron_schedule, enabled as i32, async_mode as i32, email_on_failure as i32, timezone, corpus, trigger_config, queue_config],
        )?;
        self.get_workflow(id)
    }

    pub fn delete_workflow(&self, id: &str) -> rusqlite::Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM runs WHERE workflow_id = ?1", params![id])?;
        conn.execute("DELETE FROM workflows WHERE id = ?1", params![id])?;
        Ok(())
    }

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

    pub fn finish_run(
        &self,
        id: &str,
        exit_code: i32,
        stdout: &str,
        stderr: &str,
        result_url: Option<&str>,
    ) -> rusqlite::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let status = if exit_code == 0 { "success" } else { "failed" };
        let conn = self.conn()?;
        conn.execute(
            "UPDATE runs SET finished_at = ?2, exit_code = ?3, stdout = ?4, stderr = ?5, result_url = ?6, status = ?7 WHERE id = ?1",
            params![id, now, exit_code, stdout, stderr, result_url, status],
        )?;
        conn.execute(
            "UPDATE queued_runs SET status = ?2, finished_at = ?3 WHERE run_id = ?1",
            params![id, status, now],
        )?;
        let _ = self.release_mutex_locks(id);
        Ok(())
    }

    pub fn finish_run_with_status(
        &self,
        id: &str,
        status: &str,
        stdout: &str,
        stderr: &str,
    ) -> rusqlite::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn()?;
        conn.execute(
            "UPDATE runs SET finished_at = ?2, stdout = ?3, stderr = ?4, status = ?5 WHERE id = ?1",
            params![id, now, stdout, stderr, status],
        )?;
        conn.execute(
            "UPDATE queued_runs SET status = ?2, finished_at = ?3 WHERE run_id = ?1",
            params![id, status, now],
        )?;
        let _ = self.release_mutex_locks(id);
        Ok(())
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
                last_writer_run_id = COALESCE(excluded.last_writer_run_id, scheduler_assets.last_writer_run_id),
                freshness_policy_json = COALESCE(excluded.freshness_policy_json, scheduler_assets.freshness_policy_json),
                updated_at = datetime('now')",
            params![asset_id, asset_kind, asset_namespace, asset_partition, last_action, last_written_at, last_writer_run_id, freshness_policy_json],
        )?;
        Ok(asset_id)
    }

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
            None,
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
            "INSERT OR IGNORE INTO scheduler_idempotency_keys (key, run_id, task_id, attempt_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
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

    pub fn insert_queue_event(
        &self,
        queue_name: &str,
        corpus: &str,
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
            "INSERT INTO queue_events (id, queue_name, corpus, workflow_id, run_id, event_type, reason, emitted_at, details_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![id, queue_name, corpus, workflow_id, run_id, event_type, reason, emitted_at, details_json],
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
                id, run_id, workflow_id, queue_name, corpus, pid, sampled_at, cpu_percent, memory_rss_bytes, memory_vms_bytes, swap_bytes, labels_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                id,
                sample.run_id.as_deref(),
                &sample.workflow_id,
                sample.queue_name.as_deref(),
                &sample.corpus,
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
            "SELECT COUNT(*) FROM runs WHERE status = 'running'",
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

    pub fn queue_capacity(&self, queue_name: &str, corpus: &str) -> rusqlite::Result<i64> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT capacity FROM queues WHERE name = ?1 AND corpus = ?2")?;
        let mut rows = stmt.query(params![queue_name, corpus])?;
        if let Some(row) = rows.next()? {
            let capacity: i64 = row.get(0)?;
            Ok(capacity.max(1))
        } else {
            Ok(1)
        }
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

    pub fn validate_queue_cap_lattice(&self) -> rusqlite::Result<Vec<String>> {
        let conn = self.conn()?;
        let global_cap = self.global_parallelism_cap()?;
        let mut errors = Vec::new();
        if global_cap < 1 {
            errors.push("global_parallelism_cap must be >= 1".to_string());
        }
        let mut stmt = conn
            .prepare("SELECT name, corpus, capacity, tag_cap FROM queues ORDER BY corpus, name")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<i64>>(3)?,
            ))
        })?;
        for row in rows {
            let (name, corpus, capacity, tag_cap) = row?;
            let label = format!("{}/{}", corpus, name);
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
            "SELECT name, corpus, capacity, tag_cap, max_queued, updated_at FROM queues ORDER BY corpus, name",
        )?;
        let rows = stmt.query_map([], |row| {
            let name: String = row.get(0)?;
            let corpus: String = row.get(1)?;
            Ok(QueueInfo {
                active_count: self.running_count_for_queue(&name, &corpus)?,
                queued_count: self.queued_count_for_queue(&name, &corpus)?,
                name,
                corpus,
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
        corpus: &str,
        capacity: i64,
        tag_cap: Option<i64>,
        max_queued: Option<i64>,
    ) -> rusqlite::Result<QueueInfo> {
        validate_queue_values(
            name,
            corpus,
            capacity,
            tag_cap,
            max_queued,
            self.global_parallelism_cap()?,
        )?;
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO queues (name, corpus, capacity, tag_cap, max_queued, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))
             ON CONFLICT(name, corpus) DO UPDATE SET
               capacity = excluded.capacity,
               tag_cap = excluded.tag_cap,
               max_queued = excluded.max_queued,
               updated_at = datetime('now')",
            params![name, corpus, capacity, tag_cap, max_queued],
        )?;
        self.get_queue(name, corpus)
    }

    pub fn get_queue(&self, name: &str, corpus: &str) -> rusqlite::Result<QueueInfo> {
        let global_cap = self.global_parallelism_cap()?;
        let conn = self.conn()?;
        conn.query_row(
            "SELECT name, corpus, capacity, tag_cap, max_queued, updated_at FROM queues WHERE name = ?1 AND corpus = ?2",
            params![name, corpus],
            |row| {
                Ok(QueueInfo {
                    name: row.get(0)?,
                    corpus: row.get(1)?,
                    capacity: row.get(2)?,
                    tag_cap: row.get(3)?,
                    max_queued: row.get(4)?,
                    active_count: self.running_count_for_queue(name, corpus)?,
                    queued_count: self.queued_count_for_queue(name, corpus)?,
                    global_parallelism_cap: global_cap,
                    updated_at: row.get(5)?,
                })
            },
        )
    }

    pub fn list_queued_runs(&self, limit: i64) -> rusqlite::Result<Vec<QueuedRun>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT q.id, q.run_id, q.workflow_id, w.name, q.queue_name, w.corpus, q.priority, q.status, q.queued_at, q.admitted_at, q.finished_at
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
                corpus: row
                    .get::<_, Option<String>>(5)?
                    .unwrap_or_else(|| "source".to_string()),
                priority: row.get(6)?,
                status: row.get(7)?,
                queued_at: row.get(8)?,
                admitted_at: row.get(9)?,
                finished_at: row.get(10)?,
            })
        })?;
        rows.collect()
    }

    pub fn upsert_queued_run(
        &self,
        workflow_id: &str,
        queue_name: &str,
        priority: i64,
    ) -> rusqlite::Result<String> {
        let conn = self.conn()?;
        let existing: rusqlite::Result<String> = conn.query_row(
            "SELECT id FROM queued_runs WHERE workflow_id = ?1 AND status = 'queued' ORDER BY queued_at ASC LIMIT 1",
            params![workflow_id],
            |row| row.get(0),
        );
        if let Ok(id) = existing {
            conn.execute(
                "UPDATE queued_runs SET queue_name = ?2, priority = ?3 WHERE id = ?1",
                params![id, queue_name, priority],
            )?;
            return Ok(id);
        }

        let workflow = self.get_workflow(workflow_id)?;
        let max_queued: Option<i64> = conn
            .query_row(
                "SELECT max_queued FROM queues WHERE name = ?1 AND corpus = ?2",
                params![queue_name, workflow.corpus],
                |row| row.get(0),
            )
            .unwrap_or(None);
        if let Some(max_queued) = max_queued {
            if self.queued_count_for_queue(queue_name, &workflow.corpus)? >= max_queued {
                return Err(rusqlite::Error::InvalidParameterName(format!(
                    "queue {} max queued threshold {} reached",
                    queue_name, max_queued
                )));
            }
        }

        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO queued_runs (id, workflow_id, queue_name, priority, status, queued_at)
             VALUES (?1, ?2, ?3, ?4, 'queued', ?5)",
            params![id, workflow_id, queue_name, priority, now],
        )?;
        Ok(id)
    }

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
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE queued_runs SET run_id = ?2, status = 'admitted', admitted_at = ?3 WHERE id = ?1",
            params![id, run_id, now],
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

    fn running_count_for_queue(&self, queue_name: &str, corpus: &str) -> rusqlite::Result<i64> {
        let mut count = 0;
        for run in self.get_running_runs()? {
            let workflow = self.get_workflow(&run.workflow_id)?;
            let (run_queue, run_corpus) =
                queue_identity_from_config(workflow.queue_config.as_deref(), &workflow.corpus);
            if run_queue == queue_name && run_corpus == corpus {
                count += 1;
            }
        }
        Ok(count)
    }

    fn queued_count_for_queue(&self, queue_name: &str, corpus: &str) -> rusqlite::Result<i64> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT COUNT(*)
             FROM queued_runs q
             LEFT JOIN workflows w ON q.workflow_id = w.id
             WHERE q.queue_name = ?1 AND COALESCE(w.corpus, 'source') = ?2 AND q.status = 'queued'",
            params![queue_name, corpus],
            |row| row.get(0),
        )
    }

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
        let tx = conn.transaction()?;
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
             FROM runs r LEFT JOIN workflows w ON r.workflow_id = w.id WHERE r.status = 'running' ORDER BY r.started_at DESC"
        )?;
        let rows = stmt.query_map([], |row| Ok(run_from_row(row)))?;
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

fn scheduler_asset_id(asset_kind: &str, asset_namespace: &str, asset_partition: &str) -> String {
    format!("{}:{}:{}", asset_kind, asset_namespace, asset_partition)
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
    corpus: &str,
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
    if !matches!(corpus, "source" | "instance") {
        return Err(rusqlite::Error::InvalidParameterName(
            "queue corpus must be source or instance".to_string(),
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

fn queue_identity_from_config(queue_config: Option<&str>, corpus: &str) -> (String, String) {
    let default_queue = format!("{}-default", corpus);
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
    (queue, corpus.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

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
            "INSERT OR REPLACE INTO queues (name, corpus, capacity, tag_cap) VALUES ('too-big', 'source', 5, 6)",
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
            corpus: "source".to_string(),
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
            ("scheduler_assets", 1),
            ("run_assets", 2),
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
                Some(r#"{"queue":"source-default"}"#),
            )
            .unwrap();
        let run = db
            .create_run_with_context(&workflow.id, Some("manual"), None, None, None, None)
            .unwrap();
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
            corpus: "source".to_string(),
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
