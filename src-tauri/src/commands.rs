use crate::db::{
    DashboardBlastRadius, DashboardBlockTaxonomy, DashboardExecutionSlots, DashboardKpiDelta,
    DashboardKpiSummary, DashboardQueueHealthSummary, DashboardQueueUtilizationHistory,
    DashboardStatusCount, DashboardTrendSeries, DashboardWaitRuntimeTrend,
    DashboardWorkflowBaseline, DashboardWorkflowFailureCount, Database, EmailConfig, EmailProfile,
    HistoryReadModel, MissionControlNeedsAttentionItem, MissionControlPanelAvailability,
    MissionControlPreferences, MissionControlSnapshot, MissionControlUpcomingRun, NextRun,
    QueueInfo, QueuedRun, RetentionPreview, Run, RunAttempt, RunMetric, RunRelationship, RunTask,
    SchedulerAsset, SchedulerDeadLetter, SchedulerStatus, SlaViolation, Workflow,
    WorkflowHistoryBucket, WorkflowResourceSample, WorkflowTokenUsageRollup,
};
use crate::scheduler::{self, WorkflowScheduler};
use crate::service::{SchedulerService, WorkflowDraft};
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
    pub service: SchedulerService,
    pub workspace_root: String,
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
        // `workspace_root` is the canonical key; `chaos_labs_root` retained for
        // one transition version for backward-compatible frontends/scripts.
        "workspace_root": state.workspace_root,
        "chaos_labs_root": state.workspace_root,
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
#[allow(clippy::too_many_arguments)] // Tauri IPC command: arg list mirrors the JS invoke() payload.
pub fn create_workflow(
    state: State<AppState>,
    name: String,
    description: Option<String>,
    script_path: String,
    cron_schedule: String,
    async_mode: Option<bool>,
    email_on_failure: Option<bool>,
    timezone: Option<String>,
    environment: Option<String>,
    domain: Option<String>,
    trigger_config: Option<String>,
    queue_config: Option<String>,
) -> Result<Workflow, String> {
    let environment = environment.unwrap_or_else(|| "production".to_string());
    let draft = WorkflowDraft {
        name,
        description,
        script_path,
        cron_schedule,
        async_mode: async_mode.unwrap_or(false),
        email_on_failure: email_on_failure.unwrap_or(true),
        timezone: timezone.unwrap_or_else(|| "UTC".to_string()),
        environment,
        domain,
        trigger_config,
        queue_config,
    };
    // UI-originated: not permitted to mint externally-managed definitions.
    state
        .service
        .create_workflow(draft, false)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)] // Tauri IPC command: arg list mirrors the JS invoke() payload.
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
    environment: Option<String>,
    domain: Option<String>,
    trigger_config: Option<String>,
    queue_config: Option<String>,
) -> Result<Workflow, String> {
    let existing = state.db.get_workflow(&id).map_err(|e| e.to_string())?;
    let environment = environment.unwrap_or_else(|| existing.environment.clone());
    let draft = WorkflowDraft {
        name,
        description,
        script_path,
        cron_schedule,
        async_mode: async_mode.unwrap_or(existing.async_mode),
        email_on_failure: email_on_failure.unwrap_or(true),
        timezone: timezone.unwrap_or_else(|| "UTC".to_string()),
        environment,
        domain: domain.or_else(|| existing.domain.clone()),
        trigger_config: trigger_config.or_else(|| existing.trigger_config.clone()),
        queue_config: queue_config.or_else(|| existing.queue_config.clone()),
    };
    // UI-originated: may only touch runtime prefs on managed workflows.
    state
        .service
        .update_workflow(&id, enabled, draft, false)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_workflow(state: State<AppState>, id: String) -> Result<(), String> {
    state
        .service
        .delete_workflow(&id, false)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_environments(state: State<AppState>) -> Result<Vec<crate::db::Environment>, String> {
    state.service.list_environments().map_err(|e| e.to_string())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn create_environment(
    state: State<AppState>,
    name: String,
    description: Option<String>,
    working_dir: Option<String>,
    default_queue_capacity: Option<i64>,
    default_tag_cap: Option<i64>,
    default_max_queued: Option<i64>,
) -> Result<crate::db::Environment, String> {
    state
        .service
        .create_environment(
            &name,
            description.as_deref(),
            working_dir.as_deref(),
            default_queue_capacity,
            default_tag_cap,
            default_max_queued,
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_environment(state: State<AppState>, id: String) -> Result<(), String> {
    state
        .service
        .delete_environment(&id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn update_environment(
    state: State<AppState>,
    id: String,
    name: String,
    description: Option<String>,
    working_dir: Option<String>,
    default_queue_capacity: Option<i64>,
    default_tag_cap: Option<i64>,
    default_max_queued: Option<i64>,
) -> Result<crate::db::Environment, String> {
    state
        .service
        .update_environment(
            &id,
            &name,
            description.as_deref(),
            working_dir.as_deref(),
            default_queue_capacity,
            default_tag_cap,
            default_max_queued,
        )
        .map_err(|e| e.to_string())
}

/// Validate + persist a workflow's execution spec (kind + spec_json).
#[tauri::command]
pub fn set_workflow_spec(
    state: State<AppState>,
    id: String,
    spec: crate::workflow_spec::WorkflowSpec,
) -> Result<Workflow, String> {
    state
        .service
        .set_workflow_spec(&id, &spec, false)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_api_keys(state: State<AppState>) -> Result<Vec<crate::db::ApiKeyInfo>, String> {
    state.service.list_api_keys().map_err(|e| e.to_string())
}

/// Check the configured updater endpoint for a newer release, routed through
/// the shared background-check path (`update::run_check`) so the manual
/// Settings button and the launch/6h timer are one code path (updater UX
/// plan, Section 1). Degrades gracefully (returns `available: false` with an
/// `error` note) when the updater is unconfigured or the network is
/// unavailable, so the UI never hard-fails.
#[tauri::command]
pub async fn check_for_update(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let snapshot = crate::update::run_check(&app, true).await;
    Ok(serde_json::json!({
        "available": snapshot.phase == crate::update::UpdatePhase::Available,
        // `version` kept for one release cycle alongside the `latest_version`
        // alias the frontend actually reads (Section 4 discrepancy fix).
        "version": snapshot.latest_version,
        "latest_version": snapshot.latest_version,
        "current_version": snapshot.current_version,
        "notes": snapshot.notes,
        "error": snapshot.last_error.as_ref().map(|e| e.message.clone()),
    }))
}

/// No-network hydration for the frontend update hook (mount) and for any
/// window created after the launch background check already ran.
#[tauri::command]
pub fn get_app_update_status(
    update_state: State<crate::update::UpdateState>,
) -> crate::update::UpdateSnapshot {
    update_state.snapshot()
}

/// Persists the two updater preferences (Sections 2 & 5): background checks
/// and a single per-exact-version skip. `clear_skip` is a distinct flag
/// rather than a nullable `skipped_version` so the Tauri IPC boundary never
/// has to distinguish "omitted" from "explicit null" — both collapse to the
/// same `None` once they cross serde's `Option` handling.
///
/// The in-memory snapshot mutation + broadcast is delegated to
/// [`crate::update::apply_preferences`], the same helper `run_check`/`apply`
/// use to emit `update-status`, so a Skip / background-check-toggle change
/// reaches every window and the tray tooltip immediately instead of only the
/// calling window's own return value.
#[tauri::command]
pub fn set_updater_preferences(
    app: tauri::AppHandle,
    state: State<AppState>,
    background_check_enabled: Option<bool>,
    skipped_version: Option<String>,
    clear_skip: Option<bool>,
) -> Result<crate::update::UpdateSnapshot, String> {
    if let Some(enabled) = background_check_enabled {
        state
            .db
            .set_updater_background_check_enabled(enabled)
            .map_err(|e| e.to_string())?;
    }
    let clearing_skip = clear_skip.unwrap_or(false);
    if clearing_skip {
        state
            .db
            .set_updater_skipped_version(None)
            .map_err(|e| e.to_string())?;
    } else if let Some(version) = skipped_version.as_deref() {
        state
            .db
            .set_updater_skipped_version(Some(version))
            .map_err(|e| e.to_string())?;
    }

    Ok(crate::update::apply_preferences(
        &app,
        background_check_enabled,
        skipped_version,
        clearing_skip,
    ))
}

/// Download + install the available update, then relaunch. Routed through
/// [`crate::update::apply`] so download progress and the pre-install
/// scheduler drain are visible through the same `update-status` event the
/// check path emits. `expected_version`, when supplied, must match the
/// version currently on offer or the call is refused (Section 6 consent
/// guard) — the frontend passes the version shown in the banner/dialog the
/// user actually clicked "Install" on.
///
/// On success this only returns for the "nothing to install" case; once an
/// install is underway the process restarts (`app.restart()` never returns),
/// so the frontend should not expect a response for a real update.
#[tauri::command]
pub async fn apply_update(
    app: tauri::AppHandle,
    expected_version: Option<String>,
) -> Result<crate::update::UpdateSnapshot, String> {
    crate::update::apply(&app, expected_version).await
}

/// Read-only status for the managed Cursor/MCP integration card in
/// Integrations. Never fails hard — a missing Node install, an unreachable
/// API, or a Cursor config conflict are all reported as status fields, not
/// as a rejected promise, so the UI can always render a coherent card.
#[tauri::command]
pub fn get_mcp_integration_status(
    app: tauri::AppHandle,
    state: State<AppState>,
) -> Result<crate::mcp::McpIntegrationStatus, String> {
    use tauri::Manager;
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let config_path = crate::mcp::cursor_mcp_config_path()?;
    Ok(crate::mcp::status(
        &app_data_dir,
        &state.service,
        &config_path,
    ))
}

/// Enable (or repair/re-provision) the managed Cursor/MCP integration:
/// mints/reuses a scoped API key, installs the pinned `mcp-server` version
/// into an app-owned directory, verifies it, and registers it in
/// `~/.cursor/mcp.json`. Idempotent. `force` takes over a pre-existing
/// unmanaged `chaos-scheduler` entry instead of reporting a conflict.
///
/// Shares a single-flight lock with [`remove_mcp_integration`] and the
/// startup re-provision hook so concurrent callers can never race the same
/// staging directory or config file.
#[tauri::command]
pub fn provision_mcp_integration(
    app: tauri::AppHandle,
    state: State<AppState>,
    mcp_state: State<crate::mcp::McpState>,
    force: Option<bool>,
) -> Result<crate::mcp::McpIntegrationStatus, String> {
    use tauri::Manager;
    let _guard = crate::mcp::try_lock_recovering(&mcp_state).map_err(|e| e.to_string())?;
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let config_path = crate::mcp::cursor_mcp_config_path()?;
    let result = crate::mcp::provision(
        &app_data_dir,
        &state.service,
        &config_path,
        force.unwrap_or(false),
    );
    // Emit regardless of outcome, mirroring `spawn_reprovision_on_startup`:
    // `provision` now always persists a failure to the manifest before
    // returning `Err` (see `fail_provision`), so re-deriving a fresh status
    // here — rather than only on `Ok` — keeps every other window and the
    // tray honest about a failed attempt too, not just the caller that
    // triggered it.
    let status = crate::mcp::status(&app_data_dir, &state.service, &config_path);
    crate::mcp::emit_status_changed(&app, &status);
    result
}

/// Remove the managed integration: drop the managed `mcp.json` entry (only if
/// app-owned), delete the app-managed install directory, and revoke the
/// managed API key. `prepare_to_uninstall` additionally removes the
/// launch-at-login launchd agent.
#[tauri::command]
pub fn remove_mcp_integration(
    app: tauri::AppHandle,
    state: State<AppState>,
    mcp_state: State<crate::mcp::McpState>,
    prepare_to_uninstall: Option<bool>,
) -> Result<crate::mcp::McpIntegrationStatus, String> {
    use tauri::Manager;
    let _guard = crate::mcp::try_lock_recovering(&mcp_state).map_err(|e| e.to_string())?;
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let config_path = crate::mcp::cursor_mcp_config_path()?;
    let result = crate::mcp::remove(
        &app_data_dir,
        &state.service,
        &config_path,
        prepare_to_uninstall.unwrap_or(false),
    );
    if let Ok(status) = &result {
        crate::mcp::emit_status_changed(&app, status);
    }
    result
}

#[tauri::command]
pub fn revoke_api_key(state: State<AppState>, id: String) -> Result<(), String> {
    state.service.revoke_api_key(&id).map_err(|e| e.to_string())
}

/// Mint a new HTTP API key. Returns the plaintext token exactly once.
#[tauri::command]
pub fn create_api_key(
    state: State<AppState>,
    name: Option<String>,
    scopes: Option<Vec<String>>,
) -> Result<serde_json::Value, String> {
    let scope_refs: Vec<&str> = scopes
        .as_ref()
        .map(|s| s.iter().map(String::as_str).collect())
        .unwrap_or_else(|| vec!["read"]);
    let key = state
        .service
        .create_api_key(name.as_deref(), &scope_refs)
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "id": key.id,
        "token": key.token,
        "scopes": key.scopes,
    }))
}

#[tauri::command]
pub fn enqueue_workflow(
    state: State<AppState>,
    id: String,
    idempotency_key: Option<String>,
) -> Result<scheduler::DispatchOutcome, String> {
    // Centralized admission choke point: gate + idempotency + dispatch. The
    // desktop enqueue path carries no payload/rerun context, so the fingerprint
    // reduces to the historic `id + "ui_enqueue"` digest.
    state
        .service
        .dispatch_manual_run(
            &state.workspace_root,
            &state.python_path,
            &id,
            "ui_enqueue",
            idempotency_key.as_deref(),
            None,
            None,
            None,
        )
        .map_err(|e| e.to_string())
}

/// Dispatch a Cursor **cloud** fix agent for a FAILED run (D05 / F10). This is
/// invoked ONLY behind the explicit human-consent Modal in run detail — there
/// is no automated caller. Every spend + safety gate (enabled toggle, non-prod
/// target, per-run idempotency, per-hour rate cap, source-must-be-failed) lives
/// in [`SchedulerService::dispatch_fix_agent`]; this adapter just stringifies
/// the typed refusal. The initiator is recorded as `ui` for the desktop
/// surface. Inherits the shared admission behavior (queues at capacity, else
/// runs inline).
#[tauri::command]
pub fn dispatch_fix_agent(
    state: State<AppState>,
    run_id: String,
) -> Result<scheduler::DispatchOutcome, String> {
    state
        .service
        .dispatch_fix_agent(&state.workspace_root, &state.python_path, &run_id, "ui")
        .map_err(|e| e.to_string())
}

/// Re-run a workflow. Routed through the shared admission choke point
/// ([`SchedulerService::dispatch_manual_run`]) so a rerun respects dependency
/// and capacity gating (it queues instead of spawning immediately) and honors
/// an optional idempotency key — the same policy as the desktop enqueue and the
/// REST run/enqueue/rerun/webhook handlers.
#[tauri::command]
pub fn rerun_workflow(
    state: State<AppState>,
    workflow_id: String,
    source_run_id: Option<String>,
    input_override_json: Option<String>,
    idempotency_key: Option<String>,
) -> Result<scheduler::DispatchOutcome, String> {
    dispatch_rerun(
        &state.service,
        &state.workspace_root,
        &state.python_path,
        workflow_id,
        source_run_id,
        input_override_json,
        idempotency_key,
    )
}

/// Build the rerun trigger payload and route it through admission control.
/// Extracted from the Tauri command so it is unit-testable without a
/// `State<AppState>`. The `ui_rerun` trigger kind and the source run id
/// (carried as `rerun_of`) mirror the REST `api_rerun` handler.
fn dispatch_rerun(
    service: &SchedulerService,
    workspace_root: &str,
    python_path: &str,
    workflow_id: String,
    source_run_id: Option<String>,
    input_override_json: Option<String>,
    idempotency_key: Option<String>,
) -> Result<scheduler::DispatchOutcome, String> {
    if let Some(input) = &input_override_json {
        serde_json::from_str::<serde_json::Value>(input)
            .map_err(|e| format!("Invalid input override JSON: {}", e))?;
    }
    let payload = serde_json::json!({
        "source_run_id": source_run_id,
        "input_override": input_override_json
            .as_ref()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok()),
    })
    .to_string();
    service
        .dispatch_manual_run(
            workspace_root,
            python_path,
            &workflow_id,
            "ui_rerun",
            idempotency_key.as_deref(),
            Some(&payload),
            source_run_id.as_deref(),
            input_override_json.as_deref(),
        )
        .map_err(|e| e.to_string())
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
    if !dry_run {
        state
            .service
            .ensure_workflow_execution_allowed(&workflow_id)
            .map_err(|e| e.to_string())?;
    }
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
            &state.workspace_root,
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
        &state.workspace_root,
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
    environment_filter: Option<String>,
    domain_filter: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<Run>, String> {
    state
        .db
        .get_global_run_history(
            status_filter.as_deref(),
            trigger_kind.as_deref(),
            environment_filter.as_deref(),
            domain_filter.as_deref(),
            limit.unwrap_or(100),
        )
        .map_err(|e| e.to_string())
}

/// Lean, LOG-FREE history read model backing the filtered history table + its
/// KPI header: rows (never `stdout`/`stderr`) plus aggregate KPIs
/// `{ total, endedOk, endedNotOk, running, p95Duration }`. Keyed by
/// `(environmentFilter, workflowFilter, statusFilter, lookback)`, all optional
/// (omit or `"all"` to disable a filter). The status filter scopes the ROWS
/// only — the KPIs always report the full breakdown for the
/// `(environment, workflow, lookback)` scope so the header is stable as the
/// table is filtered. `lookback` accepts the shared dashboard grammar (`1d`,
/// `3d`, `7d`, `30d`, `<n>h`, `all`) and defaults to `1d`; p95 duration is
/// workflow-scoped whenever a workflow filter is applied. Tauri-IPC-only (no
/// REST/SDK/MCP surface). Environment provenance is SNAPSHOTTED at run creation
/// time (schema v13), not the current workflow env — see
/// [`Database::history_read_model`].
#[tauri::command]
pub fn get_run_history_model(
    state: State<AppState>,
    environment_filter: Option<String>,
    workflow_filter: Option<String>,
    status_filter: Option<String>,
    lookback: Option<String>,
    limit: Option<i64>,
) -> Result<HistoryReadModel, String> {
    let (window_modifier, _window_seconds) = parse_lookback(lookback.as_deref())?;
    let environment_filter = normalize_all_filter(environment_filter);
    let workflow_filter = normalize_all_filter(workflow_filter);
    let status_filter = normalize_all_filter(status_filter);
    state
        .db
        .history_read_model(
            Some(&environment_filter),
            Some(&workflow_filter),
            Some(&status_filter),
            &window_modifier,
            limit.unwrap_or(200),
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

/// Windowed KPI roll-up (throughput/hr, avg + max runtime, success rate,
/// median + max wait) for the v3 dashboard, scoped to `(environmentFilter,
/// lookback)`. `lookback` accepts the shared grammar (`1d`, `3d`, `7d`,
/// `30d`, `<n>h`, `all`); defaults to `1d`.
#[tauri::command]
pub fn get_dashboard_kpi_summary(
    state: State<AppState>,
    environment_filter: Option<String>,
    lookback: Option<String>,
) -> Result<DashboardKpiSummary, String> {
    let (window_modifier, window_seconds) = parse_lookback(lookback.as_deref())?;
    let environment_filter = normalize_mission_environment_filter(environment_filter, "all");
    state
        .db
        .dashboard_kpi_summary(&environment_filter, &window_modifier, window_seconds)
        .map_err(|e| e.to_string())
}

/// Per-status run counts for the v3 status donut, scoped to `(environmentFilter,
/// lookback)`. `lookback` accepts the shared grammar; defaults to `1d`.
#[tauri::command]
pub fn get_dashboard_status_distribution(
    state: State<AppState>,
    environment_filter: Option<String>,
    lookback: Option<String>,
) -> Result<Vec<DashboardStatusCount>, String> {
    let (window_modifier, _window_seconds) = parse_lookback(lookback.as_deref())?;
    let environment_filter = normalize_mission_environment_filter(environment_filter, "all");
    state
        .db
        .dashboard_status_distribution(&environment_filter, &window_modifier)
        .map_err(|e| e.to_string())
}

/// Cross-workflow success/fail trend for the v3 dashboard, scoped to
/// `(environmentFilter, lookback)`. The bucket grain (`hour`/`day`) is chosen
/// from the lookback and returned alongside the buckets. `lookback` accepts the
/// shared grammar; defaults to `1d`.
#[tauri::command]
pub fn get_dashboard_success_fail_trend(
    state: State<AppState>,
    environment_filter: Option<String>,
    lookback: Option<String>,
) -> Result<DashboardTrendSeries, String> {
    let (window_modifier, window_seconds) = parse_lookback(lookback.as_deref())?;
    let grain = bucket_grain(window_seconds);
    let environment_filter = normalize_mission_environment_filter(environment_filter, "all");
    let buckets = state
        .db
        .dashboard_success_fail_trend(&environment_filter, &window_modifier, grain)
        .map_err(|e| e.to_string())?;
    Ok(DashboardTrendSeries {
        grain: grain.to_string(),
        buckets,
    })
}

/// Wait + runtime duration trends for the v3 dashboard, scoped to
/// `(environmentFilter, lookback)`, each bucketed at a grain chosen from the
/// lookback with a 30-day trailing-average baseline per bucket. `lookback`
/// accepts the shared grammar; defaults to `1d`.
#[tauri::command]
pub fn get_dashboard_wait_runtime_trend(
    state: State<AppState>,
    environment_filter: Option<String>,
    lookback: Option<String>,
) -> Result<DashboardWaitRuntimeTrend, String> {
    let (window_modifier, window_seconds) = parse_lookback(lookback.as_deref())?;
    let grain = bucket_grain(window_seconds);
    let environment_filter = normalize_mission_environment_filter(environment_filter, "all");
    state
        .db
        .dashboard_wait_runtime_trend(&environment_filter, &window_modifier, grain)
        .map_err(|e| e.to_string())
}

/// Per-workflow failure recurrence for the v3 dashboard, scoped to
/// `(environmentFilter, lookback)`. Only workflows with at least one failure in
/// the window are returned, worst first. `lookback` accepts the shared grammar;
/// defaults to `1d`.
#[tauri::command]
pub fn get_dashboard_failure_recurrence(
    state: State<AppState>,
    environment_filter: Option<String>,
    lookback: Option<String>,
) -> Result<Vec<DashboardWorkflowFailureCount>, String> {
    let (window_modifier, _window_seconds) = parse_lookback(lookback.as_deref())?;
    let environment_filter = normalize_mission_environment_filter(environment_filter, "all");
    state
        .db
        .dashboard_failure_recurrence(&environment_filter, &window_modifier)
        .map_err(|e| e.to_string())
}

/// Current queue-health summary for the v3 dashboard, scoped to
/// `environmentFilter` (`all` for every queue). Reflects live occupancy, so it
/// takes no lookback.
#[tauri::command]
pub fn get_dashboard_queue_health(
    state: State<AppState>,
    environment_filter: Option<String>,
) -> Result<DashboardQueueHealthSummary, String> {
    let environment_filter = normalize_mission_environment_filter(environment_filter, "all");
    state
        .db
        .dashboard_queue_health(&environment_filter)
        .map_err(|e| e.to_string())
}

/// Week-over-week KPI comparison for the v3 dashboard: the current window vs the
/// immediately-prior equal window, plus deltas. `lookback` accepts the shared
/// grammar but defaults to `7d` so the out-of-the-box comparison is a true
/// week-over-week; pass e.g. `1d` for day-over-day.
#[tauri::command]
pub fn get_dashboard_kpi_wow(
    state: State<AppState>,
    environment_filter: Option<String>,
    lookback: Option<String>,
) -> Result<DashboardKpiDelta, String> {
    let (_window_modifier, window_seconds) =
        parse_lookback(Some(lookback.as_deref().unwrap_or("7d")))?;
    let environment_filter = normalize_mission_environment_filter(environment_filter, "all");
    state
        .db
        .dashboard_kpi_wow(&environment_filter, window_seconds)
        .map_err(|e| e.to_string())
}

/// Rolling per-workflow runtime baselines (expected p50 + mean) over a trailing
/// window, for the race-track `%`-complete estimate and long-runner deviation.
/// `lookback` accepts the shared grammar but defaults to `30d` (not `1d`): a
/// baseline needs a longer trailing window than the live dashboard lookback.
#[tauri::command]
pub fn get_dashboard_workflow_baselines(
    state: State<AppState>,
    environment_filter: Option<String>,
    lookback: Option<String>,
) -> Result<Vec<DashboardWorkflowBaseline>, String> {
    let (window_modifier, _window_seconds) =
        parse_lookback(Some(lookback.as_deref().unwrap_or("30d")))?;
    let environment_filter = normalize_mission_environment_filter(environment_filter, "all");
    state
        .db
        .dashboard_workflow_runtime_baselines(&environment_filter, &window_modifier)
        .map_err(|e| e.to_string())
}

/// Blocked/waiting reason taxonomy for the status board: the currently-`queued`
/// set classified into `{resource, event, host, workload, user, unknown}` with
/// per-category counts + Σ current wait, the current-wait totals, the trailing
/// (historical) admission-wait stats over the lookback, and the heaviest
/// per-workflow blockers by Σ current wait. `lookback` scopes only the trailing
/// stats; the current-blocked set is always "now".
#[tauri::command]
pub fn get_dashboard_block_taxonomy(
    state: State<AppState>,
    environment_filter: Option<String>,
    lookback: Option<String>,
) -> Result<DashboardBlockTaxonomy, String> {
    let (window_modifier, _window_seconds) =
        parse_lookback(Some(lookback.as_deref().unwrap_or("7d")))?;
    let environment_filter = normalize_mission_environment_filter(environment_filter, "all");
    state
        .db
        .dashboard_block_taxonomy(&environment_filter, &window_modifier)
        .map_err(|e| e.to_string())
}

/// Queue-utilization history for the v3 dashboard: a per-bucket occupancy series
/// over (environment, lookback) at a grain chosen from the lookback (sub-day ->
/// `hour`, else `day`), plus the healthy/warn/degraded utilization thresholds.
/// Backed by the periodic queue-occupancy sampler.
#[tauri::command]
pub fn get_dashboard_queue_utilization_history(
    state: State<AppState>,
    environment_filter: Option<String>,
    lookback: Option<String>,
) -> Result<DashboardQueueUtilizationHistory, String> {
    let (window_modifier, window_seconds) = parse_lookback(lookback.as_deref())?;
    let grain = bucket_grain(window_seconds);
    let environment_filter = normalize_mission_environment_filter(environment_filter, "all");
    state
        .db
        .dashboard_queue_utilization_history(&environment_filter, &window_modifier, grain)
        .map_err(|e| e.to_string())
}

/// Observable execution slots for the v3 dashboard: running runs vs configured
/// concurrency capacity, per queue and globally. Live snapshot (no lookback).
#[tauri::command]
pub fn get_dashboard_execution_slots(
    state: State<AppState>,
    environment_filter: Option<String>,
) -> Result<DashboardExecutionSlots, String> {
    let environment_filter = normalize_mission_environment_filter(environment_filter, "all");
    state
        .db
        .dashboard_execution_slots(&environment_filter)
        .map_err(|e| e.to_string())
}

/// Downstream blast-radius per workflow for the v3 dashboard: from
/// `run_relationships` chain edges, the downstream dependent count + max chain
/// depth of each in-window run, rolled up per workflow. `lookback` defaults to
/// `7d` (blast radius wants a wider window than the live dashboard lookback).
#[tauri::command]
pub fn get_dashboard_blast_radius(
    state: State<AppState>,
    environment_filter: Option<String>,
    lookback: Option<String>,
) -> Result<Vec<DashboardBlastRadius>, String> {
    let (window_modifier, _window_seconds) =
        parse_lookback(Some(lookback.as_deref().unwrap_or("7d")))?;
    let environment_filter = normalize_mission_environment_filter(environment_filter, "all");
    state
        .db
        .dashboard_blast_radius(&environment_filter, &window_modifier)
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
            "environment".to_string(),
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

/// Parse a dashboard lookback into `(sqlite_modifier, window_seconds)`.
///
/// Accepts the same grammar as [`time_window_modifier`] (`<n>h`, `<n>d`,
/// `<n>m`, `"<n> hours"`, `all`); the second tuple element is the nominal
/// window length in seconds used for rate math (e.g. throughput/hour). `all`
/// maps to a 100-year window.
fn parse_lookback(value: Option<&str>) -> Result<(String, i64), String> {
    let raw = value.unwrap_or("1d").trim().to_ascii_lowercase();
    if raw == "all" {
        return Ok(("-100 years".to_string(), 100 * 365 * 24 * 60 * 60));
    }
    let (number, unit) = split_window(&raw).ok_or_else(|| {
        "lookback must be all, <number>h, <number>d, <number>m, or '<number> hours/days/minutes'"
            .to_string()
    })?;
    let count: i64 = number
        .parse()
        .map_err(|_| "lookback count must be a positive integer".to_string())?;
    if count <= 0 {
        return Err("lookback count must be positive".to_string());
    }
    let (sqlite_unit, unit_seconds) = match unit {
        "m" | "min" | "minute" | "minutes" => ("minutes", 60),
        "h" | "hr" | "hour" | "hours" => ("hours", 60 * 60),
        "d" | "day" | "days" => ("days", 24 * 60 * 60),
        _ => return Err(format!("Unsupported lookback unit: {}", unit)),
    };
    Ok((format!("-{} {}", count, sqlite_unit), count * unit_seconds))
}

/// Choose a trend bucket grain from the nominal window length: sub-day
/// (`"hour"`) for windows up to 3 days, otherwise `"day"`. Keeps short
/// lookbacks (1d/3d) hourly and longer ones (7d/30d) daily.
fn bucket_grain(window_seconds: i64) -> &'static str {
    if window_seconds <= 3 * 24 * 60 * 60 {
        "hour"
    } else {
        "day"
    }
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

/// Normalize a mission-control environment filter. Environments are
/// user-managed, so any non-empty value is passed through as the partition to
/// filter on; empty / "all" means no environment filter.
/// Normalize an optional `(all | <value>)` filter token: trim it, and map an
/// empty string or a case-insensitive `"all"` to the `"all"` sentinel the read
/// model uses to disable a filter. Used by [`get_run_history_model`] for its
/// environment/workflow/status filters.
fn normalize_all_filter(value: Option<String>) -> String {
    match value {
        Some(v) if !v.trim().is_empty() && !v.trim().eq_ignore_ascii_case("all") => {
            v.trim().to_string()
        }
        _ => "all".to_string(),
    }
}

fn normalize_mission_environment_filter(value: Option<String>, default: &str) -> String {
    let raw = value.unwrap_or_else(|| default.to_string());
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("all") {
        "all".to_string()
    } else {
        trimmed.to_string()
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
            filter_behavior: "environment/domain filters apply before counting".to_string(),
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
                "Unattributed assets are labeled and withheld from environment/domain filters"
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
    environment_filter: String,
    domain_filter: String,
) -> Result<MissionControlPreferences, String> {
    state
        .db
        .set_mission_control_preferences(&default_landing, &environment_filter, &domain_filter)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_mission_control_snapshot(
    state: State<AppState>,
    environment_filter: Option<String>,
    domain_filter: Option<String>,
) -> Result<MissionControlSnapshot, String> {
    let preferences = state
        .db
        .get_mission_control_preferences()
        .map_err(|e| e.to_string())?;
    let explicit_domain_filter = domain_filter.is_some();
    let environment_filter =
        normalize_mission_environment_filter(environment_filter, &preferences.environment_filter);
    let requested_domain_filter =
        normalize_mission_domain_filter(domain_filter, &preferences.domain_filter);
    let domains = state
        .db
        .mission_control_domains(&environment_filter)
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
                &environment_filter,
                "all",
            );
        }
        "all".to_string()
    };

    let workflows = state
        .db
        .list_workflows_filtered(&environment_filter, &domain_filter)
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
        .mission_control_header(&environment_filter, &domain_filter)
        .map_err(|e| e.to_string())?;
    let sla = state
        .db
        .mission_control_sla_summary(
            &environment_filter,
            &domain_filter,
            sla_violations.len() as i64,
            // Snapshot preserves the legacy 24h summary window; the v3
            // dashboard uses the windowed `get_dashboard_kpi_summary` command.
            "-1 day",
        )
        .map_err(|e| e.to_string())?;
    let recent_runs = state
        .db
        .mission_control_recent_runs(&environment_filter, &domain_filter, 12)
        .map_err(|e| e.to_string())?;
    let failed_runs = state
        .db
        .mission_control_failed_runs(&environment_filter, &domain_filter, 4)
        .map_err(|e| e.to_string())?;
    let failed_run_count = state
        .db
        .mission_control_failed_run_count(&environment_filter, &domain_filter)
        .map_err(|e| e.to_string())?;
    let live_activity = state
        .db
        .mission_control_live_activity(&environment_filter, &domain_filter, 10)
        .map_err(|e| e.to_string())?;
    let freshness_ledger = state
        .db
        .mission_control_freshness_ledger(&environment_filter, &domain_filter, 24 * 60 * 60, 12)
        .map_err(|e| e.to_string())?;
    let workflow_telemetry = state
        .db
        .mission_control_workflow_telemetry(&environment_filter, &domain_filter, "-24 hours", 12)
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
                            environment: workflow.environment.clone(),
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
            environment_filter,
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
                        environment: w.environment.clone(),
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
            environment: "production".to_string(),
            managed_externally: true,
            kind: "generic".to_string(),
            spec_json: None,
            domain: Some("scheduler".to_string()),
            timezone: "UTC".to_string(),
            trigger_config: trigger_config.map(str::to_string),
            queue_config: None,
            email_profile_id: None,
            last_run_at: None,
            created_at: "2026-05-12T00:00:00Z".to_string(),
            updated_at: "2026-05-12T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn bucket_grain_switches_from_hour_to_day_above_three_days() {
        assert_eq!(bucket_grain(24 * 60 * 60), "hour"); // 1d
        assert_eq!(bucket_grain(3 * 24 * 60 * 60), "hour"); // 3d (inclusive)
        assert_eq!(bucket_grain(3 * 24 * 60 * 60 + 1), "day"); // just over 3d
        assert_eq!(bucket_grain(7 * 24 * 60 * 60), "day"); // 7d
        assert_eq!(bucket_grain(30 * 24 * 60 * 60), "day"); // 30d
    }

    #[test]
    fn open_url_allows_only_http_https_cursor_and_rejects_flag_injection() {
        // Allowlisted schemes (case-insensitive).
        assert!(validate_open_url("https://cursor.com").is_ok());
        assert!(validate_open_url("HTTP://localhost:9618/x").is_ok());
        assert!(validate_open_url("cursor://file/Users/me/script.py").is_ok());

        // Blocked schemes — the renderer allowlist is bypassable, so these
        // must be rejected server-side.
        for blocked in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "ftp://host/x",
            "vscode://anything",
            "data:text/html,x",
        ] {
            assert!(
                validate_open_url(blocked).is_err(),
                "expected {blocked} to be blocked"
            );
        }

        // Flag-injection and malformed inputs.
        assert!(validate_open_url("-e").is_err());
        assert!(validate_open_url("--reveal").is_err());
        assert!(validate_open_url("").is_err());
        assert!(validate_open_url("   ").is_err());
        assert!(validate_open_url("not-a-url").is_err());
    }

    #[test]
    fn resolve_local_script_confines_under_workspace_root() {
        let root = std::env::temp_dir().join(format!("chaos-cursor-open-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("scripts")).unwrap();
        std::fs::write(root.join("scripts/ok.py"), b"# noop\n").unwrap();
        let root_str = root.to_str().unwrap();

        // A confined, existing script resolves.
        let resolved = resolve_local_script(root_str, "scripts/ok.py").expect("should resolve");
        assert!(resolved.ends_with("scripts/ok.py"));

        // Traversal / absolute paths escaping the root are rejected — this
        // reuses the shared canonicalizing, symlink-safe confinement, NOT a
        // weaker lexical `..` check.
        assert!(resolve_local_script(root_str, "../../../../etc/passwd").is_none());
        assert!(resolve_local_script(root_str, "/etc/passwd").is_none());

        // A confined but non-existent path (e.g. a cloud-only workflow) → None.
        assert!(resolve_local_script(root_str, "scripts/missing.py").is_none());
        // Empty script path → None.
        assert!(resolve_local_script(root_str, "  ").is_none());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn build_cursor_file_link_uses_guarded_cursor_scheme() {
        let link = build_cursor_file_link(std::path::Path::new("/Users/me/project/x.py"));
        assert_eq!(link, "cursor://file/Users/me/project/x.py");
        // The generated link must pass the server-side open_url scheme guard.
        assert!(validate_open_url(&link).is_ok());
    }

    #[test]
    fn run_is_failed_classifies_terminal_states() {
        let base = Run {
            id: "r".into(),
            workflow_id: "w".into(),
            started_at: "2026-07-12T00:00:00Z".into(),
            finished_at: None,
            exit_code: None,
            stdout: None,
            stderr: None,
            result_url: None,
            status: "running".into(),
            workflow_name: None,
            summary: None,
            error_analysis: None,
            trigger_kind: None,
            trigger_payload: None,
            upstream_run_id: None,
            input_json: None,
            rerun_of_run_id: None,
        };
        assert!(!run_is_failed(&base)); // running, no exit code
        assert!(run_is_failed(&Run {
            status: "failed".into(),
            ..base.clone()
        }));
        assert!(run_is_failed(&Run {
            status: "poll_exhausted".into(),
            ..base.clone()
        }));
        assert!(run_is_failed(&Run {
            exit_code: Some(1),
            ..base.clone()
        }));
        assert!(!run_is_failed(&Run {
            status: "success".into(),
            exit_code: Some(0),
            ..base.clone()
        }));
    }

    #[test]
    fn parse_lookback_maps_grammar_to_modifier_and_seconds() {
        assert_eq!(
            parse_lookback(Some("1d")).unwrap(),
            ("-1 days".to_string(), 86_400)
        );
        assert_eq!(
            parse_lookback(Some("3d")).unwrap(),
            ("-3 days".to_string(), 3 * 86_400)
        );
        assert_eq!(
            parse_lookback(Some("12h")).unwrap(),
            ("-12 hours".to_string(), 12 * 3_600)
        );
        assert_eq!(
            parse_lookback(None).unwrap(),
            ("-1 days".to_string(), 86_400)
        );
        assert!(parse_lookback(Some("0d")).is_err());
        assert!(parse_lookback(Some("bogus")).is_err());
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
}

#[tauri::command]
pub fn list_queues(state: State<AppState>) -> Result<Vec<QueueInfo>, String> {
    state.db.list_queues().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_queue(
    state: State<AppState>,
    name: String,
    environment: String,
    capacity: i64,
    tag_cap: Option<i64>,
    max_queued: Option<i64>,
) -> Result<QueueInfo, String> {
    state
        .service
        .ensure_environment_target_writable(&environment, "update queue")
        .map_err(|e| e.to_string())?;
    state
        .db
        .upsert_queue(&name, &environment, capacity, tag_cap, max_queued)
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

/// Extract and lowercase the URL scheme (the substring before the first `:`),
/// validating it against the RFC 3986 scheme grammar
/// (`ALPHA *( ALPHA / DIGIT / "+" / "-" / "." )`). Returns `None` for a missing
/// or malformed scheme. Dependency-free (no `url` crate), mirroring the manual
/// parsing style in `operators.rs`.
fn open_url_scheme(url: &str) -> Option<String> {
    let idx = url.find(':')?;
    let scheme = &url[..idx];
    let mut chars = scheme.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.') {
        return None;
    }
    Some(scheme.to_ascii_lowercase())
}

/// Server-side scheme allowlist for [`open_url`]. Allows ONLY `http`, `https`,
/// and `cursor`, and rejects a leading `-` (flag injection into `open`). The
/// renderer-side allowlist (`src/lib/openExternalSafe.ts`) is UX-only and
/// bypassable, so this is the authoritative gate.
fn validate_open_url(url: &str) -> Result<(), String> {
    if url.trim().is_empty() {
        return Err("open_url: URL must not be empty".into());
    }
    // A leading '-' would be parsed by `open` as an option even before the
    // positional `--` separator is reached during arg construction.
    if url.starts_with('-') {
        return Err("open_url: URL must not begin with '-'".into());
    }
    match open_url_scheme(url).as_deref() {
        Some("http") | Some("https") | Some("cursor") => Ok(()),
        Some(other) => Err(format!("open_url: blocked URL scheme '{other}'")),
        None => Err("open_url: URL has no valid scheme".into()),
    }
}

#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    validate_open_url(&url)?;
    // `--` stops `open` from interpreting the URL as an option flag (belt and
    // suspenders on top of the leading-'-' rejection above).
    std::process::Command::new("open")
        .arg("--")
        .arg(&url)
        .spawn()
        .map_err(|e| format!("Failed to open URL: {}", e))?;
    Ok(())
}

/// Build a Cursor "open file" deep link for an absolute local path. Cursor
/// (VS Code-derived) uses `cursor://file/{absolute-path}` where the URL
/// authority is exactly `file`; since `abs` is a confined absolute path (leading
/// `/`), concatenation yields e.g. `cursor://file/Users/me/x.py`.
fn build_cursor_file_link(abs: &std::path::Path) -> String {
    format!("cursor://file{}", abs.to_string_lossy())
}

/// Whether the Cursor desktop app appears installed (macOS), so the UI can
/// hide/disable "Open in Cursor" when the `cursor://` handler is absent.
fn cursor_installed() -> bool {
    if std::path::Path::new("/Applications/Cursor.app").exists() {
        return true;
    }
    if let Ok(home) = std::env::var("HOME") {
        if std::path::Path::new(&format!("{home}/Applications/Cursor.app")).exists() {
            return true;
        }
    }
    false
}

/// Resolve a workflow's `script_path` to a confined, existing local path under
/// `workspace_root`. Returns `None` when the path escapes the root (workflow
/// rows are author-supplied via MCP/API, so the confinement base MUST be the
/// app-config `workspace_root`, never a per-workflow value) or when no local
/// file/dir exists (e.g. a cloud-only operator with a placeholder script_path).
fn resolve_local_script(workspace_root: &str, script_path: &str) -> Option<std::path::PathBuf> {
    if script_path.trim().is_empty() {
        return None;
    }
    let confined = crate::operators::confine_path_under_root(workspace_root, script_path).ok()?;
    confined.exists().then_some(confined)
}

/// Whether a run ended in a non-OK terminal state (eligible for a fix agent).
fn run_is_failed(run: &Run) -> bool {
    matches!(
        run.status.as_str(),
        "failed" | "poll_exhausted" | "timed_out" | "error" | "cancelled"
    ) || run.exit_code.is_some_and(|code| code != 0)
}

/// Open the failed run's local script/repo location in the Cursor desktop app
/// via a `cursor://` deep link. Side-effect-free (no spend); the path is
/// confined under the app-configured `workspace_root` and the URL is routed
/// through the same scheme-guarded `open` path as every other external link.
#[tauri::command]
pub fn open_run_in_cursor(state: State<AppState>, run_id: String) -> Result<(), String> {
    let run = state.db.get_run(&run_id).map_err(|e| e.to_string())?;
    let workflow = state
        .db
        .get_workflow(&run.workflow_id)
        .map_err(|e| e.to_string())?;
    let path = resolve_local_script(&state.workspace_root, &workflow.script_path)
        .ok_or_else(|| "open_run_in_cursor: no local script for this run".to_string())?;
    open_url(build_cursor_file_link(&path))
}

/// Per-run eligibility for the Cursor integration affordances. Open-in-Cursor is
/// decoupled from the spend/`enabled` gate (it is safe and side-effect-free);
/// fix-agent dispatch requires the feature enabled, a designated fix workflow,
/// and a failed source run.
#[tauri::command]
pub fn get_cursor_integration_status(
    state: State<AppState>,
    run_id: String,
) -> Result<serde_json::Value, String> {
    let run = state.db.get_run(&run_id).map_err(|e| e.to_string())?;
    let workflow = state
        .db
        .get_workflow(&run.workflow_id)
        .map_err(|e| e.to_string())?;
    let prefs = state
        .db
        .get_cursor_integration_prefs()
        .map_err(|e| e.to_string())?;
    let installed = cursor_installed();
    let open_in_cursor_available =
        installed && resolve_local_script(&state.workspace_root, &workflow.script_path).is_some();
    let dispatch_fix_available =
        prefs.enabled && prefs.fix_workflow_id.is_some() && run_is_failed(&run);
    Ok(serde_json::json!({
        "open_in_cursor_available": open_in_cursor_available,
        "dispatch_fix_available": dispatch_fix_available,
        "cursor_installed": installed,
    }))
}

#[tauri::command]
pub fn quit_app(app: tauri::AppHandle) -> Result<(), String> {
    // The `RunEvent::ExitRequested` handler in `run()` owns the shutdown sequence
    // (re-entrancy guard, SHUTDOWN signal, off-main-thread grace exit). Every quit
    // path routes through `exit()`, so this stays a thin trigger.
    app.exit(0);
    Ok(())
}

#[tauri::command]
pub fn get_launch_at_login() -> Result<bool, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    let plist_path = format!(
        "{}/Library/LaunchAgents/{}.plist",
        home,
        scheduler::SCHEDULER_BUNDLE_ID
    );
    Ok(std::path::Path::new(&plist_path).exists())
}

#[tauri::command]
pub fn set_launch_at_login(enabled: bool) -> Result<String, String> {
    if enabled {
        scheduler::install_launchd_plist(scheduler::CANONICAL_EXECUTABLE_PATH)
    } else {
        scheduler::uninstall_launchd_plist()?;
        Ok("Removed".to_string())
    }
}

#[tauri::command]
pub fn list_available_scripts(state: State<AppState>) -> Result<Vec<AvailableScript>, String> {
    let root = &state.workspace_root;
    let workflows_dir = std::path::Path::new(root).join("scripts").join("workflows");

    let mut scripts = Vec::new();

    if workflows_dir.exists() {
        collect_scripts(&workflows_dir, root, &mut scripts)?;
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

/// Read the Cursor integration preferences (D05 / F10). SAFE BY DEFAULT:
/// disabled, no designated fix-agent workflow, low per-hour dispatch ceiling.
#[tauri::command]
pub fn get_cursor_integration_prefs(
    state: State<AppState>,
) -> Result<crate::db::CursorIntegrationPrefs, String> {
    state
        .db
        .get_cursor_integration_prefs()
        .map_err(|e| e.to_string())
}

/// Persist the Cursor integration preferences and return the canonical stored
/// state (the per-hour ceiling is floored at 1). The fix-agent dispatch path
/// stays gated on `enabled` (default off) elsewhere; this only stores prefs.
#[tauri::command]
pub fn set_cursor_integration_prefs(
    state: State<AppState>,
    enabled: bool,
    fix_workflow_id: Option<String>,
    max_dispatches_per_hour: u32,
) -> Result<crate::db::CursorIntegrationPrefs, String> {
    state
        .db
        .set_cursor_integration_prefs(enabled, fix_workflow_id.as_deref(), max_dispatches_per_hour)
        .map_err(|e| e.to_string())
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

    let root = &state.workspace_root;
    let python_path = &state.python_path;
    let script_path = format!("{}/scripts/analyze_error.py", root);

    let context = serde_json::json!({
        "workflow_name": workflow.name,
        "script_path": workflow.script_path,
        "exit_code": run.exit_code,
        "stderr": run.stderr,
        "stdout": run.stdout,
    });

    let output = std::process::Command::new(python_path)
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
    let root = &state.workspace_root;
    let python_path = &state.python_path;
    let analysis_script = format!("{}/scripts/analyze_workflow.py", root);

    if !std::path::Path::new(&analysis_script).exists() {
        return Err("analyze_workflow.py not found — run deploy.py to sync scripts".to_string());
    }

    let context = serde_json::json!({
        "script_path": script_path,
        "chaos_labs_root": root,
    });

    let output = std::process::Command::new(python_path)
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

    send_email_alert(&config, None, "test")
}

#[tauri::command]
pub fn list_email_profiles(state: State<AppState>) -> Result<Vec<EmailProfile>, String> {
    state
        .service
        .list_email_profiles()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_email_profile(
    state: State<AppState>,
    profile: EmailProfile,
) -> Result<EmailProfile, String> {
    state
        .service
        .save_email_profile(profile)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_email_profile(state: State<AppState>, id: String) -> Result<(), String> {
    state
        .service
        .delete_email_profile(&id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn test_email_profile(state: State<AppState>, id: String) -> Result<serde_json::Value, String> {
    let profile = state.db.get_email_profile(&id).map_err(|e| e.to_string())?;
    let config = profile.to_email_config();
    if config.alert_email.is_empty() || config.smtp_host.is_empty() {
        return Err("Email profile is incomplete".to_string());
    }
    send_email_alert(&config, None, "test")
}

#[tauri::command]
pub fn set_workflow_email_profile(
    state: State<AppState>,
    workflow_id: String,
    profile_id: Option<String>,
) -> Result<(), String> {
    state
        .service
        .set_workflow_email_profile(&workflow_id, profile_id.as_deref())
        .map_err(|e| e.to_string())
}

/// Send scheduler email natively via `lettre` (replaces the former
/// `email_alert.py` subprocess). Returns the `{success, error?}` JSON shape the
/// callers/UI expect. Used by both `test_email_config` and `send_failure_email`.
pub fn send_email_alert(
    config: &EmailConfig,
    run_context: Option<&serde_json::Value>,
    mode: &str,
) -> Result<serde_json::Value, String> {
    let (subject, body) = match mode {
        "test" => (
            "Chaos Scheduler — test email".to_string(),
            "This is a test message from Chaos Scheduler. Your SMTP configuration works."
                .to_string(),
        ),
        _ => match run_context {
            Some(ctx) => crate::email::compose_failure_alert(ctx),
            None => (
                "Chaos Scheduler alert".to_string(),
                "A workflow reported a failure.".to_string(),
            ),
        },
    };

    match crate::email::send_email(config, &config.alert_email, &subject, &body) {
        Ok(()) => Ok(serde_json::json!({ "success": true })),
        Err(e) => Ok(serde_json::json!({ "success": false, "error": e })),
    }
}

#[cfg(test)]
mod rerun_admission_tests {
    use super::*;
    use crate::service::{NoopNotifier, SchedulerService};

    fn service_and_db() -> (SchedulerService, Arc<Database>, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("chaos-rerun-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Arc::new(Database::new(&dir));
        // No protected environments so the gate admits `production`.
        let service = SchedulerService::with_protection_config(
            db.clone(),
            Arc::new(NoopNotifier),
            vec![],
            false,
        );
        (service, db, dir)
    }

    fn seed_dep_gated(db: &Arc<Database>) -> String {
        db.create_workflow(
            "Rerun Gated",
            None,
            "scripts/noop.py",
            "0 0 * * *",
            false,
            false,
            "UTC",
            "production",
            None,
            None,
            Some(r#"{"queue":"production-default","depends_on":["upstream-never"]}"#),
        )
        .unwrap()
        .id
    }

    #[test]
    fn rerun_queues_through_admission_when_dependency_is_unmet() {
        let (service, db, dir) = service_and_db();
        let id = seed_dep_gated(&db);

        let outcome = dispatch_rerun(
            &service,
            dir.to_str().unwrap(),
            "python3",
            id,
            Some("run-old".to_string()),
            None,
            None,
        )
        .expect("rerun should dispatch");

        // The dependency-gated rerun must admission-QUEUE instead of spawning
        // immediately (the pre-change command called execute_workflow_with_context,
        // bypassing the gate).
        assert_eq!(outcome.status, "queued");
        assert!(outcome.queued_run_id.is_some());
        assert!(outcome.run_id.is_none());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rerun_reused_idempotency_key_is_duplicate_without_redispatch() {
        let (service, db, dir) = service_and_db();
        let id = seed_dep_gated(&db);

        let first = dispatch_rerun(
            &service,
            dir.to_str().unwrap(),
            "python3",
            id.clone(),
            Some("run-old".to_string()),
            None,
            Some("rerun-key".to_string()),
        )
        .expect("first rerun should dispatch");
        assert_eq!(first.status, "queued");

        let second = dispatch_rerun(
            &service,
            dir.to_str().unwrap(),
            "python3",
            id,
            Some("run-old".to_string()),
            None,
            Some("rerun-key".to_string()),
        )
        .expect("second rerun should replay");
        assert_eq!(second.status, "duplicate");
        assert_eq!(second.queued_run_id, first.queued_run_id);
        assert!(second.run_id.is_none());

        // The reused key must not enqueue a second row.
        assert_eq!(db.list_queued_runs(10).unwrap().len(), 1);

        let _ = std::fs::remove_dir_all(dir);
    }
}
