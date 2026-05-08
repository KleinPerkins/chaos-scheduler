use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;

fn default_utc() -> String {
    "UTC".to_string()
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
    #[serde(default = "default_utc")]
    pub timezone: String,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerStatus {
    pub active_workflows: usize,
    pub running_count: usize,
    pub next_runs: Vec<NextRun>,
    pub recent_runs: Vec<Run>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextRun {
    pub workflow_id: String,
    pub workflow_name: String,
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
                status TEXT DEFAULT 'running'
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
        let _ = conn.execute_batch("ALTER TABLE workflows ADD COLUMN timezone TEXT DEFAULT 'UTC';");
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
        Ok(())
    }

    pub fn list_workflows(&self) -> rusqlite::Result<Vec<Workflow>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, description, script_path, cron_schedule, enabled, async_mode, last_run_at, created_at, updated_at, email_on_failure, timezone FROM workflows ORDER BY name"
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
            })
        })?;
        rows.collect()
    }

    pub fn get_workflow(&self, id: &str) -> rusqlite::Result<Workflow> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT id, name, description, script_path, cron_schedule, enabled, async_mode, last_run_at, created_at, updated_at, email_on_failure, timezone FROM workflows WHERE id = ?1",
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
    ) -> rusqlite::Result<Workflow> {
        let id = uuid::Uuid::new_v4().to_string();
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO workflows (id, name, description, script_path, cron_schedule, async_mode, email_on_failure, timezone) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, name, description, script_path, cron_schedule, async_mode as i32, email_on_failure as i32, timezone],
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
    ) -> rusqlite::Result<Workflow> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE workflows SET name = ?2, description = ?3, script_path = ?4, cron_schedule = ?5, enabled = ?6, async_mode = ?7, email_on_failure = ?8, timezone = ?9, updated_at = datetime('now') WHERE id = ?1",
            params![id, name, description, script_path, cron_schedule, enabled as i32, async_mode as i32, email_on_failure as i32, timezone],
        )?;
        self.get_workflow(id)
    }

    pub fn delete_workflow(&self, id: &str) -> rusqlite::Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM runs WHERE workflow_id = ?1", params![id])?;
        conn.execute("DELETE FROM workflows WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn create_run(&self, workflow_id: &str) -> rusqlite::Result<Run> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO runs (id, workflow_id, started_at, status) VALUES (?1, ?2, ?3, 'running')",
            params![id, workflow_id, now],
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
        Ok(())
    }

    pub fn get_run(&self, id: &str) -> rusqlite::Result<Run> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT r.id, r.workflow_id, r.started_at, r.finished_at, r.exit_code, r.stdout, r.stderr, r.result_url, r.status, w.name, r.error_analysis
             FROM runs r LEFT JOIN workflows w ON r.workflow_id = w.id WHERE r.id = ?1",
            params![id],
            |row| Ok(run_from_row(row)),
        )
    }

    pub fn get_run_history(&self, workflow_id: &str, limit: i64) -> rusqlite::Result<Vec<Run>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT r.id, r.workflow_id, r.started_at, r.finished_at, r.exit_code, r.stdout, r.stderr, r.result_url, r.status, w.name, r.error_analysis
             FROM runs r LEFT JOIN workflows w ON r.workflow_id = w.id WHERE r.workflow_id = ?1 ORDER BY r.started_at DESC LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![workflow_id, limit], |row| Ok(run_from_row(row)))?;
        rows.collect()
    }

    pub fn get_recent_runs(&self, limit: i64) -> rusqlite::Result<Vec<Run>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT r.id, r.workflow_id, r.started_at, r.finished_at, r.exit_code, r.stdout, r.stderr, r.result_url, r.status, w.name, r.error_analysis
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

    pub fn get_running_count(&self) -> rusqlite::Result<usize> {
        let conn = self.conn()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM runs WHERE status = 'running'",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    pub fn get_running_runs(&self) -> rusqlite::Result<Vec<Run>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT r.id, r.workflow_id, r.started_at, r.finished_at, r.exit_code, r.stdout, r.stderr, r.result_url, r.status, w.name, r.error_analysis
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
}
