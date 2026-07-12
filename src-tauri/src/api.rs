//! Secure `/api/v1` HTTP surface (axum).
//!
//! Binds loopback by default, authenticates with hashed, scoped API keys,
//! records authenticated audit events, applies request-body and rate limits,
//! and reuses [`SchedulerService`] for **all** governance/validation so
//! there is no duplicated business logic vs the Tauri commands.

use crate::db::{Database, EmailProfile};
use crate::scheduler::DispatchOutcome;
use crate::service::{
    ApiIdentity, ManualDispatchError, SchedulerService, ServiceError, WorkflowDraft,
};
use crate::workflow_spec::WorkflowSpec;
use axum::{
    extract::{ConnectInfo, Extension, Path, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{hash_map::DefaultHasher, HashMap};
use std::hash::{Hash, Hasher};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Shared server state (cheap to clone).
#[derive(Clone)]
pub struct ApiState {
    pub service: SchedulerService,
    pub db: Arc<Database>,
    pub workspace_root: String,
    pub python_path: String,
    pub rate: Arc<Mutex<RateLimiter>>,
    /// Pre-authentication limiter keyed by bearer token hash or anonymous request source.
    pub preauth_rate: Arc<Mutex<RateLimiter>>,
    /// Allowed Host header values in addition to loopback hosts.
    pub host_allowlist: Vec<String>,
    /// Allowed CORS origins. Empty = no cross-origin (same-origin/loopback),
    /// the secure default.
    pub cors_allowlist: Vec<String>,
    /// Recently accepted inbound webhook event IDs for replay protection.
    pub webhook_replays: Arc<Mutex<HashMap<String, Instant>>>,
}

/// Fixed-window per-key rate limiter.
pub struct RateLimiter {
    window: Duration,
    limit: u32,
    window_start: Instant,
    counts: HashMap<String, u32>,
}

impl RateLimiter {
    pub fn new(limit: u32, window: Duration) -> Self {
        Self {
            window,
            limit,
            window_start: Instant::now(),
            counts: HashMap::new(),
        }
    }

    /// Returns true if the caller is within budget for the current window.
    fn allow(&mut self, key: &str) -> bool {
        if self.window_start.elapsed() > self.window {
            self.window_start = Instant::now();
            self.counts.clear();
        }
        let entry = self.counts.entry(key.to_string()).or_insert(0);
        *entry += 1;
        *entry <= self.limit
    }
}

/// Error type that renders as a JSON body with the right status code.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

impl From<ServiceError> for ApiError {
    fn from(err: ServiceError) -> Self {
        let status =
            StatusCode::from_u16(err.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        ApiError::new(status, err.to_string())
    }
}

/// Extract the bearer token from the Authorization header.
fn bearer(headers: &HeaderMap) -> Option<String> {
    let value = headers.get("authorization")?.to_str().ok()?;
    let token = value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))?
        .trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

fn hash_value(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn preauth_rate_key(headers: &HeaderMap) -> String {
    if let Some(token) = bearer(headers) {
        return format!("bearer:{:x}", hash_value(&token));
    }
    if let Some(origin) = headers.get("origin").and_then(|v| v.to_str().ok()) {
        return format!("origin:{}", origin.trim().to_ascii_lowercase());
    }
    if let Some(host) = headers.get("host").and_then(|v| v.to_str().ok()) {
        return format!("host:{}", host.trim().to_ascii_lowercase());
    }
    "anonymous".to_string()
}

fn normalize_host(host: &str) -> String {
    let value = host.trim().to_ascii_lowercase();
    if value.starts_with('[') {
        if let Some(end) = value.find(']') {
            return value[1..end].to_string();
        }
    }
    value.split(':').next().unwrap_or("").to_string()
}

fn is_loopback_host(host: &str) -> bool {
    let host = normalize_host(host);
    host == "localhost"
        || host == "::1"
        || host == "0:0:0:0:0:0:0:1"
        || host == "127.0.0.1"
        || host.starts_with("127.")
}

fn host_allowed(state: &ApiState, headers: &HeaderMap) -> bool {
    let Some(host) = headers.get("host").and_then(|v| v.to_str().ok()) else {
        return true;
    };
    if is_loopback_host(host) {
        return true;
    }
    let host = normalize_host(host);
    state
        .host_allowlist
        .iter()
        .map(|h| normalize_host(h))
        .any(|allowed| allowed == host)
}

fn origin_allowed(state: &ApiState, headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get("origin").and_then(|v| v.to_str().ok()) else {
        return true;
    };
    state.cors_allowlist.iter().any(|allowed| allowed == origin)
}

#[derive(Clone, Default)]
struct AuditTrail(Arc<Mutex<Option<ApiAuditEvent>>>);

struct ApiAuditEvent {
    key_id: String,
    method: String,
    path: String,
}

impl AuditTrail {
    fn remember(&self, key_id: &str, method: &str, path: &str) {
        if let Ok(mut event) = self.0.lock() {
            *event = Some(ApiAuditEvent {
                key_id: key_id.to_string(),
                method: method.to_string(),
                path: path.to_string(),
            });
        }
    }

    fn take(&self) -> Option<ApiAuditEvent> {
        self.0.lock().ok()?.take()
    }
}

async fn request_guard(
    State(state): State<ApiState>,
    mut req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    if !host_allowed(&state, req.headers()) {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "host not allowed"));
    }
    if !origin_allowed(&state, req.headers()) {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "origin not allowed"));
    }
    let remote = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|connect| connect.0.to_string());
    let audit = AuditTrail::default();
    req.extensions_mut().insert(audit.clone());

    let response = next.run(req).await;
    if let Some(event) = audit.take() {
        let _ = state.db.record_api_audit(
            Some(&event.key_id),
            &event.method,
            &event.path,
            response.status().as_u16(),
            remote.as_deref(),
        );
    }

    Ok(response)
}

/// Authenticate + authorize a request, enforce rate limits, and audit the
/// outcome. Returns the identity on success.
fn authorize(
    state: &ApiState,
    headers: &HeaderMap,
    required_scope: &str,
    method: &str,
    path: &str,
    audit: &AuditTrail,
) -> Result<ApiIdentity, ApiError> {
    if let Ok(mut limiter) = state.preauth_rate.lock() {
        if !limiter.allow(&preauth_rate_key(headers)) {
            return Err(ApiError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "pre-auth rate limit exceeded",
            ));
        }
    }
    let Some(token) = bearer(headers) else {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "missing bearer token",
        ));
    };
    let Some(identity) = state.service.verify_api_key(&token) else {
        return Err(ApiError::new(StatusCode::UNAUTHORIZED, "invalid API key"));
    };
    audit.remember(&identity.id, method, path);
    if !identity.has_scope(required_scope) {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            format!("API key lacks required scope '{required_scope}'"),
        ));
    }
    if let Ok(mut limiter) = state.rate.lock() {
        if !limiter.allow(&identity.id) {
            return Err(ApiError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "rate limit exceeded",
            ));
        }
    }
    Ok(identity)
}

/// Build the router.
pub fn router(state: ApiState) -> Router {
    use tower_http::limit::RequestBodyLimitLayer;

    // CORS allowlist (secure default = no cross-origin for a loopback API).
    let cors = if state.cors_allowlist.is_empty() {
        None
    } else {
        let origins: Vec<axum::http::HeaderValue> = state
            .cors_allowlist
            .iter()
            .filter_map(|o| o.parse().ok())
            .collect();
        Some(
            tower_http::cors::CorsLayer::new()
                .allow_origin(origins)
                .allow_methods(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any),
        )
    };

    let router = Router::new()
        .route("/health", get(health))
        .route("/api/v1/health", get(health))
        .route("/api/v1/version", get(version))
        .route(
            "/api/v1/environments",
            get(list_environments).post(create_environment),
        )
        .route(
            "/api/v1/workflows",
            get(list_workflows).post(register_workflow),
        )
        .route(
            "/api/v1/workflows/{id}",
            get(get_workflow)
                .patch(update_workflow)
                .delete(delete_workflow),
        )
        .route("/api/v1/workflows/{id}/rerun", post(rerun_workflow))
        .route("/api/v1/workflows/{id}/spec", post(set_spec))
        .route("/api/v1/workflows/{id}/run", post(run_now))
        .route("/api/v1/workflows/{id}/enqueue", post(enqueue))
        .route("/api/v1/workflows/{id}/dispatch", post(inbound_dispatch))
        .route("/api/v1/workflows/{id}/runs", get(list_runs))
        .route("/api/v1/runs/{id}", get(get_run))
        .route("/api/v1/runs/{id}/logs", get(get_run_logs))
        .route("/api/v1/runs/{id}/tasks", get(get_run_tasks))
        .route("/api/v1/runs/{id}/metrics", get(get_run_metrics))
        .route("/api/v1/queues", get(list_queues))
        .route("/api/v1/queued-runs", get(list_queued_runs))
        .route(
            "/api/v1/email-profiles",
            get(list_email_profiles).post(create_email_profile),
        )
        .route(
            "/api/v1/email-profiles/{id}",
            patch(update_email_profile).delete(delete_email_profile),
        )
        .route(
            "/api/v1/workflows/{id}/email-profile",
            post(set_workflow_email_profile),
        )
        .route("/api/v1/integrations/cursor/webhook", post(cursor_webhook))
        .layer(RequestBodyLimitLayer::new(256 * 1024))
        .layer(middleware::from_fn_with_state(state.clone(), request_guard));

    let router = match cors {
        Some(layer) => router.layer(layer),
        None => router,
    };
    router.with_state(state)
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn version() -> Json<Value> {
    Json(json!({
        "product": crate::branding::PRODUCT_NAME,
        "version": env!("CARGO_PKG_VERSION"),
        "schema_version": crate::db::CURRENT_SCHEMA_VERSION,
        "api": "v1",
    }))
}

async fn list_environments(
    State(st): State<ApiState>,
    Extension(audit): Extension<AuditTrail>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    authorize(&st, &headers, "read", "GET", "/api/v1/environments", &audit)?;
    let envs = st.service.list_environments()?;
    Ok(Json(json!({ "environments": envs })))
}

#[derive(Deserialize)]
struct CreateEnvironmentBody {
    name: String,
    description: Option<String>,
    working_dir: Option<String>,
    default_queue_capacity: Option<i64>,
    default_tag_cap: Option<i64>,
    default_max_queued: Option<i64>,
}

async fn create_environment(
    State(st): State<ApiState>,
    Extension(audit): Extension<AuditTrail>,
    headers: HeaderMap,
    Json(body): Json<CreateEnvironmentBody>,
) -> Result<Json<Value>, ApiError> {
    authorize(
        &st,
        &headers,
        "write",
        "POST",
        "/api/v1/environments",
        &audit,
    )?;
    let env = st.service.create_environment(
        &body.name,
        body.description.as_deref(),
        body.working_dir.as_deref(),
        body.default_queue_capacity,
        body.default_tag_cap,
        body.default_max_queued,
    )?;
    Ok(Json(json!({ "environment": env })))
}

async fn list_workflows(
    State(st): State<ApiState>,
    Extension(audit): Extension<AuditTrail>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let identity = authorize(&st, &headers, "read", "GET", "/api/v1/workflows", &audit)?;
    let workflows = st.service.list_workflows_for_scopes(&identity.scopes)?;
    Ok(Json(json!({ "workflows": workflows })))
}

async fn get_workflow(
    State(st): State<ApiState>,
    Extension(audit): Extension<AuditTrail>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let identity = authorize(
        &st,
        &headers,
        "read",
        "GET",
        "/api/v1/workflows/{id}",
        &audit,
    )?;
    let wf = st.service.get_workflow_for_scopes(&id, &identity.scopes)?;
    Ok(Json(json!({ "workflow": wf })))
}

#[derive(Deserialize)]
struct RegisterWorkflowBody {
    name: String,
    description: Option<String>,
    script_path: String,
    cron_schedule: String,
    #[serde(default)]
    environment: Option<String>,
    #[serde(default)]
    async_mode: Option<bool>,
    #[serde(default)]
    email_on_failure: Option<bool>,
    #[serde(default)]
    timezone: Option<String>,
    #[serde(default)]
    domain: Option<String>,
    #[serde(default)]
    trigger_config: Option<String>,
    #[serde(default)]
    queue_config: Option<String>,
    #[serde(default)]
    spec: Option<WorkflowSpec>,
}

async fn register_workflow(
    State(st): State<ApiState>,
    Extension(audit): Extension<AuditTrail>,
    headers: HeaderMap,
    Json(body): Json<RegisterWorkflowBody>,
) -> Result<Json<Value>, ApiError> {
    authorize(&st, &headers, "write", "POST", "/api/v1/workflows", &audit)?;
    let environment = body.environment.unwrap_or_else(|| "production".to_string());
    let draft = WorkflowDraft {
        name: body.name,
        description: body.description,
        script_path: body.script_path,
        cron_schedule: body.cron_schedule,
        async_mode: body.async_mode.unwrap_or(false),
        email_on_failure: body.email_on_failure.unwrap_or(true),
        timezone: body.timezone.unwrap_or_else(|| "UTC".to_string()),
        environment,
        domain: body.domain,
        trigger_config: body.trigger_config,
        queue_config: body.queue_config,
    };
    // API-registered workflows are externally-managed by definition.
    let mut wf = st.service.create_workflow(draft, true)?;
    if let Some(spec) = body.spec {
        match st.service.set_workflow_spec(&wf.id, &spec, true) {
            Ok(updated) => wf = updated,
            Err(e) => {
                // Roll back the partially-created workflow so an invalid spec
                // never leaves a spec-less workflow behind.
                let _ = st.service.delete_workflow(&wf.id, true);
                return Err(e.into());
            }
        }
    }
    Ok(Json(json!({ "workflow": wf })))
}

async fn delete_workflow(
    State(st): State<ApiState>,
    Extension(audit): Extension<AuditTrail>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    authorize(
        &st,
        &headers,
        "write",
        "DELETE",
        "/api/v1/workflows/{id}",
        &audit,
    )?;
    st.service.delete_workflow(&id, true)?;
    Ok(Json(json!({ "deleted": id })))
}

#[derive(Deserialize)]
struct UpdateWorkflowBody {
    name: Option<String>,
    description: Option<String>,
    script_path: Option<String>,
    cron_schedule: Option<String>,
    enabled: Option<bool>,
    async_mode: Option<bool>,
    email_on_failure: Option<bool>,
    timezone: Option<String>,
    environment: Option<String>,
    domain: Option<String>,
    trigger_config: Option<String>,
    queue_config: Option<String>,
}

async fn update_workflow(
    State(st): State<ApiState>,
    Extension(audit): Extension<AuditTrail>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateWorkflowBody>,
) -> Result<Json<Value>, ApiError> {
    authorize(
        &st,
        &headers,
        "write",
        "PATCH",
        "/api/v1/workflows/{id}",
        &audit,
    )?;
    let existing = st.service.get_workflow(&id)?;
    let environment = body
        .environment
        .clone()
        .unwrap_or_else(|| existing.environment.clone());
    let draft = WorkflowDraft {
        name: body.name.unwrap_or(existing.name),
        description: body.description.or(existing.description),
        script_path: body.script_path.unwrap_or(existing.script_path),
        cron_schedule: body.cron_schedule.unwrap_or(existing.cron_schedule),
        async_mode: body.async_mode.unwrap_or(existing.async_mode),
        email_on_failure: body.email_on_failure.unwrap_or(existing.email_on_failure),
        timezone: body.timezone.unwrap_or(existing.timezone),
        environment,
        domain: body.domain.or(existing.domain),
        trigger_config: body.trigger_config.or(existing.trigger_config),
        queue_config: body.queue_config.or(existing.queue_config),
    };
    let enabled = body.enabled.unwrap_or(existing.enabled);
    let wf = st.service.update_workflow(&id, enabled, draft, true)?;
    Ok(Json(json!({ "workflow": wf })))
}

#[derive(Deserialize)]
struct RerunWorkflowBody {
    source_run_id: Option<String>,
    input_override: Option<serde_json::Value>,
}

async fn rerun_workflow(
    State(st): State<ApiState>,
    Extension(audit): Extension<AuditTrail>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<RerunWorkflowBody>,
) -> Result<Json<Value>, ApiError> {
    authorize(
        &st,
        &headers,
        "write",
        "POST",
        "/api/v1/workflows/{id}/rerun",
        &audit,
    )?;
    let input_json = body.input_override.as_ref().map(|value| value.to_string());
    let payload = json!({
        "source_run_id": body.source_run_id,
        "input_override": body.input_override,
    })
    .to_string();
    dispatch_with_idempotency(
        &st,
        &headers,
        &id,
        "api_rerun",
        Some(payload.as_str()),
        body.source_run_id.as_deref(),
        input_json.as_deref(),
    )
}

async fn set_spec(
    State(st): State<ApiState>,
    Extension(audit): Extension<AuditTrail>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(spec): Json<WorkflowSpec>,
) -> Result<Json<Value>, ApiError> {
    authorize(
        &st,
        &headers,
        "write",
        "POST",
        "/api/v1/workflows/{id}/spec",
        &audit,
    )?;
    let wf = st.service.set_workflow_spec(&id, &spec, true)?;
    Ok(Json(json!({ "workflow": wf })))
}

/// Compact `{status, run_id, queued_run_id}` projection returned for an
/// idempotent replay. This is the exact historical REST shape for a duplicate
/// (narrower than the full `DispatchOutcome` serialized for a fresh dispatch).
fn duplicate_dispatch_value(outcome: &DispatchOutcome) -> Value {
    json!({
        "status": "duplicate",
        "run_id": outcome.run_id.as_deref(),
        "queued_run_id": outcome.queued_run_id.as_deref(),
    })
}

/// Map a free-form dispatch error to an accurate HTTP status. Not-found and
/// governance cases are already handled up-front by
/// `ensure_workflow_execution_allowed`; here we separate client conflicts
/// (disabled / already-existing) and internal persistence failures from plain
/// bad requests.
fn map_dispatch_error(message: &str) -> ApiError {
    let lower = message.to_ascii_lowercase();
    let status = if lower.contains("is disabled") || lower.contains("already") {
        StatusCode::CONFLICT
    } else if lower.starts_with("failed to") {
        StatusCode::INTERNAL_SERVER_ERROR
    } else {
        StatusCode::BAD_REQUEST
    };
    ApiError::new(status, message.to_string())
}

fn dispatch_with_idempotency(
    st: &ApiState,
    headers: &HeaderMap,
    id: &str,
    trigger_kind: &str,
    payload: Option<&str>,
    rerun_of_run_id: Option<&str>,
    input_json: Option<&str>,
) -> Result<Json<Value>, ApiError> {
    let idempotency_key = headers.get("idempotency-key").and_then(|v| v.to_str().ok());
    let outcome = st
        .service
        .dispatch_manual_run(
            &st.workspace_root,
            &st.python_path,
            id,
            trigger_kind,
            idempotency_key,
            payload,
            rerun_of_run_id,
            input_json,
        )
        .map_err(|e| match e {
            // The gate maps by ServiceError status; a free-form admission error
            // keeps its existing HTTP classification (e.g. 409 for disabled /
            // fingerprint-mismatch).
            ManualDispatchError::Gate(se) => ApiError::from(se),
            ManualDispatchError::Dispatch(msg) => map_dispatch_error(&msg),
        })?;
    // Preserve the historical REST wire shape exactly: an idempotent replay
    // returns the compact {status, run_id, queued_run_id} projection, while a
    // fresh dispatch serializes the full DispatchOutcome. `dedupe` is never
    // enabled on this path, so `status == "duplicate"` uniquely identifies a
    // replay.
    if outcome.status == "duplicate" {
        Ok(Json(duplicate_dispatch_value(&outcome)))
    } else {
        Ok(Json(
            serde_json::to_value(&outcome).unwrap_or_else(|_| json!({})),
        ))
    }
}

async fn run_now(
    State(st): State<ApiState>,
    Extension(audit): Extension<AuditTrail>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    authorize(
        &st,
        &headers,
        "write",
        "POST",
        "/api/v1/workflows/{id}/run",
        &audit,
    )?;
    dispatch_with_idempotency(&st, &headers, &id, "api_run", None, None, None)
}

async fn enqueue(
    State(st): State<ApiState>,
    Extension(audit): Extension<AuditTrail>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    authorize(
        &st,
        &headers,
        "write",
        "POST",
        "/api/v1/workflows/{id}/enqueue",
        &audit,
    )?;
    dispatch_with_idempotency(&st, &headers, &id, "api_enqueue", None, None, None)
}

/// Inbound signed webhook trigger. Requires write scope; if an
/// `inbound_webhook_secret` is configured, the request must include
/// `X-Chaos-Timestamp`, `X-Chaos-Event-Id`, and `X-Chaos-Signature`, where the
/// HMAC covers method, concrete path, timestamp, and body hash.
async fn inbound_dispatch(
    State(st): State<ApiState>,
    Extension(audit): Extension<AuditTrail>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: String,
) -> Result<Json<Value>, ApiError> {
    authorize(
        &st,
        &headers,
        "write",
        "POST",
        "/api/v1/workflows/{id}/dispatch",
        &audit,
    )?;
    if let Ok(Some(secret)) = st.db.get_scheduler_config("inbound_webhook_secret") {
        if !secret.trim().is_empty() {
            let path = format!("/api/v1/workflows/{id}/dispatch");
            verify_inbound_webhook(&st, &headers, "POST", &path, body.as_bytes(), &secret)?;
        }
    }
    let payload = if body.trim().is_empty() {
        None
    } else {
        Some(body.as_str())
    };
    dispatch_with_idempotency(&st, &headers, &id, "webhook", payload, None, None)
}

async fn list_runs(
    State(st): State<ApiState>,
    Extension(audit): Extension<AuditTrail>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    authorize(
        &st,
        &headers,
        "read",
        "GET",
        "/api/v1/workflows/{id}/runs",
        &audit,
    )?;
    let runs = st
        .db
        .get_run_history(&id, 50)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({ "runs": runs })))
}

async fn get_run(
    State(st): State<ApiState>,
    Extension(audit): Extension<AuditTrail>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    authorize(&st, &headers, "read", "GET", "/api/v1/runs/{id}", &audit)?;
    let run = st
        .db
        .get_run(&id)
        .map_err(|_| ApiError::new(StatusCode::NOT_FOUND, format!("run {id} not found")))?;
    Ok(Json(json!({ "run": run })))
}

async fn get_run_logs(
    State(st): State<ApiState>,
    Extension(audit): Extension<AuditTrail>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    authorize(
        &st,
        &headers,
        "read",
        "GET",
        "/api/v1/runs/{id}/logs",
        &audit,
    )?;
    let run = st
        .db
        .get_run(&id)
        .map_err(|_| ApiError::new(StatusCode::NOT_FOUND, format!("run {id} not found")))?;
    Ok(Json(json!({
        "run_id": run.id,
        "status": run.status,
        "exit_code": run.exit_code,
        "stdout": run.stdout,
        "stderr": run.stderr,
        "result_url": run.result_url,
    })))
}

async fn get_run_tasks(
    State(st): State<ApiState>,
    Extension(audit): Extension<AuditTrail>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    authorize(
        &st,
        &headers,
        "read",
        "GET",
        "/api/v1/runs/{id}/tasks",
        &audit,
    )?;
    let tasks = st
        .db
        .get_run_tasks(&id)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let attempts = st
        .db
        .get_run_attempts(&id)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({ "tasks": tasks, "attempts": attempts })))
}

async fn get_run_metrics(
    State(st): State<ApiState>,
    Extension(audit): Extension<AuditTrail>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    authorize(
        &st,
        &headers,
        "read",
        "GET",
        "/api/v1/runs/{id}/metrics",
        &audit,
    )?;
    let metrics = st
        .db
        .get_run_metrics(&id)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({ "metrics": metrics })))
}

async fn list_queues(
    State(st): State<ApiState>,
    Extension(audit): Extension<AuditTrail>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    authorize(&st, &headers, "read", "GET", "/api/v1/queues", &audit)?;
    let queues = st
        .db
        .list_queues()
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({ "queues": queues })))
}

async fn list_queued_runs(
    State(st): State<ApiState>,
    Extension(audit): Extension<AuditTrail>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    authorize(&st, &headers, "read", "GET", "/api/v1/queued-runs", &audit)?;
    let queued = st
        .db
        .list_queued_runs(100)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({ "queued_runs": queued })))
}

async fn list_email_profiles(
    State(st): State<ApiState>,
    Extension(audit): Extension<AuditTrail>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    authorize(
        &st,
        &headers,
        "read",
        "GET",
        "/api/v1/email-profiles",
        &audit,
    )?;
    let profiles = st.service.list_email_profiles()?;
    Ok(Json(json!({ "email_profiles": profiles })))
}

async fn create_email_profile(
    State(st): State<ApiState>,
    Extension(audit): Extension<AuditTrail>,
    headers: HeaderMap,
    Json(mut profile): Json<EmailProfile>,
) -> Result<Json<Value>, ApiError> {
    authorize(
        &st,
        &headers,
        "write",
        "POST",
        "/api/v1/email-profiles",
        &audit,
    )?;
    // Ignore any client-supplied id on create; the store assigns one.
    profile.id = String::new();
    let saved = st.service.save_email_profile(profile)?;
    Ok(Json(json!({ "email_profile": saved })))
}

async fn update_email_profile(
    State(st): State<ApiState>,
    Extension(audit): Extension<AuditTrail>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(mut profile): Json<EmailProfile>,
) -> Result<Json<Value>, ApiError> {
    authorize(
        &st,
        &headers,
        "write",
        "PATCH",
        "/api/v1/email-profiles/{id}",
        &audit,
    )?;
    // The path is authoritative for which profile is updated.
    profile.id = id;
    let saved = st.service.save_email_profile(profile)?;
    Ok(Json(json!({ "email_profile": saved })))
}

async fn delete_email_profile(
    State(st): State<ApiState>,
    Extension(audit): Extension<AuditTrail>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    authorize(
        &st,
        &headers,
        "write",
        "DELETE",
        "/api/v1/email-profiles/{id}",
        &audit,
    )?;
    st.service.delete_email_profile(&id)?;
    Ok(Json(json!({ "deleted": id })))
}

#[derive(Deserialize)]
struct SetWorkflowEmailProfileBody {
    #[serde(default)]
    profile_id: Option<String>,
}

async fn set_workflow_email_profile(
    State(st): State<ApiState>,
    Extension(audit): Extension<AuditTrail>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<SetWorkflowEmailProfileBody>,
) -> Result<Json<Value>, ApiError> {
    authorize(
        &st,
        &headers,
        "write",
        "POST",
        "/api/v1/workflows/{id}/email-profile",
        &audit,
    )?;
    st.service
        .set_workflow_email_profile(&id, body.profile_id.as_deref())?;
    Ok(Json(json!({
        "workflow_id": id,
        "email_profile_id": body.profile_id,
    })))
}

/// Cursor Cloud Agent completion webhook receiver.
///
/// v1 uses SSE + polling as the primary completion channel (Cursor's v1
/// webhooks are not yet GA). This endpoint is an **unsigned** acknowledgement stub
/// (it does not verify signatures today) so the route can be wired when GA lands.
async fn cursor_webhook(
    State(_st): State<ApiState>,
    _headers: HeaderMap,
    _body: String,
) -> Response {
    log::info!("Received Cursor completion webhook (v1 uses SSE/polling; receiver is a stub)");
    (
        StatusCode::OK,
        Json(json!({ "status": "accepted", "note": "v1 uses SSE/polling" })),
    )
        .into_response()
}

fn constant_time_str(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

const INBOUND_WEBHOOK_TTL: Duration = Duration::from_secs(5 * 60);

fn unix_timestamp_now() -> Result<i64, ApiError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .map_err(|_| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "system clock before epoch",
            )
        })
}

fn inbound_canonical_payload(method: &str, path: &str, timestamp: &str, body: &[u8]) -> String {
    let mut body_hash = Sha256::new();
    body_hash.update(body);
    format!(
        "{}\n{}\n{}\n{}",
        method.to_ascii_uppercase(),
        path,
        timestamp,
        hex::encode(body_hash.finalize())
    )
}

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(name)?
        .to_str()
        .ok()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn verify_inbound_webhook(
    state: &ApiState,
    headers: &HeaderMap,
    method: &str,
    path: &str,
    body: &[u8],
    secret: &str,
) -> Result<(), ApiError> {
    let timestamp = header_str(headers, "x-chaos-timestamp")
        .ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "missing webhook timestamp"))?;
    let timestamp_seconds = timestamp
        .parse::<i64>()
        .map_err(|_| ApiError::new(StatusCode::UNAUTHORIZED, "invalid webhook timestamp"))?;
    let now = unix_timestamp_now()?;
    if (now - timestamp_seconds).abs() > INBOUND_WEBHOOK_TTL.as_secs() as i64 {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "webhook timestamp outside replay window",
        ));
    }

    let event_id = header_str(headers, "x-chaos-event-id")
        .ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "missing webhook event id"))?;
    if event_id.len() > 160 || event_id.chars().any(|c| c.is_control()) {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid webhook event id",
        ));
    }

    let provided = header_str(headers, "x-chaos-signature")
        .and_then(|value| value.strip_prefix("sha256="))
        .unwrap_or("");
    let canonical = inbound_canonical_payload(method, path, timestamp, body);
    let expected = crate::actions::sign_payload(secret, canonical.as_bytes());
    if !constant_time_str(provided, &expected) {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid webhook signature",
        ));
    }

    let mut cache = state.webhook_replays.lock().map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "webhook replay cache unavailable",
        )
    })?;
    cache.retain(|_, seen_at| seen_at.elapsed() <= INBOUND_WEBHOOK_TTL);
    let replay_key = format!("{path}:{event_id}");
    if cache.contains_key(&replay_key) {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "duplicate webhook event",
        ));
    }
    cache.insert(replay_key, Instant::now());
    Ok(())
}

/// Refuse a non-loopback REST/metrics bind unless the operator explicitly opts
/// in with `CHAOS_SCHEDULER_ALLOW_REMOTE_API=1`. A loopback bind is always fine.
pub fn validate_remote_api_bind(addr: &str) -> Result<(), String> {
    let socket: std::net::SocketAddr = addr
        .parse()
        .map_err(|e| format!("invalid bind address '{addr}': {e}"))?;
    if socket.ip().is_loopback() {
        return Ok(());
    }
    if std::env::var("CHAOS_SCHEDULER_ALLOW_REMOTE_API").as_deref() == Ok("1") {
        return Ok(());
    }
    Err(format!(
        "refusing non-loopback bind '{addr}'; set CHAOS_SCHEDULER_ALLOW_REMOTE_API=1 to opt in"
    ))
}

/// Spawn the API server on its own tokio runtime + thread. Never blocks the
/// caller. Binds `addr` (loopback by default; non-loopback requires the
/// remote-API opt-in flag).
pub fn start_api_server(state: ApiState, addr: String) {
    if let Err(err) = validate_remote_api_bind(&addr) {
        log::error!("{err}");
        return;
    }
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                log::error!("Failed to build API server runtime: {e}");
                return;
            }
        };
        runtime.block_on(async move {
            let listener = match tokio::net::TcpListener::bind(&addr).await {
                Ok(l) => l,
                Err(e) => {
                    log::error!("Failed to bind API server on {addr}: {e}");
                    return;
                }
            };
            log::info!("Chaos Scheduler API listening on {addr}");
            if let Err(e) = axum::serve(
                listener,
                router(state).into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            {
                log::error!("API server error: {e}");
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limiter_enforces_window_budget() {
        let mut rl = RateLimiter::new(2, Duration::from_secs(60));
        assert!(rl.allow("k"));
        assert!(rl.allow("k"));
        assert!(!rl.allow("k"), "third call in window is blocked");
        // A different key has its own budget.
        assert!(rl.allow("other"));
    }

    #[test]
    fn bearer_parsing() {
        let mut h = HeaderMap::new();
        h.insert("authorization", "Bearer abc.def".parse().unwrap());
        assert_eq!(bearer(&h).as_deref(), Some("abc.def"));
    }

    use crate::service::{NoopNotifier, SchedulerService};
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn test_state() -> (ApiState, String) {
        let dir = std::env::temp_dir().join(format!("chaos-api-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Arc::new(Database::new(&dir));
        let service = SchedulerService::new(db.clone(), Arc::new(NoopNotifier));
        let key = service
            .create_api_key(Some("test"), &["read", "write"])
            .unwrap();
        let state = ApiState {
            service,
            db,
            workspace_root: "/tmp".to_string(),
            python_path: "python3".to_string(),
            rate: Arc::new(Mutex::new(RateLimiter::new(1000, Duration::from_secs(60)))),
            preauth_rate: Arc::new(Mutex::new(RateLimiter::new(1000, Duration::from_secs(60)))),
            host_allowlist: vec![],
            cors_allowlist: vec![],
            webhook_replays: Arc::new(Mutex::new(HashMap::new())),
        };
        (state, key.token)
    }

    async fn body_json(resp: Response) -> (StatusCode, Value) {
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, value)
    }

    fn signed_inbound_headers(
        secret: &str,
        method: &str,
        path: &str,
        body: &[u8],
        event_id: &str,
    ) -> HeaderMap {
        let timestamp = unix_timestamp_now().unwrap().to_string();
        let canonical = inbound_canonical_payload(method, path, &timestamp, body);
        let signature = crate::actions::sign_payload(secret, canonical.as_bytes());
        let mut headers = HeaderMap::new();
        headers.insert("x-chaos-timestamp", timestamp.parse().unwrap());
        headers.insert("x-chaos-event-id", event_id.parse().unwrap());
        headers.insert(
            "x-chaos-signature",
            format!("sha256={signature}").parse().unwrap(),
        );
        headers
    }

    #[test]
    fn inbound_webhook_requires_canonical_signature_and_blocks_replay() {
        let (state, _) = test_state();
        let secret = "hook-secret";
        let path = "/api/v1/workflows/wf-1/dispatch";
        let body = br#"{"ok":true}"#;
        let headers = signed_inbound_headers(secret, "POST", path, body, "event-1");
        verify_inbound_webhook(&state, &headers, "POST", path, body, secret).unwrap();

        let replay =
            verify_inbound_webhook(&state, &headers, "POST", path, body, secret).unwrap_err();
        assert_eq!(replay.status, StatusCode::CONFLICT);

        let mut legacy = signed_inbound_headers(secret, "POST", path, body, "event-2");
        legacy.insert(
            "x-chaos-signature",
            format!("sha256={}", crate::actions::sign_payload(secret, body))
                .parse()
                .unwrap(),
        );
        let err = verify_inbound_webhook(&state, &legacy, "POST", path, body, secret).unwrap_err();
        assert_eq!(err.status, StatusCode::UNAUTHORIZED);
    }

    #[derive(serde::Deserialize)]
    struct WebhookVectorsFile {
        inbound: Vec<InboundVector>,
        outbound: Vec<OutboundVector>,
    }

    #[derive(serde::Deserialize)]
    struct InboundVector {
        method: String,
        path: String,
        timestamp: String,
        #[allow(dead_code)]
        event_id: String,
        body: String,
        secret: String,
        signature_hex: String,
    }

    #[derive(serde::Deserialize)]
    struct OutboundVector {
        secret: String,
        body: String,
        signature_hex: String,
    }

    /// Cross-checks the shared cross-language vectors in
    /// `packages/test-fixtures/webhook-vectors.v1.json` (owned by the TS SDK) against the
    /// Rust backend's own signing primitives, proving both sides agree on the exact same
    /// canonical bytes for inbound requests and the same raw-body signature for outbound.
    #[test]
    fn webhook_vectors_match_sdk_fixtures() {
        const RAW: &str = include_str!("../../packages/test-fixtures/webhook-vectors.v1.json");
        let vectors: WebhookVectorsFile = serde_json::from_str(RAW).expect("parse vectors");
        for outbound in &vectors.outbound {
            let sig = crate::actions::sign_payload(&outbound.secret, outbound.body.as_bytes());
            assert_eq!(sig, outbound.signature_hex);
        }
        for inbound in &vectors.inbound {
            let canonical = inbound_canonical_payload(
                &inbound.method,
                &inbound.path,
                &inbound.timestamp,
                inbound.body.as_bytes(),
            );
            let sig = crate::actions::sign_payload(&inbound.secret, canonical.as_bytes());
            assert_eq!(sig, inbound.signature_hex);
        }
    }

    #[test]
    fn validate_remote_api_bind_allows_loopback_and_blocks_remote_without_flag() {
        std::env::remove_var("CHAOS_SCHEDULER_ALLOW_REMOTE_API");
        assert!(validate_remote_api_bind("127.0.0.1:9618").is_ok());
        assert!(validate_remote_api_bind("[::1]:9618").is_ok());
        assert!(validate_remote_api_bind("0.0.0.0:9618").is_err());
        std::env::set_var("CHAOS_SCHEDULER_ALLOW_REMOTE_API", "1");
        assert!(validate_remote_api_bind("0.0.0.0:9618").is_ok());
        std::env::remove_var("CHAOS_SCHEDULER_ALLOW_REMOTE_API");
    }

    /// Seed a workflow whose spec carries a webhook secret, using the service
    /// directly (the IPC/desktop path), so the REST redaction test has data.
    fn seed_workflow_with_secret(state: &ApiState) -> String {
        let draft = WorkflowDraft {
            name: "Secretful".into(),
            description: None,
            script_path: "scripts/noop.py".into(),
            cron_schedule: "0 0 * * *".into(),
            async_mode: false,
            email_on_failure: true,
            timezone: "UTC".into(),
            environment: "production".into(),
            domain: None,
            trigger_config: None,
            queue_config: None,
        };
        let wf = state.service.create_workflow(draft, false).unwrap();
        let spec = WorkflowSpec {
            kind: crate::workflow_spec::WorkflowKind::Generic,
            environment: Some("production".into()),
            generic: Some(crate::workflow_spec::GenericSpec {
                steps: vec![crate::workflow_spec::StepSpec {
                    id: "s1".into(),
                    command: Some("echo hi".into()),
                    script: None,
                    args: vec![],
                    working_dir: None,
                    depends_on: vec![],
                    retry: None,
                    timeout_seconds: None,
                    continue_on_error: false,
                }],
            }),
            typed: None,
            on_success: vec![crate::actions::ActionSpec::Webhook {
                url: "https://example.com/h".into(),
                secret: Some("topsecret".into()),
                max_retries: 0,
            }],
            on_failure: vec![],
        };
        state
            .service
            .set_workflow_spec(&wf.id, &spec, false)
            .unwrap();
        wf.id
    }

    #[tokio::test]
    async fn rest_get_workflow_redacts_secret_for_read_scope_only() {
        let (state, write_token) = test_state();
        let read_token = state
            .service
            .create_api_key(Some("ro"), &["read"])
            .unwrap()
            .token;
        let id = seed_workflow_with_secret(&state);
        let app = router(state);

        // Read-only scope: secret must be redacted and never present in bytes.
        let (status, value) = body_json(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri(format!("/api/v1/workflows/{id}"))
                        .header("authorization", format!("Bearer {read_token}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let read_body = value.to_string();
        assert!(read_body.contains("__redacted__"), "{read_body}");
        assert!(!read_body.contains("topsecret"), "{read_body}");

        // Write scope: round-trip edit flow keeps the real secret.
        let (status, value) = body_json(
            app.oneshot(
                Request::builder()
                    .uri(format!("/api/v1/workflows/{id}"))
                    .header("authorization", format!("Bearer {write_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(value.to_string().contains("topsecret"));
    }

    #[tokio::test]
    async fn rest_email_profile_crud_and_selection_roundtrip() {
        let (state, token) = test_state();
        let db = state.db.clone();
        // A workflow to select the profile onto.
        let wf_id = seed_workflow_with_secret(&state);
        let app = router(state);
        let auth = format!("Bearer {token}");

        let post = |body: serde_json::Value| {
            Request::builder()
                .method("POST")
                .uri("/api/v1/email-profiles")
                .header("authorization", &auth)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap()
        };

        // Create: the real password is stored but never returned.
        let (status, value) = body_json(
            app.clone()
                .oneshot(post(json!({
                    "name": "Primary",
                    "enabled": true,
                    "alert_email": "alerts@example.com",
                    "smtp_host": "smtp.example.com",
                    "smtp_port": 587,
                    "smtp_user": "mailer",
                    "smtp_password": "realpw",
                    "from_address": "from@example.com",
                    "from_name": "Chaos"
                })))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let id = value["email_profile"]["id"].as_str().unwrap().to_string();
        assert_eq!(value["email_profile"]["smtp_password"], "••••••••");
        assert_eq!(db.get_email_profile(&id).unwrap().smtp_password, "realpw");

        // List: masked, never leaks the stored secret.
        let (status, value) = body_json(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/v1/email-profiles")
                        .header("authorization", &auth)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let list_str = value.to_string();
        assert!(!list_str.contains("realpw"), "{list_str}");
        assert!(list_str.contains("••••••••"), "{list_str}");

        let patch = |body: serde_json::Value| {
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/email-profiles/{id}"))
                .header("authorization", &auth)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap()
        };

        // Update echoing the mask keeps the stored password; other fields change.
        let (status, _) = body_json(
            app.clone()
                .oneshot(patch(json!({
                    "name": "Renamed",
                    "enabled": false,
                    "alert_email": "alerts@example.com",
                    "smtp_host": "smtp.example.com",
                    "smtp_port": 587,
                    "smtp_user": "mailer",
                    "smtp_password": "••••••••",
                    "from_address": "from@example.com",
                    "from_name": "Chaos"
                })))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let stored = db.get_email_profile(&id).unwrap();
        assert_eq!(stored.smtp_password, "realpw", "mask echo preserves secret");
        assert_eq!(stored.name, "Renamed");
        assert!(!stored.enabled);

        // Update with a fresh password replaces the stored secret.
        let (status, _) = body_json(
            app.clone()
                .oneshot(patch(json!({
                    "name": "Renamed",
                    "enabled": false,
                    "alert_email": "alerts@example.com",
                    "smtp_host": "smtp.example.com",
                    "smtp_port": 587,
                    "smtp_user": "mailer",
                    "smtp_password": "newpw",
                    "from_address": "from@example.com",
                    "from_name": "Chaos"
                })))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(db.get_email_profile(&id).unwrap().smtp_password, "newpw");

        // Select the profile onto the workflow, then clear it.
        let select = |body: serde_json::Value| {
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/workflows/{wf_id}/email-profile"))
                .header("authorization", &auth)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap()
        };
        let (status, _) = body_json(
            app.clone()
                .oneshot(select(json!({ "profile_id": id })))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            db.get_workflow(&wf_id).unwrap().email_profile_id.as_deref(),
            Some(id.as_str())
        );
        let (status, _) = body_json(
            app.clone()
                .oneshot(select(json!({ "profile_id": null })))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(db.get_workflow(&wf_id).unwrap().email_profile_id.is_none());

        // Delete.
        let (status, _) = body_json(
            app.clone()
                .oneshot(
                    Request::builder()
                        .method("DELETE")
                        .uri(format!("/api/v1/email-profiles/{id}"))
                        .header("authorization", &auth)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(db.get_email_profile(&id).is_err());
    }

    #[tokio::test]
    async fn rest_email_profile_write_requires_write_scope() {
        let (state, _) = test_state();
        let read_token = state
            .service
            .create_api_key(Some("ro"), &["read"])
            .unwrap()
            .token;
        let app = router(state);
        let (status, _) = body_json(
            app.oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/email-profiles")
                    .header("authorization", format!("Bearer {read_token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "X",
                            "enabled": true,
                            "alert_email": "a@e.com",
                            "smtp_host": "h",
                            "smtp_port": 25,
                            "smtp_user": "u",
                            "smtp_password": "p",
                            "from_address": "f@e.com",
                            "from_name": "N"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn health_is_unauthenticated() {
        let (state, _) = test_state();
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn workflows_require_auth() {
        let (state, _) = test_state();
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/workflows")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn rejects_unallowlisted_host_header() {
        let (state, _) = test_state();
        let app = router(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/health")
                    .header("host", "evil.example:9618")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn rejects_unallowlisted_origin_and_allows_configured_origin() {
        let (mut state, _) = test_state();
        state.cors_allowlist = vec!["https://trusted.example".to_string()];
        let app = router(state);

        let blocked = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/health")
                    .header("origin", "https://evil.example")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(blocked.status(), StatusCode::FORBIDDEN);

        let allowed = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/health")
                    .header("origin", "https://trusted.example")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(allowed.status(), StatusCode::OK);
        assert_eq!(
            allowed
                .headers()
                .get("access-control-allow-origin")
                .and_then(|v| v.to_str().ok()),
            Some("https://trusted.example")
        );
    }

    #[tokio::test]
    async fn missing_auth_is_preauth_rate_limited_without_audit_growth() {
        let (state, _) = test_state();
        let audit_db_path = state.db.path().to_string();
        rusqlite::Connection::open(&audit_db_path)
            .unwrap()
            .execute("DELETE FROM api_audit_log", [])
            .unwrap();
        let app = router(ApiState {
            preauth_rate: Arc::new(Mutex::new(RateLimiter::new(2, Duration::from_secs(60)))),
            ..state
        });

        let mut statuses = Vec::new();
        for _ in 0..3 {
            let resp = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/v1/workflows")
                        .header("host", "127.0.0.1:9618")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            statuses.push(resp.status());
        }

        assert_eq!(
            statuses,
            vec![
                StatusCode::UNAUTHORIZED,
                StatusCode::UNAUTHORIZED,
                StatusCode::TOO_MANY_REQUESTS,
            ]
        );
        let audit_count: i64 = rusqlite::Connection::open(&audit_db_path)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM api_audit_log", [], |row| row.get(0))
            .unwrap();
        assert_eq!(audit_count, 0, "unauthenticated 401/429s are not persisted");
    }

    #[tokio::test]
    async fn audit_records_final_status_remote_and_never_body() {
        let (state, token) = test_state();
        let audit_db_path = state.db.path().to_string();
        rusqlite::Connection::open(&audit_db_path)
            .unwrap()
            .execute("DELETE FROM api_audit_log", [])
            .unwrap();
        let app = router(state);
        let mut req = Request::builder()
            .uri("/api/v1/runs/missing?token=secret-token")
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(Body::from(r#"{"password":"super-secret"}"#))
            .unwrap();
        req.extensions_mut().insert(ConnectInfo(
            "203.0.113.7:49152".parse::<SocketAddr>().unwrap(),
        ));

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let row: (String, String, i64, Option<String>) = rusqlite::Connection::open(&audit_db_path)
            .unwrap()
            .query_row(
                "SELECT method, path, status, remote FROM api_audit_log",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(row.0, "GET");
        assert_eq!(row.1, "/api/v1/runs/{id}");
        assert_eq!(row.2, StatusCode::NOT_FOUND.as_u16() as i64);
        assert_eq!(row.3.as_deref(), Some("203.0.113.7:49152"));
        assert!(!row.1.contains("secret-token"));
        assert!(!row.1.contains("super-secret"));
    }

    #[tokio::test]
    async fn register_workflow_marks_managed_and_lists() {
        let (state, token) = test_state();
        let app = router(state);
        let body = json!({
            "name": "API WF",
            "script_path": "scripts/x.py",
            "cron_schedule": "0 0 * * *",
            "environment": "production"
        });
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/workflows")
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, value) = body_json(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(value["workflow"]["managed_externally"], json!(true));
        assert_eq!(value["workflow"]["environment"], json!("production"));

        // And it lists back.
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/workflows")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, value) = body_json(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(value["workflows"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn map_dispatch_error_assigns_accurate_status() {
        assert_eq!(
            map_dispatch_error("Workflow abc is disabled").status,
            StatusCode::CONFLICT
        );
        assert_eq!(
            map_dispatch_error("matching run already exists").status,
            StatusCode::CONFLICT
        );
        assert_eq!(
            map_dispatch_error("Failed to queue workflow: db locked").status,
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            map_dispatch_error("bad trigger payload").status,
            StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn register_workflow_rolls_back_on_invalid_spec() {
        let (state, token) = test_state();
        let db = state.db.clone();
        let app = router(state);
        // A typed spec referencing an unknown operator passes structural
        // validation but fails operator validation inside set_workflow_spec.
        let body = json!({
            "name": "Rollback WF",
            "script_path": "scripts/x.py",
            "cron_schedule": "0 0 * * *",
            "environment": "production",
            "spec": { "kind": "typed", "typed": { "operator_type": "does_not_exist" } }
        });
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/workflows")
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, _value) = body_json(resp).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        // The partially-created workflow must not survive the failed spec set.
        assert!(
            db.list_workflows().unwrap().is_empty(),
            "invalid-spec registration must not leave an orphan workflow"
        );
    }

    #[tokio::test]
    async fn register_workflow_rolls_back_when_spec_validation_fails() {
        let (state, token) = test_state();
        let db = state.db.clone();
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/workflows")
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "Rollback WF",
                            "script_path": "scripts/x.py",
                            "cron_schedule": "0 0 * * *",
                            "environment": "production",
                            "spec": {"kind":"generic", "generic":{"steps":[]}}
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert!(
            db.list_workflows()
                .unwrap()
                .iter()
                .all(|workflow| workflow.name != "Rollback WF"),
            "failed spec registration must not leave a spec-less workflow"
        );
    }

    #[tokio::test]
    async fn protected_environment_write_endpoints_are_rejected() {
        let dir = std::env::temp_dir().join(format!("chaos-api-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Arc::new(Database::new(&dir));
        let service = SchedulerService::with_protection_config(
            db.clone(),
            Arc::new(NoopNotifier),
            vec!["production".into()],
            false,
        );
        let key = service
            .create_api_key(Some("test"), &["read", "write"])
            .unwrap();
        let state = ApiState {
            service,
            db: db.clone(),
            workspace_root: "/tmp".to_string(),
            python_path: "python3".to_string(),
            rate: Arc::new(Mutex::new(RateLimiter::new(1000, Duration::from_secs(60)))),
            preauth_rate: Arc::new(Mutex::new(RateLimiter::new(1000, Duration::from_secs(60)))),
            host_allowlist: vec![],
            cors_allowlist: vec![],
            webhook_replays: Arc::new(Mutex::new(HashMap::new())),
        };
        let token = key.token;
        let wf = state
            .db
            .create_workflow(
                "Prod WF",
                None,
                "scripts/prod.py",
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
        let app = router(state);

        let register = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/workflows")
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "blocked",
                            "script_path": "scripts/x.py",
                            "cron_schedule": "0 0 * * *",
                            "environment": "production"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(register.status(), StatusCode::FORBIDDEN);

        let run = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/workflows/{}/run", wf.id))
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(run.status(), StatusCode::FORBIDDEN);

        let spec = json!({
            "kind": "generic",
            "generic": {
                "steps": [{ "id": "s1", "command": "echo hi" }]
            }
        });
        let set_spec = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/workflows/{}/spec", wf.id))
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(spec.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(set_spec.status(), StatusCode::FORBIDDEN);

        let delete = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/v1/workflows/{}", wf.id))
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(delete.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn queue_reads_require_auth_and_return_seeded_queues() {
        let (state, token) = test_state();
        let app = router(state);
        // Unauthenticated -> 401.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/queues")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // Authenticated (read) -> seeded queues present.
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/queues")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, value) = body_json(resp).await;
        assert_eq!(status, StatusCode::OK);
        let queues = value["queues"].as_array().unwrap();
        assert!(queues
            .iter()
            .any(|q| q["environment"] == json!("production")
                && q["name"] == json!("production-default")));
    }

    #[tokio::test]
    async fn run_read_endpoints_return_logs_tasks_metrics() {
        let (state, token) = test_state();
        // Seed a workflow + finished run with a task directly via the db.
        let wf = state
            .db
            .create_workflow(
                "R",
                None,
                "s.py",
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
        let run = state
            .db
            .create_run_with_context(&wf.id, Some("manual"), None, None, None, None)
            .unwrap();
        state
            .db
            .finish_run(&run.id, 0, "hello-out", "", None)
            .unwrap();
        let attempt = state
            .db
            .insert_run_attempt(&run.id, "step1", 0, "running", None)
            .unwrap();
        state
            .db
            .insert_run_task(&run.id, Some(&attempt), "step1", "success", 0, None)
            .unwrap();
        let run_id = run.id.clone();
        let app = router(state);

        // logs
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/runs/{run_id}/logs"))
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, value) = body_json(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(value["stdout"], json!("hello-out"));

        // tasks
        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/runs/{run_id}/tasks"))
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, value) = body_json(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(value["tasks"].as_array().unwrap().len(), 1);
        assert_eq!(value["tasks"][0]["task_id"], json!("step1"));
    }

    #[tokio::test]
    async fn read_scope_cannot_write() {
        let dir = std::env::temp_dir().join(format!("chaos-api-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Arc::new(Database::new(&dir));
        let service = SchedulerService::new(db.clone(), Arc::new(NoopNotifier));
        let key = service.create_api_key(Some("ro"), &["read"]).unwrap();
        let state = ApiState {
            service,
            db,
            workspace_root: "/tmp".into(),
            python_path: "python3".into(),
            rate: Arc::new(Mutex::new(RateLimiter::new(1000, Duration::from_secs(60)))),
            preauth_rate: Arc::new(Mutex::new(RateLimiter::new(1000, Duration::from_secs(60)))),
            host_allowlist: vec![],
            cors_allowlist: vec![],
            webhook_replays: Arc::new(Mutex::new(HashMap::new())),
        };
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/environments")
                    .header("authorization", format!("Bearer {}", key.token))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"name":"staging"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn patch_workflow_updates_runtime_fields() {
        let (state, token) = test_state();
        let wf = state
            .db
            .create_workflow(
                "Patchable",
                None,
                "s.py",
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
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/v1/workflows/{}", wf.id))
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "enabled": false,
                            "cron_schedule": "0 1 * * *",
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, value) = body_json(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(value["workflow"]["enabled"], false);
        assert_eq!(value["workflow"]["cron_schedule"], "0 1 * * *");
    }

    #[tokio::test]
    async fn rerun_workflow_requires_auth_and_dispatches() {
        let (state, token) = test_state();
        let wf = state
            .db
            .create_workflow(
                "Rerun",
                None,
                "s.py",
                "0 0 * * *",
                true,
                false,
                "UTC",
                "production",
                None,
                None,
                None,
            )
            .unwrap();
        let source = state
            .db
            .create_run_with_context(&wf.id, Some("manual"), None, None, None, None)
            .unwrap();
        state
            .db
            .finish_run(&source.id, 1, "", "failed", None)
            .unwrap();
        let app = router(state);
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/workflows/{}/rerun", wf.id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({ "source_run_id": source.id }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/workflows/{}/rerun", wf.id))
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({ "source_run_id": source.id }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, value) = body_json(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert!(value.get("run_id").is_some() || value.get("queued_run_id").is_some());
    }

    /// A previously-recorded `Idempotency-Key` short-circuits dispatch and
    /// returns the original run without re-dispatching. This is the replay-safety
    /// guarantee external callers depend on for at-least-once delivery.
    #[tokio::test]
    async fn idempotency_key_replay_returns_duplicate_without_redispatch() {
        let (state, token) = test_state();
        let db = state.db.clone();
        // Simulate a prior dispatch: a real workflow + run, keyed by an
        // Idempotency-Key (the FK requires the run to actually exist).
        let wf = db
            .create_workflow(
                "Idem",
                None,
                "s.py",
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
            .create_run_with_context(&wf.id, Some("manual"), None, None, None, None)
            .unwrap();
        db.insert_idempotency_key("idem-1", Some(&run.id), None, None)
            .unwrap();
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/workflows/{}/enqueue", wf.id))
                    .header("authorization", format!("Bearer {token}"))
                    .header("idempotency-key", "idem-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, value) = body_json(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(value["status"], json!("duplicate"));
        assert_eq!(
            value["run_id"],
            json!(run.id),
            "replay must return the original run id, not a new dispatch"
        );
    }

    #[tokio::test]
    async fn idempotency_key_reuse_with_different_fingerprint_conflicts() {
        let (state, token) = test_state();
        let db = state.db.clone();
        let wf = db
            .create_workflow(
                "Idem Conflict",
                None,
                "s.py",
                "0 0 * * *",
                false,
                false,
                "UTC",
                "production",
                None,
                None,
                Some(r#"{"queue":"production-default","depends_on":["upstream-never"]}"#),
            )
            .unwrap();
        let app = router(state);

        let first = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/workflows/{}/enqueue", wf.id))
                    .header("authorization", format!("Bearer {token}"))
                    .header("idempotency-key", "idem-conflict")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(first.status(), StatusCode::OK);

        let second = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/workflows/{}/run", wf.id))
                    .header("authorization", format!("Bearer {token}"))
                    .header("idempotency-key", "idem-conflict")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(second.status(), StatusCode::CONFLICT);

        let _ = std::fs::remove_dir_all(db.path().trim_end_matches("/scheduler.db"));
    }

    /// End-to-end smoke over the external surface: register a workflow via the
    /// REST API, enqueue it on demand, then deliver a signed run-result webhook
    /// back to a source system — the register -> enqueue -> result-webhook loop.
    #[tokio::test]
    async fn register_enqueue_then_result_webhook_smoke() {
        use crate::actions::{dispatch_actions, ActionContext, ActionSpec};

        let (state, token) = test_state();
        let db = state.db.clone();
        let app = router(state);

        // 1) Register a workflow via the API (marked managed_externally).
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/workflows")
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "Smoke WF",
                            "script_path": "scripts/smoke.py",
                            "cron_schedule": "0 0 * * *",
                            "environment": "production",
                            // Depend on an upstream that never runs so the enqueue
                            // deterministically queues instead of admitting (no
                            // subprocess spawned during the test).
                            "queue_config": "{\"queue\":\"production-default\",\"depends_on\":[\"upstream-never\"]}"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, value) = body_json(resp).await;
        assert_eq!(status, StatusCode::OK);
        let wf_id = value["workflow"]["id"].as_str().unwrap().to_string();

        // 2) Enqueue on demand via the API. The reused idempotency key must
        // replay the queued_run_id instead of double-queueing.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/workflows/{wf_id}/enqueue"))
                    .header("authorization", format!("Bearer {token}"))
                    .header("idempotency-key", "smoke-enqueue")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, value) = body_json(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            value["status"], "queued",
            "dependency-gated enqueue should queue: {value}"
        );
        let queued_run_id = value["queued_run_id"].as_str().unwrap().to_string();

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/workflows/{wf_id}/enqueue"))
                    .header("authorization", format!("Bearer {token}"))
                    .header("idempotency-key", "smoke-enqueue")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, value) = body_json(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(value["status"], "duplicate");
        assert_eq!(value["run_id"], Value::Null);
        assert_eq!(
            value["queued_run_id"].as_str(),
            Some(queued_run_id.as_str())
        );

        // 3) Local result-webhook targets are blocked before any network send to
        //    prevent outbound SSRF against loopback/private services.
        let ctx = ActionContext {
            db,
            notifier: Arc::new(NoopNotifier),
            workflow_name: "Smoke WF".into(),
            run_id: "smoke-run".into(),
            success: false,
            result_payload: json!({
                "workflow_id": wf_id,
                "run_id": "smoke-run",
                "status": "failed"
            }),
            email_profile_id: None,
        };
        let action = ActionSpec::Webhook {
            url: "http://127.0.0.1:9/results".into(),
            secret: Some("hook-secret".into()),
            max_retries: 0,
        };
        let results = dispatch_actions(std::slice::from_ref(&action), &ctx);
        assert_eq!(results.len(), 1);
        assert!(!results[0].success);
        assert!(results[0].message.contains("blocked local/private"));
    }
}
