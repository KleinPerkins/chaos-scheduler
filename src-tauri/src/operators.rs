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
}

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
fn confine_path_under_root(workspace_root: &str, requested: &str) -> Result<PathBuf, String> {
    if workspace_root.trim().is_empty() {
        return Err("git_pull: workspace_root is not configured".into());
    }
    let root = std::fs::canonicalize(workspace_root)
        .map_err(|e| format!("git_pull: invalid workspace_root '{workspace_root}': {e}"))?;
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
        Err(format!(
            "git_pull: path '{requested}' escapes workspace_root"
        ))
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
    let mut resolved = std::fs::canonicalize(&existing)
        .map_err(|e| format!("git_pull: cannot resolve path prefix: {e}"))?;
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
const CURSOR_DEFAULT_POLL_ATTEMPTS: u64 = 150;
const CURSOR_MAX_POLL_ATTEMPTS: u64 = 300;
const CURSOR_DEFAULT_POLL_INTERVAL_MS: u64 = 2_000;
const CURSOR_MAX_POLL_INTERVAL_MS: u64 = 30_000;

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
        let repository = match config.get("repository").and_then(|v| v.as_str()) {
            Some(r) if !r.trim().is_empty() => r.to_string(),
            _ => {
                return OperatorOutcome::failure("cursor_agent cloud mode requires a `repository`")
            }
        };
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

        // Build the launch payload (Cursor Cloud Agents v1).
        let mut source = serde_json::json!({ "repository": repository });
        if let Some(git_ref) = config.get("ref").and_then(|v| v.as_str()) {
            source["ref"] = Value::String(git_ref.to_string());
        }
        let mut payload = serde_json::json!({
            "prompt": { "text": prompt },
            "source": source,
        });
        if let Some(model) = config.get("model").and_then(|v| v.as_str()) {
            payload["model"] = Value::String(model.to_string());
        }
        if config
            .get("auto_create_pr")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            payload["target"] = serde_json::json!({ "autoCreatePr": true });
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
        let agent_id = match launch.body.get("id").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => {
                return OperatorOutcome::failure("cursor_agent: launch response missing agent id")
            }
        };

        // Poll for completion. (SSE is the documented streaming channel; polling
        // is the primary, GA-safe path used here per the plan.)
        let poll_attempts =
            clamp_cursor_poll_attempts(config.get("poll_attempts").and_then(|v| v.as_u64()));
        let poll_interval_ms =
            clamp_cursor_poll_interval_ms(config.get("poll_interval_ms").and_then(|v| v.as_u64()));

        let status_url = format!("{api_base}/v1/agents/{agent_id}");
        let mut last_body = launch.body.clone();
        let mut terminal_status = status_str(&launch.body);
        if !is_terminal(&terminal_status) {
            for attempt in 0..poll_attempts {
                if crate::scheduler::SHUTDOWN.load(std::sync::atomic::Ordering::Relaxed) {
                    terminal_status = "POLL_EXHAUSTED".to_string();
                    break;
                }
                if attempt > 0 && poll_interval_ms > 0 {
                    crate::scheduler::sleep_interruptible(std::time::Duration::from_millis(
                        poll_interval_ms,
                    ));
                }
                match ctx.http.get_json(&status_url, &headers) {
                    Ok(resp) if resp.is_success() => {
                        terminal_status = status_str(&resp.body);
                        last_body = resp.body;
                        if is_terminal(&terminal_status) {
                            break;
                        }
                    }
                    Ok(resp) => {
                        return OperatorOutcome::failure(format!(
                            "cursor_agent: poll failed (status {})",
                            resp.status
                        ))
                    }
                    Err(e) => {
                        return OperatorOutcome::failure(format!("cursor_agent: poll error: {e}"))
                    }
                }
            }
            if !is_terminal(&terminal_status) {
                terminal_status = "POLL_EXHAUSTED".to_string();
            }
        }

        let success = is_success_status(&terminal_status);
        let pr_url = last_body
            .get("target")
            .and_then(|t| t.get("prUrl"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let summary_text = last_body
            .get("summary")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| format!("cursor_agent {agent_id} -> {terminal_status}"));
        OperatorOutcome {
            success,
            summary: summary_text,
            details: serde_json::json!({
                "mode": "cloud",
                "agent_id": agent_id,
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
            match config.get("repository").and_then(|v| v.as_str()) {
                Some(r) if !r.trim().is_empty() => {}
                _ => return Err("cursor_agent cloud mode requires a `repository`".into()),
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
    fn cursor_agent_cloud_launches_polls_and_reports_pr() {
        let runner = FakeRunner {
            code: 0,
            calls: Mutex::new(vec![]),
        };
        let http = MockHttp {
            launch: serde_json::json!({"id": "bc_123", "status": "RUNNING"}),
            polls: Mutex::new(vec![
                serde_json::json!({"id": "bc_123", "status": "RUNNING"}),
                serde_json::json!({"id": "bc_123", "status": "FINISHED", "summary": "done", "target": {"prUrl": "https://gh/pr/1"}}),
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
                "repository": "https://github.com/acme/app",
                "api_base": "https://attacker.example",
                "poll_interval_ms": 0
            }),
        );
        assert!(outcome.success, "FINISHED => success: {}", outcome.summary);
        assert_eq!(outcome.details["agent_id"], serde_json::json!("bc_123"));
        assert_eq!(
            outcome.details["pr_url"],
            serde_json::json!("https://gh/pr/1")
        );
        // The launch POST carried a Bearer auth header only to Cursor's pinned host.
        let posted = http.posted.lock().unwrap();
        assert_eq!(posted[0].0, "https://api.cursor.com/v1/agents");
        assert!(posted[0].2, "authorization header present");
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
            launch: serde_json::json!({"id": "x", "status": "RUNNING"}),
            polls: Mutex::new(vec![]),
            get_count: Mutex::new(0),
            posted: Mutex::new(vec![]),
        };
        let secrets = no_secrets();
        let ctx = cursor_ctx(&runner, &http, &secrets);
        let outcome = CursorAgentOperator.execute(
            &ctx,
            &serde_json::json!({"mode": "cloud", "prompt": "p", "repository": "r"}),
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
            launch: serde_json::json!({"id": "bc_9", "status": "ERROR"}),
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
            &serde_json::json!({"mode": "cloud", "prompt": "p", "repository": "r", "api_base": "https://mock.local"}),
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
            launch: serde_json::json!({"id": "bc_poll", "status": "RUNNING"}),
            polls: Mutex::new(vec![
                serde_json::json!({"id": "bc_poll", "status": "RUNNING"}),
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
                "repository": "r",
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
            launch: serde_json::json!({"id": "bc_shutdown", "status": "RUNNING"}),
            polls: Mutex::new(vec![
                serde_json::json!({"id": "bc_shutdown", "status": "RUNNING"}),
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
                "repository": "r",
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
