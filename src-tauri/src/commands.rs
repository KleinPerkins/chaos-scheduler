use crate::db::{
    Database, EmailConfig, MissionControlNeedsAttentionItem, MissionControlPanelAvailability,
    MissionControlPreferences, MissionControlSnapshot, MissionControlUpcomingRun, NextRun,
    QueueInfo, QueuedRun, RetentionPreview, Run, RunAttempt, RunMetric, RunRelationship, RunTask,
    SchedulerAsset, SchedulerDeadLetter, SchedulerStatus, SlaViolation, Workflow,
    WorkflowHistoryBucket, WorkflowResourceSample, WorkflowTokenUsageRollup,
};
use crate::scheduler::{self, WorkflowScheduler};
use chrono::{DateTime, Datelike, Duration, Timelike, Utc};
use serde::Serialize;
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};
use tauri::State;

pub struct AppState {
    pub db: Arc<Database>,
    pub scheduler: Arc<Mutex<WorkflowScheduler>>,
    pub chaos_labs_root: String,
    pub python_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackfillPlan {
    pub workflow_id: String,
    pub trigger_kind: String,
    pub chain_suppressed: bool,
    pub logical_dates: Vec<String>,
    pub count: usize,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackfillDispatchResult {
    pub plan: BackfillPlan,
    pub outcomes: Vec<scheduler::DispatchOutcome>,
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

fn normalized_opt(value: Option<&str>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn source_definition_changed(
    existing: &Workflow,
    name: &str,
    description: Option<&str>,
    script_path: &str,
    cron_schedule: &str,
    async_mode: bool,
    corpus: &str,
    domain: Option<&str>,
    trigger_config: Option<&str>,
    queue_config: Option<&str>,
) -> bool {
    existing.name != name
        || existing.description != normalized_opt(description)
        || existing.script_path != script_path
        || existing.cron_schedule != cron_schedule
        || existing.async_mode != async_mode
        || existing.corpus != corpus
        || existing.domain != normalized_opt(domain)
        || existing.trigger_config != normalized_opt(trigger_config)
        || existing.queue_config != normalized_opt(queue_config)
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
    domain: Option<String>,
    trigger_config: Option<String>,
    queue_config: Option<String>,
) -> Result<Workflow, String> {
    if corpus.as_deref().unwrap_or("instance") == "source" {
        return Err("source-corpus workflow definitions are source-controlled; create instance workflows from the Scheduler UI".to_string());
    }
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
            domain.as_deref(),
            trigger_config.as_deref(),
            queue_config.as_deref(),
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
    domain: Option<String>,
    trigger_config: Option<String>,
    queue_config: Option<String>,
) -> Result<Workflow, String> {
    let existing = state.db.get_workflow(&id).map_err(|e| e.to_string())?;
    let next_corpus = corpus.as_deref().unwrap_or(&existing.corpus);
    let next_domain = domain.as_deref().or(existing.domain.as_deref());
    let next_trigger_config = trigger_config
        .as_deref()
        .or(existing.trigger_config.as_deref());
    let next_queue_config = queue_config.as_deref().or(existing.queue_config.as_deref());
    let next_async_mode = async_mode.unwrap_or(existing.async_mode);
    if existing.corpus == "source"
        && source_definition_changed(
            &existing,
            &name,
            description.as_deref(),
            &script_path,
            &cron_schedule,
            next_async_mode,
            next_corpus,
            next_domain,
            next_trigger_config,
            next_queue_config,
        )
    {
        return Err("source-corpus workflow definitions are source-controlled; only enabled, email, and timezone runtime preferences are editable in the Scheduler UI".to_string());
    }
    state
        .db
        .update_workflow(
            &id,
            &name,
            description.as_deref(),
            &script_path,
            &cron_schedule,
            enabled,
            next_async_mode,
            email_on_failure.unwrap_or(true),
            timezone.as_deref().unwrap_or("UTC"),
            next_corpus,
            next_domain,
            next_trigger_config,
            next_queue_config,
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_workflow(state: State<AppState>, id: String) -> Result<(), String> {
    let existing = state.db.get_workflow(&id).map_err(|e| e.to_string())?;
    if existing.corpus == "source" {
        return Err("source-corpus workflows are source-controlled; remove them from the product registry instead of deleting from the Scheduler UI".to_string());
    }
    state.db.delete_workflow(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn trigger_workflow(state: State<AppState>, id: String) -> Result<String, String> {
    let result = scheduler::execute_workflow_with_context(
        &state.db,
        &state.chaos_labs_root,
        &state.python_path,
        &id,
        true,
        true,
        false,
        Some("manual"),
        None,
        None,
        None,
        None,
        None,
        None,
    )?;
    if result.completed {
        scheduler::trigger_on_completion(
            &state.db,
            &state.chaos_labs_root,
            &state.python_path,
            &id,
            &result.run_id,
            result.success,
            true,
            true,
            false,
        );
    }
    Ok(result.run_id)
}

#[tauri::command]
pub fn rerun_workflow(
    state: State<AppState>,
    workflow_id: String,
    source_run_id: Option<String>,
    input_override_json: Option<String>,
) -> Result<String, String> {
    if let Some(input) = &input_override_json {
        serde_json::from_str::<serde_json::Value>(input)
            .map_err(|e| format!("Invalid input override JSON: {}", e))?;
    }
    let payload = serde_json::json!({
        "source_run_id": source_run_id,
        "input_override": input_override_json.as_ref().and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok()),
    })
    .to_string();
    let result = scheduler::execute_workflow_with_context(
        &state.db,
        &state.chaos_labs_root,
        &state.python_path,
        &workflow_id,
        true,
        true,
        false,
        Some("manual"),
        Some(&payload),
        None,
        input_override_json.as_deref(),
        source_run_id.as_deref(),
        None,
        None,
    )?;
    if result.completed {
        scheduler::trigger_on_completion(
            &state.db,
            &state.chaos_labs_root,
            &state.python_path,
            &workflow_id,
            &result.run_id,
            result.success,
            true,
            true,
            false,
        );
    }
    Ok(result.run_id)
}

fn parse_backfill_dt(value: &str) -> Result<DateTime<Utc>, String> {
    let parsed = DateTime::parse_from_rfc3339(value)
        .map_err(|e| format!("Invalid RFC3339 datetime {value:?}: {e}"))?;
    Ok(parsed.with_timezone(&Utc))
}

fn field_matches(field: &str, value: u32) -> bool {
    if field == "*" {
        return true;
    }
    field
        .split(',')
        .any(|part| part.parse::<u32>().ok() == Some(value))
}

fn cron_matches(expr: &str, instant: DateTime<Utc>) -> bool {
    let mut fields: Vec<&str> = expr.split_whitespace().collect();
    if fields.len() == 6 {
        fields.remove(0);
    }
    if fields.len() != 5 {
        return false;
    }
    let weekday = instant.weekday().num_days_from_sunday();
    field_matches(fields[0], instant.minute())
        && field_matches(fields[1], instant.hour())
        && field_matches(fields[2], instant.day())
        && field_matches(fields[3], instant.month())
        && field_matches(fields[4], weekday)
}

fn cron_entries_for_workflow(workflow: &Workflow) -> Vec<String> {
    let mut entries = vec![];
    if let Some(raw) = workflow.trigger_config.as_deref() {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(raw) {
            let triggers = parsed
                .get("triggers")
                .and_then(serde_json::Value::as_array)
                .cloned()
                .or_else(|| parsed.as_array().cloned())
                .unwrap_or_default();
            for trigger in triggers {
                if trigger.get("kind").and_then(serde_json::Value::as_str) == Some("cron") {
                    if let Some(cron) = trigger.get("cron").and_then(serde_json::Value::as_str) {
                        entries.push(cron.to_string());
                    }
                }
            }
        }
    }
    if entries.is_empty() {
        entries.extend(
            workflow
                .cron_schedule
                .split(';')
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .map(str::to_string),
        );
    }
    entries
}

fn build_backfill_plan(
    db: &Arc<Database>,
    workflow_id: &str,
    since: &str,
    until: &str,
    max_runs: Option<i64>,
    dry_run: bool,
) -> Result<BackfillPlan, String> {
    let workflow = db.get_workflow(workflow_id).map_err(|e| e.to_string())?;
    let since = parse_backfill_dt(since)?.with_second(0).unwrap();
    let until = parse_backfill_dt(until)?.with_second(0).unwrap();
    if until < since {
        return Err("Backfill until must be >= since".to_string());
    }
    let max_runs = max_runs.unwrap_or(i64::MAX).max(0) as usize;
    let cron_entries = cron_entries_for_workflow(&workflow);
    let mut logical_dates = vec![];
    let mut cursor = since;
    while cursor <= until && logical_dates.len() < max_runs {
        if cron_entries.iter().any(|expr| cron_matches(expr, cursor)) {
            logical_dates.push(cursor.to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
        }
        cursor += Duration::minutes(1);
    }
    Ok(BackfillPlan {
        workflow_id: workflow_id.to_string(),
        trigger_kind: "backfill".to_string(),
        chain_suppressed: true,
        count: logical_dates.len(),
        logical_dates,
        dry_run,
    })
}

#[tauri::command]
pub fn plan_backfill(
    state: State<AppState>,
    workflow_id: String,
    since: String,
    until: String,
    max_runs: Option<i64>,
) -> Result<BackfillPlan, String> {
    build_backfill_plan(&state.db, &workflow_id, &since, &until, max_runs, true)
}

#[tauri::command]
pub fn dispatch_backfill(
    state: State<AppState>,
    workflow_id: String,
    since: String,
    until: String,
    max_runs: Option<i64>,
    dry_run: Option<bool>,
) -> Result<BackfillDispatchResult, String> {
    let dry_run = dry_run.unwrap_or(false);
    let plan = build_backfill_plan(&state.db, &workflow_id, &since, &until, max_runs, dry_run)?;
    if dry_run {
        return Ok(BackfillDispatchResult {
            plan,
            outcomes: vec![],
        });
    }
    let mut outcomes = vec![];
    for logical_date in &plan.logical_dates {
        let payload = serde_json::json!({
            "logical_date": logical_date,
            "chain_suppressed": true,
        })
        .to_string();
        let input = serde_json::json!({
            "backfill": {
                "logical_date": logical_date,
            }
        })
        .to_string();
        outcomes.push(scheduler::dispatch_non_cron_workflow(
            &state.db,
            &state.chaos_labs_root,
            &state.python_path,
            &workflow_id,
            scheduler::NonCronDispatchOptions {
                notify_on_success: false,
                notify_on_failure: true,
                email_on_failure_enabled: false,
                trigger_kind: "backfill",
                trigger_payload: Some(&payload),
                upstream_run_id: None,
                input_json: Some(&input),
                rerun_of_run_id: None,
                suppress_completion_triggers: true,
                dedupe: true,
                app_handle: None,
            },
        )?);
    }
    Ok(BackfillDispatchResult { plan, outcomes })
}

#[tauri::command]
pub fn list_dead_letters(
    state: State<AppState>,
    include_acknowledged: Option<bool>,
    limit: Option<i64>,
) -> Result<Vec<SchedulerDeadLetter>, String> {
    state
        .db
        .list_scheduler_dead_letters(include_acknowledged.unwrap_or(false), limit.unwrap_or(50))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_dead_letter(state: State<AppState>, id: String) -> Result<SchedulerDeadLetter, String> {
    state
        .db
        .get_scheduler_dead_letter(&id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn acknowledge_dead_letter(
    state: State<AppState>,
    id: String,
    reason: String,
    operator: Option<String>,
    reenable_workflow: Option<bool>,
) -> Result<SchedulerDeadLetter, String> {
    if reason.trim().is_empty() {
        return Err("Acknowledgement reason is required".to_string());
    }
    let before = state
        .db
        .get_scheduler_dead_letter(&id)
        .map_err(|e| e.to_string())?;
    let updated = state
        .db
        .acknowledge_scheduler_dead_letter(&id, reason.trim(), operator.as_deref())
        .map_err(|e| e.to_string())?;
    if updated == 0 {
        return Err("Dead-letter row is already acknowledged or missing".to_string());
    }
    if reenable_workflow.unwrap_or(false) {
        state
            .db
            .set_workflow_enabled(&before.workflow_id, true)
            .map_err(|e| e.to_string())?;
    }
    state
        .db
        .get_scheduler_dead_letter(&id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn recover_dead_letter(
    state: State<AppState>,
    id: String,
    reenable_workflow: Option<bool>,
) -> Result<scheduler::DispatchOutcome, String> {
    let dead_letter = state
        .db
        .get_scheduler_dead_letter(&id)
        .map_err(|e| e.to_string())?;
    if reenable_workflow.unwrap_or(false) {
        state
            .db
            .set_workflow_enabled(&dead_letter.workflow_id, true)
            .map_err(|e| e.to_string())?;
    }
    let payload = serde_json::json!({
        "dead_letter_id": dead_letter.id,
        "source_run_id": dead_letter.run_id,
        "task_id": dead_letter.task_id,
    })
    .to_string();
    let outcome = scheduler::dispatch_non_cron_workflow(
        &state.db,
        &state.chaos_labs_root,
        &state.python_path,
        &dead_letter.workflow_id,
        scheduler::NonCronDispatchOptions {
            notify_on_success: false,
            notify_on_failure: true,
            email_on_failure_enabled: false,
            trigger_kind: "dead_letter_recovery",
            trigger_payload: Some(&payload),
            upstream_run_id: Some(&dead_letter.run_id),
            input_json: None,
            rerun_of_run_id: Some(&dead_letter.run_id),
            suppress_completion_triggers: true,
            dedupe: false,
            app_handle: None,
        },
    )?;
    if let Some(run_id) = outcome.run_id.as_deref() {
        let _ = state.db.link_dead_letter_recovery(&id, run_id);
    }
    Ok(outcome)
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
pub fn get_global_run_history(
    state: State<AppState>,
    status_filter: Option<String>,
    trigger_kind: Option<String>,
    corpus_filter: Option<String>,
    domain_filter: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<Run>, String> {
    state
        .db
        .get_global_run_history(
            status_filter.as_deref(),
            trigger_kind.as_deref(),
            corpus_filter.as_deref(),
            domain_filter.as_deref(),
            limit.unwrap_or(100),
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cleanup_retention(
    state: State<AppState>,
    older_than_days: i64,
    dry_run: bool,
) -> Result<RetentionPreview, String> {
    state
        .db
        .cleanup_retention(older_than_days, dry_run)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_run_log(state: State<AppState>, run_id: String) -> Result<Run, String> {
    state.db.get_run(&run_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_run_tasks(state: State<AppState>, run_id: String) -> Result<Vec<RunTask>, String> {
    state.db.get_run_tasks(&run_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_run_attempts(state: State<AppState>, run_id: String) -> Result<Vec<RunAttempt>, String> {
    state
        .db
        .get_run_attempts(&run_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_run_metrics(state: State<AppState>, run_id: String) -> Result<Vec<RunMetric>, String> {
    state.db.get_run_metrics(&run_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_run_relationships(
    state: State<AppState>,
    run_id: String,
) -> Result<Vec<RunRelationship>, String> {
    state
        .db
        .list_run_relationships(&run_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_workflow_history_buckets(
    state: State<AppState>,
    workflow_id: String,
    days: Option<i64>,
) -> Result<Vec<WorkflowHistoryBucket>, String> {
    state
        .db
        .workflow_history_buckets(&workflow_id, days.unwrap_or(30))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_sla_violations(state: State<AppState>) -> Result<Vec<SlaViolation>, String> {
    state
        .db
        .evaluate_sla_violations()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn query_resource_samples(
    state: State<AppState>,
    workflow_id: String,
    time_window: Option<String>,
) -> Result<Vec<WorkflowResourceSample>, String> {
    let window = time_window_modifier(time_window.as_deref())?;
    state
        .db
        .query_workflow_resource_samples(&workflow_id, &window)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn query_token_usage_rollup(
    state: State<AppState>,
    group_by: Option<Vec<String>>,
    time_window: Option<String>,
    time_bucket: Option<String>,
) -> Result<Vec<WorkflowTokenUsageRollup>, String> {
    let window = time_window_modifier(time_window.as_deref())?;
    let group_by = group_by.unwrap_or_else(|| {
        vec![
            "time_bucket".to_string(),
            "workflow_id".to_string(),
            "corpus".to_string(),
            "domain".to_string(),
            "queue_name".to_string(),
            "provider".to_string(),
            "model".to_string(),
            "token_kind".to_string(),
        ]
    });
    let bucket = normalize_time_bucket(time_bucket.as_deref())?;
    state
        .db
        .query_token_usage_rollup(&group_by, &window, &bucket)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn query_stale_assets(
    state: State<AppState>,
    max_age_seconds: Option<i64>,
    asset_kind: Option<String>,
) -> Result<Vec<SchedulerAsset>, String> {
    let max_age_seconds = max_age_seconds.unwrap_or(24 * 60 * 60);
    if max_age_seconds < 0 {
        return Err("max_age_seconds must be non-negative".to_string());
    }
    state
        .db
        .query_stale_assets(max_age_seconds, asset_kind.as_deref())
        .map_err(|e| e.to_string())
}

fn time_window_modifier(value: Option<&str>) -> Result<String, String> {
    let raw = value.unwrap_or("24h").trim().to_ascii_lowercase();
    if raw == "all" {
        return Ok("-100 years".to_string());
    }
    let (number, unit) = split_window(&raw).ok_or_else(|| {
        "time_window must be all, <number>h, <number>d, <number>m, or '<number> hours/days/minutes'".to_string()
    })?;
    let count: i64 = number
        .parse()
        .map_err(|_| "time_window count must be a positive integer".to_string())?;
    if count <= 0 {
        return Err("time_window count must be positive".to_string());
    }
    let sqlite_unit = match unit {
        "m" | "min" | "minute" | "minutes" => "minutes",
        "h" | "hr" | "hour" | "hours" => "hours",
        "d" | "day" | "days" => "days",
        _ => return Err(format!("Unsupported time_window unit: {}", unit)),
    };
    Ok(format!("-{} {}", count, sqlite_unit))
}

fn split_window(value: &str) -> Option<(&str, &str)> {
    if let Some((n, unit)) = value.split_once(' ') {
        return Some((n.trim(), unit.trim()));
    }
    let digit_count = value.chars().take_while(|c| c.is_ascii_digit()).count();
    if digit_count == 0 || digit_count == value.len() {
        return None;
    }
    Some((&value[..digit_count], &value[digit_count..]))
}

fn normalize_time_bucket(value: Option<&str>) -> Result<String, String> {
    let bucket = value.unwrap_or("hour").trim().to_ascii_lowercase();
    match bucket.as_str() {
        "minute" | "hour" | "day" => Ok(bucket),
        other => Err(format!("Unsupported time_bucket: {}", other)),
    }
}

fn normalize_mission_corpus_filter(value: Option<String>, default: &str) -> String {
    match value
        .as_deref()
        .unwrap_or(default)
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "source" => "source".to_string(),
        "instance" => "instance".to_string(),
        _ => "all".to_string(),
    }
}

fn normalize_mission_domain_filter(value: Option<String>, default: &str) -> String {
    let raw = value.unwrap_or_else(|| default.to_string());
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("all") {
        "all".to_string()
    } else if trimmed.eq_ignore_ascii_case("unowned") || trimmed.eq_ignore_ascii_case("__unowned__")
    {
        "__unowned__".to_string()
    } else {
        trimmed.to_string()
    }
}

fn workflow_owner(workflow: &Workflow) -> String {
    workflow
        .domain
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Unowned")
        .to_string()
}

fn mission_attention_severity_rank(severity: &str) -> i32 {
    match severity {
        "critical" => 0,
        "error" => 1,
        "warning" => 2,
        _ => 3,
    }
}

fn cron_triggers_for_workflow(workflow: &Workflow) -> Vec<(String, String)> {
    let legacy = || vec![("legacy_cron".to_string(), workflow.cron_schedule.clone())];
    let Some(raw) = workflow
        .trigger_config
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return legacy();
    };
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(raw) else {
        return legacy();
    };
    let triggers = parsed
        .as_array()
        .cloned()
        .or_else(|| parsed.get("triggers").and_then(|v| v.as_array()).cloned())
        .unwrap_or_default();
    if triggers.is_empty() {
        return legacy();
    }
    triggers
        .into_iter()
        .filter(|trigger| trigger.get("kind").and_then(|v| v.as_str()) == Some("cron"))
        .filter_map(|trigger| {
            trigger
                .get("cron")
                .and_then(|v| v.as_str())
                .map(|cron| ("cron".to_string(), cron.to_string()))
        })
        .collect()
}

fn mission_control_availability() -> Vec<MissionControlPanelAvailability> {
    vec![
        MissionControlPanelAvailability {
            panel: "Header status".to_string(),
            source_tables: vec![
                "workflows".to_string(),
                "runs".to_string(),
                "queued_runs".to_string(),
            ],
            command: "get_mission_control_snapshot".to_string(),
            filter_behavior: "corpus/domain filters apply before counting".to_string(),
            empty_state: "No workflows match the current filters".to_string(),
            degraded_state: "Counts omit unavailable scheduler.db rows".to_string(),
            click_through_target: Some("Dashboard workflows".to_string()),
            persistence_required: false,
        },
        MissionControlPanelAvailability {
            panel: "SLA strip".to_string(),
            source_tables: vec![
                "runs".to_string(),
                "queued_runs".to_string(),
                "workflows.queue_config".to_string(),
            ],
            command: "get_mission_control_snapshot".to_string(),
            filter_behavior: "success-rate and queue waits are computed after workflow filters"
                .to_string(),
            empty_state: "No recent terminal runs yet".to_string(),
            degraded_state: "SLA metrics show partial state when no wait samples exist".to_string(),
            click_through_target: Some("Run history".to_string()),
            persistence_required: false,
        },
        MissionControlPanelAvailability {
            panel: "Needs Attention".to_string(),
            source_tables: vec![
                "runs".to_string(),
                "queued_runs".to_string(),
                "workflows.queue_config".to_string(),
            ],
            command: "get_mission_control_snapshot".to_string(),
            filter_behavior: "items are generated only from filtered durable rows".to_string(),
            empty_state: "No persisted issues need attention".to_string(),
            degraded_state:
                "Dependency wait reasons only appear when persisted queue/run state exists"
                    .to_string(),
            click_through_target: Some("Run detail or Queue".to_string()),
            persistence_required: false,
        },
        MissionControlPanelAvailability {
            panel: "Live Activity".to_string(),
            source_tables: vec!["runs".to_string(), "workflows".to_string()],
            command: "get_mission_control_snapshot".to_string(),
            filter_behavior: "filtered before ordering and limit".to_string(),
            empty_state: "No run activity for this filter".to_string(),
            degraded_state: "Only persisted run transitions are shown".to_string(),
            click_through_target: Some("Run detail".to_string()),
            persistence_required: false,
        },
        MissionControlPanelAvailability {
            panel: "Upcoming Runs".to_string(),
            source_tables: vec![
                "workflows".to_string(),
                "workflows.trigger_config".to_string(),
            ],
            command: "get_mission_control_snapshot".to_string(),
            filter_behavior: "enabled workflows are filtered before cron expansion".to_string(),
            empty_state: "No fixed-time cron triggers match this filter".to_string(),
            degraded_state:
                "Event-driven triggers have no ETA until durable readiness state exists".to_string(),
            click_through_target: Some("Dashboard workflows".to_string()),
            persistence_required: false,
        },
        MissionControlPanelAvailability {
            panel: "SLA & Freshness ledger".to_string(),
            source_tables: vec![
                "scheduler_assets".to_string(),
                "runs".to_string(),
                "workflows".to_string(),
            ],
            command: "get_mission_control_snapshot".to_string(),
            filter_behavior:
                "assets join through last_writer_run_id; unattributed assets show only in All"
                    .to_string(),
            empty_state: "No stale assets for this filter".to_string(),
            degraded_state:
                "Unattributed assets are labeled and withheld from corpus/domain filters"
                    .to_string(),
            click_through_target: None,
            persistence_required: false,
        },
        MissionControlPanelAvailability {
            panel: "Per-workflow telemetry".to_string(),
            source_tables: vec![
                "workflow_resource_samples".to_string(),
                "workflow_token_usage".to_string(),
            ],
            command: "get_mission_control_snapshot".to_string(),
            filter_behavior: "one bounded backend batch over visible workflows".to_string(),
            empty_state: "No resource or token samples yet".to_string(),
            degraded_state: "Cards show no-sample state instead of synthetic sparklines"
                .to_string(),
            click_through_target: Some("Run history".to_string()),
            persistence_required: false,
        },
    ]
}

#[tauri::command]
pub fn get_mission_control_preferences(
    state: State<AppState>,
) -> Result<MissionControlPreferences, String> {
    state
        .db
        .get_mission_control_preferences()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_mission_control_preferences(
    state: State<AppState>,
    default_landing: String,
    corpus_filter: String,
    domain_filter: String,
) -> Result<MissionControlPreferences, String> {
    state
        .db
        .set_mission_control_preferences(&default_landing, &corpus_filter, &domain_filter)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_mission_control_snapshot(
    state: State<AppState>,
    corpus_filter: Option<String>,
    domain_filter: Option<String>,
) -> Result<MissionControlSnapshot, String> {
    let preferences = state
        .db
        .get_mission_control_preferences()
        .map_err(|e| e.to_string())?;
    let explicit_domain_filter = domain_filter.is_some();
    let corpus_filter = normalize_mission_corpus_filter(corpus_filter, &preferences.corpus_filter);
    let requested_domain_filter =
        normalize_mission_domain_filter(domain_filter, &preferences.domain_filter);
    let domains = state
        .db
        .mission_control_domains(&corpus_filter)
        .map_err(|e| e.to_string())?;
    let known_domains: HashSet<String> =
        domains.iter().map(|domain| domain.value.clone()).collect();
    let domain_known =
        requested_domain_filter == "all" || known_domains.contains(&requested_domain_filter);
    let domain_filter = if domain_known {
        requested_domain_filter.clone()
    } else {
        if !explicit_domain_filter {
            let _ = state.db.set_mission_control_preferences(
                &preferences.default_landing,
                &corpus_filter,
                "all",
            );
        }
        "all".to_string()
    };

    let workflows = state
        .db
        .list_workflows_filtered(&corpus_filter, &domain_filter)
        .map_err(|e| e.to_string())?;
    let visible_workflow_ids: HashSet<String> = workflows
        .iter()
        .map(|workflow| workflow.id.clone())
        .collect();

    let sla_violations: Vec<SlaViolation> = state
        .db
        .evaluate_sla_violations()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|violation| visible_workflow_ids.contains(&violation.workflow_id))
        .collect();

    let header = state
        .db
        .mission_control_header(&corpus_filter, &domain_filter)
        .map_err(|e| e.to_string())?;
    let sla = state
        .db
        .mission_control_sla_summary(&corpus_filter, &domain_filter, sla_violations.len() as i64)
        .map_err(|e| e.to_string())?;
    let recent_runs = state
        .db
        .mission_control_recent_runs(&corpus_filter, &domain_filter, 12)
        .map_err(|e| e.to_string())?;
    let failed_runs = state
        .db
        .mission_control_failed_runs(&corpus_filter, &domain_filter, 4)
        .map_err(|e| e.to_string())?;
    let failed_run_count = state
        .db
        .mission_control_failed_run_count(&corpus_filter, &domain_filter)
        .map_err(|e| e.to_string())?;
    let live_activity = state
        .db
        .mission_control_live_activity(&corpus_filter, &domain_filter, 10)
        .map_err(|e| e.to_string())?;
    let freshness_ledger = state
        .db
        .mission_control_freshness_ledger(&corpus_filter, &domain_filter, 24 * 60 * 60, 12)
        .map_err(|e| e.to_string())?;
    let workflow_telemetry = state
        .db
        .mission_control_workflow_telemetry(&corpus_filter, &domain_filter, "-24 hours", 12)
        .map_err(|e| e.to_string())?;

    let mut upcoming_runs = workflows
        .iter()
        .filter(|workflow| workflow.enabled)
        .flat_map(|workflow| {
            cron_triggers_for_workflow(workflow).into_iter().filter_map(
                move |(trigger_kind, cron)| {
                    scheduler::get_next_run_time(&cron, &workflow.timezone).map(|next_time| {
                        MissionControlUpcomingRun {
                            workflow_id: workflow.id.clone(),
                            workflow_name: workflow.name.clone(),
                            corpus: workflow.corpus.clone(),
                            domain: workflow_owner(workflow),
                            trigger_kind,
                            trigger_label: cron,
                            next_time,
                        }
                    })
                },
            )
        })
        .collect::<Vec<_>>();
    upcoming_runs.sort_by(|a, b| a.next_time.cmp(&b.next_time));
    upcoming_runs.truncate(12);

    let mut needs_attention = sla_violations
        .into_iter()
        .map(|violation| MissionControlNeedsAttentionItem {
            id: format!("sla:{}:{}", violation.workflow_id, violation.violation_type),
            severity: violation.severity,
            title: format!("SLA risk: {}", violation.workflow_name),
            detail: violation.message,
            workflow_id: Some(violation.workflow_id),
            workflow_name: Some(violation.workflow_name),
            run_id: None,
            target: "history".to_string(),
        })
        .collect::<Vec<_>>();
    needs_attention.extend(
        failed_runs
            .iter()
            .map(|run| MissionControlNeedsAttentionItem {
                id: format!("run:{}", run.id),
                severity: "error".to_string(),
                title: format!(
                    "{} failed",
                    run.workflow_name
                        .as_deref()
                        .unwrap_or(run.workflow_id.as_str())
                ),
                detail: format!("Terminal status: {}", run.status),
                workflow_id: Some(run.workflow_id.clone()),
                workflow_name: run.workflow_name.clone(),
                run_id: Some(run.id.clone()),
                target: "run_detail".to_string(),
            }),
    );
    if sla.blocked_count > 0 {
        needs_attention.push(MissionControlNeedsAttentionItem {
            id: "queued:blocking".to_string(),
            severity: "warning".to_string(),
            title: format!("{} queued runs waiting", sla.blocked_count),
            detail: "Queue-capacity, dependency, or mutex wait reasons are shown when persisted in scheduler.db.".to_string(),
            workflow_id: None,
            workflow_name: None,
            run_id: None,
            target: "queues".to_string(),
        });
    }
    needs_attention.sort_by(|a, b| {
        mission_attention_severity_rank(&a.severity)
            .cmp(&mission_attention_severity_rank(&b.severity))
            .then_with(|| a.title.cmp(&b.title))
            .then_with(|| a.id.cmp(&b.id))
    });
    let hidden_failed_run_count = failed_run_count.saturating_sub(failed_runs.len() as i64);
    let needs_attention_total = needs_attention.len() as i64 + hidden_failed_run_count;
    needs_attention.truncate(8);
    let needs_attention_truncated = needs_attention_total > needs_attention.len() as i64;

    Ok(MissionControlSnapshot {
        preferences: MissionControlPreferences {
            default_landing: preferences.default_landing,
            corpus_filter,
            domain_filter,
        },
        domains,
        header,
        sla,
        needs_attention,
        needs_attention_total,
        needs_attention_truncated,
        live_activity,
        upcoming_runs,
        freshness_ledger,
        recent_runs,
        workflow_telemetry,
        availability: mission_control_availability(),
    })
}

#[tauri::command]
pub fn get_scheduler_status(state: State<AppState>) -> Result<SchedulerStatus, String> {
    let workflows = state.db.list_workflows().map_err(|e| e.to_string())?;

    let active_workflows = workflows.iter().filter(|w| w.enabled).count();
    let running_count = state.db.get_running_count().map_err(|e| e.to_string())?;

    let mut next_runs: Vec<NextRun> = workflows
        .iter()
        .filter(|w| w.enabled)
        .flat_map(|w| {
            cron_triggers_for_workflow(w)
                .into_iter()
                .filter_map(|(_, cron)| {
                    scheduler::get_next_run_time(&cron, &w.timezone).map(|t| NextRun {
                        workflow_id: w.id.clone(),
                        workflow_name: w.name.clone(),
                        corpus: w.corpus.clone(),
                        next_time: t,
                    })
                })
        })
        .collect();
    next_runs.sort_by(|a, b| a.next_time.cmp(&b.next_time));

    let recent_runs = state.db.get_recent_runs(10).map_err(|e| e.to_string())?;

    Ok(SchedulerStatus {
        active_workflows,
        running_count,
        next_runs,
        recent_runs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workflow_with_trigger(trigger_config: Option<&str>, legacy_cron: &str) -> Workflow {
        Workflow {
            id: "wf".to_string(),
            name: "Workflow".to_string(),
            description: None,
            script_path: "scripts/workflows/noop.py".to_string(),
            cron_schedule: legacy_cron.to_string(),
            enabled: true,
            async_mode: false,
            email_on_failure: true,
            corpus: "source".to_string(),
            domain: Some("scheduler".to_string()),
            timezone: "UTC".to_string(),
            trigger_config: trigger_config.map(str::to_string),
            queue_config: None,
            last_run_at: None,
            created_at: "2026-05-12T00:00:00Z".to_string(),
            updated_at: "2026-05-12T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn cron_triggers_fall_back_to_legacy_only_when_config_is_missing_or_invalid() {
        let missing = workflow_with_trigger(None, "0 0 * * *");
        assert_eq!(
            cron_triggers_for_workflow(&missing),
            vec![("legacy_cron".to_string(), "0 0 * * *".to_string())]
        );

        let invalid = workflow_with_trigger(Some("{not-json"), "0 1 * * *");
        assert_eq!(
            cron_triggers_for_workflow(&invalid),
            vec![("legacy_cron".to_string(), "0 1 * * *".to_string())]
        );

        let event_only = workflow_with_trigger(
            Some(r#"{"triggers":[{"kind":"file_arrival","path":"data/*.json"}]}"#),
            "0 2 * * *",
        );
        assert!(cron_triggers_for_workflow(&event_only).is_empty());
    }

    #[test]
    fn cron_triggers_use_explicit_cron_entries() {
        let workflow = workflow_with_trigger(
            Some(
                r#"{"triggers":[{"kind":"cron","cron":"0 3 * * *"},{"kind":"cron","cron":"30 3 * * *"}]}"#,
            ),
            "0 2 * * *",
        );
        assert_eq!(
            cron_triggers_for_workflow(&workflow),
            vec![
                ("cron".to_string(), "0 3 * * *".to_string()),
                ("cron".to_string(), "30 3 * * *".to_string()),
            ]
        );
    }

    #[test]
    fn source_definition_change_detects_only_source_controlled_fields() {
        let workflow = workflow_with_trigger(
            Some(r#"{"triggers":[{"kind":"cron","cron":"0 3 * * *"}]}"#),
            "0 2 * * *",
        );

        assert!(!source_definition_changed(
            &workflow,
            "Workflow",
            None,
            "scripts/workflows/noop.py",
            "0 2 * * *",
            false,
            "source",
            Some("scheduler"),
            Some(r#"{"triggers":[{"kind":"cron","cron":"0 3 * * *"}]}"#),
            None,
        ));
        assert!(source_definition_changed(
            &workflow,
            "Workflow",
            None,
            "scripts/workflows/replaced.py",
            "0 2 * * *",
            false,
            "source",
            Some("scheduler"),
            Some(r#"{"triggers":[{"kind":"cron","cron":"0 3 * * *"}]}"#),
            None,
        ));
        assert!(source_definition_changed(
            &workflow,
            "Workflow",
            None,
            "scripts/workflows/noop.py",
            "0 2 * * *",
            false,
            "instance",
            Some("scheduler"),
            Some(r#"{"triggers":[{"kind":"cron","cron":"0 3 * * *"}]}"#),
            None,
        ));
    }
}

#[tauri::command]
pub fn list_queues(state: State<AppState>) -> Result<Vec<QueueInfo>, String> {
    state.db.list_queues().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_queue(
    state: State<AppState>,
    name: String,
    corpus: String,
    capacity: i64,
    tag_cap: Option<i64>,
    max_queued: Option<i64>,
) -> Result<QueueInfo, String> {
    state
        .db
        .upsert_queue(&name, &corpus, capacity, tag_cap, max_queued)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_queued_runs(
    state: State<AppState>,
    limit: Option<i64>,
) -> Result<Vec<QueuedRun>, String> {
    state
        .db
        .list_queued_runs(limit.unwrap_or(50))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cancel_queued_run(state: State<AppState>, id: String) -> Result<(), String> {
    let updated = state.db.cancel_queued_run(&id).map_err(|e| e.to_string())?;
    if updated == 0 {
        return Err("Queued run is no longer cancellable".to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn open_dashboard(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::{Emitter, Manager};
    if let Some(main) = app.get_webview_window("main") {
        main.show().map_err(|e| e.to_string())?;
        main.set_focus().map_err(|e| e.to_string())?;
        main.emit("navigate-to-mission-control", serde_json::json!({}))
            .map_err(|e| e.to_string())?;
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
    state
        .db
        .set_notification_prefs(notify_on_failure, notify_on_success)
        .map_err(|e| e.to_string())?;
    let scheduler = state.scheduler.lock().map_err(|e| e.to_string())?;
    scheduler.set_notify_on_failure(notify_on_failure);
    scheduler.set_notify_on_success(notify_on_success);
    Ok(())
}

#[tauri::command]
pub fn get_notification_prefs(state: State<AppState>) -> Result<serde_json::Value, String> {
    let (notify_on_failure, notify_on_success) = state
        .db
        .get_notification_prefs()
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "notify_on_failure": notify_on_failure,
        "notify_on_success": notify_on_success,
    }))
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
pub fn set_email_config(state: State<AppState>, mut config: EmailConfig) -> Result<(), String> {
    if config.smtp_password == "••••••••" {
        config.smtp_password = state
            .db
            .get_email_config()
            .map(|c| c.smtp_password)
            .unwrap_or_default();
    }
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
