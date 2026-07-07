//! GUI-agnostic scheduler core.
//!
//! [`SchedulerService`] is the single home for business logic, validation, and
//! governance. Tauri IPC commands and the HTTP API are both thin adapters that
//! call the same methods here — there is no duplicated governance across
//! surfaces. The service has no `tauri::AppHandle` dependency; the only
//! GUI-specific concern (desktop notifications) is injected via the [`Notifier`]
//! trait, and time/process side effects are abstracted via [`Clock`] and
//! [`ProcessRunner`] so the core is testable in isolation.

use crate::db::{Database, EmailProfile, Workflow};
use crate::operators::OperatorRegistry;
use crate::workflow_spec::{WorkflowKind, WorkflowSpec};
use chrono::{DateTime, Utc};
use std::process::Output;
use std::sync::Arc;

/// Sentinel that replaces a stored SMTP password whenever a profile leaves the
/// service boundary. Clients echo it back unchanged to keep the existing
/// secret; any other value is treated as a new password.
pub const MASKED_SECRET: &str = "••••••••";

/// Mask a profile's SMTP password for read/return paths. A blank password is
/// left blank so clients can distinguish "no secret set" from "secret hidden".
fn mask_email_profile(mut profile: EmailProfile) -> EmailProfile {
    if !profile.smtp_password.is_empty() {
        profile.smtp_password = MASKED_SECRET.to_string();
    }
    profile
}

/// Desktop notification sink. `DesktopNotifier` bridges to Tauri; tests and
/// headless contexts use `NoopNotifier`.
pub trait Notifier: Send + Sync {
    fn notify(&self, title: &str, body: &str);
}

/// A notifier that drops everything (tests / headless).
#[allow(dead_code)] // Constructed by tests and headless callers.
pub struct NoopNotifier;
impl Notifier for NoopNotifier {
    fn notify(&self, _title: &str, _body: &str) {}
}

/// Injectable clock so time-dependent logic is deterministic under test.
/// Reserved for the core-service execution migration (Phase 1): time-dependent
/// engine logic currently lives in `scheduler.rs` free functions and moves onto
/// the service over time.
#[allow(dead_code)]
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
    fn now_rfc3339(&self) -> String {
        self.now().to_rfc3339()
    }
}

/// Wall-clock implementation.
pub struct SystemClock;
impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Abstraction over spawning workflow/operator child processes, so the
/// step-flow engine and operators can be unit-tested with a fake runner.
pub trait ProcessRunner: Send + Sync {
    /// Run `program` with `args` in `cwd` (if given) with the supplied
    /// environment additions, returning captured output.
    fn run(
        &self,
        program: &str,
        args: &[String],
        cwd: Option<&str>,
        env: &[(String, String)],
    ) -> std::io::Result<Output>;
}

/// Deny-list of the scheduler's OWN secrets to strip from spawned child
/// processes. Deliberately narrow: a broad heuristic (`*_TOKEN`, `*_API_KEY`,
/// `contains("SECRET")`) would strip user credentials that personal scripts
/// legitimately need (`GITHUB_TOKEN`, `ANTHROPIC_API_KEY`, cloud CLI creds). We
/// only remove secrets the scheduler itself owns.
pub(crate) fn should_scrub_child_env_key(key: &str) -> bool {
    if matches!(key, "CURSOR_API_KEY" | "SMTP_PASSWORD") {
        return true;
    }
    if key.starts_with("CHAOS_SCHEDULER_API_") {
        return true;
    }
    key.starts_with("CHAOS_SCHEDULER_") && (key.ends_with("_SECRET") || key.ends_with("_TOKEN"))
}

/// Real process runner backed by `std::process::Command`.
pub struct SystemProcessRunner;
impl ProcessRunner for SystemProcessRunner {
    fn run(
        &self,
        program: &str,
        args: &[String],
        cwd: Option<&str>,
        env: &[(String, String)],
    ) -> std::io::Result<Output> {
        let mut cmd = std::process::Command::new(program);
        cmd.args(args);
        for (key, _) in std::env::vars() {
            if should_scrub_child_env_key(&key) {
                cmd.env_remove(key);
            }
        }
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        for (k, v) in env {
            cmd.env(k, v);
        }
        cmd.output()
    }
}

/// Typed error surface shared by every entry point. Adapters map the variant to
/// their transport: Tauri commands stringify; the HTTP API maps to status codes.
#[derive(Debug, Clone)]
pub enum ServiceError {
    /// Input failed validation (HTTP 400).
    Validation(String),
    /// Blocked by governance policy, e.g. editing an externally-managed
    /// workflow definition (HTTP 403).
    Governance(String),
    /// Entity not found (HTTP 404).
    NotFound(String),
    /// Unexpected internal / persistence failure (HTTP 500).
    Internal(String),
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceError::Validation(m)
            | ServiceError::Governance(m)
            | ServiceError::NotFound(m)
            | ServiceError::Internal(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for ServiceError {}

impl ServiceError {
    /// HTTP status code used by the Phase 6 API adapter.
    pub fn status_code(&self) -> u16 {
        match self {
            ServiceError::Validation(_) => 400,
            ServiceError::Governance(_) => 403,
            ServiceError::NotFound(_) => 404,
            ServiceError::Internal(_) => 500,
        }
    }
}

/// From a rusqlite error to an internal service error.
impl From<rusqlite::Error> for ServiceError {
    fn from(err: rusqlite::Error) -> Self {
        ServiceError::Internal(err.to_string())
    }
}

/// Result alias for service operations.
pub type ServiceResult<T> = Result<T, ServiceError>;

/// The full definition of a workflow as accepted by every registration surface.
#[derive(Debug, Clone)]
pub struct WorkflowDraft {
    pub name: String,
    pub description: Option<String>,
    pub script_path: String,
    pub cron_schedule: String,
    pub async_mode: bool,
    pub email_on_failure: bool,
    pub timezone: String,
    /// First-class environment (authoritative partition). May be any registered
    /// environment name (e.g. `production`, `staging`, `prod`, or a per-org
    /// container). Governance is carried separately by the `managed` flag on
    /// the create/update call, not by the environment name.
    pub environment: String,
    pub domain: Option<String>,
    pub trigger_config: Option<String>,
    pub queue_config: Option<String>,
}

impl WorkflowDraft {
    /// The effective environment (partition) for this draft. Falls back to
    /// `production` when a caller leaves it blank.
    fn effective_environment(&self) -> String {
        let trimmed = self.environment.trim();
        if trimmed.is_empty() {
            "production".to_string()
        } else {
            trimmed.to_string()
        }
    }
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

/// GUI-agnostic core service. Cheap to clone (`Arc` internals).
#[derive(Clone)]
pub struct SchedulerService {
    db: Arc<Database>,
    // Injected for the core-service execution migration (Phase 1); the service
    // owns these dependencies but the engine methods that read them still live
    // in `scheduler.rs` free functions today.
    #[allow(dead_code)]
    notifier: Arc<dyn Notifier>,
    #[allow(dead_code)]
    clock: Arc<dyn Clock>,
    protected_environments: Vec<String>,
    allow_protected_writes: bool,
}

impl SchedulerService {
    pub fn new(db: Arc<Database>, notifier: Arc<dyn Notifier>) -> Self {
        Self::with_protection_config(
            db,
            notifier,
            protected_environments_from_env(),
            protected_writes_allowed_from_env(),
        )
    }

    pub fn with_protection_config(
        db: Arc<Database>,
        notifier: Arc<dyn Notifier>,
        protected_environments: Vec<String>,
        allow_protected_writes: bool,
    ) -> Self {
        Self {
            db,
            notifier,
            clock: Arc::new(SystemClock),
            protected_environments: normalize_environment_names(protected_environments),
            allow_protected_writes,
        }
    }

    /// Construct with an explicit clock (tests).
    #[allow(dead_code)] // Deterministic-clock constructor for the Phase 1 core migration.
    pub fn with_clock(
        db: Arc<Database>,
        notifier: Arc<dyn Notifier>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            db,
            notifier,
            clock,
            protected_environments: protected_environments_from_env(),
            allow_protected_writes: protected_writes_allowed_from_env(),
        }
    }

    #[allow(dead_code)] // Accessor for adapters that will read the shared DB handle.
    pub fn db(&self) -> &Arc<Database> {
        &self.db
    }

    #[allow(dead_code)] // Accessor for the injected desktop notifier (Phase 1 migration).
    pub fn notifier(&self) -> &Arc<dyn Notifier> {
        &self.notifier
    }

    #[allow(dead_code)] // Accessor for the injected clock (Phase 1 migration).
    pub fn clock(&self) -> &Arc<dyn Clock> {
        &self.clock
    }

    /// Whether a workflow's definition is owned by an external source of truth
    /// and therefore read-only in the UI/API. Reads the dedicated
    /// `managed_externally` governance column (decoupled from `corpus`).
    pub fn is_managed_externally(&self, workflow: &Workflow) -> bool {
        workflow.managed_externally
    }

    pub fn is_protected_environment_name(&self, environment: &str) -> bool {
        let normalized = normalize_environment_name(environment);
        !normalized.is_empty()
            && self
                .protected_environments
                .iter()
                .any(|protected| protected == &normalized)
    }

    pub fn ensure_environment_target_writable(
        &self,
        environment: &str,
        action: &str,
    ) -> ServiceResult<()> {
        if self.allow_protected_writes || !self.is_protected_environment_name(environment) {
            return Ok(());
        }
        Err(ServiceError::Governance(format!(
            "environment '{environment}' is protected; refusing to {action}. Set CHAOS_SCHEDULER_ALLOW_PROTECTED_WRITES=1 only for an intentional local-code-execution write"
        )))
    }

    fn ensure_environment_record_writable(
        &self,
        environment: &crate::db::Environment,
        action: &str,
    ) -> ServiceResult<()> {
        if self.allow_protected_writes {
            return Ok(());
        }
        if environment.managed_externally || self.is_protected_environment_name(&environment.name) {
            return Err(ServiceError::Governance(format!(
                "environment '{}' is protected; refusing to {action}. Set CHAOS_SCHEDULER_ALLOW_PROTECTED_WRITES=1 only for an intentional local-code-execution write",
                environment.name
            )));
        }
        Ok(())
    }

    pub fn ensure_workflow_execution_allowed(&self, id: &str) -> ServiceResult<Workflow> {
        let workflow = self.get_workflow(id)?;
        self.ensure_environment_target_writable(&workflow.environment, "execute workflow")?;
        Ok(workflow)
    }

    /// Validate a workflow spec (structure + operator config) without persisting.
    pub fn validate_spec(&self, spec: &WorkflowSpec) -> ServiceResult<()> {
        spec.validate().map_err(ServiceError::Validation)?;
        if spec.kind == WorkflowKind::Typed {
            if let Some(typed) = &spec.typed {
                let registry = OperatorRegistry::with_builtins();
                registry
                    .validate(&typed.operator_type, &typed.config)
                    .map_err(ServiceError::Validation)?;
            }
        }
        Ok(())
    }

    /// Validate and persist a workflow's execution spec (`kind` + `spec_json`).
    pub fn set_workflow_spec(
        &self,
        id: &str,
        spec: &WorkflowSpec,
        allow_managed_edit: bool,
    ) -> ServiceResult<Workflow> {
        self.validate_spec(spec)?;
        let existing = self.get_workflow(id)?;
        self.ensure_environment_target_writable(&existing.environment, "set workflow spec")?;
        if !allow_managed_edit && self.is_managed_externally(&existing) {
            return Err(ServiceError::Governance(
                "externally-managed workflow specs are source-controlled; edit them through their external owner".into(),
            ));
        }
        self.db
            .set_workflow_spec(id, spec.kind.as_str(), Some(&spec.to_json()))
            .map_err(|e| ServiceError::Internal(e.to_string()))?;
        self.get_workflow(id)
    }

    // --- API key management (hashed, scoped) ---

    /// Mint a new API key. The plaintext token is returned exactly once (never
    /// stored); only a salted SHA-256 hash is persisted.
    pub fn create_api_key(&self, name: Option<&str>, scopes: &[&str]) -> ServiceResult<NewApiKey> {
        use rand::RngCore;
        let mut secret_bytes = [0u8; 24];
        rand::rng().fill_bytes(&mut secret_bytes);
        let secret = hex::encode(secret_bytes);
        let mut salt_bytes = [0u8; 16];
        rand::rng().fill_bytes(&mut salt_bytes);
        let salt = hex::encode(salt_bytes);
        let scope_str = normalize_scopes(scopes);
        let key_hash = hash_key(&salt, &secret);
        let id = self
            .db
            .insert_api_key(name, &key_hash, &salt, &scope_str)
            .map_err(|e| ServiceError::Internal(e.to_string()))?;
        Ok(NewApiKey {
            token: format!("{id}.{secret}"),
            id,
            scopes: scope_str,
        })
    }

    /// Verify a presented `id.secret` token in constant time. Returns the
    /// authenticated identity (id + granted scopes) on success.
    pub fn verify_api_key(&self, token: &str) -> Option<ApiIdentity> {
        let (id, secret) = token.split_once('.')?;
        let (hash, salt, scopes) = self.db.get_api_key(id).ok()??;
        let computed = hash_key(&salt, secret);
        if constant_time_eq(computed.as_bytes(), hash.as_bytes()) {
            let _ = self.db.touch_api_key(id);
            Some(ApiIdentity {
                id: id.to_string(),
                scopes: scopes.split(',').map(|s| s.trim().to_string()).collect(),
            })
        } else {
            None
        }
    }

    /// List API key metadata (no secrets).
    pub fn list_api_keys(&self) -> ServiceResult<Vec<crate::db::ApiKeyInfo>> {
        Ok(self.db.list_api_keys()?)
    }

    /// Revoke an API key by id.
    pub fn revoke_api_key(&self, id: &str) -> ServiceResult<()> {
        let affected = self
            .db
            .revoke_api_key(id)
            .map_err(|e| ServiceError::Internal(e.to_string()))?;
        if affected == 0 {
            return Err(ServiceError::NotFound(format!("api key {id} not found")));
        }
        Ok(())
    }

    /// Update a user-managed environment's metadata/caps.
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
    ) -> ServiceResult<crate::db::Environment> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(ServiceError::Validation(
                "environment name is required".into(),
            ));
        }
        let env = self
            .db
            .get_environment(id)
            .map_err(|_| ServiceError::NotFound(format!("environment {id} not found")))?;
        self.ensure_environment_record_writable(&env, "update environment")?;
        self.ensure_environment_target_writable(trimmed, "rename environment")?;
        // Reject renaming onto another environment's name.
        if let Some(existing) = self
            .db
            .get_environment_by_name(trimmed)
            .map_err(|e| ServiceError::Internal(e.to_string()))?
        {
            if existing.id != id {
                return Err(ServiceError::Validation(format!(
                    "environment '{trimmed}' already exists"
                )));
            }
        }
        self.db
            .update_environment(
                id,
                trimmed,
                description,
                working_dir,
                default_queue_capacity,
                default_tag_cap,
                default_max_queued,
            )
            .map_err(|e| ServiceError::Internal(e.to_string()))
    }

    pub fn list_environments(&self) -> ServiceResult<Vec<crate::db::Environment>> {
        Ok(self.db.list_environments()?)
    }

    /// Create a user-managed environment. Names must be unique and non-empty.
    #[allow(clippy::too_many_arguments)]
    pub fn create_environment(
        &self,
        name: &str,
        description: Option<&str>,
        working_dir: Option<&str>,
        default_queue_capacity: Option<i64>,
        default_tag_cap: Option<i64>,
        default_max_queued: Option<i64>,
    ) -> ServiceResult<crate::db::Environment> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(ServiceError::Validation(
                "environment name is required".into(),
            ));
        }
        self.ensure_environment_target_writable(trimmed, "create environment")?;
        if self
            .db
            .get_environment_by_name(trimmed)
            .map_err(|e| ServiceError::Internal(e.to_string()))?
            .is_some()
        {
            return Err(ServiceError::Validation(format!(
                "environment '{trimmed}' already exists"
            )));
        }
        self.db
            .create_environment(
                trimmed,
                description,
                working_dir,
                default_queue_capacity,
                default_tag_cap,
                default_max_queued,
                false,
            )
            .map_err(|e| ServiceError::Internal(e.to_string()))
    }

    /// Delete an environment. Refused if any workflow still references it, to
    /// avoid orphaning partitions.
    pub fn delete_environment(&self, id: &str) -> ServiceResult<()> {
        let env = self
            .db
            .get_environment(id)
            .map_err(|_| ServiceError::NotFound(format!("environment {id} not found")))?;
        self.ensure_environment_record_writable(&env, "delete environment")?;
        let count = self
            .db
            .count_workflows_in_environment(&env.name)
            .map_err(|e| ServiceError::Internal(e.to_string()))?;
        if count > 0 {
            return Err(ServiceError::Validation(format!(
                "environment '{}' still has {count} workflow(s); reassign them first",
                env.name
            )));
        }
        self.db
            .delete_environment(id)
            .map_err(|e| ServiceError::Internal(e.to_string()))
    }

    // --- Email profiles (named, reusable SMTP delivery configs) -------------
    //
    // Business logic (password masking on read + mask-echo restoration on
    // write) lives here so every adapter — Tauri IPC, REST, SDK, MCP — shares
    // one implementation instead of reimplementing the secret handling.

    pub fn list_email_profiles(&self) -> ServiceResult<Vec<EmailProfile>> {
        Ok(self
            .db
            .list_email_profiles()?
            .into_iter()
            .map(mask_email_profile)
            .collect())
    }

    /// Upsert a profile. A profile whose `smtp_password` is the mask sentinel
    /// keeps its previously-stored password (so clients can round-trip a masked
    /// profile without leaking or clobbering the secret). The returned profile
    /// is masked.
    pub fn save_email_profile(&self, mut profile: EmailProfile) -> ServiceResult<EmailProfile> {
        if profile.name.trim().is_empty() {
            return Err(ServiceError::Validation(
                "email profile name is required".into(),
            ));
        }
        if profile.smtp_password == MASKED_SECRET {
            profile.smtp_password = if profile.id.trim().is_empty() {
                String::new()
            } else {
                self.db
                    .get_email_profile(&profile.id)
                    .map(|p| p.smtp_password)
                    .unwrap_or_default()
            };
        }
        let saved = self
            .db
            .upsert_email_profile(&profile)
            .map_err(|e| ServiceError::Internal(e.to_string()))?;
        Ok(mask_email_profile(saved))
    }

    pub fn delete_email_profile(&self, id: &str) -> ServiceResult<()> {
        self.db
            .delete_email_profile(id)
            .map_err(|e| ServiceError::Internal(e.to_string()))
    }

    /// Select (or clear, with `None`/blank) the email profile a workflow uses
    /// for failure alerts.
    pub fn set_workflow_email_profile(
        &self,
        workflow_id: &str,
        profile_id: Option<&str>,
    ) -> ServiceResult<()> {
        let profile_id = profile_id.filter(|s| !s.trim().is_empty());
        self.db
            .set_workflow_email_profile(workflow_id, profile_id)
            .map_err(|e| ServiceError::Internal(e.to_string()))
    }

    pub fn list_workflows(&self) -> ServiceResult<Vec<Workflow>> {
        Ok(self.db.list_workflows()?)
    }

    pub fn get_workflow(&self, id: &str) -> ServiceResult<Workflow> {
        self.db
            .get_workflow(id)
            .map_err(|_| ServiceError::NotFound(format!("workflow {id} not found")))
    }

    /// Stable sentinel substituted for secret fields on read-scoped API/MCP
    /// responses. Distinct from an empty string so callers can tell "redacted"
    /// apart from "unset".
    pub const READ_SCOPE_SECRET_SENTINEL: &str = "__redacted__";

    /// Read scope (and anything narrower) gets secrets redacted; write/admin
    /// scopes keep them so the round-trip edit flow works.
    pub fn workflow_secrets_redacted_for_scopes(scopes: &[String]) -> bool {
        !scopes.iter().any(|s| s == "write" || s == "admin")
    }

    /// Replace secret material inside a workflow's spec/trigger JSON with the
    /// sentinel. Applied in the service layer so REST, MCP tools, and the
    /// `chaos://workflows/{id}` resource all inherit identical redaction.
    pub fn redact_workflow_secrets(mut wf: Workflow) -> Workflow {
        if let Some(spec) = wf.spec_json.as_mut() {
            if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(spec) {
                redact_secret_fields(&mut value);
                *spec = value.to_string();
            }
        }
        if let Some(trigger) = wf.trigger_config.as_mut() {
            if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(trigger) {
                redact_secret_fields(&mut value);
                *trigger = value.to_string();
            }
        }
        wf
    }

    pub fn get_workflow_for_scopes(&self, id: &str, scopes: &[String]) -> ServiceResult<Workflow> {
        let wf = self.get_workflow(id)?;
        if Self::workflow_secrets_redacted_for_scopes(scopes) {
            Ok(Self::redact_workflow_secrets(wf))
        } else {
            Ok(wf)
        }
    }

    pub fn list_workflows_for_scopes(&self, scopes: &[String]) -> ServiceResult<Vec<Workflow>> {
        let workflows = self.list_workflows()?;
        if Self::workflow_secrets_redacted_for_scopes(scopes) {
            Ok(workflows
                .into_iter()
                .map(Self::redact_workflow_secrets)
                .collect())
        } else {
            Ok(workflows)
        }
    }

    /// Shared validation applied to every workflow registration.
    fn validate_draft(&self, draft: &WorkflowDraft) -> ServiceResult<()> {
        if draft.name.trim().is_empty() {
            return Err(ServiceError::Validation("workflow name is required".into()));
        }
        if draft.script_path.trim().is_empty() {
            return Err(ServiceError::Validation(
                "script_path/command is required".into(),
            ));
        }
        if draft.cron_schedule.trim().is_empty() {
            return Err(ServiceError::Validation(
                "cron_schedule is required (use a non-cron trigger config for event workflows)"
                    .into(),
            ));
        }
        if let Some(cfg) = &draft.trigger_config {
            if !cfg.trim().is_empty() {
                serde_json::from_str::<serde_json::Value>(cfg).map_err(|e| {
                    ServiceError::Validation(format!("invalid trigger_config: {e}"))
                })?;
            }
        }
        if let Some(cfg) = &draft.queue_config {
            if !cfg.trim().is_empty() {
                serde_json::from_str::<serde_json::Value>(cfg)
                    .map_err(|e| ServiceError::Validation(format!("invalid queue_config: {e}")))?;
            }
        }
        Ok(())
    }

    /// Create a workflow. `managed` indicates the caller registers an
    /// externally-owned (source-controlled / API-registered) definition; the UI
    /// passes `false` and is blocked from minting managed definitions.
    pub fn create_workflow(&self, draft: WorkflowDraft, managed: bool) -> ServiceResult<Workflow> {
        self.validate_draft(&draft)?;
        let environment = draft.effective_environment();
        self.ensure_environment_target_writable(&environment, "create workflow")?;
        let workflow = self
            .db
            .create_workflow(
                &draft.name,
                draft.description.as_deref(),
                &draft.script_path,
                &draft.cron_schedule,
                draft.async_mode,
                draft.email_on_failure,
                &draft.timezone,
                &environment,
                draft.domain.as_deref(),
                draft.trigger_config.as_deref(),
                draft.queue_config.as_deref(),
            )
            .map_err(|e| ServiceError::Internal(e.to_string()))?;
        let mut needs_reload = false;
        // Governance is decoupled from the environment string: the API path may
        // register a managed workflow in any environment.
        if managed != workflow.managed_externally {
            self.db
                .set_workflow_managed_externally(&workflow.id, managed)
                .map_err(|e| ServiceError::Internal(e.to_string()))?;
            needs_reload = true;
        }
        if needs_reload {
            return self.get_workflow(&workflow.id);
        }
        Ok(workflow)
    }

    /// Whether the requested update mutates a source-controlled definition's
    /// governed fields (everything except enabled/email/timezone runtime prefs).
    fn managed_definition_changed(existing: &Workflow, draft: &WorkflowDraft) -> bool {
        existing.name != draft.name
            || existing.description != normalized_opt(draft.description.as_deref())
            || existing.script_path != draft.script_path
            || existing.cron_schedule != draft.cron_schedule
            || existing.async_mode != draft.async_mode
            || existing.environment != draft.effective_environment()
            || existing.domain != normalized_opt(draft.domain.as_deref())
            || existing.trigger_config != normalized_opt(draft.trigger_config.as_deref())
            || existing.queue_config != normalized_opt(draft.queue_config.as_deref())
    }

    /// Update a workflow. `allow_managed_edit` is set by the API registration
    /// path (which owns the external definition); UI edits pass `false` and may
    /// only touch runtime preferences on managed workflows.
    pub fn update_workflow(
        &self,
        id: &str,
        enabled: bool,
        draft: WorkflowDraft,
        allow_managed_edit: bool,
    ) -> ServiceResult<Workflow> {
        let existing = self.get_workflow(id)?;
        self.validate_draft(&draft)?;
        if !allow_managed_edit
            && self.is_managed_externally(&existing)
            && Self::managed_definition_changed(&existing, &draft)
        {
            return Err(ServiceError::Governance(
                "externally-managed workflow definitions are source-controlled; only enabled, email, and timezone runtime preferences are editable in the Scheduler UI".into(),
            ));
        }
        let environment = draft.effective_environment();
        self.ensure_environment_target_writable(&environment, "update workflow")?;
        let updated = self
            .db
            .update_workflow(
                id,
                &draft.name,
                draft.description.as_deref(),
                &draft.script_path,
                &draft.cron_schedule,
                enabled,
                draft.async_mode,
                draft.email_on_failure,
                &draft.timezone,
                &environment,
                draft.domain.as_deref(),
                draft.trigger_config.as_deref(),
                draft.queue_config.as_deref(),
            )
            .map_err(|e| ServiceError::Internal(e.to_string()))?;
        Ok(updated)
    }

    /// Delete a workflow, honoring governance. `force` is used by the API owner
    /// to deregister an externally-managed workflow.
    pub fn delete_workflow(&self, id: &str, force: bool) -> ServiceResult<()> {
        let existing = self.get_workflow(id)?;
        self.ensure_environment_target_writable(&existing.environment, "delete workflow")?;
        if !force && self.is_managed_externally(&existing) {
            return Err(ServiceError::Governance(
                "externally-managed workflows are source-controlled; remove them from the product registry instead of deleting from the Scheduler UI".into(),
            ));
        }
        self.db
            .delete_workflow(id)
            .map_err(|e| ServiceError::Internal(e.to_string()))
    }
}

/// A freshly-minted API key. `token` is shown to the caller exactly once.
#[derive(Debug, Clone)]
pub struct NewApiKey {
    pub id: String,
    pub token: String,
    pub scopes: String,
}

/// An authenticated API caller.
#[derive(Debug, Clone)]
pub struct ApiIdentity {
    pub id: String,
    pub scopes: Vec<String>,
}

impl ApiIdentity {
    /// Whether this identity holds `required` (or the superuser `admin` scope).
    pub fn has_scope(&self, required: &str) -> bool {
        self.scopes.iter().any(|s| s == required || s == "admin")
    }
}

/// Recursively replace known secret-bearing fields with the read-scope sentinel.
/// Matches by key name so it covers webhook `secret`, operator `cursor_api_key`,
/// SMTP `smtp_password`, and `signature_secret` wherever they nest in the JSON.
fn redact_secret_fields(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                if matches!(
                    key.as_str(),
                    "secret" | "signature_secret" | "cursor_api_key" | "smtp_password"
                ) && child.as_str().is_some_and(|s| !s.is_empty())
                {
                    *child = serde_json::Value::String(
                        SchedulerService::READ_SCOPE_SECRET_SENTINEL.into(),
                    );
                } else {
                    redact_secret_fields(child);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                redact_secret_fields(item);
            }
        }
        _ => {}
    }
}

/// Salted SHA-256 hash of an API secret, hex-encoded.
fn hash_key(salt: &str, secret: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(b"|");
    hasher.update(secret.as_bytes());
    hex::encode(hasher.finalize())
}

/// Constant-time byte comparison to avoid leaking match length via timing.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Normalize/whitelist requested scopes; defaults to `read` if none valid.
fn normalize_scopes(scopes: &[&str]) -> String {
    let mut out: Vec<&str> = scopes
        .iter()
        .filter_map(|s| match s.trim() {
            "read" => Some("read"),
            "write" => Some("write"),
            "admin" => Some("admin"),
            _ => None,
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    if out.is_empty() {
        out.push("read");
    }
    out.join(",")
}

fn normalize_environment_name(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn normalize_environment_names(values: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = values
        .into_iter()
        .map(|value| normalize_environment_name(&value))
        .filter(|value| !value.is_empty())
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

fn env_list(value: Option<String>, defaults: &[&str]) -> Vec<String> {
    value
        .map(|raw| {
            raw.split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_else(|| defaults.iter().map(|item| item.to_string()).collect())
}

fn env_bool(value: Option<String>) -> bool {
    value
        .map(|raw| {
            matches!(
                raw.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn protected_environments_from_env() -> Vec<String> {
    normalize_environment_names(env_list(
        std::env::var("CHAOS_SCHEDULER_PROTECTED_ENVIRONMENTS").ok(),
        &[],
    ))
}

fn protected_writes_allowed_from_env() -> bool {
    env_bool(std::env::var("CHAOS_SCHEDULER_ALLOW_PROTECTED_WRITES").ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service(dir: &std::path::Path) -> SchedulerService {
        let db = Arc::new(Database::new(dir));
        SchedulerService::with_protection_config(
            db,
            Arc::new(NoopNotifier),
            vec![crate::branding::DEFAULT_ENVIRONMENT.into()],
            false,
        )
    }

    fn service_with_db(
        db: Arc<Database>,
        protected: Vec<&str>,
        allow_protected_writes: bool,
    ) -> SchedulerService {
        SchedulerService::with_protection_config(
            db,
            Arc::new(NoopNotifier),
            protected.into_iter().map(str::to_string).collect(),
            allow_protected_writes,
        )
    }

    fn draft(name: &str, environment: &str) -> WorkflowDraft {
        WorkflowDraft {
            name: name.to_string(),
            description: None,
            script_path: "scripts/noop.py".to_string(),
            cron_schedule: "0 0 * * *".to_string(),
            async_mode: false,
            email_on_failure: true,
            timezone: "UTC".to_string(),
            environment: environment.to_string(),
            domain: None,
            trigger_config: None,
            queue_config: None,
        }
    }

    fn tmpdir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("chaos-core-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn email_profile(name: &str, password: &str) -> EmailProfile {
        EmailProfile {
            id: String::new(),
            name: name.to_string(),
            enabled: true,
            alert_email: "alerts@example.com".to_string(),
            smtp_host: "smtp.example.com".to_string(),
            smtp_port: 587,
            smtp_user: "mailer".to_string(),
            smtp_password: password.to_string(),
            from_address: "from@example.com".to_string(),
            from_name: "Chaos".to_string(),
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn email_profile_masks_on_read_and_preserves_secret_on_mask_echo() {
        let dir = tmpdir();
        let svc = service(&dir);

        // Save with a real password: the returned profile is masked, but the
        // stored secret is intact.
        let saved = svc
            .save_email_profile(email_profile("Primary", "realpw"))
            .unwrap();
        assert_eq!(saved.smtp_password, MASKED_SECRET);
        assert_eq!(
            svc.db().get_email_profile(&saved.id).unwrap().smtp_password,
            "realpw"
        );

        // List is masked and never leaks the secret.
        let listed = svc.list_email_profiles().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].smtp_password, MASKED_SECRET);

        // Echoing the mask back keeps the stored secret; a real value replaces it.
        let mut echo = saved.clone();
        echo.name = "Renamed".to_string();
        echo.smtp_password = MASKED_SECRET.to_string();
        svc.save_email_profile(echo).unwrap();
        let stored = svc.db().get_email_profile(&saved.id).unwrap();
        assert_eq!(stored.smtp_password, "realpw");
        assert_eq!(stored.name, "Renamed");

        let mut replace = saved.clone();
        replace.smtp_password = "newpw".to_string();
        svc.save_email_profile(replace).unwrap();
        assert_eq!(
            svc.db().get_email_profile(&saved.id).unwrap().smtp_password,
            "newpw"
        );

        // A blank-name profile is rejected.
        assert!(matches!(
            svc.save_email_profile(email_profile("  ", "x")),
            Err(ServiceError::Validation(_))
        ));

        // Selection + delete round-trip.
        let wf = svc.create_workflow(draft("wf", "sandbox"), false).unwrap();
        svc.set_workflow_email_profile(&wf.id, Some(&saved.id))
            .unwrap();
        assert_eq!(
            svc.db()
                .get_workflow(&wf.id)
                .unwrap()
                .email_profile_id
                .as_deref(),
            Some(saved.id.as_str())
        );
        svc.set_workflow_email_profile(&wf.id, None).unwrap();
        assert!(svc
            .db()
            .get_workflow(&wf.id)
            .unwrap()
            .email_profile_id
            .is_none());

        svc.delete_email_profile(&saved.id).unwrap();
        assert!(svc.list_email_profiles().unwrap().is_empty());

        let _ = std::fs::remove_dir_all(dir);
    }

    fn spec() -> crate::workflow_spec::WorkflowSpec {
        crate::workflow_spec::WorkflowSpec {
            kind: crate::workflow_spec::WorkflowKind::Generic,
            environment: Some("sandbox".into()),
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
            on_success: vec![],
            on_failure: vec![],
        }
    }

    #[test]
    fn ui_create_is_never_externally_managed() {
        // Governance is decoupled from the environment name: a UI-originated
        // create (managed = false) is never externally-managed, even when it
        // targets an environment that happens to be named "production".
        let dir = tmpdir();
        let svc = service(&dir);
        let wf = svc.create_workflow(draft("wf", "sandbox"), false).unwrap();
        assert!(!svc.is_managed_externally(&wf));
        assert_eq!(wf.environment, "sandbox");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn api_may_register_managed_workflow() {
        let dir = tmpdir();
        let svc = service(&dir);
        let wf = svc.create_workflow(draft("wf", "sandbox"), true).unwrap();
        assert!(svc.is_managed_externally(&wf));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn ui_edit_of_managed_definition_is_blocked_but_runtime_prefs_allowed() {
        let dir = tmpdir();
        let svc = service(&dir);
        let wf = svc.create_workflow(draft("wf", "sandbox"), true).unwrap();

        // Changing the script (a governed field) from the UI is blocked.
        let mut changed = draft("wf", "sandbox");
        changed.script_path = "scripts/other.py".to_string();
        let err = svc
            .update_workflow(&wf.id, true, changed, false)
            .unwrap_err();
        assert!(matches!(err, ServiceError::Governance(_)));

        // Toggling enabled (a runtime pref) is allowed.
        let same = draft("wf", "sandbox");
        assert!(svc.update_workflow(&wf.id, false, same, false).is_ok());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn process_runner_scrubs_secret_child_env_keys() {
        for key in [
            "CURSOR_API_KEY",
            "SMTP_PASSWORD",
            "CHAOS_SCHEDULER_API_TOKEN",
            "CHAOS_SCHEDULER_WEBHOOK_SECRET",
        ] {
            assert!(should_scrub_child_env_key(key), "{key} should be scrubbed");
        }
        // User credentials personal scripts rely on must be PRESERVED; only the
        // scheduler's own secrets are stripped.
        for key in ["PATH", "HOME", "RUST_LOG", "SSH_AUTH_SOCK", "GITHUB_TOKEN"] {
            assert!(
                !should_scrub_child_env_key(key),
                "{key} should be preserved"
            );
        }
    }

    #[test]
    fn api_key_roundtrip_verifies_and_rejects_tampered() {
        let dir = tmpdir();
        let svc = service(&dir);
        let key = svc.create_api_key(Some("ci"), &["read", "write"]).unwrap();
        let ident = svc
            .verify_api_key(&key.token)
            .expect("valid token verifies");
        assert!(ident.has_scope("read"));
        assert!(ident.has_scope("write"));
        assert!(!ident.has_scope("admin"));

        // Tampered secret is rejected.
        let tampered = format!("{}.deadbeef", key.id);
        assert!(svc.verify_api_key(&tampered).is_none());
        // Garbage token is rejected.
        assert!(svc.verify_api_key("nope").is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn admin_scope_implies_all() {
        let ident = ApiIdentity {
            id: "k".into(),
            scopes: vec!["admin".into()],
        };
        assert!(ident.has_scope("read"));
        assert!(ident.has_scope("write"));
    }

    #[test]
    fn set_workflow_spec_validates_and_persists() {
        let dir = tmpdir();
        let svc = service(&dir);
        let wf = svc.create_workflow(draft("wf", "sandbox"), false).unwrap();
        let spec = spec();
        let updated = svc.set_workflow_spec(&wf.id, &spec, false).unwrap();
        assert_eq!(updated.kind, "generic");
        assert!(updated.spec_json.is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn ui_cannot_set_managed_workflow_spec() {
        let dir = tmpdir();
        let svc = service(&dir);
        let wf = svc.create_workflow(draft("wf", "sandbox"), true).unwrap();
        let err = svc.set_workflow_spec(&wf.id, &spec(), false).unwrap_err();
        assert!(matches!(err, ServiceError::Governance(_)));
        assert!(svc.set_workflow_spec(&wf.id, &spec(), true).is_ok());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn protected_environment_blocks_workflow_writes_and_execution() {
        let dir = tmpdir();
        let db = Arc::new(Database::new(&dir));
        let svc = service_with_db(db.clone(), vec!["prod"], false);

        let draft_prod = draft("prod wf", "prod");
        assert!(matches!(
            svc.create_workflow(draft_prod.clone(), false).unwrap_err(),
            ServiceError::Governance(_)
        ));

        let override_svc = service_with_db(db.clone(), vec!["prod"], true);
        let wf = override_svc
            .create_workflow(draft_prod.clone(), false)
            .unwrap();

        assert!(matches!(
            svc.update_workflow(&wf.id, true, draft_prod.clone(), false)
                .unwrap_err(),
            ServiceError::Governance(_)
        ));
        assert!(matches!(
            svc.set_workflow_spec(&wf.id, &spec(), true).unwrap_err(),
            ServiceError::Governance(_)
        ));
        assert!(matches!(
            svc.ensure_workflow_execution_allowed(&wf.id).unwrap_err(),
            ServiceError::Governance(_)
        ));
        assert!(matches!(
            svc.delete_workflow(&wf.id, true).unwrap_err(),
            ServiceError::Governance(_)
        ));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn protected_and_managed_environment_metadata_is_read_only() {
        let dir = tmpdir();
        let svc = service(&dir);

        assert!(matches!(
            svc.create_environment("production", None, None, None, None, None)
                .unwrap_err(),
            ServiceError::Governance(_)
        ));

        let source = svc
            .db
            .get_environment_by_name("production")
            .unwrap()
            .expect("production env seeded");
        assert!(matches!(
            svc.update_environment(&source.id, "renamed", None, None, None, None, None)
                .unwrap_err(),
            ServiceError::Governance(_)
        ));
        assert!(matches!(
            svc.delete_environment(&source.id).unwrap_err(),
            ServiceError::Governance(_)
        ));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn protected_write_override_is_explicit() {
        let dir = tmpdir();
        let db = Arc::new(Database::new(&dir));
        let svc = service_with_db(db, vec!["production"], true);
        let env = svc
            .create_environment("staging", None, None, None, None, None)
            .unwrap();
        assert_eq!(env.name, "staging");
        let d = draft("staging wf", "staging");
        assert!(svc.create_workflow(d, false).is_ok());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn create_workflow_targets_explicit_environment() {
        let dir = tmpdir();
        let svc = service(&dir);
        let d = draft("wf", "staging");
        let wf = svc.create_workflow(d, false).unwrap();
        assert_eq!(wf.environment, "staging");
        assert!(!wf.managed_externally);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn update_workflow_can_move_environment() {
        let dir = tmpdir();
        let svc = service(&dir);
        let wf = svc.create_workflow(draft("wf", "sandbox"), false).unwrap();
        let d = draft("wf", "staging");
        let updated = svc.update_workflow(&wf.id, true, d, false).unwrap();
        assert_eq!(updated.environment, "staging");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn update_environment_renames_and_rejects_duplicate() {
        let dir = tmpdir();
        let svc = service(&dir);
        let a = svc
            .create_environment("staging", None, None, None, None, None)
            .unwrap();
        svc.create_environment("qa", None, None, None, None, None)
            .unwrap();
        // Rename staging -> staging2 ok.
        let renamed = svc
            .update_environment(&a.id, "staging2", None, None, None, None, None)
            .unwrap();
        assert_eq!(renamed.name, "staging2");
        // Renaming onto an existing name is rejected.
        let err = svc
            .update_environment(&a.id, "qa", None, None, None, None, None)
            .unwrap_err();
        assert!(matches!(err, ServiceError::Validation(_)));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn revoke_api_key_prevents_verification() {
        let dir = tmpdir();
        let svc = service(&dir);
        let key = svc.create_api_key(Some("ci"), &["read"]).unwrap();
        assert!(svc.verify_api_key(&key.token).is_some());
        svc.revoke_api_key(&key.id).unwrap();
        assert!(
            svc.verify_api_key(&key.token).is_none(),
            "revoked key must not verify"
        );
        // Revoked key still appears (as revoked) in metadata listing.
        let listed = svc.list_api_keys().unwrap();
        assert!(listed.iter().any(|k| k.id == key.id && k.revoked));
        // Revoking a nonexistent key is a NotFound.
        assert!(matches!(
            svc.revoke_api_key("nope").unwrap_err(),
            ServiceError::NotFound(_)
        ));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn validation_rejects_bad_queue_config_json() {
        let dir = tmpdir();
        let svc = service(&dir);
        let mut d = draft("wf", "sandbox");
        d.queue_config = Some("{not json".to_string());
        let err = svc.create_workflow(d, false).unwrap_err();
        assert!(matches!(err, ServiceError::Validation(_)));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn read_scope_workflow_redaction_hides_secrets_but_write_preserves() {
        use crate::actions::ActionSpec;

        let dir = tmpdir();
        let svc = service(&dir);
        let wf = svc.create_workflow(draft("wf", "sandbox"), false).unwrap();
        let mut spec = spec();
        spec.on_success = vec![ActionSpec::Webhook {
            url: "https://example.com/h".into(),
            secret: Some("topsecret".into()),
            max_retries: 0,
        }];
        svc.set_workflow_spec(&wf.id, &spec, false).unwrap();

        let read = svc
            .get_workflow_for_scopes(&wf.id, &["read".to_string()])
            .unwrap();
        assert!(read.spec_json.as_ref().unwrap().contains("__redacted__"));
        assert!(!read.spec_json.as_ref().unwrap().contains("topsecret"));

        let write = svc
            .get_workflow_for_scopes(&wf.id, &["write".to_string()])
            .unwrap();
        assert!(write.spec_json.as_ref().unwrap().contains("topsecret"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
