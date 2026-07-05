//! Secure `/api/v1` HTTP surface (axum).
//!
//! Binds loopback by default, authenticates with hashed, scoped API keys,
//! records authenticated audit events, applies request-body and rate limits,
//! and reuses [`SchedulerService`] for **all** governance/validation so
//! there is no duplicated business logic vs the Tauri commands.

use crate::db::{Database, IdempotencyReservation};
use crate::scheduler::{dispatch_non_cron_workflow, NonCronDispatchOptions};
use crate::service::{ApiIdentity, SchedulerService, ServiceError, WorkflowDraft};
use crate::workflow_spec::WorkflowSpec;
use axum::{
    extract::{Path, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{hash_map::DefaultHasher, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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

async fn request_guard(
    State(state): State<ApiState>,
    req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    if !host_allowed(&state, req.headers()) {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "host not allowed"));
    }
    if !origin_allowed(&state, req.headers()) {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "origin not allowed"));
    }
    Ok(next.run(req).await)
}

/// Authenticate + authorize a request, enforce rate limits, and audit the
/// outcome. Returns the identity on success.
fn authorize(
    state: &ApiState,
    headers: &HeaderMap,
    required_scope: &str,
    method: &str,
    path: &str,
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
    if !identity.has_scope(required_scope) {
        let _ = state
            .db
            .record_api_audit(Some(&identity.id), method, path, 403, None);
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            format!("API key lacks required scope '{required_scope}'"),
        ));
    }
    if let Ok(mut limiter) = state.rate.lock() {
        if !limiter.allow(&identity.id) {
            let _ = state
                .db
                .record_api_audit(Some(&identity.id), method, path, 429, None);
            return Err(ApiError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "rate limit exceeded",
            ));
        }
    }
    let _ = state
        .db
        .record_api_audit(Some(&identity.id), method, path, 200, None);
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
            get(get_workflow).delete(delete_workflow),
        )
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
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    authorize(&st, &headers, "read", "GET", "/api/v1/environments")?;
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
    headers: HeaderMap,
    Json(body): Json<CreateEnvironmentBody>,
) -> Result<Json<Value>, ApiError> {
    authorize(&st, &headers, "write", "POST", "/api/v1/environments")?;
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
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    authorize(&st, &headers, "read", "GET", "/api/v1/workflows")?;
    let workflows = st.service.list_workflows()?;
    Ok(Json(json!({ "workflows": workflows })))
}

async fn get_workflow(
    State(st): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    authorize(&st, &headers, "read", "GET", "/api/v1/workflows/{id}")?;
    let wf = st.service.get_workflow(&id)?;
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
    headers: HeaderMap,
    Json(body): Json<RegisterWorkflowBody>,
) -> Result<Json<Value>, ApiError> {
    authorize(&st, &headers, "write", "POST", "/api/v1/workflows")?;
    let environment = body.environment.unwrap_or_else(|| "instance".to_string());
    let draft = WorkflowDraft {
        name: body.name,
        description: body.description,
        script_path: body.script_path,
        cron_schedule: body.cron_schedule,
        async_mode: body.async_mode.unwrap_or(false),
        email_on_failure: body.email_on_failure.unwrap_or(true),
        timezone: body.timezone.unwrap_or_else(|| "UTC".to_string()),
        corpus: environment.clone(),
        environment: Some(environment),
        domain: body.domain,
        trigger_config: body.trigger_config,
        queue_config: body.queue_config,
    };
    // API-registered workflows are externally-managed by definition.
    let mut wf = st.service.create_workflow(draft, true)?;
    if let Some(spec) = body.spec {
        wf = st.service.set_workflow_spec(&wf.id, &spec, true)?;
    }
    Ok(Json(json!({ "workflow": wf })))
}

async fn delete_workflow(
    State(st): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    authorize(&st, &headers, "write", "DELETE", "/api/v1/workflows/{id}")?;
    st.service.delete_workflow(&id, true)?;
    Ok(Json(json!({ "deleted": id })))
}

async fn set_spec(
    State(st): State<ApiState>,
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
    )?;
    let wf = st.service.set_workflow_spec(&id, &spec, true)?;
    Ok(Json(json!({ "workflow": wf })))
}

fn idempotency_fingerprint(workflow_id: &str, trigger_kind: &str, payload: Option<&str>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(workflow_id.as_bytes());
    hasher.update([0]);
    hasher.update(trigger_kind.as_bytes());
    hasher.update([0]);
    hasher.update(payload.unwrap_or_default().as_bytes());
    hex::encode(hasher.finalize())
}

fn duplicate_dispatch_value(record: &crate::db::IdempotencyRecord) -> Value {
    json!({
        "status": "duplicate",
        "run_id": record.run_id.as_deref(),
        "queued_run_id": record.queued_run_id.as_deref(),
    })
}

fn dispatch_with_idempotency(
    st: &ApiState,
    headers: &HeaderMap,
    id: &str,
    trigger_kind: &str,
    payload: Option<&str>,
) -> Result<Json<Value>, ApiError> {
    st.service.ensure_workflow_execution_allowed(id)?;
    let idem = headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let fingerprint = idem
        .as_ref()
        .map(|_| idempotency_fingerprint(id, trigger_kind, payload));
    if let (Some(key), Some(fingerprint)) = (&idem, &fingerprint) {
        match st
            .db
            .reserve_idempotency_key(key, id, fingerprint)
            .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        {
            IdempotencyReservation::Reserved => {}
            IdempotencyReservation::Existing(record) => {
                if let Some(existing) = record.request_fingerprint.as_deref() {
                    if existing != fingerprint.as_str() {
                        return Err(ApiError::new(
                            StatusCode::CONFLICT,
                            "idempotency key was already used for a different request",
                        ));
                    }
                }
                return Ok(Json(duplicate_dispatch_value(&record)));
            }
        }
    }
    let options = NonCronDispatchOptions {
        notify_on_success: false,
        notify_on_failure: true,
        email_on_failure_enabled: false,
        trigger_kind,
        trigger_payload: payload,
        upstream_run_id: None,
        input_json: None,
        rerun_of_run_id: None,
        suppress_completion_triggers: false,
        dedupe: false,
        app_handle: None,
    };
    let outcome = match dispatch_non_cron_workflow(
        &st.db,
        &st.workspace_root,
        &st.python_path,
        id,
        options,
    ) {
        Ok(outcome) => outcome,
        Err(e) => {
            if let (Some(key), Some(fingerprint)) = (&idem, &fingerprint) {
                let _ = st.db.delete_idempotency_reservation(key, fingerprint);
            }
            return Err(ApiError::new(StatusCode::BAD_REQUEST, e));
        }
    };
    if let Some(key) = &idem {
        let _ = st.db.complete_idempotency_key(
            key,
            outcome.run_id.as_deref(),
            outcome.queued_run_id.as_deref(),
            &outcome.status,
        );
    }
    Ok(Json(
        serde_json::to_value(&outcome).unwrap_or_else(|_| json!({})),
    ))
}

async fn run_now(
    State(st): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    authorize(&st, &headers, "write", "POST", "/api/v1/workflows/{id}/run")?;
    dispatch_with_idempotency(&st, &headers, &id, "api_run", None)
}

async fn enqueue(
    State(st): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    authorize(
        &st,
        &headers,
        "write",
        "POST",
        "/api/v1/workflows/{id}/enqueue",
    )?;
    dispatch_with_idempotency(&st, &headers, &id, "api_enqueue", None)
}

/// Inbound signed webhook trigger. Requires write scope; if an
/// `inbound_webhook_secret` is configured, the raw body's HMAC-SHA256 must match
/// the `X-Chaos-Signature: sha256=<hex>` header (replay-protected via the
/// idempotency key when supplied).
async fn inbound_dispatch(
    State(st): State<ApiState>,
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
    )?;
    if let Ok(Some(secret)) = st.db.get_scheduler_config("inbound_webhook_secret") {
        if !secret.trim().is_empty() {
            let provided = headers
                .get("x-chaos-signature")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("sha256="))
                .unwrap_or("");
            let expected = crate::actions::sign_payload(&secret, body.as_bytes());
            if !constant_time_str(provided, &expected) {
                return Err(ApiError::new(
                    StatusCode::UNAUTHORIZED,
                    "invalid webhook signature",
                ));
            }
        }
    }
    let payload = if body.trim().is_empty() {
        None
    } else {
        Some(body.as_str())
    };
    dispatch_with_idempotency(&st, &headers, &id, "webhook", payload)
}

async fn list_runs(
    State(st): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    authorize(&st, &headers, "read", "GET", "/api/v1/workflows/{id}/runs")?;
    let runs = st
        .db
        .get_run_history(&id, 50)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({ "runs": runs })))
}

async fn get_run(
    State(st): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    authorize(&st, &headers, "read", "GET", "/api/v1/runs/{id}")?;
    let run = st
        .db
        .get_run(&id)
        .map_err(|_| ApiError::new(StatusCode::NOT_FOUND, format!("run {id} not found")))?;
    Ok(Json(json!({ "run": run })))
}

async fn get_run_logs(
    State(st): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    authorize(&st, &headers, "read", "GET", "/api/v1/runs/{id}/logs")?;
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
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    authorize(&st, &headers, "read", "GET", "/api/v1/runs/{id}/tasks")?;
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
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    authorize(&st, &headers, "read", "GET", "/api/v1/runs/{id}/metrics")?;
    let metrics = st
        .db
        .get_run_metrics(&id)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({ "metrics": metrics })))
}

async fn list_queues(
    State(st): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    authorize(&st, &headers, "read", "GET", "/api/v1/queues")?;
    let queues = st
        .db
        .list_queues()
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({ "queues": queues })))
}

async fn list_queued_runs(
    State(st): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    authorize(&st, &headers, "read", "GET", "/api/v1/queued-runs")?;
    let queued = st
        .db
        .list_queued_runs(100)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({ "queued_runs": queued })))
}

/// Cursor Cloud Agent completion webhook receiver.
///
/// v1 uses SSE + polling as the primary completion channel (Cursor's v1
/// webhooks are not yet GA); this endpoint is a signature-verifying stub that
/// acknowledges receipt so the integration can be flipped on when available.
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

/// Spawn the API server on its own tokio runtime + thread. Never blocks the
/// caller. Binds `addr` (loopback by default).
pub fn start_api_server(state: ApiState, addr: String) {
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
            if let Err(e) = axum::serve(listener, router(state)).await {
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
    async fn register_workflow_marks_managed_and_lists() {
        let (state, token) = test_state();
        let app = router(state);
        let body = json!({
            "name": "API WF",
            "script_path": "scripts/x.py",
            "cron_schedule": "0 0 * * *",
            "environment": "instance"
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
        assert_eq!(value["workflow"]["environment"], json!("instance"));

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

    #[tokio::test]
    async fn protected_environment_write_endpoints_are_rejected() {
        let (state, token) = test_state();
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
                "prod",
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
                            "environment": "prod"
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
            .any(|q| q["environment"] == json!("source") && q["name"] == json!("source-default")));
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
                "instance",
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
                "instance",
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
                "instance",
                None,
                None,
                Some(r#"{"queue":"instance-default","depends_on":["upstream-never"]}"#),
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
        use crate::actions::{dispatch_actions, sign_payload, ActionContext, ActionSpec};
        use std::io::{Read, Write};
        use std::net::TcpListener;

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
                            "environment": "instance",
                            // Depend on an upstream that never runs so the enqueue
                            // deterministically queues instead of admitting (no
                            // subprocess spawned during the test).
                            "queue_config": "{\"queue\":\"instance-default\",\"depends_on\":[\"upstream-never\"]}"
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

        // 3) Deliver the run result back to the source system via a signed
        //    outbound webhook (the results-feedback contract).
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = vec![0u8; 8192];
            let n = stream.read(&mut buf).unwrap();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
            String::from_utf8_lossy(&buf[..n]).to_string()
        });

        let result_payload = json!({
            "workflow_id": wf_id,
            "run_id": "smoke-run",
            "status": "failed"
        });
        // The signature the receiver must observe (HMAC of the exact bytes sent).
        let expected = sign_payload("hook-secret", &serde_json::to_vec(&result_payload).unwrap());
        let ctx = ActionContext {
            db,
            notifier: Arc::new(NoopNotifier),
            workflow_name: "Smoke WF".into(),
            run_id: "smoke-run".into(),
            success: false,
            result_payload,
        };
        let action = ActionSpec::Webhook {
            url: format!("http://{addr}/results"),
            secret: Some("hook-secret".into()),
            max_retries: 0,
        };
        // dispatch_webhook uses a blocking HTTP client (its own runtime); run it
        // off the async test runtime to avoid a nested-runtime drop panic.
        let results = tokio::task::spawn_blocking(move || {
            dispatch_actions(std::slice::from_ref(&action), &ctx)
        })
        .await
        .unwrap();
        assert_eq!(results.len(), 1);
        assert!(
            results[0].success,
            "result webhook delivery failed: {}",
            results[0].message
        );

        let request = server.join().unwrap();
        let lower = request.to_lowercase();
        assert!(
            lower.contains("x-chaos-signature: sha256="),
            "result webhook must be HMAC-signed"
        );
        assert!(
            request.contains(&expected),
            "delivered signature must match the result body HMAC"
        );
    }
}
