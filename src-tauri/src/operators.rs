//! Built-in typed operators and their registry.
//!
//! An [`Operator`] encapsulates a self-contained unit of work (validated per
//! operator) executed via the injected [`ProcessRunner`], so operators are
//! testable without spawning real processes. `git_pull` ships built-in;
//! `cursor_agent` and others (Phase 4b+) register the same way.

use crate::service::ProcessRunner;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

/// A minimal HTTP response used by operators.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Value,
}

impl HttpResponse {
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// Injectable HTTP client so network-driven operators (e.g. `cursor_agent`
/// cloud mode) are unit-testable against a mock without real network access.
pub trait HttpClient: Send + Sync {
    fn post_json(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &Value,
    ) -> Result<HttpResponse, String>;
    fn get_json(&self, url: &str, headers: &[(String, String)]) -> Result<HttpResponse, String>;
}

/// Resolves named secrets (e.g. `cursor_api_key`) from a secure source
/// (scheduler config / OS environment). Values are never logged.
pub trait SecretResolver: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
}

/// A secret resolver that reads from process environment variables. Used as a
/// fallback and in headless contexts.
pub struct EnvSecretResolver;
impl SecretResolver for EnvSecretResolver {
    fn get(&self, key: &str) -> Option<String> {
        // Try the exact key and an upper-cased variant (e.g. cursor_api_key ->
        // CURSOR_API_KEY).
        std::env::var(key)
            .ok()
            .or_else(|| std::env::var(key.to_ascii_uppercase()).ok())
            .filter(|v| !v.trim().is_empty())
    }
}

/// Runtime context handed to an operator. `http` and `secrets` are injectable so
/// operators remain testable without real network access or secret stores.
pub struct OperatorContext<'a> {
    pub runner: &'a dyn ProcessRunner,
    pub http: &'a dyn HttpClient,
    pub secrets: &'a dyn SecretResolver,
    pub workspace_root: &'a str,
    /// Called by an operator to report interim progress (e.g. a remote
    /// agent/run id) before a long-running poll loop begins, so a crash or
    /// kill mid-execution still leaves a traceable record. No-op by default.
    pub on_progress: &'a dyn Fn(&Value),
}

/// An `on_progress` that discards every update; used by operators/tests that
/// have nothing worth persisting mid-execution.
#[allow(dead_code)] // Used by test `OperatorContext` construction below.
pub fn noop_progress(_progress: &Value) {}

/// The result of running an operator.
#[derive(Debug, Clone)]
pub struct OperatorOutcome {
    pub success: bool,
    pub summary: String,
    pub details: Value,
}

impl OperatorOutcome {
    fn failure(msg: impl Into<String>) -> Self {
        OperatorOutcome {
            success: false,
            summary: msg.into(),
            details: Value::Null,
        }
    }
}

/// A typed operator.
pub trait Operator: Send + Sync {
    fn operator_type(&self) -> &'static str;
    /// Validate the operator config at registration time.
    fn validate(&self, config: &Value) -> Result<(), String>;
    /// Execute the operator.
    fn execute(&self, ctx: &OperatorContext, config: &Value) -> OperatorOutcome;
}

/// Registry of available operators.
pub struct OperatorRegistry {
    ops: HashMap<&'static str, Box<dyn Operator>>,
}

impl OperatorRegistry {
    /// Registry seeded with all built-in operators.
    pub fn with_builtins() -> Self {
        let mut ops: HashMap<&'static str, Box<dyn Operator>> = HashMap::new();
        let git = GitPullOperator;
        ops.insert(git.operator_type(), Box::new(git));
        let cursor = CursorAgentOperator;
        ops.insert(cursor.operator_type(), Box::new(cursor));
        Self { ops }
    }

    pub fn get(&self, operator_type: &str) -> Option<&dyn Operator> {
        self.ops.get(operator_type).map(|b| b.as_ref())
    }

    #[allow(dead_code)] // Reserved for the API/MCP "list operators" surface.
    pub fn operator_types(&self) -> Vec<&'static str> {
        self.ops.keys().copied().collect()
    }

    /// Validate a typed spec's operator + config, returning a clear error if the
    /// operator is unknown.
    pub fn validate(&self, operator_type: &str, config: &Value) -> Result<(), String> {
        match self.get(operator_type) {
            Some(op) => op.validate(config),
            None => Err(format!("unknown operator_type: {operator_type}")),
        }
    }
}

impl Default for OperatorRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}

/// Allowed transports for `git_pull` `repo_url`: HTTPS and SSH only. This
/// rejects git's local / transport-helper syntaxes (`ext::`, `fd::`, `file://`,
/// `http://`, `git://`, ...) that can execute commands or read local files.
fn validate_git_url(url: &str) -> Result<(), String> {
    let u = url.trim();
    if u.is_empty() {
        return Err("git_pull: `repo_url` must not be empty".into());
    }
    if u.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(
            "git_pull: `repo_url` must not contain whitespace or control characters".into(),
        );
    }
    if u.starts_with('-') {
        return Err("git_pull: `repo_url` must not begin with '-'".into());
    }
    // Transport-helper syntax (e.g. `ext::`, `fd::`) uses `::`.
    if u.contains("::") {
        return Err("git_pull: `repo_url` transport-helper syntax is not allowed".into());
    }
    let lower = u.to_ascii_lowercase();
    let allowed =
        lower.starts_with("https://") || lower.starts_with("ssh://") || is_scp_like_ssh(u);
    if allowed {
        Ok(())
    } else {
        Err("git_pull: `repo_url` scheme not allowed (only https:// and ssh are permitted)".into())
    }
}

/// scp-like SSH syntax: `user@host:path` (no explicit scheme). Requires a
/// non-empty user, a hostname-shaped host, and a `:` that precedes any `/`.
fn is_scp_like_ssh(u: &str) -> bool {
    let Some(at) = u.find('@') else {
        return false;
    };
    let (user, rest) = (&u[..at], &u[at + 1..]);
    if user.is_empty() || user.contains('/') {
        return false;
    }
    let Some(colon) = rest.find(':') else {
        return false;
    };
    let host = &rest[..colon];
    if host.is_empty() || host.contains('/') {
        return false;
    }
    host.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

/// Reject branch / ref names that could smuggle git options or shell / control
/// characters (defense in depth on top of the `--` positional separator).
fn validate_git_ref(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("git_pull: `branch` must not be empty".into());
    }
    if name.starts_with('-') {
        return Err("git_pull: `branch` must not begin with '-'".into());
    }
    if name.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("git_pull: `branch` must not contain whitespace or control characters".into());
    }
    Ok(())
}

/// Resolve `requested` to an absolute path confined within `workspace_root`,
/// defeating `..` traversal and symlink escapes. Relative paths are joined onto
/// the workspace root. The deepest existing ancestor is canonicalized (so a
/// symlink cannot redirect outside the root) and any not-yet-created tail (the
/// clone target) is re-appended.
/// Shared, operation-agnostic path confinement (used by `git_pull` and the
/// Cursor "open in editor" command). Error strings are intentionally NOT
/// namespaced to a single operator so callers can surface them directly.
pub(crate) fn confine_path_under_root(
    workspace_root: &str,
    requested: &str,
) -> Result<PathBuf, String> {
    if workspace_root.trim().is_empty() {
        return Err("workspace_root is not configured".into());
    }
    let root = std::fs::canonicalize(workspace_root)
        .map_err(|e| format!("invalid workspace_root '{workspace_root}': {e}"))?;
    let requested_path = Path::new(requested);
    let absolute = if requested_path.is_absolute() {
        requested_path.to_path_buf()
    } else {
        root.join(requested_path)
    };
    let normalized = normalize_lexical(&absolute);
    let resolved = canonicalize_existing_prefix(&normalized)?;
    if resolved == root || resolved.starts_with(&root) {
        Ok(resolved)
    } else {
        Err(format!("path '{requested}' escapes workspace_root"))
    }
}

/// Lexically normalize a path (resolve `.` and `..`) without touching the FS.
fn normalize_lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Canonicalize the longest existing ancestor of `path`, then re-append the
/// remaining (not-yet-existing) components. Defeats symlink escapes on the
/// portion of the path that exists (e.g. `/tmp` -> `/private/tmp` on macOS).
fn canonicalize_existing_prefix(path: &Path) -> Result<PathBuf, String> {
    let mut existing = path.to_path_buf();
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    while !existing.exists() {
        match existing.file_name() {
            Some(name) => {
                tail.push(name.to_os_string());
                match existing.parent() {
                    Some(parent) => existing = parent.to_path_buf(),
                    None => break,
                }
            }
            None => break,
        }
    }
    let mut resolved =
        std::fs::canonicalize(&existing).map_err(|e| format!("cannot resolve path prefix: {e}"))?;
    for name in tail.iter().rev() {
        resolved.push(name);
    }
    Ok(resolved)
}

/// `git_pull` — clone (if absent) or fast-forward/rebase a repository via the
/// system `git`. Config:
/// `{ "path": "...", "repo_url"?: "...", "branch"?: "...", "rebase"?: bool, "depth"?: u32 }`.
pub struct GitPullOperator;

impl Operator for GitPullOperator {
    fn operator_type(&self) -> &'static str {
        "git_pull"
    }

    fn validate(&self, config: &Value) -> Result<(), String> {
        let path = config.get("path").and_then(|v| v.as_str());
        match path {
            Some(p) if !p.trim().is_empty() => {}
            _ => return Err("git_pull requires a non-empty `path`".into()),
        }
        if let Some(depth) = config.get("depth") {
            if !depth.is_u64() {
                return Err("git_pull `depth` must be a positive integer".into());
            }
        }
        if let Some(rebase) = config.get("rebase") {
            if !rebase.is_boolean() {
                return Err("git_pull `rebase` must be a boolean".into());
            }
        }
        if let Some(url) = config.get("repo_url").and_then(|v| v.as_str()) {
            validate_git_url(url)?;
        }
        if let Some(branch) = config.get("branch").and_then(|v| v.as_str()) {
            validate_git_ref(branch)?;
        }
        Ok(())
    }

    fn execute(&self, ctx: &OperatorContext, config: &Value) -> OperatorOutcome {
        let path = match config.get("path").and_then(|v| v.as_str()) {
            Some(p) if !p.trim().is_empty() => p.to_string(),
            _ => return OperatorOutcome::failure("git_pull: missing path"),
        };
        let branch = config.get("branch").and_then(|v| v.as_str());
        let rebase = config
            .get("rebase")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let repo_url = config.get("repo_url").and_then(|v| v.as_str());
        let depth = config.get("depth").and_then(|v| v.as_u64());

        if let Some(b) = branch {
            if let Err(e) = validate_git_ref(b) {
                return OperatorOutcome::failure(e);
            }
        }

        // Confine the clone/working path under the workspace root, defeating
        // `..` traversal and symlink escapes before it ever reaches `git`.
        let path = match confine_path_under_root(ctx.workspace_root, &path) {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(e) => return OperatorOutcome::failure(e),
        };

        let git_dir_exists = std::path::Path::new(&path).join(".git").exists();

        let args: Vec<String> = if !git_dir_exists {
            // Clone into the target path.
            let Some(url) = repo_url else {
                return OperatorOutcome::failure(
                    "git_pull: path is not a git repo and no repo_url was provided to clone",
                );
            };
            if let Err(e) = validate_git_url(url) {
                return OperatorOutcome::failure(e);
            }
            let mut a = vec!["clone".to_string()];
            if let Some(d) = depth {
                a.push("--depth".to_string());
                a.push(d.to_string());
            }
            if let Some(b) = branch {
                a.push("--branch".to_string());
                a.push(b.to_string());
            }
            // `--` terminates option parsing so a crafted URL or path beginning
            // with `-` can never be reinterpreted as a git flag.
            a.push("--".to_string());
            a.push(url.to_string());
            a.push(path.clone());
            a
        } else {
            // Pull in the existing repo.
            let mut a = vec!["-C".to_string(), path.clone(), "pull".to_string()];
            if rebase {
                a.push("--rebase".to_string());
            }
            if let Some(b) = branch {
                a.push("--".to_string());
                a.push("origin".to_string());
                a.push(b.to_string());
            }
            a
        };

        match ctx.runner.run("git", &args, Some(ctx.workspace_root), &[]) {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let success = output.status.success();
                OperatorOutcome {
                    success,
                    summary: if success {
                        format!(
                            "git_pull ok ({} {})",
                            if git_dir_exists { "pull" } else { "clone" },
                            path
                        )
                    } else {
                        format!("git_pull failed for {path}")
                    },
                    details: serde_json::json!({
                        "args": args,
                        "exit_code": output.status.code(),
                        "stdout": stdout,
                        "stderr": stderr,
                    }),
                }
            }
            Err(e) => OperatorOutcome::failure(format!("git_pull: failed to run git: {e}")),
        }
    }
}

/// Real HTTP client backed by blocking `reqwest` (rustls). Used in production;
/// tests inject a mock `HttpClient`.
pub struct ReqwestHttpClient {
    client: reqwest::blocking::Client,
}

impl Default for ReqwestHttpClient {
    fn default() -> Self {
        Self {
            client: reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }
}

impl ReqwestHttpClient {
    fn send(
        &self,
        req: reqwest::blocking::RequestBuilder,
        headers: &[(String, String)],
    ) -> Result<HttpResponse, String> {
        let mut req = req;
        for (k, v) in headers {
            req = req.header(k.as_str(), v.as_str());
        }
        let resp = req.send().map_err(|e| format!("request error: {e}"))?;
        let status = resp.status().as_u16();
        // Parse JSON if possible; otherwise wrap raw text.
        let text = resp.text().unwrap_or_default();
        let body = serde_json::from_str::<Value>(&text)
            .unwrap_or_else(|_| serde_json::json!({ "raw": text }));
        Ok(HttpResponse { status, body })
    }
}

impl HttpClient for ReqwestHttpClient {
    fn post_json(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &Value,
    ) -> Result<HttpResponse, String> {
        self.send(self.client.post(url).json(body), headers)
    }

    fn get_json(&self, url: &str, headers: &[(String, String)]) -> Result<HttpResponse, String> {
        self.send(self.client.get(url), headers)
    }
}

/// `cursor_agent` — drive a Cursor coding agent, either via the Cursor **Cloud
/// Agents v1 REST API** (`cloud` mode, default) or the local **`cursor-agent`
/// CLI** (`cli` mode).
///
/// Config:
/// ```json
/// {
///   "mode": "cloud" | "cli",
///   "prompt": "…",                       // required
///   "repository": "https://github.com/…",// cloud: required
///   "ref": "main",                        // cloud: optional branch
///   "model": "…",                         // optional
///   "auto_create_pr": true,               // cloud: optional
///   "api_base": "https://api.cursor.com", // ignored; Cursor Cloud is host-pinned
///   "api_key_secret": "cursor_api_key",   // secret name to resolve
///   "poll_attempts": 150,                 // cloud: optional
///   "poll_interval_ms": 2000,             // cloud: optional
///   "cli_path": "cursor-agent"            // cli: optional
/// }
/// ```
/// The service-account API key is resolved from the injected [`SecretResolver`]
/// (scheduler config / OS env) and is never logged.
pub struct CursorAgentOperator;

const CURSOR_DEFAULT_API_BASE: &str = "https://api.cursor.com";
const CURSOR_DEFAULT_SECRET: &str = "cursor_api_key";
// Real coding-agent runs (clone + edit + test + push) commonly take well
// beyond a few minutes; the default budget is sized at ~10 minutes
// (300 * 2s) so normal-length tasks don't hit `POLL_EXHAUSTED` prematurely,
// with room to opt into a longer budget (up to the max) via config for
// slower tasks.
const CURSOR_DEFAULT_POLL_ATTEMPTS: u64 = 300;
const CURSOR_MAX_POLL_ATTEMPTS: u64 = 600;
const CURSOR_DEFAULT_POLL_INTERVAL_MS: u64 = 2_000;
const CURSOR_MAX_POLL_INTERVAL_MS: u64 = 30_000;
// A single flaky GET (transient network error or a 429/5xx from Cursor's
// API) shouldn't nuke a multi-minute agent run; retry a bounded number of
// *consecutive* transient failures with exponential backoff before giving up.
const CURSOR_MAX_CONSECUTIVE_POLL_FAILURES: u32 = 5;

impl CursorAgentOperator {
    fn mode(config: &Value) -> &str {
        config
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("cloud")
    }

    fn run_cloud(&self, ctx: &OperatorContext, config: &Value) -> OperatorOutcome {
        let prompt = match config.get("prompt").and_then(|v| v.as_str()) {
            Some(p) if !p.trim().is_empty() => p.to_string(),
            _ => return OperatorOutcome::failure("cursor_agent: `prompt` is required"),
        };
        let repository = match cursor_repository_value(config) {
            Some(r) => r.to_string(),
            None => {
                return OperatorOutcome::failure("cursor_agent cloud mode requires a `repository`")
            }
        };
        // Defense in depth: `validate()` already checks this at registration
        // time, but a spec created before this check existed (or edited
        // directly in the DB) could reach execution unvalidated.
        if let Err(e) = validate_cursor_repository(&repository) {
            return OperatorOutcome::failure(e);
        }
        let api_base = CURSOR_DEFAULT_API_BASE;
        let secret_name = config
            .get("api_key_secret")
            .and_then(|v| v.as_str())
            .unwrap_or(CURSOR_DEFAULT_SECRET);
        let Some(api_key) = ctx.secrets.get(secret_name) else {
            return OperatorOutcome::failure(format!(
                "cursor_agent: missing service-account API key (secret '{secret_name}')"
            ));
        };
        // Authorization carries the secret; never include it in summaries/details.
        let headers = vec![("authorization".to_string(), format!("Bearer {api_key}"))];

        // Build the launch payload (Cursor Cloud Agents v1: `POST /v1/agents`).
        // `repos[0].url` must be a full GitHub URL; accept an `owner/repo`
        // shorthand for convenience and normalize it.
        let repo_url = if repository.starts_with("http://") || repository.starts_with("https://") {
            repository.clone()
        } else {
            format!("https://github.com/{repository}")
        };
        let mut repo_entry = serde_json::json!({ "url": repo_url });
        if let Some(git_ref) = config.get("ref").and_then(|v| v.as_str()) {
            repo_entry["startingRef"] = Value::String(git_ref.to_string());
        }
        let mut payload = serde_json::json!({
            "prompt": { "text": prompt },
            "repos": [repo_entry],
        });
        if let Some(model) = config.get("model").and_then(|v| v.as_str()) {
            payload["model"] = serde_json::json!({ "id": model });
        }
        if config
            .get("auto_create_pr")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            payload["autoCreatePR"] = Value::Bool(true);
        }

        let launch = match ctx
            .http
            .post_json(&format!("{api_base}/v1/agents"), &headers, &payload)
        {
            Ok(resp) if resp.is_success() => resp,
            Ok(resp) => {
                return OperatorOutcome::failure(format!(
                    "cursor_agent: launch failed (status {})",
                    resp.status
                ))
            }
            Err(e) => return OperatorOutcome::failure(format!("cursor_agent: launch error: {e}")),
        };
        // The create response nests the durable agent and its initial run:
        // `{ "agent": { "id": ... }, "run": { "id": ..., "status": ... } }`.
        let agent_id = match launch.body["agent"]["id"].as_str() {
            Some(id) => id.to_string(),
            None => {
                return OperatorOutcome::failure("cursor_agent: launch response missing agent id")
            }
        };
        let run_id = match launch.body["run"]["id"].as_str() {
            Some(id) => id.to_string(),
            None => {
                return OperatorOutcome::failure("cursor_agent: launch response missing run id")
            }
        };

        // Persist the remote identifiers *before* the (possibly multi-minute)
        // poll loop starts, so a scheduler kill mid-poll still leaves a
        // traceable record of which Cursor Cloud Agent run this local run
        // corresponds to (rather than only recording it on completion).
        (ctx.on_progress)(&serde_json::json!({
            "mode": "cloud",
            "agent_id": agent_id,
            "run_id": run_id,
        }));

        // Poll for completion. Execution status lives on the *run*, not the
        // agent (the agent's own `status` stays `ACTIVE` across runs) — SSE is
        // the documented streaming channel; polling `GET .../runs/{run_id}` is
        // the primary, GA-safe path used here per the plan.
        let poll_attempts =
            clamp_cursor_poll_attempts(config.get("poll_attempts").and_then(|v| v.as_u64()));
        let poll_interval_ms =
            clamp_cursor_poll_interval_ms(config.get("poll_interval_ms").and_then(|v| v.as_u64()));

        let status_url = format!("{api_base}/v1/agents/{agent_id}/runs/{run_id}");
        let mut last_body = launch.body["run"].clone();
        let mut terminal_status = status_str(&last_body);
        if !is_terminal(&terminal_status) {
            let mut consecutive_poll_failures: u32 = 0;
            // Set after a retry's backoff sleep so the *next* iteration's
            // normal top-of-loop interval sleep is skipped. Without this, a
            // retried poll sleeps once for its backoff and then immediately
            // again for `poll_interval_ms` on the next iteration, roughly
            // doubling the effective wait after a retry versus what
            // `poll_attempts * poll_interval_ms` leads a user to expect.
            let mut skip_next_interval_sleep = false;
            for attempt in 0..poll_attempts {
                if crate::scheduler::SHUTDOWN.load(std::sync::atomic::Ordering::Relaxed) {
                    terminal_status = "POLL_EXHAUSTED".to_string();
                    break;
                }
                if attempt > 0 && poll_interval_ms > 0 && !skip_next_interval_sleep {
                    crate::scheduler::sleep_interruptible(std::time::Duration::from_millis(
                        poll_interval_ms,
                    ));
                }
                skip_next_interval_sleep = false;
                match ctx.http.get_json(&status_url, &headers) {
                    Ok(resp) if resp.is_success() => {
                        consecutive_poll_failures = 0;
                        terminal_status = status_str(&resp.body);
                        last_body = resp.body;
                        if is_terminal(&terminal_status) {
                            break;
                        }
                    }
                    // A single flaky GET (rate-limited or a transient server
                    // error) shouldn't fail a whole multi-minute run; retry
                    // with bounded exponential backoff before giving up.
                    Ok(resp) if is_retryable_poll_status(resp.status) => {
                        consecutive_poll_failures += 1;
                        if consecutive_poll_failures > CURSOR_MAX_CONSECUTIVE_POLL_FAILURES {
                            return OperatorOutcome::failure(format!(
                                "cursor_agent: poll failed repeatedly (status {}) after {consecutive_poll_failures} consecutive attempts",
                                resp.status
                            ));
                        }
                        crate::scheduler::sleep_interruptible(std::time::Duration::from_millis(
                            poll_retry_backoff_ms(poll_interval_ms, consecutive_poll_failures),
                        ));
                        skip_next_interval_sleep = true;
                    }
                    Ok(resp) => {
                        return OperatorOutcome::failure(format!(
                            "cursor_agent: poll failed (status {})",
                            resp.status
                        ))
                    }
                    Err(e) => {
                        consecutive_poll_failures += 1;
                        if consecutive_poll_failures > CURSOR_MAX_CONSECUTIVE_POLL_FAILURES {
                            return OperatorOutcome::failure(format!(
                                "cursor_agent: poll error after {consecutive_poll_failures} consecutive attempts: {e}"
                            ));
                        }
                        crate::scheduler::sleep_interruptible(std::time::Duration::from_millis(
                            poll_retry_backoff_ms(poll_interval_ms, consecutive_poll_failures),
                        ));
                        skip_next_interval_sleep = true;
                    }
                }
            }
            if !is_terminal(&terminal_status) {
                terminal_status = "POLL_EXHAUSTED".to_string();
            }
        }

        let success = is_success_status(&terminal_status);
        // `git` is a per-agent snapshot returned on the run body once a branch
        // has been pushed: `{ branches: [{ repoUrl, branch?, prUrl? }] }`.
        let pr_url = last_body
            .get("git")
            .and_then(|g| g.get("branches"))
            .and_then(|b| b.as_array())
            .and_then(|branches| branches.iter().find_map(|b| b.get("prUrl")))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let summary_text = last_body
            .get("result")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| format!("cursor_agent {agent_id} -> {terminal_status}"));
        OperatorOutcome {
            success,
            summary: summary_text,
            details: serde_json::json!({
                "mode": "cloud",
                "agent_id": agent_id,
                "run_id": run_id,
                "status": terminal_status,
                "poll_attempts": poll_attempts,
                "poll_interval_ms": poll_interval_ms,
                "pr_url": pr_url,
                "result": last_body,
            }),
        }
    }

    fn run_cli(&self, ctx: &OperatorContext, config: &Value) -> OperatorOutcome {
        let prompt = match config.get("prompt").and_then(|v| v.as_str()) {
            Some(p) if !p.trim().is_empty() => p.to_string(),
            _ => return OperatorOutcome::failure("cursor_agent: `prompt` is required"),
        };
        let cli_path = config
            .get("cli_path")
            .and_then(|v| v.as_str())
            .unwrap_or("cursor-agent")
            .to_string();
        let args = vec![
            "-p".to_string(),
            prompt,
            "--output-format".to_string(),
            "json".to_string(),
            "--force".to_string(),
        ];
        match ctx
            .runner
            .run(&cli_path, &args, Some(ctx.workspace_root), &[])
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let success = output.status.success();
                let parsed = serde_json::from_str::<Value>(stdout.trim()).ok();
                OperatorOutcome {
                    success,
                    summary: if success {
                        "cursor_agent CLI completed".to_string()
                    } else {
                        "cursor_agent CLI failed".to_string()
                    },
                    details: serde_json::json!({
                        "mode": "cli",
                        "exit_code": output.status.code(),
                        "result": parsed,
                        "stderr": stderr,
                    }),
                }
            }
            Err(e) => OperatorOutcome::failure(format!("cursor_agent: failed to run CLI: {e}")),
        }
    }
}

fn status_str(body: &Value) -> String {
    body.get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_ascii_uppercase()
}

fn clamp_cursor_poll_attempts(value: Option<u64>) -> u64 {
    value
        .unwrap_or(CURSOR_DEFAULT_POLL_ATTEMPTS)
        .clamp(1, CURSOR_MAX_POLL_ATTEMPTS)
}

fn clamp_cursor_poll_interval_ms(value: Option<u64>) -> u64 {
    value
        .unwrap_or(CURSOR_DEFAULT_POLL_INTERVAL_MS)
        .min(CURSOR_MAX_POLL_INTERVAL_MS)
}

fn is_terminal(status: &str) -> bool {
    matches!(
        status,
        "FINISHED" | "COMPLETED" | "ERROR" | "FAILED" | "CANCELLED" | "EXPIRED"
    )
}

fn is_success_status(status: &str) -> bool {
    matches!(status, "FINISHED" | "COMPLETED")
}

/// Status codes worth retrying on the poll GET: rate-limiting and transient
/// server errors. Anything else (4xx auth/not-found errors) won't be fixed by
/// retrying, so those fail fast.
fn is_retryable_poll_status(status: u16) -> bool {
    status == 429 || (500..600).contains(&status)
}

/// Bounded exponential backoff for a retried poll GET, seeded from the
/// configured poll interval. `poll_interval_ms == 0` (the "no delay" test
/// convention) is preserved as no delay.
fn poll_retry_backoff_ms(poll_interval_ms: u64, consecutive_failures: u32) -> u64 {
    if poll_interval_ms == 0 {
        return 0;
    }
    let multiplier = 1u64 << consecutive_failures.clamp(1, 4).saturating_sub(1); // 1,2,4,8
    poll_interval_ms
        .saturating_mul(multiplier)
        .min(CURSOR_MAX_POLL_INTERVAL_MS)
}

/// Read the `cursor_agent` `repository` config value, falling back to the
/// legacy `repo` key when `repository` is absent or empty. This operator's
/// repository key was renamed `repo` -> `repository` to match the field this
/// operator actually reads; specs saved before that rename still have their
/// value sitting under the old key. Rather than a DB migration, fall back to
/// it here — editing the workflow in the UI naturally migrates it forward
/// since writes only ever go to `repository`.
fn cursor_repository_value(config: &Value) -> Option<&str> {
    config
        .get("repository")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            config
                .get("repo")
                .and_then(|v| v.as_str())
                .filter(|s| !s.trim().is_empty())
        })
}

/// Sanity-check a `cursor_agent` `repository` value before it's sent to
/// Cursor's API. Cursor's own API is the real trust boundary (it only acts on
/// GitHub repos it has installed access to) — this is cheap local hygiene,
/// not a security boundary: reject embedded URL credentials, non-TLS URLs,
/// and shorthand that doesn't actually look like `owner/repo`.
fn validate_cursor_repository(repository: &str) -> Result<(), String> {
    let r = repository.trim();
    if r.is_empty() {
        return Err("cursor_agent: `repository` must not be empty".into());
    }
    if r.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(
            "cursor_agent: `repository` must not contain whitespace or control characters".into(),
        );
    }
    if r.starts_with("http://") || r.starts_with("https://") {
        if r.starts_with("http://") {
            return Err("cursor_agent: `repository` URL must use https://".into());
        }
        let authority = r["https://".len()..].split('/').next().unwrap_or("");
        if authority.is_empty() {
            return Err("cursor_agent: `repository` URL must include a host".into());
        }
        if authority.contains('@') {
            return Err(
                "cursor_agent: `repository` URL must not contain embedded credentials".into(),
            );
        }
    } else {
        let parts: Vec<&str> = r.split('/').collect();
        if r.starts_with('-') || parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
            return Err("cursor_agent: `repository` shorthand must look like 'owner/repo'".into());
        }
    }
    Ok(())
}

impl Operator for CursorAgentOperator {
    fn operator_type(&self) -> &'static str {
        "cursor_agent"
    }

    fn validate(&self, config: &Value) -> Result<(), String> {
        let mode = Self::mode(config);
        if !matches!(mode, "cloud" | "cli") {
            return Err(format!(
                "cursor_agent: unknown mode '{mode}' (use cloud|cli)"
            ));
        }
        match config.get("prompt").and_then(|v| v.as_str()) {
            Some(p) if !p.trim().is_empty() => {}
            _ => return Err("cursor_agent: `prompt` is required".into()),
        }
        if mode == "cloud" {
            match cursor_repository_value(config) {
                Some(r) => validate_cursor_repository(r)?,
                None => return Err("cursor_agent cloud mode requires a `repository`".into()),
            }
            if let Some(v) = config.get("ref") {
                if !v.is_string() {
                    return Err("cursor_agent: `ref` must be a string".into());
                }
            }
            if let Some(v) = config.get("model") {
                if !v.is_string() {
                    return Err("cursor_agent: `model` must be a string".into());
                }
            }
            if let Some(v) = config.get("auto_create_pr") {
                if !v.is_boolean() {
                    return Err("cursor_agent: `auto_create_pr` must be a boolean".into());
                }
            }
            if let Some(v) = config.get("api_key_secret") {
                if !v.is_string() {
                    return Err("cursor_agent: `api_key_secret` must be a string".into());
                }
            }
        }
        if let Some(v) = config.get("poll_attempts") {
            if v.as_u64().is_none() {
                return Err("cursor_agent: `poll_attempts` must be a non-negative integer".into());
            }
        }
        if let Some(v) = config.get("poll_interval_ms") {
            if v.as_u64().is_none() {
                return Err(
                    "cursor_agent: `poll_interval_ms` must be a non-negative integer".into(),
                );
            }
        }
        if mode == "cli" {
            if let Some(v) = config.get("cli_path") {
                if !v.is_string() {
                    return Err("cursor_agent: `cli_path` must be a string".into());
                }
            }
        }
        Ok(())
    }

    fn execute(&self, ctx: &OperatorContext, config: &Value) -> OperatorOutcome {
        match Self::mode(config) {
            "cli" => self.run_cli(ctx, config),
            _ => self.run_cloud(ctx, config),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Output;
    use std::sync::Mutex;

    /// Injectable no-op HTTP client for operators that don't use HTTP.
    struct NoopHttp;
    impl HttpClient for NoopHttp {
        fn post_json(
            &self,
            _u: &str,
            _h: &[(String, String)],
            _b: &Value,
        ) -> Result<HttpResponse, String> {
            Err("no http in test".into())
        }
        fn get_json(&self, _u: &str, _h: &[(String, String)]) -> Result<HttpResponse, String> {
            Err("no http in test".into())
        }
    }

    /// Map-backed secret resolver for tests.
    struct MapSecrets(HashMap<String, String>);
    impl SecretResolver for MapSecrets {
        fn get(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
    }
    fn no_secrets() -> MapSecrets {
        MapSecrets(HashMap::new())
    }

    #[cfg(unix)]
    fn output(code: i32) -> Output {
        use std::os::unix::process::ExitStatusExt;
        Output {
            status: std::process::ExitStatus::from_raw((code & 0xff) << 8),
            stdout: b"ok".to_vec(),
            stderr: Vec::new(),
        }
    }

    struct FakeRunner {
        code: i32,
        calls: Mutex<Vec<(String, Vec<String>)>>,
    }
    impl ProcessRunner for FakeRunner {
        fn run(
            &self,
            program: &str,
            args: &[String],
            _cwd: Option<&str>,
            _env: &[(String, String)],
        ) -> std::io::Result<Output> {
            self.calls
                .lock()
                .unwrap()
                .push((program.to_string(), args.to_vec()));
            Ok(output(self.code))
        }
    }

    #[test]
    fn git_pull_validate_requires_path() {
        let op = GitPullOperator;
        assert!(op.validate(&serde_json::json!({})).is_err());
        assert!(op
            .validate(&serde_json::json!({"path": "/tmp/repo"}))
            .is_ok());
    }

    #[test]
    #[cfg(unix)]
    fn git_pull_clones_when_no_git_dir_and_url_present() {
        let runner = FakeRunner {
            code: 0,
            calls: Mutex::new(vec![]),
        };
        let http = NoopHttp;
        let secrets = no_secrets();
        let ctx = OperatorContext {
            runner: &runner,
            http: &http,
            secrets: &secrets,
            workspace_root: "/tmp",
            on_progress: &noop_progress,
        };
        // A path that certainly has no .git dir.
        let path = format!("/tmp/does-not-exist-{}", uuid::Uuid::new_v4());
        let outcome = GitPullOperator.execute(
            &ctx,
            &serde_json::json!({"path": path, "repo_url": "https://example.com/r.git"}),
        );
        assert!(outcome.success);
        let calls = runner.calls.lock().unwrap();
        assert_eq!(calls[0].0, "git");
        assert_eq!(calls[0].1[0], "clone");
    }

    #[test]
    fn git_pull_validate_rejects_dangerous_url_schemes() {
        let op = GitPullOperator;
        // Transport-helper and non-https/ssh schemes are rejected.
        for bad in [
            "ext::sh -c 'touch /tmp/pwned'",
            "file:///etc/passwd",
            "http://example.com/r.git",
            "git://example.com/r.git",
            "-oProxyCommand=evil",
            "fd::17/foo",
        ] {
            assert!(
                op.validate(&serde_json::json!({"path": "/tmp/repo", "repo_url": bad}))
                    .is_err(),
                "expected rejection for repo_url: {bad}"
            );
        }
        // HTTPS and SSH transports are accepted.
        for good in [
            "https://example.com/r.git",
            "ssh://git@example.com/org/r.git",
            "git@github.com:org/repo.git",
        ] {
            assert!(
                op.validate(&serde_json::json!({"path": "/tmp/repo", "repo_url": good}))
                    .is_ok(),
                "expected acceptance for repo_url: {good}"
            );
        }
        // Branch names that could smuggle options are rejected.
        assert!(op
            .validate(&serde_json::json!({"path": "/tmp/repo", "branch": "--upload-pack=evil"}))
            .is_err());
    }

    #[test]
    #[cfg(unix)]
    fn git_pull_clone_confines_path_and_terminates_options() {
        let root = std::env::temp_dir().join(format!("chaos-gp-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let runner = FakeRunner {
            code: 0,
            calls: Mutex::new(vec![]),
        };
        let http = NoopHttp;
        let secrets = no_secrets();
        let root_str = root.to_string_lossy().to_string();
        let ctx = OperatorContext {
            runner: &runner,
            http: &http,
            secrets: &secrets,
            workspace_root: &root_str,
            on_progress: &noop_progress,
        };
        // Relative path is joined onto the workspace root.
        let outcome = GitPullOperator.execute(
            &ctx,
            &serde_json::json!({"path": "repo", "repo_url": "https://example.com/r.git"}),
        );
        assert!(outcome.success, "{}", outcome.summary);
        let calls = runner.calls.lock().unwrap();
        let args = &calls[0].1;
        assert_eq!(args[0], "clone");
        // `--` must separate options from the positional url + path.
        let sep = args.iter().position(|a| a == "--").expect("`--` present");
        assert_eq!(args[sep + 1], "https://example.com/r.git");
        let confined = &args[sep + 2];
        let canonical_root = std::fs::canonicalize(&root).unwrap();
        assert!(
            std::path::Path::new(confined).starts_with(&canonical_root),
            "clone path {confined} must be confined under {canonical_root:?}"
        );
    }

    #[test]
    #[cfg(unix)]
    fn git_pull_rejects_path_escaping_workspace_root() {
        let root = std::env::temp_dir().join(format!("chaos-gp-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let runner = FakeRunner {
            code: 0,
            calls: Mutex::new(vec![]),
        };
        let http = NoopHttp;
        let secrets = no_secrets();
        let root_str = root.to_string_lossy().to_string();
        let ctx = OperatorContext {
            runner: &runner,
            http: &http,
            secrets: &secrets,
            workspace_root: &root_str,
            on_progress: &noop_progress,
        };
        // Traversal outside the workspace root is refused before git runs.
        let outcome = GitPullOperator.execute(
            &ctx,
            &serde_json::json!({"path": "../../../../etc", "repo_url": "https://example.com/r.git"}),
        );
        assert!(!outcome.success);
        assert!(outcome.summary.contains("escapes workspace_root"));
        assert!(runner.calls.lock().unwrap().is_empty(), "git must not run");
    }

    #[test]
    #[cfg(unix)]
    fn git_pull_execute_rejects_bad_url_before_running_git() {
        let root = std::env::temp_dir().join(format!("chaos-gp-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let runner = FakeRunner {
            code: 0,
            calls: Mutex::new(vec![]),
        };
        let http = NoopHttp;
        let secrets = no_secrets();
        let root_str = root.to_string_lossy().to_string();
        let ctx = OperatorContext {
            runner: &runner,
            http: &http,
            secrets: &secrets,
            workspace_root: &root_str,
            on_progress: &noop_progress,
        };
        let outcome = GitPullOperator.execute(
            &ctx,
            &serde_json::json!({"path": "repo", "repo_url": "ext::sh -c evil"}),
        );
        assert!(!outcome.success);
        assert!(runner.calls.lock().unwrap().is_empty(), "git must not run");
    }

    #[test]
    #[cfg(unix)]
    fn registry_exposes_git_pull_and_cursor_agent() {
        let reg = OperatorRegistry::with_builtins();
        assert!(reg.get("git_pull").is_some());
        assert!(reg.get("cursor_agent").is_some());
        assert!(reg.get("nope").is_none());
        assert!(reg
            .validate("git_pull", &serde_json::json!({"path": "/tmp/x"}))
            .is_ok());
        assert!(reg.validate("nope", &serde_json::json!({})).is_err());
    }

    // --- cursor_agent ---

    /// Scripted mock HTTP client: first POST returns a launch body; subsequent
    /// GETs return queued status entries (last repeats).
    struct MockHttp {
        launch: Value,
        polls: Mutex<Vec<Value>>,
        get_count: Mutex<usize>,
        posted: Mutex<Vec<(String, Value, bool)>>, // (url, body, had_auth)
    }
    impl HttpClient for MockHttp {
        fn post_json(
            &self,
            url: &str,
            headers: &[(String, String)],
            body: &Value,
        ) -> Result<HttpResponse, String> {
            let had_auth = headers
                .iter()
                .any(|(k, v)| k.eq_ignore_ascii_case("authorization") && v.starts_with("Bearer "));
            self.posted
                .lock()
                .unwrap()
                .push((url.to_string(), body.clone(), had_auth));
            Ok(HttpResponse {
                status: 200,
                body: self.launch.clone(),
            })
        }
        fn get_json(
            &self,
            _url: &str,
            _headers: &[(String, String)],
        ) -> Result<HttpResponse, String> {
            *self.get_count.lock().unwrap() += 1;
            let mut polls = self.polls.lock().unwrap();
            let body = if polls.len() > 1 {
                polls.remove(0)
            } else {
                polls.first().cloned().unwrap_or(Value::Null)
            };
            Ok(HttpResponse { status: 200, body })
        }
    }

    fn cursor_ctx<'a>(
        runner: &'a dyn ProcessRunner,
        http: &'a dyn HttpClient,
        secrets: &'a dyn SecretResolver,
    ) -> OperatorContext<'a> {
        OperatorContext {
            runner,
            http,
            secrets,
            workspace_root: "/tmp",
            on_progress: &noop_progress,
        }
    }

    #[test]
    fn cursor_agent_validate_modes_and_required_fields() {
        let op = CursorAgentOperator;
        assert!(op
            .validate(&serde_json::json!({"mode": "weird", "prompt": "x"}))
            .is_err());
        assert!(op.validate(&serde_json::json!({"mode": "cloud"})).is_err()); // no prompt
        assert!(op
            .validate(&serde_json::json!({"mode": "cloud", "prompt": "fix"}))
            .is_err()); // no repository
        assert!(op
            .validate(
                &serde_json::json!({"mode": "cloud", "prompt": "fix", "repository": "https://gh/x"})
            )
            .is_ok());
        assert!(op
            .validate(&serde_json::json!({"mode": "cli", "prompt": "fix"}))
            .is_ok());
    }

    #[test]
    fn cursor_agent_validate_rejects_malformed_numeric_and_typed_fields() {
        let op = CursorAgentOperator;
        let base = |extra: Value| {
            let mut c = serde_json::json!({
                "mode": "cloud", "prompt": "fix", "repository": "acme/repo"
            });
            c.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            c
        };
        // poll_attempts / poll_interval_ms must be non-negative integers, not
        // strings, floats, or negative numbers that would otherwise silently
        // fall back to the default at execution time.
        for bad in [
            serde_json::json!({"poll_attempts": -5}),
            serde_json::json!({"poll_attempts": "5"}),
            serde_json::json!({"poll_attempts": 5.5}),
            serde_json::json!({"poll_interval_ms": -1}),
            serde_json::json!({"poll_interval_ms": "1000"}),
        ] {
            assert!(
                op.validate(&base(bad.clone())).is_err(),
                "expected rejection for {bad}"
            );
        }
        // auto_create_pr / model / ref / api_key_secret must be the right type.
        for bad in [
            serde_json::json!({"auto_create_pr": "true"}),
            serde_json::json!({"model": 123}),
            serde_json::json!({"ref": 123}),
            serde_json::json!({"api_key_secret": 123}),
        ] {
            assert!(
                op.validate(&base(bad.clone())).is_err(),
                "expected rejection for {bad}"
            );
        }
        // Valid values of the same fields are accepted.
        assert!(op
            .validate(&base(serde_json::json!({
                "poll_attempts": 10,
                "poll_interval_ms": 500,
                "auto_create_pr": true,
                "model": "claude",
                "ref": "main",
                "api_key_secret": "cursor_api_key"
            })))
            .is_ok());
        // cli_path must be a string when present in cli mode.
        assert!(CursorAgentOperator
            .validate(&serde_json::json!({"mode": "cli", "prompt": "p", "cli_path": 123}))
            .is_err());
    }

    #[test]
    fn cursor_agent_validate_repository_hygiene() {
        for bad in [
            "https://user:pass@github.com/acme/repo",
            "http://github.com/acme/repo",
            "not-a-repo-shorthand",
            "owner/",
            "/repo",
            "owner/repo/extra",
            "-owner/repo",
            "https://",
        ] {
            assert!(
                validate_cursor_repository(bad).is_err(),
                "expected rejection for repository: {bad}"
            );
        }
        for good in [
            "acme/repo",
            "https://github.com/acme/repo",
            "https://ghe.internal.example.com/acme/repo",
        ] {
            assert!(
                validate_cursor_repository(good).is_ok(),
                "expected acceptance for repository: {good}"
            );
        }
    }

    #[test]
    fn cursor_agent_validate_falls_back_to_legacy_repo_key() {
        let op = CursorAgentOperator;
        // A spec saved before the `repo` -> `repository` rename has its value
        // sitting under the old key only; it must still validate.
        assert!(op
            .validate(&serde_json::json!({"mode": "cloud", "prompt": "fix", "repo": "acme/legacy"}))
            .is_ok());
        // An empty/blank `repository` alongside a populated legacy `repo`
        // still falls back rather than treating the field as present-but-bad.
        assert!(op
            .validate(
                &serde_json::json!({"mode": "cloud", "prompt": "fix", "repository": "", "repo": "acme/legacy"})
            )
            .is_ok());
        // A legacy value that fails repository hygiene still fails.
        assert!(op
            .validate(&serde_json::json!({"mode": "cloud", "prompt": "fix", "repo": "not-a-repo-shorthand"}))
            .is_err());
        // Neither key present still fails with the standard message.
        assert!(op
            .validate(&serde_json::json!({"mode": "cloud", "prompt": "fix"}))
            .is_err());
    }

    #[test]
    fn cursor_agent_cloud_launches_with_legacy_repo_key_only() {
        let runner = FakeRunner {
            code: 0,
            calls: Mutex::new(vec![]),
        };
        let http = MockHttp {
            launch: serde_json::json!({"agent": {"id": "bc_legacy"}, "run": {"id": "run_legacy", "status": "FINISHED", "result": "done"}}),
            polls: Mutex::new(vec![]),
            get_count: Mutex::new(0),
            posted: Mutex::new(vec![]),
        };
        let mut m = HashMap::new();
        m.insert("cursor_api_key".to_string(), "k".to_string());
        let secrets = MapSecrets(m);
        let ctx = cursor_ctx(&runner, &http, &secrets);
        // Config has only the legacy `repo` key (no `repository`), mirroring
        // a workflow spec saved before the rename — it must still launch.
        let outcome = CursorAgentOperator.execute(
            &ctx,
            &serde_json::json!({
                "mode": "cloud",
                "prompt": "fix the bug",
                "repo": "acme/legacy-app"
            }),
        );
        assert!(
            outcome.success,
            "legacy `repo`-only config must still launch: {}",
            outcome.summary
        );
        let posted = http.posted.lock().unwrap();
        assert_eq!(
            posted[0].1["repos"][0]["url"],
            serde_json::json!("https://github.com/acme/legacy-app")
        );
    }

    #[test]
    fn cursor_agent_cloud_launches_polls_and_reports_pr() {
        let runner = FakeRunner {
            code: 0,
            calls: Mutex::new(vec![]),
        };
        let http = MockHttp {
            launch: serde_json::json!({"agent": {"id": "bc_123"}, "run": {"id": "run_1", "status": "RUNNING"}}),
            polls: Mutex::new(vec![
                serde_json::json!({"id": "run_1", "status": "RUNNING"}),
                serde_json::json!({"id": "run_1", "status": "FINISHED", "result": "done", "git": {"branches": [{"prUrl": "https://gh/pr/1"}]}}),
            ]),
            get_count: Mutex::new(0),
            posted: Mutex::new(vec![]),
        };
        let mut secrets = HashMap::new();
        secrets.insert("cursor_api_key".to_string(), "sk-secret".to_string());
        let secrets = MapSecrets(secrets);
        let ctx = cursor_ctx(&runner, &http, &secrets);
        let outcome = CursorAgentOperator.execute(
            &ctx,
            &serde_json::json!({
                "mode": "cloud",
                "prompt": "fix the bug",
                "repository": "acme/app",
                "ref": "main",
                "model": "claude-4-sonnet-thinking",
                "auto_create_pr": true,
                "api_base": "https://attacker.example",
                "poll_interval_ms": 0
            }),
        );
        assert!(outcome.success, "FINISHED => success: {}", outcome.summary);
        assert_eq!(outcome.summary, "done");
        assert_eq!(outcome.details["agent_id"], serde_json::json!("bc_123"));
        assert_eq!(outcome.details["run_id"], serde_json::json!("run_1"));
        assert_eq!(
            outcome.details["pr_url"],
            serde_json::json!("https://gh/pr/1")
        );
        // The launch POST carried a Bearer auth header only to Cursor's pinned
        // host, and the body matches the real Cloud Agents v1 schema: a
        // `repos[]` array with a full GitHub URL (a bare `owner/repo`
        // shorthand is normalized), a `model` *object* (not a bare string),
        // and top-level `autoCreatePR` (not nested under a `target` object —
        // the real API rejects both a `source` key and a nested `target`).
        let posted = http.posted.lock().unwrap();
        assert_eq!(posted[0].0, "https://api.cursor.com/v1/agents");
        assert!(posted[0].2, "authorization header present");
        assert_eq!(
            posted[0].1,
            serde_json::json!({
                "prompt": { "text": "fix the bug" },
                "repos": [{ "url": "https://github.com/acme/app", "startingRef": "main" }],
                "model": { "id": "claude-4-sonnet-thinking" },
                "autoCreatePR": true,
            })
        );
        let details_str = outcome.details.to_string();
        assert!(
            !details_str.contains("sk-secret"),
            "secret must not leak into details"
        );
    }

    #[test]
    fn cursor_agent_cloud_missing_key_fails_cleanly() {
        let runner = FakeRunner {
            code: 0,
            calls: Mutex::new(vec![]),
        };
        let http = MockHttp {
            launch: serde_json::json!({"agent": {"id": "x"}, "run": {"id": "run_x", "status": "RUNNING"}}),
            polls: Mutex::new(vec![]),
            get_count: Mutex::new(0),
            posted: Mutex::new(vec![]),
        };
        let secrets = no_secrets();
        let ctx = cursor_ctx(&runner, &http, &secrets);
        let outcome = CursorAgentOperator.execute(
            &ctx,
            &serde_json::json!({"mode": "cloud", "prompt": "p", "repository": "acme/repo"}),
        );
        assert!(!outcome.success);
        assert!(outcome.summary.contains("API key"));
        // No POST attempted without a key.
        assert!(http.posted.lock().unwrap().is_empty());
    }

    #[test]
    fn cursor_agent_cloud_error_status_is_failure() {
        let runner = FakeRunner {
            code: 0,
            calls: Mutex::new(vec![]),
        };
        let http = MockHttp {
            launch: serde_json::json!({"agent": {"id": "bc_9"}, "run": {"id": "run_9", "status": "ERROR"}}),
            polls: Mutex::new(vec![]),
            get_count: Mutex::new(0),
            posted: Mutex::new(vec![]),
        };
        let mut m = HashMap::new();
        m.insert("cursor_api_key".to_string(), "k".to_string());
        let secrets = MapSecrets(m);
        let ctx = cursor_ctx(&runner, &http, &secrets);
        let outcome = CursorAgentOperator.execute(
            &ctx,
            &serde_json::json!({"mode": "cloud", "prompt": "p", "repository": "acme/repo", "api_base": "https://mock.local"}),
        );
        assert!(!outcome.success);
        assert_eq!(outcome.details["status"], serde_json::json!("ERROR"));
    }

    #[test]
    fn cursor_agent_poll_bounds_and_exhaustion_are_reported() {
        let runner = FakeRunner {
            code: 0,
            calls: Mutex::new(vec![]),
        };
        let http = MockHttp {
            launch: serde_json::json!({"agent": {"id": "bc_poll"}, "run": {"id": "run_poll", "status": "RUNNING"}}),
            polls: Mutex::new(vec![
                serde_json::json!({"id": "run_poll", "status": "RUNNING"}),
            ]),
            get_count: Mutex::new(0),
            posted: Mutex::new(vec![]),
        };
        let mut m = HashMap::new();
        m.insert("cursor_api_key".to_string(), "k".to_string());
        let secrets = MapSecrets(m);
        let ctx = cursor_ctx(&runner, &http, &secrets);

        // `poll_interval_ms: 0` keeps the test fast (no sleeps) while a huge
        // `poll_attempts` still exercises the attempt clamp and exhaustion path.
        let outcome = CursorAgentOperator.execute(
            &ctx,
            &serde_json::json!({
                "mode": "cloud",
                "prompt": "p",
                "repository": "acme/repo",
                "poll_attempts": 999_999,
                "poll_interval_ms": 0
            }),
        );

        assert!(!outcome.success);
        assert_eq!(
            outcome.details["status"],
            serde_json::json!("POLL_EXHAUSTED")
        );
        assert_eq!(
            outcome.details["poll_attempts"],
            serde_json::json!(CURSOR_MAX_POLL_ATTEMPTS)
        );
        assert_eq!(outcome.details["poll_interval_ms"], serde_json::json!(0));
        assert_eq!(
            *http.get_count.lock().unwrap(),
            CURSOR_MAX_POLL_ATTEMPTS as usize
        );
    }

    /// Scripted GET outcomes for retry/backoff tests: a fixed sequence, the
    /// last entry repeating once exhausted (mirrors `MockHttp`).
    #[derive(Clone)]
    enum ScriptedGet {
        Status(u16, Value),
        Err(String),
    }
    struct FlakyMockHttp {
        launch: Value,
        gets: Mutex<Vec<ScriptedGet>>,
        get_count: Mutex<usize>,
    }
    impl HttpClient for FlakyMockHttp {
        fn post_json(
            &self,
            _url: &str,
            _headers: &[(String, String)],
            _body: &Value,
        ) -> Result<HttpResponse, String> {
            Ok(HttpResponse {
                status: 200,
                body: self.launch.clone(),
            })
        }
        fn get_json(
            &self,
            _url: &str,
            _headers: &[(String, String)],
        ) -> Result<HttpResponse, String> {
            *self.get_count.lock().unwrap() += 1;
            let mut gets = self.gets.lock().unwrap();
            let item = if gets.len() > 1 {
                gets.remove(0)
            } else {
                gets.first()
                    .cloned()
                    .unwrap_or(ScriptedGet::Status(200, Value::Null))
            };
            match item {
                ScriptedGet::Status(status, body) => Ok(HttpResponse { status, body }),
                ScriptedGet::Err(e) => Err(e),
            }
        }
    }

    #[test]
    fn cursor_agent_poll_retries_transient_5xx_then_succeeds() {
        let runner = FakeRunner {
            code: 0,
            calls: Mutex::new(vec![]),
        };
        let http = FlakyMockHttp {
            launch: serde_json::json!({"agent": {"id": "bc_flaky"}, "run": {"id": "run_flaky", "status": "RUNNING"}}),
            gets: Mutex::new(vec![
                ScriptedGet::Status(503, Value::Null),
                ScriptedGet::Status(502, Value::Null),
                ScriptedGet::Status(
                    200,
                    serde_json::json!({"id": "run_flaky", "status": "FINISHED", "result": "done"}),
                ),
            ]),
            get_count: Mutex::new(0),
        };
        let mut m = HashMap::new();
        m.insert("cursor_api_key".to_string(), "sk-flaky-secret".to_string());
        let secrets = MapSecrets(m);
        let ctx = cursor_ctx(&runner, &http, &secrets);
        let outcome = CursorAgentOperator.execute(
            &ctx,
            &serde_json::json!({
                "mode": "cloud",
                "prompt": "p",
                "repository": "acme/repo",
                "poll_interval_ms": 0
            }),
        );
        assert!(
            outcome.success,
            "a couple of transient 5xx polls must not fail the run: {}",
            outcome.summary
        );
        assert_eq!(*http.get_count.lock().unwrap(), 3);
        let details_str = outcome.details.to_string();
        assert!(!details_str.contains("sk-flaky-secret"));
    }

    #[test]
    fn cursor_agent_poll_retries_transient_network_errors_then_succeeds() {
        let runner = FakeRunner {
            code: 0,
            calls: Mutex::new(vec![]),
        };
        let http = FlakyMockHttp {
            launch: serde_json::json!({"agent": {"id": "bc_net"}, "run": {"id": "run_net", "status": "RUNNING"}}),
            gets: Mutex::new(vec![
                ScriptedGet::Err("connection reset".to_string()),
                ScriptedGet::Status(
                    200,
                    serde_json::json!({"id": "run_net", "status": "FINISHED", "result": "done"}),
                ),
            ]),
            get_count: Mutex::new(0),
        };
        let mut m = HashMap::new();
        m.insert("cursor_api_key".to_string(), "k".to_string());
        let secrets = MapSecrets(m);
        let ctx = cursor_ctx(&runner, &http, &secrets);
        let outcome = CursorAgentOperator.execute(
            &ctx,
            &serde_json::json!({
                "mode": "cloud",
                "prompt": "p",
                "repository": "acme/repo",
                "poll_interval_ms": 0
            }),
        );
        assert!(outcome.success, "a single flaky GET must not fail the run");
        assert_eq!(*http.get_count.lock().unwrap(), 2);
    }

    #[test]
    fn cursor_agent_poll_retry_backoff_does_not_double_with_next_interval_sleep() {
        use crate::scheduler::{lock_shutdown_test_state, take_accounted_sleep_ms};

        // Guards against the process-global SHUTDOWN flag leaking `true` from
        // another test (e.g. `shutdown_interrupts_cursor_agent_poll_promptly`)
        // into this one, which otherwise exhausts the poll loop on its first
        // iteration regardless of the fix under test.
        let _guard = lock_shutdown_test_state();
        let runner = FakeRunner {
            code: 0,
            calls: Mutex::new(vec![]),
        };
        // One retryable 5xx (triggers a backoff sleep seeded from
        // poll_interval_ms), then a terminal status on the very next poll.
        let http = FlakyMockHttp {
            launch: serde_json::json!({"agent": {"id": "bc_dedup"}, "run": {"id": "run_dedup", "status": "RUNNING"}}),
            gets: Mutex::new(vec![
                ScriptedGet::Status(503, Value::Null),
                ScriptedGet::Status(
                    200,
                    serde_json::json!({"id": "run_dedup", "status": "FINISHED", "result": "done"}),
                ),
            ]),
            get_count: Mutex::new(0),
        };
        let mut m = HashMap::new();
        m.insert("cursor_api_key".to_string(), "k".to_string());
        let secrets = MapSecrets(m);
        let ctx = cursor_ctx(&runner, &http, &secrets);

        // `execute` runs the poll loop synchronously on this thread, so its
        // `sleep_interruptible` calls accrue to this thread's sleep account.
        // Asserting on the total *requested* sleep is exact and independent
        // of CI scheduling jitter — a wall-clock measurement of a single
        // ~200ms sleep was flaky under load (it could overrun a 1.5x
        // upper bound).
        let poll_interval_ms: u64 = 200;
        take_accounted_sleep_ms(); // zero this thread's accounting first
        let outcome = CursorAgentOperator.execute(
            &ctx,
            &serde_json::json!({
                "mode": "cloud",
                "prompt": "p",
                "repository": "acme/repo",
                "poll_interval_ms": poll_interval_ms
            }),
        );
        let slept_ms = take_accounted_sleep_ms();

        assert!(outcome.success, "{}", outcome.summary);
        assert_eq!(*http.get_count.lock().unwrap(), 2);
        // Exactly one sleep should occur between the two GETs: the retry's
        // own backoff (equal to poll_interval_ms here, since this is the
        // first consecutive failure). Before the dedup fix, the very next
        // iteration's unconditional top-of-loop interval sleep would fire
        // too — the account would then read 2 * poll_interval_ms.
        assert_eq!(
            slept_ms, poll_interval_ms,
            "expected exactly one deduped backoff sleep of {poll_interval_ms}ms, \
             but {slept_ms}ms was requested (the backoff sleep and the next \
             interval sleep both fired)"
        );
    }

    #[test]
    fn cursor_agent_poll_gives_up_after_max_consecutive_transient_failures() {
        let runner = FakeRunner {
            code: 0,
            calls: Mutex::new(vec![]),
        };
        let http = FlakyMockHttp {
            launch: serde_json::json!({"agent": {"id": "bc_dead"}, "run": {"id": "run_dead", "status": "RUNNING"}}),
            gets: Mutex::new(vec![ScriptedGet::Status(503, Value::Null)]),
            get_count: Mutex::new(0),
        };
        let mut m = HashMap::new();
        m.insert("cursor_api_key".to_string(), "sk-dead-secret".to_string());
        let secrets = MapSecrets(m);
        let ctx = cursor_ctx(&runner, &http, &secrets);
        let outcome = CursorAgentOperator.execute(
            &ctx,
            &serde_json::json!({
                "mode": "cloud",
                "prompt": "p",
                "repository": "acme/repo",
                "poll_interval_ms": 0
            }),
        );
        assert!(
            !outcome.success,
            "persistent 5xx polling must eventually fail rather than exhaust silently"
        );
        assert!(outcome.summary.contains("poll failed repeatedly"));
        assert_eq!(
            *http.get_count.lock().unwrap(),
            (CURSOR_MAX_CONSECUTIVE_POLL_FAILURES + 1) as usize
        );
        assert!(!outcome.summary.contains("sk-dead-secret"));
        assert!(!outcome.details.to_string().contains("sk-dead-secret"));
    }

    #[test]
    fn cursor_agent_poll_non_retryable_status_fails_fast() {
        let runner = FakeRunner {
            code: 0,
            calls: Mutex::new(vec![]),
        };
        let http = FlakyMockHttp {
            launch: serde_json::json!({"agent": {"id": "bc_404"}, "run": {"id": "run_404", "status": "RUNNING"}}),
            gets: Mutex::new(vec![ScriptedGet::Status(404, Value::Null)]),
            get_count: Mutex::new(0),
        };
        let mut m = HashMap::new();
        m.insert("cursor_api_key".to_string(), "k".to_string());
        let secrets = MapSecrets(m);
        let ctx = cursor_ctx(&runner, &http, &secrets);
        let outcome = CursorAgentOperator.execute(
            &ctx,
            &serde_json::json!({
                "mode": "cloud",
                "prompt": "p",
                "repository": "acme/repo",
                "poll_interval_ms": 0
            }),
        );
        assert!(!outcome.success);
        // A 404 (e.g. the agent/run no longer exists) is not a transient
        // condition retrying would fix, so it must fail on the first GET.
        assert_eq!(*http.get_count.lock().unwrap(), 1);
    }

    #[test]
    fn cursor_agent_launch_failure_does_not_leak_secret() {
        let runner = FakeRunner {
            code: 0,
            calls: Mutex::new(vec![]),
        };
        let http = MockHttp {
            // A non-2xx launch response; body content is irrelevant here.
            launch: serde_json::json!({"error": "unauthorized"}),
            polls: Mutex::new(vec![]),
            get_count: Mutex::new(0),
            posted: Mutex::new(vec![]),
        };
        struct RejectingHttp(MockHttp);
        impl HttpClient for RejectingHttp {
            fn post_json(
                &self,
                url: &str,
                headers: &[(String, String)],
                body: &Value,
            ) -> Result<HttpResponse, String> {
                let _ = self.0.post_json(url, headers, body);
                Ok(HttpResponse {
                    status: 401,
                    body: serde_json::json!({"error": "unauthorized"}),
                })
            }
            fn get_json(
                &self,
                url: &str,
                headers: &[(String, String)],
            ) -> Result<HttpResponse, String> {
                self.0.get_json(url, headers)
            }
        }
        let http = RejectingHttp(http);
        let mut m = HashMap::new();
        m.insert(
            "cursor_api_key".to_string(),
            "sk-launch-fail-secret".to_string(),
        );
        let secrets = MapSecrets(m);
        let ctx = cursor_ctx(&runner, &http, &secrets);
        let outcome = CursorAgentOperator.execute(
            &ctx,
            &serde_json::json!({"mode": "cloud", "prompt": "p", "repository": "acme/repo"}),
        );
        assert!(!outcome.success);
        assert!(!outcome.summary.contains("sk-launch-fail-secret"));
        assert!(!outcome
            .details
            .to_string()
            .contains("sk-launch-fail-secret"));
    }

    #[test]
    fn shutdown_interrupts_cursor_agent_poll_promptly() {
        use crate::scheduler::{initiate_shutdown, lock_shutdown_test_state};
        use std::time::{Duration, Instant};

        let _guard = lock_shutdown_test_state();
        let runner = FakeRunner {
            code: 0,
            calls: Mutex::new(vec![]),
        };
        let http = MockHttp {
            launch: serde_json::json!({"agent": {"id": "bc_shutdown"}, "run": {"id": "run_shutdown", "status": "RUNNING"}}),
            polls: Mutex::new(vec![
                serde_json::json!({"id": "run_shutdown", "status": "RUNNING"}),
            ]),
            get_count: Mutex::new(0),
            posted: Mutex::new(vec![]),
        };
        let mut m = HashMap::new();
        m.insert("cursor_api_key".to_string(), "k".to_string());
        let secrets = MapSecrets(m);
        let ctx = cursor_ctx(&runner, &http, &secrets);
        let shutdown_handle = std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(50));
            initiate_shutdown();
        });
        let start = Instant::now();
        let outcome = CursorAgentOperator.execute(
            &ctx,
            &serde_json::json!({
                "mode": "cloud",
                "prompt": "p",
                "repository": "acme/repo",
                "api_base": "https://mock.local",
                "poll_attempts": 300,
                "poll_interval_ms": 5000
            }),
        );
        let _ = shutdown_handle.join();
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "poll loop should exit promptly when SHUTDOWN is set"
        );
        assert_eq!(
            outcome.details["status"],
            serde_json::json!("POLL_EXHAUSTED")
        );
    }

    #[test]
    fn cursor_agent_poll_clamps_are_bounded() {
        // Attempts clamp to [1, MAX]; a missing value uses the default.
        assert_eq!(clamp_cursor_poll_attempts(Some(0)), 1);
        assert_eq!(
            clamp_cursor_poll_attempts(None),
            CURSOR_DEFAULT_POLL_ATTEMPTS
        );
        assert_eq!(
            clamp_cursor_poll_attempts(Some(999_999)),
            CURSOR_MAX_POLL_ATTEMPTS
        );

        // Interval clamps to <= MAX; 0 is preserved as "no delay".
        assert_eq!(clamp_cursor_poll_interval_ms(Some(0)), 0);
        assert_eq!(
            clamp_cursor_poll_interval_ms(None),
            CURSOR_DEFAULT_POLL_INTERVAL_MS
        );
        assert_eq!(
            clamp_cursor_poll_interval_ms(Some(999_999)),
            CURSOR_MAX_POLL_INTERVAL_MS
        );
    }

    #[test]
    #[cfg(unix)]
    fn cursor_agent_cli_invokes_binary_and_parses_json() {
        let runner = FakeRunner {
            code: 0,
            calls: Mutex::new(vec![]),
        };
        let http = NoopHttp;
        let secrets = no_secrets();
        let ctx = cursor_ctx(&runner, &http, &secrets);
        let outcome = CursorAgentOperator.execute(
            &ctx,
            &serde_json::json!({"mode": "cli", "prompt": "do it", "cli_path": "cursor-agent"}),
        );
        assert!(outcome.success);
        assert_eq!(outcome.details["mode"], serde_json::json!("cli"));
        let calls = runner.calls.lock().unwrap();
        assert_eq!(calls[0].0, "cursor-agent");
        assert!(calls[0].1.contains(&"-p".to_string()));
        assert!(calls[0].1.contains(&"--output-format".to_string()));
    }
}
