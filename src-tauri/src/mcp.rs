//! Managed MCP/SDK integration lifecycle.
//!
//! The desktop app is the lifecycle owner of an opt-in Cursor/MCP integration:
//! it provisions the existing `@chaos-scheduler/mcp-server` npm package (which
//! resolves `@chaos-scheduler/sdk` as its own published dependency — this
//! module never installs the SDK separately) into an app-owned directory,
//! registers it in `~/.cursor/mcp.json`, and can repair/re-provision or
//! remove it. See `updater_ux_plan_3f850760.plan.md` Section 12 for the full
//! design; `docs/RELEASING.md` documents the release-side half (the
//! `mcp-pinned-version.txt` stamping gate this module reads from).
//!
//! Durability invariants (all deliberate, see the plan's "Managed integration
//! invariants"):
//! - **Pinned install unit** — always installs exactly
//!   `@chaos-scheduler/mcp-server@<pinned_mcp_version()>`; the SDK is never
//!   installed directly.
//! - **Atomic install** — stages into `mcp/staging-<version>-<nonce>/`, runs a
//!   CLI smoke check, then atomically renames into `mcp/versions/<version>/`.
//!   The previous version is left untouched until the new one is verified and
//!   Cursor's config has been updated; only then is it pruned.
//! - **Absolute launch command** — never depends on shell `PATH`, `npx`, or
//!   `nvm`'s shell integration. Detects and stores absolute `node`/`npm`
//!   paths and writes Cursor's config with an absolute `node` command and an
//!   absolute installed CLI path.
//! - **Non-destructive Cursor config** — backs up before writing, writes
//!   atomically, preserves every other `mcpServers` entry, and only
//!   overwrites/removes the `chaos-scheduler` entry when it carries this
//!   app's ownership marker. An unmanaged pre-existing entry is reported as a
//!   conflict rather than silently overwritten (unless the caller passes
//!   `force`).
//! - **Token lifecycle (v1 fallback, see the plan's open question)** — this
//!   is the simpler of the two documented options: rather than a Keychain-
//!   backed launcher, the managed API key's token is written directly into
//!   the app-managed Cursor config entry (same trust surface as today's
//!   manual snippet), and the key id is persisted so repair/removal can
//!   revoke/remint rather than trying to recover a token the API never
//!   returns again.

use crate::service::SchedulerService;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

/// The npm package this module provisions. The SDK is a transitive dependency
/// of this package and is never installed separately (see module docs).
pub const MCP_PACKAGE_NAME: &str = "@chaos-scheduler/mcp-server";

/// Ownership marker written into the managed Cursor config entry's `env`.
/// JSON has no comments, so this — plus `CHAOS_SCHEDULER_MANAGED_ID` — is how
/// this module tells "an entry it manages" apart from one a user wrote by
/// hand or copied from the manual snippet.
const MANAGED_BY_MARKER: &str = "Chaos Scheduler";

/// Event emitted whenever the managed integration's status may have changed
/// (provision, remove, or the background startup re-provision hook
/// completing) — mirrors the updater's `update-status` event/`emit_snapshot`
/// convention so the Integrations card can stay live without polling, even
/// when the change happens on a background thread after the page already
/// mounted.
pub const MCP_STATUS_EVENT: &str = "mcp-status-changed";

/// Best-effort emit of [`MCP_STATUS_EVENT`]; a failure to emit (e.g. no
/// window yet) never fails the caller's own provision/remove/startup flow.
pub fn emit_status_changed(app: &tauri::AppHandle, status: &McpIntegrationStatus) {
    use tauri::Emitter;
    if let Err(err) = app.emit(MCP_STATUS_EVENT, status) {
        log::warn!("Failed to emit {MCP_STATUS_EVENT}: {err}");
    }
}

/// Holds the single-flight provisioning lock shared by UI-triggered
/// provision/remove calls and the post-launch re-provision hook, so staging
/// dirs and `mcp.json` writes can never race each other.
#[derive(Default)]
pub struct McpState {
    pub lock: Mutex<()>,
}

/// Single-flight lock acquisition shared by every entry point that touches
/// `McpState::lock` (the `provision_mcp_integration` / `remove_mcp_integration`
/// commands and the startup re-provision hook).
///
/// `Mutex::try_lock` conflates two very different situations under one
/// `Err`: "someone else legitimately holds the lock right now"
/// (`WouldBlock`) and "a previous holder panicked while holding it"
/// (`Poisoned`). Treating both as "busy" — the naive
/// `try_lock().map_err(|_| "already in progress")` this module used to use
/// at all three call sites — means a single panic anywhere under the lock
/// (now, or in any future change) permanently bricks MCP provisioning with a
/// misleading "already in progress" error until the app is restarted, since
/// every future call re-observes the same poisoned mutex. `update.rs`
/// already recovers from poison on its (blocking) lock; this does the same
/// for `McpState`'s non-blocking one: only `WouldBlock` is reported as
/// "busy", while `Poisoned` is recovered via `into_inner()` (the guarded
/// value is `()`, so there is no partially-mutated state to distrust).
pub fn try_lock_recovering(
    state: &McpState,
) -> Result<std::sync::MutexGuard<'_, ()>, &'static str> {
    match state.lock.try_lock() {
        Ok(guard) => Ok(guard),
        Err(std::sync::TryLockError::Poisoned(poisoned)) => Ok(poisoned.into_inner()),
        Err(std::sync::TryLockError::WouldBlock) => Err("MCP provisioning is already in progress"),
    }
}

/// The exact `mcp-server` version this desktop build was smoke-tested and
/// stamped against by release CI (see docs/RELEASING.md "Release ordering +
/// package-installability gate"). Compiled in via `include_str!` so the
/// value baked into a shipped binary is always whatever the release pipeline
/// last proved installable — never fetched or guessed at runtime. The
/// checked-in default is a best-effort fallback for local/dev builds only.
pub fn pinned_mcp_version() -> &'static str {
    trim_pinned_version(include_str!("../mcp-pinned-version.txt"))
}

fn trim_pinned_version(raw: &str) -> &str {
    raw.trim()
}

/// The scheduler's embedded REST API address, honoring the same
/// `CHAOS_SCHEDULER_API_ADDR` override `lib.rs` uses for the API server
/// itself, so the managed config and the health check always target the
/// address the app actually bound.
fn default_api_addr() -> String {
    std::env::var("CHAOS_SCHEDULER_API_ADDR")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| crate::branding::DEFAULT_API_ADDR.to_string())
}

pub fn default_api_url() -> String {
    format!("http://{}", default_api_addr())
}

/// Resolve `~/.cursor/mcp.json`. Kept as a thin, single call site (the
/// command layer) rather than something core logic reaches for internally, so
/// the rest of this module stays testable without mutating process-global
/// `HOME`.
pub fn cursor_mcp_config_path() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME is not set".to_string())?;
    Ok(PathBuf::from(home).join(".cursor").join("mcp.json"))
}

// ---------------------------------------------------------------------------
// Absolute Node/npm detection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimePaths {
    pub node_path: String,
    pub npm_path: String,
    pub node_version: String,
}

/// Absolute-path candidates for a Homebrew/system/nvm-installed `node`, in
/// priority order. Managed Cursor config must never depend on shell `PATH`,
/// `npx`, or `nvm`'s shell function — macOS GUI apps do not inherit a login
/// shell's profile, so a bare `node`/`npm` lookup would silently break.
fn node_candidates(home: Option<&str>) -> Vec<PathBuf> {
    let mut candidates = vec![
        PathBuf::from("/opt/homebrew/bin/node"),
        PathBuf::from("/usr/local/bin/node"),
        PathBuf::from("/usr/bin/node"),
    ];
    if let Some(home) = home {
        if let Some(nvm_default) = resolve_nvm_default_node(Path::new(home)) {
            candidates.push(nvm_default);
        }
    }
    candidates
}

/// `nvm` has no fixed absolute path for "the current default node" — it's a
/// shell function, not a real binary. Its `alias/default` file records which
/// version (or alias) that shell function would resolve to, so we can read
/// that intent and construct the real absolute path ourselves, without
/// invoking `nvm`.
///
/// `alias/default` doesn't always contain a literal version string — nvm
/// also accepts (and `nvm alias default <x>` commonly gets set to) the
/// aliases `node`/`stable` (latest installed), `lts/*` or a named LTS
/// codename like `lts/hydrogen`, and `system` (defer to the system/PATH
/// `node`, i.e. the candidates already ahead of this one in
/// [`node_candidates`]). Previously only a literal version string was
/// handled — any of these common alias forms would be treated as a literal
/// version, build a non-existent path, and silently fall through to
/// `NodeUnavailable` even though a working Node install exists.
fn resolve_nvm_default_node(home: &Path) -> Option<PathBuf> {
    let alias_path = home.join(".nvm").join("alias").join("default");
    let raw = std::fs::read_to_string(&alias_path).ok()?;
    let content = raw.trim();
    if content.is_empty() {
        return None;
    }
    resolve_nvm_alias(home, content)
}

fn nvm_versions_dir(home: &Path) -> PathBuf {
    home.join(".nvm").join("versions").join("node")
}

/// Every installed `~/.nvm/versions/node/v<major>.<minor>.<patch>` entry,
/// parsed for ordering. Filesystem-driven (rather than trusting any alias
/// file) since "latest installed" can only be answered by actually looking.
fn installed_nvm_node_versions(home: &Path) -> Vec<((u32, u32, u32), PathBuf)> {
    let Ok(entries) = std::fs::read_dir(nvm_versions_dir(home)) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let stripped = name.strip_prefix('v').unwrap_or(&name);
            let mut parts = stripped.split('.');
            let major: u32 = parts.next()?.parse().ok()?;
            let minor: u32 = parts.next().unwrap_or("0").parse().ok()?;
            let patch: u32 = parts.next().unwrap_or("0").parse().ok()?;
            Some(((major, minor, patch), entry.path().join("bin").join("node")))
        })
        .collect()
}

/// The highest installed version, optionally restricted to Node's
/// even-major-number LTS convention (Node has released only even majors as
/// LTS since v4; this is a filesystem-only heuristic — no network call —
/// consistent with this module never invoking `nvm`/`npm view` at
/// detection time).
fn latest_installed_nvm_node(home: &Path, lts_only: bool) -> Option<PathBuf> {
    installed_nvm_node_versions(home)
        .into_iter()
        .filter(|((major, _, _), _)| !lts_only || major % 2 == 0)
        .max_by_key(|(version, _)| *version)
        .map(|(_, path)| path)
}

fn literal_version_node_path(home: &Path, version: &str) -> PathBuf {
    let version = if version.starts_with('v') {
        version.to_string()
    } else {
        format!("v{version}")
    };
    nvm_versions_dir(home)
        .join(version)
        .join("bin")
        .join("node")
}

/// Resolve one `alias/default`-style content string to an absolute `node`
/// path. Handles the literal-version case plus the common alias forms
/// documented on [`resolve_nvm_default_node`].
fn resolve_nvm_alias(home: &Path, content: &str) -> Option<PathBuf> {
    match content {
        // "system" explicitly defers to the system/PATH node — there is
        // nothing nvm-specific to resolve, so returning None here correctly
        // lets the non-nvm candidates already in `node_candidates` win.
        "system" => None,
        "node" | "stable" => latest_installed_nvm_node(home, false),
        _ if content == "lts/*" => latest_installed_nvm_node(home, true),
        _ if content.starts_with("lts/") => {
            // A named LTS codename (e.g. "lts/hydrogen") is itself another
            // nvm alias file, one level down, that ultimately contains a
            // literal version.
            let codename = &content["lts/".len()..];
            let named_alias_path = home.join(".nvm").join("alias").join("lts").join(codename);
            let raw = std::fs::read_to_string(named_alias_path).ok()?;
            let literal = raw.trim();
            (!literal.is_empty()).then(|| literal_version_node_path(home, literal))
        }
        literal => Some(literal_version_node_path(home, literal)),
    }
}

fn node_version_of(node_path: &Path) -> Option<String> {
    let output = Command::new(node_path).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn node_major_version(version: &str) -> Option<u32> {
    version
        .trim_start_matches('v')
        .split('.')
        .next()?
        .parse()
        .ok()
}

/// The published floor for `@chaos-scheduler/mcp-server` (`engines.node`).
const MIN_NODE_MAJOR: u32 = 18;

/// Find the first candidate that exists, runs, and satisfies the package's
/// `engines.node >=18` floor. Pure/injectable so it's unit-testable without
/// touching the real filesystem outside a test's own tempdir.
pub fn find_node(candidates: &[PathBuf]) -> Option<(PathBuf, String)> {
    candidates.iter().find_map(|candidate| {
        if !candidate.is_file() {
            return None;
        }
        let version = node_version_of(candidate)?;
        if node_major_version(&version)? >= MIN_NODE_MAJOR {
            Some((candidate.clone(), version))
        } else {
            None
        }
    })
}

/// `npm` ships alongside `node` in the same bin directory for every install
/// method this module targets (Homebrew, system, nvm), so we look there
/// rather than maintaining a second candidate list.
fn npm_candidate_for(node_path: &Path) -> Option<PathBuf> {
    let candidate = node_path.parent()?.join("npm");
    candidate.is_file().then_some(candidate)
}

/// Real absolute-path detection used by production code.
pub fn detect_runtime() -> Option<RuntimePaths> {
    let home = std::env::var("HOME").ok();
    let (node_path, node_version) = find_node(&node_candidates(home.as_deref()))?;
    let npm_path = npm_candidate_for(&node_path)?;
    Some(RuntimePaths {
        node_path: node_path.to_string_lossy().into_owned(),
        npm_path: npm_path.to_string_lossy().into_owned(),
        node_version,
    })
}

/// Build an `npm` [`Command`] with `PATH` explicitly patched to include the
/// detected node's bin directory. Homebrew/system `npm` is itself a
/// `#!/usr/bin/env node` script — invoking its absolute path alone is not
/// enough if the *inheriting process's* PATH (a GUI app's minimal default,
/// not a login shell's) can't resolve `node`. This is the other half of
/// "absolute paths, never shell PATH": we pin what PATH the child sees rather
/// than trusting whatever the parent process happened to inherit.
fn npm_command(npm_path: &str, node_path: &str) -> Command {
    let mut cmd = Command::new(npm_path);
    if let Some(bin_dir) = Path::new(node_path).parent() {
        let existing = std::env::var("PATH").unwrap_or_default();
        cmd.env(
            "PATH",
            format!("{}:/usr/bin:/bin:{existing}", bin_dir.display()),
        );
    }
    cmd
}

// ---------------------------------------------------------------------------
// Persisted manifest (`<app_data_dir>/mcp/managed-integration.json`)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ManagedManifest {
    pub enabled: bool,
    pub managed_id: Option<String>,
    pub managed_key_id: Option<String>,
    pub provisioned_version: Option<String>,
    pub node_path: Option<String>,
    pub npm_path: Option<String>,
    pub last_attempt_at: Option<String>,
    pub last_error: Option<String>,
}

impl ManagedManifest {
    fn manifest_path(app_data_dir: &Path) -> PathBuf {
        mcp_root(app_data_dir).join("managed-integration.json")
    }

    pub fn load(app_data_dir: &Path) -> Self {
        std::fs::read_to_string(Self::manifest_path(app_data_dir))
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, app_data_dir: &Path) -> Result<(), String> {
        let path = Self::manifest_path(app_data_dir);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        write_atomic(&path, json.as_bytes())
    }
}

/// Write-to-temp-then-rename so a crash or concurrent read never observes a
/// half-written manifest or Cursor config.
fn write_atomic(path: &Path, contents: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension(format!("tmp-{}", uuid::Uuid::new_v4()));
    std::fs::write(&tmp, contents).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

fn mcp_root(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("mcp")
}

fn versions_dir(app_data_dir: &Path) -> PathBuf {
    mcp_root(app_data_dir).join("versions")
}

fn version_dir(app_data_dir: &Path, version: &str) -> PathBuf {
    versions_dir(app_data_dir).join(version)
}

// ---------------------------------------------------------------------------
// Cursor `mcp.json` non-destructive merge
// ---------------------------------------------------------------------------

fn is_managed_entry(entry: &serde_json::Value) -> bool {
    entry
        .get("env")
        .and_then(|env| env.get("CHAOS_SCHEDULER_MANAGED_BY"))
        .and_then(|v| v.as_str())
        == Some(MANAGED_BY_MARKER)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeOutcome {
    Written,
    ConflictUnmanaged,
}

/// Snapshot of what's currently in `~/.cursor/mcp.json` for the
/// `chaos-scheduler` entry, used by status checks (never mutates the file).
#[derive(Debug, Clone, Copy, Default)]
pub struct CursorConfigState {
    pub registered: bool,
    pub conflict: bool,
}

pub fn inspect_cursor_config(config_path: &Path) -> CursorConfigState {
    let Ok(raw) = std::fs::read_to_string(config_path) else {
        return CursorConfigState::default();
    };
    let Ok(root) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return CursorConfigState::default();
    };
    let Some(existing) = root
        .get("mcpServers")
        .and_then(|s| s.get("chaos-scheduler"))
    else {
        return CursorConfigState::default();
    };
    if is_managed_entry(existing) {
        CursorConfigState {
            registered: true,
            conflict: false,
        }
    } else {
        CursorConfigState {
            registered: false,
            conflict: true,
        }
    }
}

/// Read back the token from our own previously-written managed entry, so a
/// repair/re-provision can reuse the working key instead of needlessly
/// reminting one (the API never returns a token after creation, so this is
/// the only way to "recover" it — see the module doc's token-lifecycle note).
fn read_existing_managed_token(config_path: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(config_path).ok()?;
    let root: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let entry = root.get("mcpServers")?.get("chaos-scheduler")?;
    if !is_managed_entry(entry) {
        return None;
    }
    entry
        .get("env")?
        .get("CHAOS_SCHEDULER_API_KEY")?
        .as_str()
        .map(str::to_string)
}

/// Build a backup path for invalid-JSON config content. A timestamp alone is
/// only second-granular, so two invalid-JSON encounters within the same
/// second (e.g. two rapid re-provision retries) would collide and silently
/// overwrite each other's backup; a uuid suffix guarantees uniqueness
/// regardless of timing while the timestamp keeps the filename
/// human-sortable.
fn invalid_json_backup_path(config_path: &Path) -> PathBuf {
    config_path.with_extension(format!(
        "json.invalid-{}-{}",
        chrono::Utc::now().format("%Y%m%dT%H%M%S"),
        uuid::Uuid::new_v4()
    ))
}

/// Non-destructively merge the managed `chaos-scheduler` entry into
/// `~/.cursor/mcp.json`: preserves every other entry, backs up before
/// writing, writes atomically, and refuses to clobber a pre-existing
/// `chaos-scheduler` entry this app didn't create unless `force` is set.
/// Invalid existing JSON is backed up (never silently discarded) and treated
/// as an empty config going forward.
#[allow(clippy::too_many_arguments)]
pub fn merge_mcp_config(
    config_path: &Path,
    managed_id: &str,
    node_path: &str,
    cli_path: &str,
    api_url: &str,
    api_key: &str,
    force: bool,
) -> Result<MergeOutcome, String> {
    let mut root: serde_json::Value = match std::fs::read_to_string(config_path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_else(|_| {
            let backup = invalid_json_backup_path(config_path);
            let _ = std::fs::copy(config_path, &backup);
            serde_json::json!({})
        }),
        Err(_) => serde_json::json!({}),
    };
    if !root.is_object() {
        root = serde_json::json!({});
    }

    let obj = root.as_object_mut().expect("checked is_object above");
    if !matches!(obj.get("mcpServers"), Some(serde_json::Value::Object(_))) {
        obj.insert("mcpServers".to_string(), serde_json::json!({}));
    }
    let servers = obj
        .get_mut("mcpServers")
        .and_then(|s| s.as_object_mut())
        .expect("just ensured mcpServers is an object");

    if let Some(existing) = servers.get("chaos-scheduler") {
        if !is_managed_entry(existing) && !force {
            return Ok(MergeOutcome::ConflictUnmanaged);
        }
    }

    servers.insert(
        "chaos-scheduler".to_string(),
        serde_json::json!({
            "command": node_path,
            "args": [cli_path],
            "env": {
                "CHAOS_SCHEDULER_URL": api_url,
                "CHAOS_SCHEDULER_API_KEY": api_key,
                "CHAOS_SCHEDULER_MANAGED_BY": MANAGED_BY_MARKER,
                "CHAOS_SCHEDULER_MANAGED_ID": managed_id,
            },
        }),
    );

    if config_path.exists() {
        let _ = std::fs::copy(config_path, config_path.with_extension("json.bak"));
    }
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    write_atomic(config_path, json.as_bytes())?;
    Ok(MergeOutcome::Written)
}

/// Remove the managed `chaos-scheduler` entry, but only if it's ours — an
/// unmanaged entry is left completely alone. Returns whether anything was
/// removed.
pub fn remove_mcp_config_entry(config_path: &Path) -> Result<bool, String> {
    let Ok(raw) = std::fs::read_to_string(config_path) else {
        return Ok(false);
    };
    let Ok(mut root) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return Ok(false);
    };
    let Some(servers) = root.get_mut("mcpServers").and_then(|s| s.as_object_mut()) else {
        return Ok(false);
    };
    let Some(existing) = servers.get("chaos-scheduler") else {
        return Ok(false);
    };
    if !is_managed_entry(existing) {
        return Ok(false);
    }
    servers.remove("chaos-scheduler");

    let _ = std::fs::copy(config_path, config_path.with_extension("json.bak"));
    let json = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    write_atomic(config_path, json.as_bytes())?;
    Ok(true)
}

// ---------------------------------------------------------------------------
// Staging / install / smoke-check / atomic promote / prune
// ---------------------------------------------------------------------------

fn installed_package_dir(root: &Path) -> PathBuf {
    root.join("node_modules")
        .join("@chaos-scheduler")
        .join("mcp-server")
}

/// Resolve the installed CLI entrypoint by reading the package's own `bin`
/// field, rather than hardcoding `dist/cli.js` — resilient to any future
/// dist-layout change in `@chaos-scheduler/mcp-server`.
///
/// Defense-in-depth: canonicalizes the resolved path and rejects it if it
/// escapes `package_dir` (e.g. a `bin` field containing `../../` or an
/// absolute path). Not independently exploitable today — the `bin` field
/// comes from a package this app itself installed via a pinned, exact npm
/// spec — but cheap to close and protects against a future compromised
/// registry entry smuggling a `bin` pointing outside the install root.
pub fn resolve_cli_path(package_dir: &Path) -> Result<PathBuf, String> {
    let pkg_json_path = package_dir.join("package.json");
    let raw = std::fs::read_to_string(&pkg_json_path)
        .map_err(|e| format!("reading {}: {e}", pkg_json_path.display()))?;
    let pkg: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| format!("parsing {}: {e}", pkg_json_path.display()))?;
    let bin = pkg
        .get("bin")
        .ok_or_else(|| format!("{} has no \"bin\" field", pkg_json_path.display()))?;
    let rel = match bin {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(map) => map
            .get("chaos-mcp-server")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "package.json \"bin\" has no chaos-mcp-server entry".to_string())?
            .to_string(),
        _ => return Err("unexpected package.json \"bin\" shape".to_string()),
    };
    let candidate = package_dir.join(&rel);

    let canonical_package_dir = package_dir
        .canonicalize()
        .map_err(|e| format!("canonicalizing {}: {e}", package_dir.display()))?;
    let canonical_candidate = candidate
        .canonicalize()
        .map_err(|e| format!("canonicalizing {}: {e}", candidate.display()))?;
    if !canonical_candidate.starts_with(&canonical_package_dir) {
        return Err(format!(
            "package.json \"bin\" entry {rel:?} resolves outside the package directory \
             ({})",
            canonical_candidate.display()
        ));
    }
    Ok(canonical_candidate)
}

fn npm_install(
    npm_path: &str,
    node_path: &str,
    prefix: &Path,
    version: &str,
) -> Result<(), String> {
    std::fs::create_dir_all(prefix).map_err(|e| e.to_string())?;
    let spec = format!("{MCP_PACKAGE_NAME}@{version}");
    let output = npm_command(npm_path, node_path)
        .args([
            "install",
            "--prefix",
            &prefix.to_string_lossy(),
            "--no-audit",
            "--no-fund",
            "--no-save",
            // Defense against npm's classic supply-chain attack vector:
            // mcp-server has no native/build-step dependency that needs a
            // lifecycle script, and this install runs non-interactively
            // (including silently from the startup re-provision thread), so
            // there is no legitimate reason to execute arbitrary
            // preinstall/postinstall code from any package in the tree.
            "--ignore-scripts",
            &spec,
        ])
        .output()
        .map_err(|e| format!("spawning npm: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "npm install {spec} failed (exit {:?}): {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

fn smoke_check(node_path: &str, cli_path: &Path) -> Result<(), String> {
    let output = Command::new(node_path)
        .arg(cli_path)
        .arg("--help")
        .output()
        .map_err(|e| format!("spawning node: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() || !stdout.contains("chaos-mcp-server") {
        return Err(format!(
            "installed CLI smoke check failed (exit {:?}): {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

/// Atomically switch `mcp/versions/<version>/` to the staged install. If a
/// dir for this exact version already exists (re-running provision for a
/// version that's already current), the old one is displaced via a same-
/// filesystem rename first so the switch itself stays a single atomic
/// rename, not a delete-then-move race.
fn promote_staging(app_data_dir: &Path, staging: &Path, version: &str) -> Result<PathBuf, String> {
    let target = version_dir(app_data_dir, version);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if target.exists() {
        let displaced = mcp_root(app_data_dir).join(format!("displaced-{}", uuid::Uuid::new_v4()));
        std::fs::rename(&target, &displaced).map_err(|e| e.to_string())?;
        let rename_result = std::fs::rename(staging, &target);
        let _ = std::fs::remove_dir_all(&displaced);
        rename_result.map_err(|e| e.to_string())?;
    } else {
        std::fs::rename(staging, &target).map_err(|e| e.to_string())?;
    }
    Ok(target)
}

/// Delete every promoted version dir except `keep_version`. Only ever called
/// after a new version has been staged, smoke-checked, promoted, *and*
/// registered in Cursor — never before, so a failed provision always leaves
/// a working previous version in place.
fn prune_old_versions(app_data_dir: &Path, keep_version: &str) {
    let Ok(entries) = std::fs::read_dir(versions_dir(app_data_dir)) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.file_name() != keep_version {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

fn mint_key(service: &SchedulerService) -> Result<(String, String), String> {
    let key = service
        .create_api_key(Some("Managed MCP integration"), &["read", "write"])
        .map_err(|e| e.to_string())?;
    Ok((key.id, key.token))
}

fn key_is_alive(service: &SchedulerService, key_id: &str) -> bool {
    service
        .list_api_keys()
        .map(|keys| keys.iter().any(|k| k.id == key_id && !k.revoked))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallStatus {
    NotInstalled,
    Installed,
    Stale,
    NodeUnavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpIntegrationStatus {
    pub enabled: bool,
    pub install_status: InstallStatus,
    pub node_available: bool,
    pub node_path: Option<String>,
    pub npm_available: bool,
    pub npm_path: Option<String>,
    pub provisioned_version: Option<String>,
    pub pinned_version: String,
    pub registered_in_cursor: bool,
    pub cursor_config_conflict: bool,
    pub api_reachable: bool,
    pub managed_key_id: Option<String>,
    pub matches: bool,
    pub last_error: Option<String>,
}

/// Core, dependency-injected status computation (no filesystem probing of
/// Node/npm, no network) — unit-tested directly. [`status`] is the thin
/// production wrapper that supplies real runtime detection + a real API
/// reachability probe.
fn status_with(
    app_data_dir: &Path,
    service: &SchedulerService,
    config_path: &Path,
    runtime: Option<&RuntimePaths>,
    api_reachable: bool,
) -> McpIntegrationStatus {
    let manifest = ManagedManifest::load(app_data_dir);
    let pinned = pinned_mcp_version().to_string();
    let cursor_state = inspect_cursor_config(config_path);

    let key_alive = manifest
        .managed_key_id
        .as_deref()
        .is_some_and(|id| key_is_alive(service, id));

    let version_matches = manifest.provisioned_version.as_deref() == Some(pinned.as_str());
    let install_status = if runtime.is_none() {
        InstallStatus::NodeUnavailable
    } else if manifest.provisioned_version.is_none() {
        InstallStatus::NotInstalled
    } else if version_matches {
        InstallStatus::Installed
    } else {
        InstallStatus::Stale
    };

    McpIntegrationStatus {
        enabled: manifest.enabled,
        install_status,
        node_available: runtime.is_some(),
        node_path: runtime
            .map(|r| r.node_path.clone())
            .or_else(|| manifest.node_path.clone()),
        npm_available: runtime.is_some(),
        npm_path: runtime
            .map(|r| r.npm_path.clone())
            .or_else(|| manifest.npm_path.clone()),
        provisioned_version: manifest.provisioned_version.clone(),
        pinned_version: pinned,
        registered_in_cursor: cursor_state.registered,
        cursor_config_conflict: cursor_state.conflict,
        api_reachable,
        managed_key_id: key_alive.then(|| manifest.managed_key_id.clone()).flatten(),
        matches: version_matches && cursor_state.registered && key_alive,
        last_error: manifest.last_error.clone(),
    }
}

fn check_api_reachable() -> bool {
    let url = format!("{}/api/v1/health", default_api_url());
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_millis(500))
        .build()
        .ok()
        .and_then(|client| client.get(&url).send().ok())
        .map(|resp| resp.status().is_success())
        .unwrap_or(false)
}

pub fn status(
    app_data_dir: &Path,
    service: &SchedulerService,
    config_path: &Path,
) -> McpIntegrationStatus {
    status_with(
        app_data_dir,
        service,
        config_path,
        detect_runtime().as_ref(),
        check_api_reachable(),
    )
}

// ---------------------------------------------------------------------------
// Provision / remove
// ---------------------------------------------------------------------------

/// Core, dependency-injected provisioning logic — takes an already-detected
/// [`RuntimePaths`] so it's unit-testable with fake `node`/`npm` fixtures
/// instead of real Homebrew paths or the real npm registry. [`provision`] is
/// the thin production wrapper that runs real Node detection first.
fn provision_with_runtime(
    app_data_dir: &Path,
    service: &SchedulerService,
    config_path: &Path,
    runtime: &RuntimePaths,
    force: bool,
) -> Result<McpIntegrationStatus, String> {
    let mut manifest = ManagedManifest::load(app_data_dir);
    manifest.last_attempt_at = Some(chrono::Utc::now().to_rfc3339());
    manifest.node_path = Some(runtime.node_path.clone());
    manifest.npm_path = Some(runtime.npm_path.clone());

    let pinned = pinned_mcp_version().to_string();
    let key_alive = manifest
        .managed_key_id
        .as_deref()
        .is_some_and(|id| key_is_alive(service, id));

    // Idempotent no-op: already provisioned at the pinned version, registered
    // in Cursor, and the managed key is still live. Re-running provision
    // (e.g. a launch-time re-provision check that finds nothing changed)
    // never re-installs or re-mints anything in this case.
    let already_current = !force
        && manifest.provisioned_version.as_deref() == Some(pinned.as_str())
        && inspect_cursor_config(config_path).registered
        && key_alive;
    if already_current {
        manifest.enabled = true;
        manifest.last_error = None;
        manifest.save(app_data_dir)?;
        return Ok(status_with(
            app_data_dir,
            service,
            config_path,
            Some(runtime),
            check_api_reachable(),
        ));
    }

    let managed_id = manifest
        .managed_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let (key_id, token) = if key_alive {
        match read_existing_managed_token(config_path) {
            Some(existing_token) => (manifest.managed_key_id.clone().unwrap(), existing_token),
            None => {
                if let Some(old_id) = &manifest.managed_key_id {
                    let _ = service.revoke_api_key(old_id);
                }
                mint_key(service)?
            }
        }
    } else {
        if let Some(old_id) = &manifest.managed_key_id {
            let _ = service.revoke_api_key(old_id);
        }
        mint_key(service)?
    };

    let staging = mcp_root(app_data_dir).join(format!("staging-{pinned}-{}", uuid::Uuid::new_v4()));
    let stage_result = npm_install(&runtime.npm_path, &runtime.node_path, &staging, &pinned)
        .and_then(|()| {
            let cli_path = resolve_cli_path(&installed_package_dir(&staging))?;
            smoke_check(&runtime.node_path, &cli_path)?;
            Ok(())
        });

    if let Err(err) = stage_result {
        let _ = std::fs::remove_dir_all(&staging);
        // Deliberately do NOT persist `managed_id`/`managed_key_id` here: a
        // newly-minted `key_id` only becomes the source of truth once it's
        // actually embedded in `mcp.json` by a successful merge below. If we
        // saved it now, `manifest.managed_key_id` would point at a live key
        // while `mcp.json` still (or never) carries its token — the next
        // launch's `key_is_alive` check would then see a live key, treat the
        // stale/absent config as "already current", and silently report a
        // healthy status while every real MCP call 401s. Leaving the old
        // value in place means a revoked old key is correctly seen as dead
        // on the next attempt, so re-provision keeps retrying instead of
        // falsely settling into "healthy".
        manifest.last_error = Some(err.clone());
        manifest.enabled = true;
        manifest.save(app_data_dir)?;
        return Err(err);
    }

    let promoted_dir = promote_staging(app_data_dir, &staging, &pinned)?;
    let cli_path = resolve_cli_path(&installed_package_dir(&promoted_dir))?;

    let merge_outcome = merge_mcp_config(
        config_path,
        &managed_id,
        &runtime.node_path,
        &cli_path.to_string_lossy(),
        &default_api_url(),
        &token,
        force,
    )?;

    if merge_outcome == MergeOutcome::ConflictUnmanaged {
        manifest.last_error = Some(
            "~/.cursor/mcp.json already has an unmanaged \"chaos-scheduler\" entry — re-provision \
             with force to take it over."
                .to_string(),
        );
        manifest.enabled = true;
        manifest.save(app_data_dir)?;
        return Ok(status_with(
            app_data_dir,
            service,
            config_path,
            Some(runtime),
            check_api_reachable(),
        ));
    }

    // Only now — after `mcp.json` itself has actually been written with this
    // `key_id`'s token — is it safe to make `managed_key_id` the source of
    // truth for "the live config is correct". See the comment on the
    // staging-failure branch above for why this can't happen any earlier.
    manifest.managed_id = Some(managed_id.clone());
    manifest.managed_key_id = Some(key_id.clone());

    // Only prune the previous version now that the new one is staged,
    // smoke-checked, promoted, and registered in Cursor.
    prune_old_versions(app_data_dir, &pinned);

    manifest.enabled = true;
    manifest.provisioned_version = Some(pinned);
    manifest.last_error = None;
    manifest.save(app_data_dir)?;

    Ok(status_with(
        app_data_dir,
        service,
        config_path,
        Some(runtime),
        check_api_reachable(),
    ))
}

/// Production entry point: detects Node/npm for real, then delegates to
/// [`provision_with_runtime`]. When Node can't be found at any known absolute
/// location, this degrades to a status report rather than an error — per the
/// plan, a missing runtime makes the *integration* unavailable, never the
/// app itself.
pub fn provision(
    app_data_dir: &Path,
    service: &SchedulerService,
    config_path: &Path,
    force: bool,
) -> Result<McpIntegrationStatus, String> {
    let Some(runtime) = detect_runtime() else {
        let mut manifest = ManagedManifest::load(app_data_dir);
        manifest.enabled = true;
        manifest.last_attempt_at = Some(chrono::Utc::now().to_rfc3339());
        manifest.last_error = Some(
            "Node.js was not found at any known absolute install location (Homebrew, system, or \
             nvm default). Install Node >=18 to enable the managed Cursor/MCP integration."
                .to_string(),
        );
        manifest.save(app_data_dir)?;
        return Ok(status_with(app_data_dir, service, config_path, None, false));
    };
    provision_with_runtime(app_data_dir, service, config_path, &runtime, force)
}

/// Remove the managed integration: drop the managed `mcp.json` entry (only if
/// it's ours), delete the app-managed install dir, and revoke the managed
/// key. Best-effort at every step (never panics on a missing file) so a
/// partially-broken prior state can always be cleaned up.
pub fn remove(
    app_data_dir: &Path,
    service: &SchedulerService,
    config_path: &Path,
    prepare_to_uninstall: bool,
) -> Result<McpIntegrationStatus, String> {
    let manifest = ManagedManifest::load(app_data_dir);

    let _ = remove_mcp_config_entry(config_path);

    if let Some(key_id) = &manifest.managed_key_id {
        let _ = service.revoke_api_key(key_id);
    }

    // Remove the whole managed root (versions, staging, and the manifest
    // itself) rather than deleting-then-resaving a default manifest: an
    // absent manifest already loads as `ManagedManifest::default()`, so
    // there's no state to preserve, and leaving no directory behind is what
    // "removed" should mean on disk.
    let _ = std::fs::remove_dir_all(mcp_root(app_data_dir));

    if prepare_to_uninstall {
        let _ = crate::scheduler::uninstall_launchd_plist();
    }

    Ok(status_with(
        app_data_dir,
        service,
        config_path,
        detect_runtime().as_ref(),
        check_api_reachable(),
    ))
}

/// Startup re-provision hook (plan Section 12 "Auto-update (re-provision)").
/// If the managed integration was previously enabled, silently repair it in
/// the background: [`provision`] is already idempotent, so this is a no-op
/// unless the pinned version, the Cursor registration, or the managed key
/// have drifted since the last launch (e.g. an app auto-update just stamped
/// a new pinned `mcp-server` version). Takes the same single-flight
/// [`McpState`] lock as the `provision_mcp_integration` /
/// `remove_mcp_integration` commands (plan invariant: "UI clicks, launch
/// retry, and post-update re-provision must share one lock"), so it simply
/// skips this launch if a user-initiated call is already in flight rather
/// than racing it. Runs on a plain OS thread rather than the async runtime
/// because `provision` performs blocking subprocess/HTTP calls. Never blocks
/// or fails app startup: a failure only updates the manifest's `last_error`
/// field for the Integrations card to surface.
/// Best-effort cleanup of leftover `mcp/staging-*` and `mcp/displaced-*`
/// directories left behind by an install that was interrupted before it
/// could finish or clean up after itself (OOM, force-quit, crash, or a
/// racing `apply_update` restart). [`promote_staging`]'s "displaced" dirs
/// and [`npm_install`]'s staging dirs are both meant to be transient — the
/// happy path always removes them — but nothing previously reclaimed them
/// if the process died mid-install, so they could accumulate indefinitely.
/// Pure/filesystem-only so it's unit-testable without a real app handle.
fn sweep_orphaned_staging_dirs(app_data_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(mcp_root(app_data_dir)) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("staging-") || name.starts_with("displaced-") {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// Production wrapper around [`sweep_orphaned_staging_dirs`]: resolves the
/// real app data dir and sweeps it. Must be called before
/// [`spawn_reprovision_on_startup`] so a fresh re-provision attempt never
/// mistakes a stale in-progress staging dir left over from a previous crash
/// for anything meaningful.
pub fn sweep_orphaned_staging_dirs_on_startup(app: &tauri::AppHandle) {
    use tauri::Manager;
    match app.path().app_data_dir() {
        Ok(dir) => sweep_orphaned_staging_dirs(&dir),
        Err(err) => log::warn!("Skipping orphaned MCP staging-dir sweep: {err}"),
    }
}

pub fn spawn_reprovision_on_startup(app: tauri::AppHandle) {
    use tauri::Manager;
    std::thread::spawn(move || {
        let app_data_dir = match app.path().app_data_dir() {
            Ok(dir) => dir,
            Err(err) => {
                log::warn!("Skipping startup MCP re-provision: {err}");
                return;
            }
        };
        if !ManagedManifest::load(&app_data_dir).enabled {
            return;
        }
        let config_path = match cursor_mcp_config_path() {
            Ok(path) => path,
            Err(err) => {
                log::warn!("Skipping startup MCP re-provision: {err}");
                return;
            }
        };
        let mcp_state = app.state::<McpState>();
        let _guard = match try_lock_recovering(&mcp_state) {
            Ok(guard) => guard,
            Err(_) => {
                log::info!(
                    "Skipping startup MCP re-provision: a provisioning call is already in flight"
                );
                return;
            }
        };
        let service = app.state::<crate::commands::AppState>().service.clone();
        if let Err(err) = provision(&app_data_dir, &service, &config_path, false) {
            log::warn!(
                "Startup MCP re-provision failed (previous integration state is left in place): {err}"
            );
        }
        // Emit regardless of the outcome above: a page that mounted before
        // this background thread finished (Ok or Err) must not be left
        // showing a stale pre-startup-hook status indefinitely.
        let status = status(&app_data_dir, &service, &config_path);
        emit_status_changed(&app, &status);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::service::{NoopNotifier, SchedulerService};
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Arc;

    fn tmpdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("chaos-mcp-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn test_service(dir: &Path) -> SchedulerService {
        let db = Arc::new(Database::new(dir));
        SchedulerService::new(db, Arc::new(NoopNotifier))
    }

    fn write_executable(path: &Path, script: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, script).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// A fake `node` fixture: reports a fixed `--version`, and for any other
    /// invocation (i.e. `<fake-node> <cli.js> --help`) prints text containing
    /// "chaos-mcp-server" so [`smoke_check`]'s substring check passes,
    /// without needing a real JS runtime.
    fn write_fake_node(path: &Path, version: &str) {
        write_executable(
            path,
            &format!(
                "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo \"{version}\"; else echo \"chaos-mcp-server fixture\"; fi\n"
            ),
        );
    }

    /// A fake `npm` fixture: ignores the real registry entirely and just
    /// materializes a minimal, valid `@chaos-scheduler/mcp-server` install
    /// (package.json + a `dist/cli.js` stub) under whatever `--prefix` it was
    /// given, so provisioning can be exercised end-to-end offline.
    fn write_fake_npm(path: &Path, installed_version: &str) {
        write_executable(
            path,
            &format!(
                r#"#!/bin/sh
prefix=""
prev=""
for arg in "$@"; do
  if [ "$prev" = "--prefix" ]; then prefix="$arg"; fi
  prev="$arg"
done
mkdir -p "$prefix/node_modules/@chaos-scheduler/mcp-server/dist"
cat > "$prefix/node_modules/@chaos-scheduler/mcp-server/package.json" <<EOF
{{"name":"@chaos-scheduler/mcp-server","version":"{installed_version}","bin":{{"chaos-mcp-server":"./dist/cli.js"}}}}
EOF
echo "// fixture" > "$prefix/node_modules/@chaos-scheduler/mcp-server/dist/cli.js"
exit 0
"#
            ),
        );
    }

    /// Same as [`write_fake_npm`] but the installed package has no `bin`
    /// field, so [`resolve_cli_path`] (and therefore the smoke check) fails —
    /// used to exercise the rollback path.
    fn write_broken_fake_npm(path: &Path) {
        write_executable(
            path,
            r#"#!/bin/sh
prefix=""
prev=""
for arg in "$@"; do
  if [ "$prev" = "--prefix" ]; then prefix="$arg"; fi
  prev="$arg"
done
mkdir -p "$prefix/node_modules/@chaos-scheduler/mcp-server"
echo '{"name":"@chaos-scheduler/mcp-server","version":"0.0.0"}' > "$prefix/node_modules/@chaos-scheduler/mcp-server/package.json"
exit 0
"#,
        );
    }

    /// A fake `npm` that behaves like [`write_fake_npm`] but also appends its
    /// full argv (one per line) to `record_path`, so a test can assert on
    /// exactly which flags `npm_install` invoked it with.
    fn write_recording_fake_npm(path: &Path, installed_version: &str, record_path: &Path) {
        write_executable(
            path,
            &format!(
                r#"#!/bin/sh
printf '%s\n' "$@" >> "{record}"
prefix=""
prev=""
for arg in "$@"; do
  if [ "$prev" = "--prefix" ]; then prefix="$arg"; fi
  prev="$arg"
done
mkdir -p "$prefix/node_modules/@chaos-scheduler/mcp-server/dist"
cat > "$prefix/node_modules/@chaos-scheduler/mcp-server/package.json" <<EOF
{{"name":"@chaos-scheduler/mcp-server","version":"{installed_version}","bin":{{"chaos-mcp-server":"./dist/cli.js"}}}}
EOF
echo "// fixture" > "$prefix/node_modules/@chaos-scheduler/mcp-server/dist/cli.js"
exit 0
"#,
                record = record_path.display()
            ),
        );
    }

    /// Regression test for the "npm install runs with lifecycle scripts
    /// enabled" finding: `npm_install` must always pass `--ignore-scripts`,
    /// closing the standard npm supply-chain `postinstall` attack vector —
    /// this install runs non-interactively, including from the silent
    /// startup re-provision thread.
    #[test]
    fn npm_install_always_passes_ignore_scripts() {
        let dir = tmpdir();
        let node_path = dir.join("bin").join("node");
        let npm_path = dir.join("bin").join("npm");
        write_fake_node(&node_path, "v20.11.0");
        let record_path = dir.join("npm-invocations.log");
        write_recording_fake_npm(&npm_path, "0.5.0", &record_path);

        let prefix = dir.join("install-prefix");
        npm_install(
            &npm_path.to_string_lossy(),
            &node_path.to_string_lossy(),
            &prefix,
            "0.5.0",
        )
        .unwrap();

        let recorded = std::fs::read_to_string(&record_path).unwrap();
        assert!(
            recorded.lines().any(|arg| arg == "--ignore-scripts"),
            "npm_install must pass --ignore-scripts, got args: {recorded:?}"
        );
    }

    fn fake_runtime(dir: &Path, node_version: &str, npm_kind: &str) -> RuntimePaths {
        let bin = dir.join("bin");
        let node_path = bin.join("node");
        let npm_path = bin.join("npm");
        write_fake_node(&node_path, node_version);
        match npm_kind {
            "broken" => write_broken_fake_npm(&npm_path),
            version => write_fake_npm(&npm_path, version),
        }
        RuntimePaths {
            node_path: node_path.to_string_lossy().into_owned(),
            npm_path: npm_path.to_string_lossy().into_owned(),
            node_version: node_version.to_string(),
        }
    }

    // --- McpState lock poison recovery ----------------------------------

    /// Regression test for the "mutex-poisoning is unhandled" finding: a
    /// panic anywhere while holding `McpState::lock` (e.g. in some future
    /// change) must not permanently brick every future provision/remove
    /// call with a misleading "already in progress" error — the lock must
    /// be recoverable, exactly like `update.rs`'s snapshot lock already is.
    #[test]
    fn try_lock_recovering_recovers_from_a_poisoned_lock() {
        let state = Arc::new(McpState::default());

        // Poison the lock by panicking on another thread while holding it.
        let poisoner = Arc::clone(&state);
        let handle = std::thread::spawn(move || {
            let _guard = poisoner.lock.lock().unwrap();
            panic!("simulated panic while holding McpState::lock");
        });
        assert!(handle.join().is_err(), "the poisoner thread must panic");
        assert!(
            state.lock.is_poisoned(),
            "the mutex must be poisoned after the panic"
        );

        // A subsequent call must still succeed rather than reporting "busy"
        // — a `std::sync::Mutex` stays flagged `is_poisoned()` forever once
        // poisoned (there is no automatic un-poisoning), so every future
        // acquisition must independently recover, not just the first one.
        for _ in 0..2 {
            let result = try_lock_recovering(&state);
            assert!(
                result.is_ok(),
                "try_lock_recovering must recover from poison, got {result:?}"
            );
        }
    }

    #[test]
    fn try_lock_recovering_reports_busy_when_genuinely_held() {
        let state = McpState::default();
        let _held = state.lock.try_lock().unwrap();

        let result = try_lock_recovering(&state);
        assert_eq!(
            result.err(),
            Some("MCP provisioning is already in progress")
        );
    }

    // --- pinned version -----------------------------------------------

    #[test]
    fn pinned_version_trims_whitespace_and_newlines() {
        assert_eq!(trim_pinned_version("0.5.0\n"), "0.5.0");
        assert_eq!(trim_pinned_version("  0.5.0  \n"), "0.5.0");
    }

    #[test]
    fn pinned_mcp_version_reads_the_checked_in_file() {
        // Sanity check that the include_str! wiring + trimming actually
        // reflects src-tauri/mcp-pinned-version.txt.
        assert!(!pinned_mcp_version().is_empty());
        assert!(!pinned_mcp_version().contains('\n'));
    }

    // --- node/npm detection ---------------------------------------------

    #[test]
    fn find_node_skips_versions_below_the_floor() {
        let dir = tmpdir();
        let too_old = dir.join("old-node");
        let ok = dir.join("new-node");
        write_fake_node(&too_old, "v14.21.3");
        write_fake_node(&ok, "v20.11.0");

        let found = find_node(&[too_old.clone(), ok.clone()]);
        assert_eq!(found, Some((ok, "v20.11.0".to_string())));
    }

    #[test]
    fn find_node_returns_none_when_nothing_matches() {
        let dir = tmpdir();
        let missing = dir.join("does-not-exist");
        let too_old = dir.join("old-node");
        write_fake_node(&too_old, "v16.0.0");

        assert_eq!(find_node(&[missing, too_old]), None);
    }

    #[test]
    fn npm_candidate_prefers_sibling_of_node() {
        let dir = tmpdir();
        let node_path = dir.join("bin").join("node");
        let npm_path = dir.join("bin").join("npm");
        write_fake_node(&node_path, "v20.0.0");
        write_executable(&npm_path, "#!/bin/sh\nexit 0\n");

        assert_eq!(npm_candidate_for(&node_path), Some(npm_path));
    }

    #[test]
    fn npm_candidate_is_none_without_a_sibling_binary() {
        let dir = tmpdir();
        let node_path = dir.join("bin").join("node");
        write_fake_node(&node_path, "v20.0.0");

        assert_eq!(npm_candidate_for(&node_path), None);
    }

    // --- nvm alias resolution -------------------------------------------

    fn write_nvm_default_alias(home: &Path, content: &str) {
        let alias_dir = home.join(".nvm").join("alias");
        std::fs::create_dir_all(&alias_dir).unwrap();
        std::fs::write(alias_dir.join("default"), content).unwrap();
    }

    fn touch_installed_nvm_version(home: &Path, version: &str) {
        std::fs::create_dir_all(nvm_versions_dir(home).join(version).join("bin")).unwrap();
    }

    #[test]
    fn resolve_nvm_default_node_handles_a_literal_version() {
        let home = tmpdir();
        write_nvm_default_alias(&home, "20.11.0");

        assert_eq!(
            resolve_nvm_default_node(&home),
            Some(
                nvm_versions_dir(&home)
                    .join("v20.11.0")
                    .join("bin")
                    .join("node")
            )
        );
    }

    /// Regression test: `alias/default` containing the common `node`/`stable`
    /// alias form (not a literal version) must resolve to the latest
    /// installed version, rather than being treated as a literal version
    /// string that builds a non-existent path.
    #[test]
    fn resolve_nvm_default_node_resolves_node_alias_to_latest_installed() {
        let home = tmpdir();
        touch_installed_nvm_version(&home, "v18.20.0");
        touch_installed_nvm_version(&home, "v22.1.0");
        touch_installed_nvm_version(&home, "v20.11.0");
        write_nvm_default_alias(&home, "node");

        assert_eq!(
            resolve_nvm_default_node(&home),
            Some(
                nvm_versions_dir(&home)
                    .join("v22.1.0")
                    .join("bin")
                    .join("node")
            )
        );
    }

    #[test]
    fn resolve_nvm_default_node_resolves_stable_alias_the_same_as_node() {
        let home = tmpdir();
        touch_installed_nvm_version(&home, "v20.11.0");
        write_nvm_default_alias(&home, "stable");

        assert_eq!(
            resolve_nvm_default_node(&home),
            Some(
                nvm_versions_dir(&home)
                    .join("v20.11.0")
                    .join("bin")
                    .join("node")
            )
        );
    }

    /// Regression test: `lts/*` must resolve to the latest installed
    /// *even-major* (LTS) version, skipping a newer odd-major (current,
    /// non-LTS) install.
    #[test]
    fn resolve_nvm_default_node_resolves_lts_star_to_latest_even_major() {
        let home = tmpdir();
        touch_installed_nvm_version(&home, "v21.5.0"); // current, non-LTS (odd)
        touch_installed_nvm_version(&home, "v20.11.0"); // LTS (even)
        touch_installed_nvm_version(&home, "v18.20.0"); // older LTS
        write_nvm_default_alias(&home, "lts/*");

        assert_eq!(
            resolve_nvm_default_node(&home),
            Some(
                nvm_versions_dir(&home)
                    .join("v20.11.0")
                    .join("bin")
                    .join("node")
            )
        );
    }

    /// Regression test: a named LTS codename (e.g. `lts/hydrogen`) is
    /// itself another nvm alias file one level down that contains the
    /// literal version.
    #[test]
    fn resolve_nvm_default_node_resolves_named_lts_codename() {
        let home = tmpdir();
        let lts_alias_dir = home.join(".nvm").join("alias").join("lts");
        std::fs::create_dir_all(&lts_alias_dir).unwrap();
        std::fs::write(lts_alias_dir.join("hydrogen"), "v18.20.0").unwrap();
        write_nvm_default_alias(&home, "lts/hydrogen");

        assert_eq!(
            resolve_nvm_default_node(&home),
            Some(
                nvm_versions_dir(&home)
                    .join("v18.20.0")
                    .join("bin")
                    .join("node")
            )
        );
    }

    /// Regression test: `system` must defer to the system/PATH node (return
    /// `None` here) rather than being misinterpreted as a literal version.
    #[test]
    fn resolve_nvm_default_node_returns_none_for_system_alias() {
        let home = tmpdir();
        write_nvm_default_alias(&home, "system");

        assert_eq!(resolve_nvm_default_node(&home), None);
    }

    // --- mcp.json merge ---------------------------------------------------

    #[test]
    fn merge_writes_new_entry_when_config_is_missing() {
        let dir = tmpdir();
        let config = dir.join("mcp.json");

        let outcome = merge_mcp_config(
            &config,
            "managed-1",
            "/bin/node",
            "/opt/cli.js",
            "http://127.0.0.1:9618",
            "tok",
            false,
        )
        .unwrap();
        assert_eq!(outcome, MergeOutcome::Written);

        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        let entry = &written["mcpServers"]["chaos-scheduler"];
        assert_eq!(entry["command"], "/bin/node");
        assert_eq!(entry["env"]["CHAOS_SCHEDULER_URL"], "http://127.0.0.1:9618");
        assert_eq!(
            entry["env"]["CHAOS_SCHEDULER_MANAGED_BY"],
            "Chaos Scheduler"
        );
        assert_eq!(entry["env"]["CHAOS_SCHEDULER_MANAGED_ID"], "managed-1");
    }

    #[test]
    fn merge_preserves_unrelated_mcp_servers() {
        let dir = tmpdir();
        let config = dir.join("mcp.json");
        std::fs::write(
            &config,
            serde_json::json!({
                "mcpServers": { "other-tool": { "command": "other", "args": [] } }
            })
            .to_string(),
        )
        .unwrap();

        merge_mcp_config(
            &config,
            "id",
            "/bin/node",
            "/cli.js",
            "http://x",
            "tok",
            false,
        )
        .unwrap();

        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(written["mcpServers"]["other-tool"]["command"], "other");
        assert!(written["mcpServers"]["chaos-scheduler"].is_object());
    }

    #[test]
    fn merge_detects_unmanaged_conflict_and_does_not_overwrite() {
        let dir = tmpdir();
        let config = dir.join("mcp.json");
        let original = serde_json::json!({
            "mcpServers": {
                "chaos-scheduler": { "command": "npx", "args": ["-y", "old"], "env": {} }
            }
        });
        std::fs::write(&config, original.to_string()).unwrap();

        let outcome = merge_mcp_config(
            &config,
            "id",
            "/bin/node",
            "/cli.js",
            "http://x",
            "tok",
            false,
        )
        .unwrap();
        assert_eq!(outcome, MergeOutcome::ConflictUnmanaged);

        let after: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(after, original, "unmanaged entry must be left untouched");
    }

    #[test]
    fn merge_with_force_overwrites_an_unmanaged_conflict() {
        let dir = tmpdir();
        let config = dir.join("mcp.json");
        std::fs::write(
            &config,
            serde_json::json!({
                "mcpServers": { "chaos-scheduler": { "command": "npx", "args": [], "env": {} } }
            })
            .to_string(),
        )
        .unwrap();

        let outcome = merge_mcp_config(
            &config,
            "id",
            "/bin/node",
            "/cli.js",
            "http://x",
            "tok",
            true,
        )
        .unwrap();
        assert_eq!(outcome, MergeOutcome::Written);

        let after: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert!(is_managed_entry(&after["mcpServers"]["chaos-scheduler"]));
    }

    #[test]
    fn merge_backs_up_invalid_json_instead_of_discarding_it() {
        let dir = tmpdir();
        let config = dir.join("mcp.json");
        std::fs::write(&config, "{ not valid json").unwrap();

        merge_mcp_config(
            &config,
            "id",
            "/bin/node",
            "/cli.js",
            "http://x",
            "tok",
            false,
        )
        .unwrap();

        // A backup of the invalid content must exist somewhere alongside it.
        let backups: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains("invalid"))
            .collect();
        assert_eq!(backups.len(), 1, "expected exactly one invalid-json backup");
        let backup_contents = std::fs::read_to_string(backups[0].path()).unwrap();
        assert_eq!(backup_contents, "{ not valid json");

        // And the config itself is now valid JSON with the managed entry.
        let after: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert!(is_managed_entry(&after["mcpServers"]["chaos-scheduler"]));
    }

    /// Regression test for the "invalid-JSON backup filenames collide at
    /// 1-second granularity" finding: two separate invalid-JSON encounters
    /// landing within the same wall-clock second (entirely plausible for two
    /// rapid-fire re-provision attempts) must each get their own backup file
    /// rather than the second silently overwriting the first.
    #[test]
    fn merge_never_collides_invalid_json_backups_within_the_same_second() {
        let dir = tmpdir();
        let config = dir.join("mcp.json");

        std::fs::write(&config, "{ not valid json (first)").unwrap();
        merge_mcp_config(
            &config,
            "id",
            "/bin/node",
            "/cli.js",
            "http://x",
            "tok",
            false,
        )
        .unwrap();

        // Force the config back into an invalid state so the second merge
        // call hits the same invalid-JSON backup path again.
        std::fs::write(&config, "{ not valid json (second)").unwrap();
        merge_mcp_config(
            &config,
            "id",
            "/bin/node",
            "/cli.js",
            "http://x",
            "tok",
            false,
        )
        .unwrap();

        let mut backups: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains("invalid"))
            .collect();
        assert_eq!(
            backups.len(),
            2,
            "two separate invalid-JSON encounters must produce two distinct backups, \
             not silently overwrite each other"
        );
        backups.sort_by_key(|e| e.file_name());
        let contents: Vec<String> = backups
            .iter()
            .map(|e| std::fs::read_to_string(e.path()).unwrap())
            .collect();
        assert!(contents.contains(&"{ not valid json (first)".to_string()));
        assert!(contents.contains(&"{ not valid json (second)".to_string()));
    }

    #[test]
    fn remove_entry_only_removes_a_managed_entry() {
        let dir = tmpdir();
        let config = dir.join("mcp.json");
        let unmanaged = serde_json::json!({
            "mcpServers": { "chaos-scheduler": { "command": "npx", "args": [], "env": {} } }
        });
        std::fs::write(&config, unmanaged.to_string()).unwrap();

        let removed = remove_mcp_config_entry(&config).unwrap();
        assert!(!removed, "must refuse to remove an entry it doesn't own");
        let after: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(after, unmanaged);
    }

    #[test]
    fn remove_entry_removes_managed_entry_and_keeps_siblings() {
        let dir = tmpdir();
        let config = dir.join("mcp.json");
        merge_mcp_config(
            &config,
            "id",
            "/bin/node",
            "/cli.js",
            "http://x",
            "tok",
            false,
        )
        .unwrap();
        // Add an unrelated sibling entry after the managed one exists.
        let mut root: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        root["mcpServers"]["other-tool"] = serde_json::json!({ "command": "other", "args": [] });
        std::fs::write(&config, root.to_string()).unwrap();

        let removed = remove_mcp_config_entry(&config).unwrap();
        assert!(removed);

        let after: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert!(after["mcpServers"]["chaos-scheduler"].is_null());
        assert_eq!(after["mcpServers"]["other-tool"]["command"], "other");
    }

    // --- staging / promote / prune -----------------------------------------

    #[test]
    fn promote_staging_moves_atomically_and_replacing_same_version_is_clean() {
        let app_data_dir = tmpdir();
        let staging_a = mcp_root(&app_data_dir).join("staging-a");
        std::fs::create_dir_all(&staging_a).unwrap();
        std::fs::write(staging_a.join("marker"), "a").unwrap();

        let promoted = promote_staging(&app_data_dir, &staging_a, "1.0.0").unwrap();
        assert_eq!(
            std::fs::read_to_string(promoted.join("marker")).unwrap(),
            "a"
        );
        assert!(!staging_a.exists());

        // Re-provisioning the same version swaps content atomically; no
        // "displaced" leftovers survive.
        let staging_b = mcp_root(&app_data_dir).join("staging-b");
        std::fs::create_dir_all(&staging_b).unwrap();
        std::fs::write(staging_b.join("marker"), "b").unwrap();
        let promoted_again = promote_staging(&app_data_dir, &staging_b, "1.0.0").unwrap();
        assert_eq!(
            std::fs::read_to_string(promoted_again.join("marker")).unwrap(),
            "b"
        );
        let leftovers: Vec<_> = std::fs::read_dir(mcp_root(&app_data_dir))
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("displaced-"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "displaced staging dir must be cleaned up"
        );
    }

    /// Regression test for the "no cleanup of orphaned staging directories"
    /// finding: leftover `staging-*` / `displaced-*` dirs from an install
    /// interrupted before it could finish or clean up after itself (crash,
    /// force-quit, OOM) must be swept, while a legitimately promoted
    /// `versions/<version>` dir is left completely untouched.
    #[test]
    fn sweep_orphaned_staging_dirs_removes_stale_staging_and_displaced_dirs_only() {
        let app_data_dir = tmpdir();
        let root = mcp_root(&app_data_dir);
        std::fs::create_dir_all(root.join("staging-0.5.0-abc123")).unwrap();
        std::fs::create_dir_all(root.join("displaced-def456")).unwrap();
        std::fs::create_dir_all(version_dir(&app_data_dir, "0.5.0")).unwrap();
        std::fs::write(version_dir(&app_data_dir, "0.5.0").join("marker"), "keep").unwrap();

        sweep_orphaned_staging_dirs(&app_data_dir);

        assert!(!root.join("staging-0.5.0-abc123").exists());
        assert!(!root.join("displaced-def456").exists());
        assert!(version_dir(&app_data_dir, "0.5.0").exists());
        assert_eq!(
            std::fs::read_to_string(version_dir(&app_data_dir, "0.5.0").join("marker")).unwrap(),
            "keep"
        );
    }

    #[test]
    fn sweep_orphaned_staging_dirs_is_a_no_op_when_mcp_root_does_not_exist() {
        let app_data_dir = tmpdir();
        // No mcp/ dir created at all — must not panic.
        sweep_orphaned_staging_dirs(&app_data_dir);
    }

    #[test]
    fn prune_old_versions_keeps_only_the_current_version() {
        let app_data_dir = tmpdir();
        std::fs::create_dir_all(version_dir(&app_data_dir, "0.4.0")).unwrap();
        std::fs::create_dir_all(version_dir(&app_data_dir, "0.5.0")).unwrap();

        prune_old_versions(&app_data_dir, "0.5.0");

        assert!(!version_dir(&app_data_dir, "0.4.0").exists());
        assert!(version_dir(&app_data_dir, "0.5.0").exists());
    }

    // --- resolve_cli_path ---------------------------------------------------

    #[test]
    fn resolve_cli_path_reads_the_bin_field() {
        let dir = tmpdir();
        std::fs::write(
            dir.join("package.json"),
            r#"{"name":"x","bin":{"chaos-mcp-server":"./dist/cli.js"}}"#,
        )
        .unwrap();
        std::fs::create_dir_all(dir.join("dist")).unwrap();
        std::fs::write(dir.join("dist").join("cli.js"), "// fixture").unwrap();

        assert_eq!(
            resolve_cli_path(&dir).unwrap(),
            dir.canonicalize().unwrap().join("dist").join("cli.js")
        );
    }

    #[test]
    fn resolve_cli_path_errors_without_a_bin_field() {
        let dir = tmpdir();
        std::fs::write(dir.join("package.json"), r#"{"name":"x"}"#).unwrap();

        assert!(resolve_cli_path(&dir).is_err());
    }

    /// Regression test for the "no path-escape validation" finding: a
    /// malicious/compromised `bin` field pointing outside the installed
    /// package directory (via `..` traversal) must be rejected rather than
    /// silently resolved and later executed.
    #[test]
    fn resolve_cli_path_rejects_a_bin_entry_that_escapes_the_package_dir() {
        let root = tmpdir();
        let package_dir = root.join("node_modules").join("mcp-server");
        std::fs::create_dir_all(&package_dir).unwrap();
        // A file outside package_dir that a malicious "bin" could point at.
        std::fs::write(root.join("outside.js"), "// secret").unwrap();
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"x","bin":{"chaos-mcp-server":"../../outside.js"}}"#,
        )
        .unwrap();

        let result = resolve_cli_path(&package_dir);
        assert!(
            result.is_err(),
            "a bin entry escaping the package dir must be rejected, got {result:?}"
        );
    }

    /// Same escape check, but via an absolute path in `bin` (Rust's
    /// `PathBuf::join` replaces the whole path when the joined component is
    /// absolute, so this is a distinct code path from the `..` case above).
    #[test]
    fn resolve_cli_path_rejects_an_absolute_bin_entry_outside_the_package_dir() {
        let root = tmpdir();
        let package_dir = root.join("node_modules").join("mcp-server");
        std::fs::create_dir_all(&package_dir).unwrap();
        let outside = root.join("outside.js");
        std::fs::write(&outside, "// secret").unwrap();
        std::fs::write(
            package_dir.join("package.json"),
            format!(
                r#"{{"name":"x","bin":{{"chaos-mcp-server":"{}"}}}}"#,
                outside.to_string_lossy().replace('\\', "\\\\")
            ),
        )
        .unwrap();

        let result = resolve_cli_path(&package_dir);
        assert!(
            result.is_err(),
            "an absolute bin entry outside the package dir must be rejected, got {result:?}"
        );
    }

    // --- end-to-end provision/remove (fake node/npm, no network) --------

    #[test]
    fn provision_stages_promotes_registers_and_is_then_idempotent() {
        let app_data_dir = tmpdir();
        let config_path = tmpdir().join("mcp.json");
        let service_dir = tmpdir();
        let service = test_service(&service_dir);
        let runtime = fake_runtime(&tmpdir(), "v20.11.0", pinned_mcp_version());

        let first = provision_with_runtime(&app_data_dir, &service, &config_path, &runtime, false)
            .expect("first provision should succeed");
        assert_eq!(first.install_status, InstallStatus::Installed);
        assert!(first.registered_in_cursor);
        assert!(first.matches);
        assert_eq!(
            first.provisioned_version.as_deref(),
            Some(pinned_mcp_version())
        );

        let key_id_after_first = first.managed_key_id.clone();
        let cursor_state = inspect_cursor_config(&config_path);
        assert!(cursor_state.registered && !cursor_state.conflict);

        // Re-provisioning when nothing changed must be a no-op: same managed
        // key (no needless remint/revoke churn), same registration.
        let second = provision_with_runtime(&app_data_dir, &service, &config_path, &runtime, false)
            .expect("idempotent re-provision should succeed");
        assert_eq!(second.managed_key_id, key_id_after_first);
        assert!(second.matches);
    }

    #[test]
    fn provision_rolls_back_staging_and_preserves_the_previous_version_on_smoke_failure() {
        let app_data_dir = tmpdir();
        let config_path = tmpdir().join("mcp.json");
        let service_dir = tmpdir();
        let service = test_service(&service_dir);

        // First, a real successful provision at version "0.4.0" so there is
        // a previous good install to protect.
        let good_runtime = fake_runtime(&tmpdir(), "v20.11.0", "0.4.0");
        // Force the "pinned" version for this call by installing at whatever
        // version the fixture reports — the manifest just needs a prior
        // provisioned_version + a real version dir on disk.
        let staged = mcp_root(&app_data_dir).join("staging-0.4.0-seed");
        npm_install(
            &good_runtime.npm_path,
            &good_runtime.node_path,
            &staged,
            "0.4.0",
        )
        .unwrap();
        promote_staging(&app_data_dir, &staged, "0.4.0").unwrap();
        let manifest = ManagedManifest {
            enabled: true,
            provisioned_version: Some("0.4.0".to_string()),
            ..Default::default()
        };
        manifest.save(&app_data_dir).unwrap();

        // Now attempt a provision whose npm fixture is broken (no `bin`
        // field, so resolve_cli_path/smoke_check fail after install).
        let broken_runtime = fake_runtime(&tmpdir(), "v20.11.0", "broken");
        let result = provision_with_runtime(
            &app_data_dir,
            &service,
            &config_path,
            &broken_runtime,
            false,
        );
        assert!(
            result.is_err(),
            "a failed smoke check must surface as an error"
        );

        // The previous good version must still be on disk...
        assert!(version_dir(&app_data_dir, "0.4.0").exists());
        // ...and no broken staging directory should be left behind.
        let leftover_staging: Vec<_> = std::fs::read_dir(mcp_root(&app_data_dir))
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("staging-"))
            .collect();
        assert!(
            leftover_staging.is_empty(),
            "failed staging dir must be cleaned up"
        );
        // ...and mcp.json was never touched (the merge step is never reached).
        assert!(!config_path.exists());

        let status_after = status_with(
            &app_data_dir,
            &service,
            &config_path,
            Some(&broken_runtime),
            false,
        );
        assert!(status_after.last_error.is_some());
        assert_eq!(status_after.provisioned_version.as_deref(), Some("0.4.0"));
    }

    /// Regression test for the "managed-token/key-id desync" finding: a
    /// successful provision followed by an out-of-band key revocation and
    /// then a *staging failure* on re-provision must never leave the status
    /// reporting healthy/current. Before the fix, `provision_with_runtime`
    /// minted the replacement key and persisted `manifest.managed_key_id`
    /// to point at it *before* attempting staging/install — so a staging
    /// failure after the mint left `manifest.managed_key_id` pointing at a
    /// live key while `mcp.json` still held the dead, revoked token, and
    /// every subsequent status check (including the next launch's
    /// `already_current` fast path) saw "a live key is tracked" and reported
    /// healthy, even though every real MCP call would 401 against the dead
    /// token still embedded in `mcp.json`.
    #[test]
    fn provision_does_not_report_healthy_after_key_revocation_and_a_failed_reprovision() {
        let app_data_dir = tmpdir();
        let config_path = tmpdir().join("mcp.json");
        let service_dir = tmpdir();
        let service = test_service(&service_dir);

        let good_runtime = fake_runtime(&tmpdir(), "v20.11.0", pinned_mcp_version());
        let first =
            provision_with_runtime(&app_data_dir, &service, &config_path, &good_runtime, false)
                .expect("first provision should succeed");
        assert!(first.matches, "first provision must report healthy");
        let original_key_id = first.managed_key_id.clone().expect("a key must be minted");

        // Simulate the managed key being revoked out-of-band (e.g. the user
        // rotated/revoked it directly), independent of any app-side call.
        service
            .revoke_api_key(&original_key_id)
            .expect("revoking the key out-of-band must succeed");
        assert!(!key_is_alive(&service, &original_key_id));

        // Re-provision now sees the tracked key is dead, mints a replacement
        // *before* staging even begins — then staging itself fails (broken
        // npm fixture: install succeeds but the package has no `bin` field,
        // so `resolve_cli_path`/`smoke_check` fail), exactly the "unrelated
        // transient failure after the mint" sequence from the finding.
        let broken_runtime = fake_runtime(&tmpdir(), "v20.11.0", "broken");
        let reprovision_result = provision_with_runtime(
            &app_data_dir,
            &service,
            &config_path,
            &broken_runtime,
            false,
        );
        assert!(
            reprovision_result.is_err(),
            "the forced staging failure must surface as an error"
        );

        // The critical assertion: a status computed as of "the next launch"
        // must NOT report the integration as matching/healthy — the dead
        // token is still all that's in `mcp.json`, so `key_alive` for
        // whatever key the manifest tracks must be false, `already_current`
        // (in a subsequent provision call) must not fast-path, and any UI
        // reading status must see something is wrong rather than "healthy".
        let status_after = status_with(
            &app_data_dir,
            &service,
            &config_path,
            Some(&broken_runtime),
            false,
        );
        assert!(
            !status_after.matches,
            "status must not report healthy/current after a failed re-provision \
             following an out-of-band key revocation, got {status_after:?}"
        );
        assert!(
            status_after.last_error.is_some(),
            "the failure must be surfaced via last_error, not silently swallowed"
        );

        // And a subsequent re-provision attempt (simulating the next launch's
        // startup hook, this time succeeding) must actually retry rather
        // than taking the `already_current` fast path.
        let healed =
            provision_with_runtime(&app_data_dir, &service, &config_path, &good_runtime, false)
                .expect("a later successful re-provision must be able to self-heal");
        assert!(healed.matches, "self-heal must result in a healthy status");
        assert_ne!(
            healed.managed_key_id.as_deref(),
            Some(original_key_id.as_str()),
            "self-heal must mint a fresh key rather than reusing the revoked one"
        );
    }

    #[test]
    fn provision_reports_node_unavailable_without_failing() {
        let app_data_dir = tmpdir();
        let config_path = tmpdir().join("mcp.json");
        let service_dir = tmpdir();
        let service = test_service(&service_dir);

        // No real detect_runtime() call here (that would depend on the host's
        // real Homebrew/system Node); directly exercise the "None" path that
        // `provision()` takes when detection fails.
        let manifest_before = ManagedManifest::load(&app_data_dir);
        assert!(!manifest_before.enabled);

        let status = status_with(&app_data_dir, &service, &config_path, None, false);
        assert_eq!(status.install_status, InstallStatus::NodeUnavailable);
        assert!(!status.node_available);
    }

    #[test]
    fn remove_revokes_the_managed_key_and_clears_the_manifest() {
        let app_data_dir = tmpdir();
        let config_path = tmpdir().join("mcp.json");
        let service_dir = tmpdir();
        let service = test_service(&service_dir);
        let runtime = fake_runtime(&tmpdir(), "v20.11.0", pinned_mcp_version());

        let provisioned =
            provision_with_runtime(&app_data_dir, &service, &config_path, &runtime, false).unwrap();
        let key_id = provisioned.managed_key_id.clone().unwrap();
        assert!(key_is_alive(&service, &key_id));

        let removed = remove(&app_data_dir, &service, &config_path, false).unwrap();
        assert!(
            !key_is_alive(&service, &key_id),
            "managed key must be revoked"
        );
        assert!(!removed.registered_in_cursor);
        assert_eq!(removed.provisioned_version, None);
        assert!(!mcp_root(&app_data_dir).exists());

        // The mcp.json file itself is preserved (other tools' entries might
        // live there); only the managed entry is gone.
        let after: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
        assert!(after["mcpServers"]["chaos-scheduler"].is_null());
    }

    #[test]
    fn remove_does_not_touch_an_unmanaged_config_entry() {
        let app_data_dir = tmpdir();
        let config_path = tmpdir().join("mcp.json");
        let unmanaged = serde_json::json!({
            "mcpServers": { "chaos-scheduler": { "command": "npx", "args": [], "env": {} } }
        });
        std::fs::write(&config_path, unmanaged.to_string()).unwrap();
        let service_dir = tmpdir();
        let service = test_service(&service_dir);

        remove(&app_data_dir, &service, &config_path, false).unwrap();

        let after: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(after, unmanaged);
    }
}
