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
        Connection::open(&self.path)
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
            "INSERT OR IGNORE INTO scheduler_config (key, value) VALUES ('global_parallelism_cap', '4');
             INSERT OR IGNORE INTO queues (name, corpus, capacity) VALUES ('source-default', 'source', 4);
             INSERT OR IGNORE INTO queues (name, corpus, capacity) VALUES ('instance-default', 'instance', 2);",
        )?;
        Ok(())
    }

    pub fn list_workflows(&self) -> rusqlite::Result<Vec<Workflow>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, description, script_path, cron_schedule, enabled, async_mode, last_run_at, created_at, updated_at, email_on_failure, timezone, corpus, trigger_config, queue_config FROM workflows ORDER BY corpus, name"
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
                trigger_config: row.get(13).unwrap_or(None),
                queue_config: row.get(14).unwrap_or(None),
            })
        })?;
        rows.collect()
    }

    pub fn get_workflow(&self, id: &str) -> rusqlite::Result<Workflow> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT id, name, description, script_path, cron_schedule, enabled, async_mode, last_run_at, created_at, updated_at, email_on_failure, timezone, corpus, trigger_config, queue_config FROM workflows WHERE id = ?1",
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
                    trigger_config: row.get(13).unwrap_or(None),
                    queue_config: row.get(14).unwrap_or(None),
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
}
