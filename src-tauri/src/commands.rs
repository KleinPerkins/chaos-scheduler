use crate::db::{Database, EmailConfig, NextRun, Run, SchedulerStatus, Workflow};
use crate::scheduler::{self, WorkflowScheduler};
use std::sync::{Arc, Mutex};
use tauri::State;

pub struct AppState {
    pub db: Arc<Database>,
    pub scheduler: Arc<Mutex<WorkflowScheduler>>,
    pub chaos_labs_root: String,
    pub python_path: String,
}

#[tauri::command]
pub fn get_app_config(state: State<AppState>) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "chaos_labs_root": state.chaos_labs_root,
        "python_path": state.python_path,
    }))
}

#[tauri::command]
pub fn list_workflows(state: State<AppState>) -> Result<Vec<Workflow>, String> {
    state.db.list_workflows().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_workflow(state: State<AppState>, id: String) -> Result<Workflow, String> {
    state.db.get_workflow(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_workflow(
    state: State<AppState>,
    name: String,
    description: Option<String>,
    script_path: String,
    cron_schedule: String,
    async_mode: Option<bool>,
    email_on_failure: Option<bool>,
    timezone: Option<String>,
    corpus: Option<String>,
) -> Result<Workflow, String> {
    state
        .db
        .create_workflow(
            &name,
            description.as_deref(),
            &script_path,
            &cron_schedule,
            async_mode.unwrap_or(false),
            email_on_failure.unwrap_or(true),
            timezone.as_deref().unwrap_or("UTC"),
            corpus.as_deref().unwrap_or("instance"),
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_workflow(
    state: State<AppState>,
    id: String,
    name: String,
    description: Option<String>,
    script_path: String,
    cron_schedule: String,
    enabled: bool,
    async_mode: Option<bool>,
    email_on_failure: Option<bool>,
    timezone: Option<String>,
    corpus: Option<String>,
) -> Result<Workflow, String> {
    let existing = state.db.get_workflow(&id).map_err(|e| e.to_string())?;
    state
        .db
        .update_workflow(
            &id,
            &name,
            description.as_deref(),
            &script_path,
            &cron_schedule,
            enabled,
            async_mode.unwrap_or(false),
            email_on_failure.unwrap_or(true),
            timezone.as_deref().unwrap_or("UTC"),
            corpus.as_deref().unwrap_or(&existing.corpus),
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_workflow(state: State<AppState>, id: String) -> Result<(), String> {
    state.db.delete_workflow(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn trigger_workflow(state: State<AppState>, id: String) -> Result<String, String> {
    let result = scheduler::execute_workflow(
        &state.db,
        &state.chaos_labs_root,
        &state.python_path,
        &id,
        true,
        true,
        false,
    )?;
    Ok(result.run_id)
}

#[tauri::command]
pub fn get_run_history(
    state: State<AppState>,
    workflow_id: String,
    limit: Option<i64>,
) -> Result<Vec<Run>, String> {
    state
        .db
        .get_run_history(&workflow_id, limit.unwrap_or(20))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_run_log(state: State<AppState>, run_id: String) -> Result<Run, String> {
    state.db.get_run(&run_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_scheduler_status(state: State<AppState>) -> Result<SchedulerStatus, String> {
    let workflows = state.db.list_workflows().map_err(|e| e.to_string())?;

    let active_workflows = workflows.iter().filter(|w| w.enabled).count();
    let running_count = state.db.get_running_count().map_err(|e| e.to_string())?;

    let next_runs: Vec<NextRun> = workflows
        .iter()
        .filter(|w| w.enabled)
        .filter_map(|w| {
            scheduler::get_next_run_time(&w.cron_schedule, &w.timezone).map(|t| NextRun {
                workflow_id: w.id.clone(),
                workflow_name: w.name.clone(),
                corpus: w.corpus.clone(),
                next_time: t,
            })
        })
        .collect();

    let recent_runs = state.db.get_recent_runs(10).map_err(|e| e.to_string())?;

    Ok(SchedulerStatus {
        active_workflows,
        running_count,
        next_runs,
        recent_runs,
    })
}

#[tauri::command]
pub fn open_dashboard(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;
    if let Some(main) = app.get_webview_window("main") {
        main.show().map_err(|e| e.to_string())?;
        main.set_focus().map_err(|e| e.to_string())?;
    }
    if let Some(popup) = app.get_webview_window("popup") {
        let _ = popup.hide();
    }
    Ok(())
}

#[tauri::command]
pub fn open_run_detail(
    app: tauri::AppHandle,
    run_id: String,
    workflow_id: String,
) -> Result<(), String> {
    use tauri::{Emitter, Manager};
    if let Some(main) = app.get_webview_window("main") {
        main.show().map_err(|e| e.to_string())?;
        main.set_focus().map_err(|e| e.to_string())?;
        main.emit(
            "navigate-to-run",
            serde_json::json!({
                "runId": run_id,
                "workflowId": workflow_id,
            }),
        )
        .map_err(|e| e.to_string())?;
    }
    if let Some(popup) = app.get_webview_window("popup") {
        let _ = popup.hide();
    }
    Ok(())
}

#[tauri::command]
pub fn hide_popup(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;
    if let Some(popup) = app.get_webview_window("popup") {
        popup.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    std::process::Command::new("open")
        .arg(&url)
        .spawn()
        .map_err(|e| format!("Failed to open URL: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn quit_app(app: tauri::AppHandle) -> Result<(), String> {
    crate::scheduler::SHUTDOWN.store(true, std::sync::atomic::Ordering::Relaxed);
    app.exit(0);
    Ok(())
}

#[tauri::command]
pub fn get_launch_at_login() -> Result<bool, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    let plist_path = format!(
        "{}/Library/LaunchAgents/com.chaoslabs.scheduler.plist",
        home
    );
    Ok(std::path::Path::new(&plist_path).exists())
}

#[tauri::command]
pub fn set_launch_at_login(enabled: bool) -> Result<String, String> {
    if enabled {
        let exe = std::env::current_exe()
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .to_string();
        scheduler::install_launchd_plist(&exe)
    } else {
        scheduler::uninstall_launchd_plist()?;
        Ok("Removed".to_string())
    }
}

#[tauri::command]
pub fn list_available_scripts(state: State<AppState>) -> Result<Vec<AvailableScript>, String> {
    let root = &state.chaos_labs_root;
    let workflows_dir = std::path::Path::new(root).join("scripts").join("workflows");

    let mut scripts = Vec::new();

    if workflows_dir.exists() {
        collect_scripts(&workflows_dir, &root, &mut scripts)?;
    }

    scripts.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(scripts)
}

fn collect_scripts(
    dir: &std::path::Path,
    root: &str,
    scripts: &mut Vec<AvailableScript>,
) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| e.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_scripts(&path, root, scripts)?;
        } else if path.extension().is_some_and(|ext| ext == "py") {
            let filename = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if filename.starts_with('_') || filename == "__init__.py" {
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| path.to_string_lossy().to_string());

            let description = read_script_docstring(&path);
            let name = filename
                .trim_end_matches(".py")
                .replace('_', " ")
                .split_whitespace()
                .map(|w| {
                    let mut c = w.chars();
                    match c.next() {
                        None => String::new(),
                        Some(f) => f.to_uppercase().to_string() + c.as_str(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");

            scripts.push(AvailableScript {
                name,
                path: relative,
                description,
            });
        }
    }
    Ok(())
}

fn read_script_docstring(path: &std::path::Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut lines = content.lines();
    for line in &mut lines {
        let trimmed = line.trim();
        if trimmed.starts_with("\"\"\"") || trimmed.starts_with("'''") {
            let delim = &trimmed[..3];
            let after = trimmed[3..].trim();
            if after.ends_with(delim) {
                return Some(after[..after.len() - 3].trim().to_string());
            }
            let mut doc = if after.is_empty() {
                String::new()
            } else {
                after.to_string()
            };
            for next_line in lines {
                if next_line.contains(delim) {
                    break;
                }
                if !doc.is_empty() {
                    doc.push(' ');
                }
                doc.push_str(next_line.trim());
            }
            return if doc.is_empty() { None } else { Some(doc) };
        }
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        break;
    }
    None
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AvailableScript {
    pub name: String,
    pub path: String,
    pub description: Option<String>,
}

#[tauri::command]
pub fn set_notification_prefs(
    state: State<AppState>,
    notify_on_failure: bool,
    notify_on_success: bool,
) -> Result<(), String> {
    let scheduler = state.scheduler.lock().map_err(|e| e.to_string())?;
    scheduler.set_notify_on_failure(notify_on_failure);
    scheduler.set_notify_on_success(notify_on_success);
    Ok(())
}

#[tauri::command]
pub fn analyze_run_error(
    state: State<AppState>,
    run_id: String,
) -> Result<serde_json::Value, String> {
    let run = state.db.get_run(&run_id).map_err(|e| e.to_string())?;
    let workflow = state
        .db
        .get_workflow(&run.workflow_id)
        .map_err(|e| e.to_string())?;

    if let Some(existing) = &run.error_analysis {
        return Ok(existing.clone());
    }

    let root = &state.chaos_labs_root;
    let python_path = &state.python_path;
    let script_path = format!("{}/scripts/analyze_error.py", root);

    let context = serde_json::json!({
        "workflow_name": workflow.name,
        "script_path": workflow.script_path,
        "exit_code": run.exit_code,
        "stderr": run.stderr,
        "stdout": run.stdout,
    });

    let output = std::process::Command::new(&python_path)
        .arg(&script_path)
        .current_dir(root)
        .env("CHAOS_LABS_ROOT", root)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                let _ = stdin.write_all(context.to_string().as_bytes());
            }
            child.wait_with_output()
        })
        .map_err(|e| format!("Failed to run analysis: {}", e))?;

    let result_str = String::from_utf8_lossy(&output.stdout);
    let analysis: serde_json::Value = serde_json::from_str(result_str.trim())
        .map_err(|e| format!("Failed to parse analysis: {} — raw: {}", e, result_str))?;

    let _ = state.db.set_error_analysis(&run_id, &analysis.to_string());

    Ok(analysis)
}

#[tauri::command]
pub fn generate_workflow_description(
    state: State<AppState>,
    script_path: String,
) -> Result<String, String> {
    let root = &state.chaos_labs_root;
    let python_path = &state.python_path;
    let analysis_script = format!("{}/scripts/analyze_workflow.py", root);

    if !std::path::Path::new(&analysis_script).exists() {
        return Err("analyze_workflow.py not found — run deploy.py to sync scripts".to_string());
    }

    let context = serde_json::json!({
        "script_path": script_path,
        "chaos_labs_root": root,
    });

    let output = std::process::Command::new(&python_path)
        .arg(&analysis_script)
        .current_dir(root)
        .env("CHAOS_LABS_ROOT", root)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                let _ = stdin.write_all(context.to_string().as_bytes());
            }
            child.wait_with_output()
        })
        .map_err(|e| format!("Failed to run analysis: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Analysis script failed: {}", stderr));
    }

    let result_str = String::from_utf8_lossy(&output.stdout);
    let result: serde_json::Value = serde_json::from_str(result_str.trim())
        .map_err(|e| format!("Failed to parse result: {} — raw: {}", e, result_str))?;

    result["description"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "No description in AI response".to_string())
}

#[tauri::command]
pub fn get_email_config(state: State<AppState>) -> Result<EmailConfig, String> {
    let mut config = state.db.get_email_config().map_err(|e| e.to_string())?;
    if !config.smtp_password.is_empty() {
        config.smtp_password = "••••••••".to_string();
    }
    Ok(config)
}

#[tauri::command]
pub fn set_email_config(
    state: State<AppState>,
    enabled: bool,
    alert_email: String,
    smtp_host: String,
    smtp_port: i32,
    smtp_user: String,
    smtp_password: String,
    from_address: String,
    from_name: String,
) -> Result<(), String> {
    let password = if smtp_password == "••••••••" {
        state
            .db
            .get_email_config()
            .map(|c| c.smtp_password)
            .unwrap_or_default()
    } else {
        smtp_password
    };

    let config = EmailConfig {
        enabled,
        alert_email,
        smtp_host,
        smtp_port,
        smtp_user,
        smtp_password: password,
        from_address,
        from_name,
    };
    state
        .db
        .set_email_config(&config)
        .map_err(|e| e.to_string())?;

    if let Ok(sched) = state.scheduler.lock() {
        sched.refresh_email_config();
    }
    Ok(())
}

#[tauri::command]
pub fn test_email_config(state: State<AppState>) -> Result<serde_json::Value, String> {
    let config = state.db.get_email_config().map_err(|e| e.to_string())?;
    if !config.enabled {
        return Err("Email alerts are not enabled".to_string());
    }
    if config.alert_email.is_empty() || config.smtp_host.is_empty() {
        return Err("Email configuration is incomplete".to_string());
    }

    run_email_script(&state.chaos_labs_root, &config, None, "test")
}

/// Invoke the Python email script with the given context.
/// Used by both test_email_config and send_failure_email.
pub fn run_email_script(
    chaos_labs_root: &str,
    config: &EmailConfig,
    run_context: Option<&serde_json::Value>,
    mode: &str,
) -> Result<serde_json::Value, String> {
    let python_path = format!("{}/.venv/bin/python3", chaos_labs_root);
    let script_path = format!("{}/scripts/email_alert.py", chaos_labs_root);

    if !std::path::Path::new(&script_path).exists() {
        return Err("email_alert.py not found — run deploy.py to sync scripts".to_string());
    }

    let mut context = run_context
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    if let Some(obj) = context.as_object_mut() {
        obj.insert("mode".to_string(), serde_json::json!(mode));
        obj.insert("smtp_host".to_string(), serde_json::json!(config.smtp_host));
        obj.insert("smtp_port".to_string(), serde_json::json!(config.smtp_port));
        obj.insert("smtp_user".to_string(), serde_json::json!(config.smtp_user));
        obj.insert(
            "smtp_password".to_string(),
            serde_json::json!(config.smtp_password),
        );
        obj.insert(
            "from_address".to_string(),
            serde_json::json!(config.from_address),
        );
        obj.insert("from_name".to_string(), serde_json::json!(config.from_name));
        obj.insert(
            "to_address".to_string(),
            serde_json::json!(config.alert_email),
        );
    }

    let output = std::process::Command::new(&python_path)
        .arg(&script_path)
        .current_dir(chaos_labs_root)
        .env("CHAOS_LABS_ROOT", chaos_labs_root)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                let _ = stdin.write_all(context.to_string().as_bytes());
            }
            child.wait_with_output()
        })
        .map_err(|e| format!("Failed to run email script: {}", e))?;

    let result_str = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(result_str.trim()).map_err(|e| {
        format!(
            "Failed to parse email script output: {} — raw: {}",
            e, result_str
        )
    })
}
