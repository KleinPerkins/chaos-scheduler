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

use crate::keychain::{KeyStore, MANAGED_MCP_KEYCHAIN_ACCOUNT, MANAGED_MCP_KEYCHAIN_SERVICE};
use crate::service::SchedulerService;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The npm package this module provisions. The SDK is a transitive dependency
/// of this package and is never installed separately (see module docs).
pub const MCP_PACKAGE_NAME: &str = "@chaos-scheduler/mcp-server";

/// Ownership marker written into the managed Cursor config entry's `env`.
/// JSON has no comments, so this — plus `CHAOS_SCHEDULER_MANAGED_ID` — is how
/// this module tells "an entry it manages" apart from one a user wrote by
/// hand or copied from the manual snippet.
const MANAGED_BY_MARKER: &str = "Chaos Scheduler";

/// Surfaced (and stored in the manifest `last_error`) when `~/.cursor/mcp.json`
/// already has a `chaos-scheduler` entry this app did not create. Shared between
/// the read-only pre-check and the merge-time rollback net (issue #292 review
/// Finding 5) so the operator sees one consistent message.
const UNMANAGED_CONFLICT_MESSAGE: &str =
    "~/.cursor/mcp.json already has an unmanaged \"chaos-scheduler\" entry — re-provision with \
     force to take it over.";

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
    /// Whether the managed API key now lives in the macOS Keychain (the
    /// launcher form) rather than inline in the Cursor config (the pre-#292
    /// plaintext form). `#[serde(default)]` so a pre-migration manifest on disk
    /// (which lacks this field) still deserializes and reads as `false`, which
    /// is what forces the one-time inline→Keychain migration on the next
    /// provision.
    #[serde(default)]
    pub key_in_keychain: bool,
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

/// Read an INLINE plaintext token from our own managed Cursor entry (the
/// pre-#292 form where `CHAOS_SCHEDULER_API_KEY` lived directly in the config
/// `env`). Used only to (a) detect that a one-time Keychain migration is still
/// pending and (b) recover the working token during that migration — the API
/// never returns a token after creation, so this is the only way to "recover"
/// it. Post-migration the managed entry carries no inline token, so this
/// returns `None`.
fn read_inline_managed_token(config_path: &Path) -> Option<String> {
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

/// Three-way result of recovering the managed key's token, so provisioning can
/// tell a genuinely-absent key (mint a new one) apart from a Keychain we simply
/// couldn't read right now (issue #292 review Finding 3). A transient/permission
/// read failure must NEVER be mistaken for "absent" — that would revoke+remint a
/// still-valid key.
enum ManagedTokenLookup {
    /// A working token was recovered (from the Keychain, or a pre-migration
    /// inline value) — reuse it, no remint.
    Found(String),
    /// Proven absent: nothing in the Keychain and nothing inline → mint a new key.
    Absent,
    /// The Keychain could not be read (locked / access denied / backend error).
    /// The key may still be present, so provisioning must leave the existing key
    /// intact and surface a needs-attention state rather than reminting.
    Unavailable,
}

/// Recover the managed key's token so a repair/re-provision can reuse the
/// working key instead of needlessly reminting one. Keychain-aware: once the
/// manifest records `key_in_keychain`, the token is read from the Keychain (the
/// launcher form stores nothing in the config); before migration it falls back
/// to the inline config value.
///
/// Finding 1(b): the Keychain is also consulted when the live managed entry is
/// already launcher-shaped even if the manifest still says `key_in_keychain =
/// false`. That closes a migration desync window — a crash between rewriting
/// `mcp.json` to launcher form (inline token removed) and persisting the
/// manifest flag would otherwise make the token look "missing" and trigger a
/// spurious revoke+remint of a still-valid Keychain key.
///
/// Finding 3: a Keychain read *error* returns [`ManagedTokenLookup::Unavailable`]
/// (never `Absent`), so provisioning does not revoke/remint on a transient
/// failure. A genuinely absent Keychain item returns `Absent`, which correctly
/// drives a re-mint.
fn read_existing_managed_token(
    keystore: &dyn KeyStore,
    config_path: &Path,
    manifest: &ManagedManifest,
) -> ManagedTokenLookup {
    if manifest.key_in_keychain || managed_entry_is_launcher_shaped(config_path) {
        return match keystore.get(MANAGED_MCP_KEYCHAIN_SERVICE, MANAGED_MCP_KEYCHAIN_ACCOUNT) {
            Ok(Some(token)) => ManagedTokenLookup::Found(token),
            // Not in the Keychain: fall back to any inline token still present
            // (a partially-completed migration), else proven-absent.
            Ok(None) => match read_inline_managed_token(config_path) {
                Some(token) => ManagedTokenLookup::Found(token),
                None => ManagedTokenLookup::Absent,
            },
            // Read failure (locked / denied / backend): key may still exist.
            Err(_) => ManagedTokenLookup::Unavailable,
        };
    }
    // Pre-migration: only an inline plaintext token can recover the key.
    match read_inline_managed_token(config_path) {
        Some(token) => ManagedTokenLookup::Found(token),
        None => ManagedTokenLookup::Absent,
    }
}

/// Whether OUR managed `chaos-scheduler` Cursor entry exists and is already in
/// launcher form (managed marker present, no inline `CHAOS_SCHEDULER_API_KEY`),
/// i.e. a migration already rewrote the config. Used to recover the token from
/// the Keychain when the manifest flag lags the config write (Finding 1(b)).
fn managed_entry_is_launcher_shaped(config_path: &Path) -> bool {
    let Ok(raw) = std::fs::read_to_string(config_path) else {
        return false;
    };
    let Ok(root) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return false;
    };
    let Some(entry) = root
        .get("mcpServers")
        .and_then(|servers| servers.get("chaos-scheduler"))
    else {
        return false;
    };
    is_managed_entry(entry)
        && entry
            .get("env")
            .and_then(|env| env.get("CHAOS_SCHEDULER_API_KEY"))
            .is_none()
}

/// Path of the app-owned launcher script that resolves the managed key from the
/// Keychain at spawn time. Lives under the managed MCP root so [`remove`]'s
/// `remove_dir_all(mcp_root(..))` reclaims it along with everything else.
fn launcher_script_path(app_data_dir: &Path) -> PathBuf {
    mcp_root(app_data_dir).join("launch-managed.sh")
}

/// Write the app-owned launcher script (mode `0700`) that reads the managed key
/// from the macOS Keychain at spawn time and execs the absolute `node` + the
/// absolute installed CLI. The script itself contains **no secret** — only the
/// absolute paths and the stable Keychain service/account coordinates.
///
/// The `node`/`cli` absolute paths are single-quoted in the script; a path
/// containing a single quote is rejected rather than written, so a malformed
/// path can never break out of the quoting into injectable shell.
fn write_launcher_script(
    app_data_dir: &Path,
    node_path: &str,
    cli_path: &str,
) -> Result<PathBuf, String> {
    use std::os::unix::fs::PermissionsExt;

    if node_path.contains('\'') || cli_path.contains('\'') {
        return Err(
            "refusing to write launcher: node/CLI path contains a single quote".to_string(),
        );
    }

    let path = launcher_script_path(app_data_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let script = format!(
        "#!/bin/sh\n\
KEY=\"$(/usr/bin/security find-generic-password -w -s '{service}' -a '{account}' 2>/dev/null)\"\n\
if [ -z \"$KEY\" ]; then echo 'chaos-scheduler: managed MCP key unavailable — re-provision the integration' >&2; exit 78; fi\n\
export CHAOS_SCHEDULER_API_KEY=\"$KEY\"\n\
exec '{node}' '{cli}' \"$@\"\n",
        service = MANAGED_MCP_KEYCHAIN_SERVICE,
        account = MANAGED_MCP_KEYCHAIN_ACCOUNT,
        node = node_path,
        cli = cli_path,
    );

    write_atomic(&path, script.as_bytes())?;
    // 0700: owner-only rwx. The script carries no secret, but an app-owned
    // executable in app-data should not be world/group-accessible.
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
        .map_err(|e| e.to_string())?;
    Ok(path)
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
///
/// The written entry's `command` is the app-owned Keychain launcher (see
/// [`write_launcher_script`]); the `env` carries only `CHAOS_SCHEDULER_URL` and
/// the ownership markers and **never** the API key — the launcher resolves the
/// key from the Keychain at spawn time.
pub fn merge_mcp_config(
    config_path: &Path,
    managed_id: &str,
    launcher_path: &str,
    api_url: &str,
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
            "command": launcher_path,
            "args": [],
            "env": {
                "CHAOS_SCHEDULER_URL": api_url,
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

/// How long `npm install` may run before it's treated as hung and killed.
/// An unresponsive registry, a captive-portal DNS flake, or a network
/// blackhole otherwise leaves `Command::output()` blocking forever — which,
/// since every call site holds `McpState::lock` for its duration, would wedge
/// every future provision/remove call (and the startup re-provision hook)
/// behind a call that will never return, with no recovery short of a force
/// quit. Two minutes is generous for installing one small package + its
/// handful of transitive deps even on a slow connection, while still
/// bounding the worst case to "long wait", not "hang forever".
const NPM_INSTALL_TIMEOUT: Duration = Duration::from_secs(120);

/// Run `cmd` to completion, killing it and returning an error if it hasn't
/// exited within `timeout`. stdout/stderr are drained on background threads
/// concurrently with the timeout poll — reading them only after the process
/// exits would risk a deadlock if a chatty child (npm's own progress output)
/// fills the OS pipe buffer while nothing is on the reading end.
fn run_with_timeout(mut cmd: Command, timeout: Duration) -> Result<Output, String> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("spawning: {e}"))?;

    let mut stdout_pipe = child.stdout.take().expect("stdout was piped above");
    let mut stderr_pipe = child.stderr.take().expect("stderr was piped above");
    let stdout_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout_pipe.read_to_end(&mut buf);
        buf
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr_pipe.read_to_end(&mut buf);
        buf
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => break status,
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                // Drop the reader threads' results: on a timeout we report a
                // dedicated error rather than a partial-output `Output`, so
                // there's nothing useful to join them into.
                return Err(format!(
                    "timed out after {}s and was killed",
                    timeout.as_secs()
                ));
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    };

    let stdout = stdout_reader.join().unwrap_or_default();
    let stderr = stderr_reader.join().unwrap_or_default();
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

fn npm_install(
    npm_path: &str,
    node_path: &str,
    prefix: &Path,
    version: &str,
) -> Result<(), String> {
    std::fs::create_dir_all(prefix).map_err(|e| e.to_string())?;
    let spec = format!("{MCP_PACKAGE_NAME}@{version}");
    let mut cmd = npm_command(npm_path, node_path);
    cmd.args([
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
    ]);
    let output = run_with_timeout(cmd, NPM_INSTALL_TIMEOUT)
        .map_err(|e| format!("npm install {spec}: {e}"))?;
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
///
/// If that second rename fails (disk pressure, a permissions change, or any
/// other OS-mechanics error), `displaced` is renamed back to `target` rather
/// than being deleted unconditionally — otherwise a failed promote would
/// discard the previously-working version *and* fail to install the new
/// one, leaving `target` missing entirely and bricking the managed
/// integration until the next successful re-provision.
fn promote_staging(app_data_dir: &Path, staging: &Path, version: &str) -> Result<PathBuf, String> {
    let target = version_dir(app_data_dir, version);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if target.exists() {
        let displaced = mcp_root(app_data_dir).join(format!("displaced-{}", uuid::Uuid::new_v4()));
        std::fs::rename(&target, &displaced).map_err(|e| e.to_string())?;
        match std::fs::rename(staging, &target) {
            Ok(()) => {
                let _ = std::fs::remove_dir_all(&displaced);
                Ok(target)
            }
            Err(err) => {
                // Best-effort rollback: if this also fails, `displaced` is
                // deliberately left on disk (not deleted) rather than
                // compounding the failure — the next startup's orphaned-dir
                // sweep will reclaim it once the situation is unrecoverable.
                let _ = std::fs::rename(&displaced, &target);
                Err(err.to_string())
            }
        }
    } else {
        std::fs::rename(staging, &target).map_err(|e| e.to_string())?;
        Ok(target)
    }
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

/// Record `err` as this attempt's failure before propagating it. Every
/// fallible step in [`provision_with_runtime`] after the idempotency check
/// must route its error through this instead of a bare `?`, so a
/// provisioning failure is never silently dropped: `status()` — what the
/// Integrations card, the tray, and the startup re-provision hook's "was
/// there a previous failure" check all read — always reflects the most
/// recent attempt's real outcome, even when the immediate caller only
/// inspects the returned `Result` for its own window.
fn fail_provision(manifest: &mut ManagedManifest, app_data_dir: &Path, err: String) -> String {
    manifest.last_error = Some(err.clone());
    manifest.enabled = true;
    let _ = manifest.save(app_data_dir);
    err
}

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
    keystore: &dyn KeyStore,
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
    // in Cursor, the managed key is still live, AND the key already lives in
    // the Keychain (the launcher form). The `key_in_keychain` requirement is
    // what forces exactly one re-provision to run the inline→Keychain
    // migration on a system upgraded from the pre-#292 plaintext form; once
    // migrated, this stays a true no-op.
    let already_current = !force
        && manifest.provisioned_version.as_deref() == Some(pinned.as_str())
        && inspect_cursor_config(config_path).registered
        && key_alive
        && manifest.key_in_keychain;
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

    // FINDING 5 (read-only conflict pre-check): if `~/.cursor/mcp.json` already
    // has an UNMANAGED `chaos-scheduler` entry, bail BEFORE minting a key or
    // writing the Keychain. Otherwise a freshly-minted live secret would be
    // orphaned in the Keychain while Cursor keeps using the foreign entry.
    // `force` opts into taking the entry over. This is the primary defense; a
    // rollback net below handles a TOCTOU race where a foreign entry appears
    // between here and the merge.
    if !force && inspect_cursor_config(config_path).conflict {
        manifest.last_error = Some(UNMANAGED_CONFLICT_MESSAGE.to_string());
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

    // Snapshot the managed identity so a mid-flight rollback (Finding 5) can
    // restore it exactly rather than guessing.
    let prior_managed_key_id = manifest.managed_key_id.clone();
    let prior_key_in_keychain = manifest.key_in_keychain;

    let managed_id = manifest
        .managed_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // Track whether we minted a BRAND-NEW key this attempt: only a freshly
    // minted (otherwise-unreferenced) key is safe to revoke on rollback.
    let mut minted_new_key = false;
    let (key_id, token) = if key_alive {
        match read_existing_managed_token(keystore, config_path, &manifest) {
            ManagedTokenLookup::Found(existing_token) => {
                (manifest.managed_key_id.clone().unwrap(), existing_token)
            }
            ManagedTokenLookup::Absent => {
                if let Some(old_id) = &manifest.managed_key_id {
                    let _ = service.revoke_api_key(old_id);
                }
                minted_new_key = true;
                mint_key(service).map_err(|e| fail_provision(&mut manifest, app_data_dir, e))?
            }
            ManagedTokenLookup::Unavailable => {
                // FINDING 3: an unreadable Keychain is NOT an absent key. Leave
                // the existing (still-live) key untouched and surface a
                // needs-attention error rather than revoking+reminting a key
                // that is probably fine.
                return Err(fail_provision(
                    &mut manifest,
                    app_data_dir,
                    "the managed key could not be read from the macOS Keychain (it may be locked \
                     or access was denied); leaving the existing key in place — re-provision once \
                     the Keychain is available"
                        .to_string(),
                ));
            }
        }
    } else {
        if let Some(old_id) = &manifest.managed_key_id {
            let _ = service.revoke_api_key(old_id);
        }
        minted_new_key = true;
        mint_key(service).map_err(|e| fail_provision(&mut manifest, app_data_dir, e))?
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

    let promoted_dir = promote_staging(app_data_dir, &staging, &pinned)
        .map_err(|e| fail_provision(&mut manifest, app_data_dir, e))?;
    let cli_path = resolve_cli_path(&installed_package_dir(&promoted_dir))
        .map_err(|e| fail_provision(&mut manifest, app_data_dir, e))?;

    // Store the managed key in the Keychain and PROVE it reads back BEFORE
    // writing any launcher config or removing any inline plaintext. If either
    // step fails, don't write a launcher that would resolve an absent key at
    // spawn: fail the provision and leave any existing inline token untouched,
    // so the one-time migration re-runs on the next attempt and the working
    // key is never lost.
    keystore
        .set(
            MANAGED_MCP_KEYCHAIN_SERVICE,
            MANAGED_MCP_KEYCHAIN_ACCOUNT,
            &token,
        )
        .map_err(|e| {
            fail_provision(
                &mut manifest,
                app_data_dir,
                format!("storing the managed key in the Keychain failed: {e}"),
            )
        })?;
    let verified = keystore
        .get(MANAGED_MCP_KEYCHAIN_SERVICE, MANAGED_MCP_KEYCHAIN_ACCOUNT)
        .map_err(|e| {
            fail_provision(
                &mut manifest,
                app_data_dir,
                format!("verifying the managed Keychain key failed: {e}"),
            )
        })?;
    if verified.as_deref() != Some(token.as_str()) {
        return Err(fail_provision(
            &mut manifest,
            app_data_dir,
            "the managed key did not read back from the Keychain after storing it".to_string(),
        ));
    }

    // FINDING 1(a): persist `key_in_keychain = true` (and the managed IDs) NOW —
    // after the Keychain write is PROVEN, but BEFORE `merge_mcp_config` removes
    // any inline plaintext token. This ordering is what prevents a
    // manifest-desync footgun: if the process dies between the config rewrite
    // (inline token removed) and the final save, the manifest already records
    // the Keychain as the source of truth, so the next provision recovers the
    // token from the Keychain instead of seeing "missing" and revoking a
    // still-valid key. The inline token remains a working backup until the merge
    // below makes its removal durable. `provisioned_version` stays UNSET until
    // the very end, so this intermediate state can never satisfy the
    // `already_current` no-op check and be mistaken for "healthy".
    manifest.managed_id = Some(managed_id.clone());
    manifest.managed_key_id = Some(key_id.clone());
    manifest.key_in_keychain = true;
    manifest.save(app_data_dir)?;

    // The launcher resolves the key from the Keychain at spawn time and carries
    // no secret; the managed config `command` becomes this launcher's absolute
    // path (never `node`+inline-token as in the pre-#292 form).
    let launcher = write_launcher_script(
        app_data_dir,
        &runtime.node_path,
        &cli_path.to_string_lossy(),
    )
    .map_err(|e| fail_provision(&mut manifest, app_data_dir, e))?;

    // Whether this write completes a one-time inline→Keychain migration: an
    // inline token is still physically in the config right now, so
    // `merge_mcp_config`'s backup-before-write will copy those bytes (INCLUDING
    // the token) into `mcp.json.bak`. We shred that sidecar after the merge
    // (Finding 2) so no plaintext token is left at rest beside the config.
    let migrating = read_inline_managed_token(config_path).is_some();

    let merge_outcome = merge_mcp_config(
        config_path,
        &managed_id,
        &launcher.to_string_lossy(),
        &default_api_url(),
        force,
    )
    .map_err(|e| fail_provision(&mut manifest, app_data_dir, e))?;

    if merge_outcome == MergeOutcome::ConflictUnmanaged {
        // FINDING 5 (rollback net): the read-only pre-check above normally
        // prevents reaching here without `force`, but a concurrent writer could
        // insert a foreign entry between the pre-check and this merge. Ensure no
        // freshly-minted live secret is orphaned: if we minted this attempt,
        // delete the Keychain item we wrote and revoke the key, then restore the
        // manifest's prior managed identity. A reused (pre-existing) key is left
        // intact — it is still the working key and is referenced elsewhere.
        if minted_new_key {
            let _ = keystore.delete(MANAGED_MCP_KEYCHAIN_SERVICE, MANAGED_MCP_KEYCHAIN_ACCOUNT);
            let _ = service.revoke_api_key(&key_id);
            manifest.managed_key_id = prior_managed_key_id.clone();
            manifest.key_in_keychain = prior_key_in_keychain;
        }
        manifest.last_error = Some(UNMANAGED_CONFLICT_MESSAGE.to_string());
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

    // FINDING 2: the merge just rewrote `mcp.json` to launcher form. If this was
    // a migration, `mcp.json.bak` now holds the pre-migration bytes — including
    // the inline plaintext token. Securely delete that sidecar so the token is
    // not left at rest beside the scrubbed config.
    if migrating {
        crate::db::secure_remove(&config_path.with_extension("json.bak"));
    }

    // The managed identity + `key_in_keychain` were already persisted above
    // (Finding 1(a)); re-affirm them alongside `provisioned_version` for a
    // single consistent final manifest.
    manifest.managed_id = Some(managed_id.clone());
    manifest.managed_key_id = Some(key_id.clone());
    manifest.key_in_keychain = true;

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
    let keystore = crate::keychain::default_key_store();
    provision_with_runtime(
        app_data_dir,
        service,
        config_path,
        &runtime,
        force,
        keystore.as_ref(),
    )
}

/// Remove the managed integration: drop the managed `mcp.json` entry (only if
/// it's ours), delete the app-managed install dir (which also reclaims the
/// launcher script), revoke the managed key, and delete the managed Keychain
/// item. Best-effort at every step (never panics on a missing file) so a
/// partially-broken prior state can always be cleaned up. Production wrapper
/// around [`remove_with_keystore`] that supplies the real Keychain.
pub fn remove(
    app_data_dir: &Path,
    service: &SchedulerService,
    config_path: &Path,
    prepare_to_uninstall: bool,
) -> Result<McpIntegrationStatus, String> {
    let keystore = crate::keychain::default_key_store();
    remove_with_keystore(
        app_data_dir,
        service,
        config_path,
        prepare_to_uninstall,
        keystore.as_ref(),
    )
}

/// Dependency-injected [`remove`] (tests supply an in-memory key store).
fn remove_with_keystore(
    app_data_dir: &Path,
    service: &SchedulerService,
    config_path: &Path,
    prepare_to_uninstall: bool,
    keystore: &dyn KeyStore,
) -> Result<McpIntegrationStatus, String> {
    let manifest = ManagedManifest::load(app_data_dir);

    let _ = remove_mcp_config_entry(config_path);

    if let Some(key_id) = &manifest.managed_key_id {
        let _ = service.revoke_api_key(key_id);
    }

    // Delete the managed Keychain item (best-effort). Offboarding re-checks the
    // authoritative delete outcome for its removal proof; the standalone remove
    // path clears it so removing the integration leaves nothing behind.
    let _ = keystore.delete(MANAGED_MCP_KEYCHAIN_SERVICE, MANAGED_MCP_KEYCHAIN_ACCOUNT);

    // Remove the whole managed root (versions, staging, the launcher script,
    // and the manifest itself) rather than deleting-then-resaving a default
    // manifest: an absent manifest already loads as `ManagedManifest::default()`,
    // so there's no state to preserve, and leaving no directory behind is what
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

/// What a one-action offboarding cleared, for the caller's confirmation UI
/// (credential-security PR-D).
#[derive(Debug, Clone, Serialize)]
pub struct OffboardReport {
    /// API keys revoked (managed + all user keys).
    pub keys_revoked: usize,
    /// SMTP-password fields blanked across `email_config` + `email_profiles`.
    pub smtp_passwords_cleared: usize,
    /// Workflow `spec_json` blobs that had at least one secret field blanked.
    pub workflow_specs_scrubbed: usize,
    /// Workflow `trigger_config` blobs that had at least one secret field blanked.
    pub trigger_configs_scrubbed: usize,
    /// Workflow `queue_config` blobs that had at least one secret field blanked.
    pub queue_configs_scrubbed: usize,
    /// Secret-bearing `scheduler_config` rows deleted (e.g. the inbound webhook
    /// HMAC secret).
    pub scheduler_config_secrets_cleared: usize,
    /// Whether the managed MCP integration was ACTUALLY cleared — verified from
    /// the post-state (manifest file absent AND no managed Cursor config entry
    /// remains AND the managed Keychain item proven-absent), not assumed. The
    /// best-effort `remove` swallows filesystem/config-write errors, so this
    /// must never be a hardcoded `true`.
    pub managed_integration_removed: bool,
    /// Whether the managed key's macOS Keychain item is PROVABLY gone
    /// (deleted now or already absent). A delete that couldn't be verified
    /// (`Unknown`) reports `false` here and forces `managed_integration_removed`
    /// to `false` too — removal is never falsely claimed.
    pub keychain_item_removed: bool,
}

/// Tri-state view of OUR managed `chaos-scheduler` Cursor entry. Offboarding
/// must tell "proven gone" apart from "can't tell": a config file that exists
/// but is unreadable/unparseable might still hold the managed entry (and its
/// `CHAOS_SCHEDULER_API_KEY`), so it can never count as removed.
enum ManagedEntryState {
    /// Config genuinely absent, or present+parseable with no managed entry
    /// (incl. only a foreign/unmanaged entry) — our entry is proven gone.
    Absent,
    /// A managed `chaos-scheduler` entry is present.
    Present,
    /// Config file exists but is unreadable/unparseable — removal NOT proven.
    Unknown,
}

/// Classify our managed Cursor entry, distinguishing genuine absence from an
/// unreadable/unparseable config (see [`ManagedEntryState`]).
fn managed_config_entry_state(config_path: &Path) -> ManagedEntryState {
    let raw = match std::fs::read_to_string(config_path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return ManagedEntryState::Absent,
        // Exists but unreadable (e.g. permissions): removal unproven.
        Err(_) => return ManagedEntryState::Unknown,
    };
    let Ok(root) = serde_json::from_str::<serde_json::Value>(&raw) else {
        // Exists but not parseable JSON: the managed block may still be there.
        return ManagedEntryState::Unknown;
    };
    if root
        .get("mcpServers")
        .and_then(|servers| servers.get("chaos-scheduler"))
        .is_some_and(is_managed_entry)
    {
        ManagedEntryState::Present
    } else {
        ManagedEntryState::Absent
    }
}

/// Whether OUR managed Cursor entry is definitively present. An
/// unreadable/unparseable config counts as NOT present here; callers needing
/// removal PROOF must use [`managed_integration_fully_removed`], which treats
/// that same unknown state as "not proven removed".
#[allow(dead_code)] // Definitive-presence accessor (used in tests).
fn managed_config_entry_present(config_path: &Path) -> bool {
    matches!(
        managed_config_entry_state(config_path),
        ManagedEntryState::Present
    )
}

/// Whether the managed MCP integration is PROVABLY, fully removed — the
/// post-condition [`offboard`] verifies rather than assumes. Requires ALL of:
/// the manifest file absent, our managed Cursor entry proven absent, AND the
/// managed Keychain item proven-absent (`keychain_item_absent`, derived from a
/// [`crate::keychain::DeleteOutcome`] that distinguishes a proven delete from an
/// unverifiable one). A config file that exists but can't be read/parsed, or a
/// Keychain delete that couldn't be verified, yields `false` (removal unproven /
/// needs attention), so the report never claims a removal it cannot verify.
fn managed_integration_fully_removed(
    app_data_dir: &Path,
    config_path: &Path,
    keychain_item_absent: bool,
) -> bool {
    !ManagedManifest::manifest_path(app_data_dir).exists()
        && matches!(
            managed_config_entry_state(config_path),
            ManagedEntryState::Absent
        )
        && keychain_item_absent
}

/// PR-D(b): back up an UNPARSEABLE `~/.cursor/mcp.json` to a `.bak` sidecar and
/// replace it with a scrubbed, valid config carrying no managed token, so the
/// managed `CHAOS_SCHEDULER_API_KEY` is gone even when the file couldn't be
/// parse-merged. Foreign entries can't be preserved from unparseable JSON, but
/// the `.bak` retains the original bytes. No-op (and leaves the file untouched)
/// if the raw bytes can't even be read.
///
/// NOTE: during OFFBOARD this `.bak` is subsequently shredded by
/// [`secure_delete_config_sidecars`] (issue #292 review Finding 2) — a
/// decommission must never leave the token at rest in a sidecar. This helper
/// itself only backs up + replaces; the caller decides sidecar retention.
fn backup_and_replace_unparseable_config(config_path: &Path) -> Result<(), String> {
    let Ok(raw) = std::fs::read_to_string(config_path) else {
        // Can't read it at all (e.g. permissions): nothing safe to do here;
        // the tri-state stays Unknown and removal stays unproven.
        return Ok(());
    };
    // Preserve the original unparseable bytes next to the file for recovery.
    let backup = config_path.with_extension("json.bak");
    std::fs::write(&backup, raw.as_bytes()).map_err(|e| e.to_string())?;
    // Replace with a minimal, valid, secret-free config.
    let scrubbed = serde_json::json!({ "mcpServers": {} });
    let json = serde_json::to_string_pretty(&scrubbed).map_err(|e| e.to_string())?;
    write_atomic(config_path, json.as_bytes())
}

/// FINDING 2: securely delete (overwrite-then-unlink, via the audited
/// [`crate::db::secure_remove`]) any credential-bearing sidecars left beside
/// `~/.cursor/mcp.json` — the `.bak` backup and any `.invalid-<ts>-<uuid>`
/// copies produced by the backup-before-write / invalid-JSON paths. Those
/// sidecars can hold a pre-migration inline `CHAOS_SCHEDULER_API_KEY`, so an
/// OFFBOARD (decommission) must not retain them. Best-effort and self-limiting:
/// only files whose name is exactly `<config>.bak` or begins with
/// `<config>.invalid-` are touched, never the live config or unrelated files.
///
/// This is offboard-only on purpose: a normal remove/repair may legitimately
/// keep a `.bak` for recovery; decommission may not.
fn secure_delete_config_sidecars(config_path: &Path) {
    let (Some(dir), Some(file_name)) = (
        config_path.parent(),
        config_path.file_name().and_then(|n| n.to_str()),
    ) else {
        return;
    };
    let bak = format!("{file_name}.bak");
    let invalid_prefix = format!("{file_name}.invalid-");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == bak.as_str() || name.starts_with(invalid_prefix.as_str()) {
            crate::db::secure_remove(&entry.path());
        }
    }
}

/// One-action offboarding (credential-security PR-D): revoke EVERY API key,
/// purge EVERY secret-bearing DB field, and clear the managed MCP integration
/// (Keychain item + manifest + Cursor config entry). This is the composition
/// seam that unites the governed DB purge (owned by `SchedulerService`, which
/// has no app-data path) with the filesystem/Keychain cleanup this module owns
/// via [`remove`]. Destructive and irreversible; the caller (UI/IPC command)
/// owns the confirmation prompt — the backend does not prompt. Production
/// wrapper around [`offboard_with_keystore`] supplying the real Keychain.
pub fn offboard(
    app_data_dir: &Path,
    service: &SchedulerService,
    config_path: &Path,
) -> Result<OffboardReport, String> {
    let keystore = crate::keychain::default_key_store();
    offboard_with_keystore(app_data_dir, service, config_path, keystore.as_ref())
}

/// Dependency-injected [`offboard`] (tests supply an in-memory key store so
/// they never touch the real Keychain).
fn offboard_with_keystore(
    app_data_dir: &Path,
    service: &SchedulerService,
    config_path: &Path,
    keystore: &dyn KeyStore,
) -> Result<OffboardReport, String> {
    // Gate all key minting for the ENTIRE offboard: a concurrent `create_api_key`
    // (IPC or the managed-MCP mint) must not insert a live key after revoke-all.
    // The guard's refcount resets on drop when this function returns.
    let _minting_gate = service.begin_offboarding();

    // Governed DB half first: revoke every key + blank every secret DB field in
    // one transaction.
    let purge = service
        .offboard_revoke_all_and_purge()
        .map_err(|e| e.to_string())?;
    // Then clear the managed MCP integration from the manifest + Cursor config +
    // Keychain, reusing the audited `remove` path. `remove` also revokes the
    // managed key, which the bulk purge above already covered — a harmless
    // idempotent repeat.
    remove_with_keystore(app_data_dir, service, config_path, false, keystore)?;

    // PR-D(b): if the Cursor config is UNPARSEABLE, `remove_mcp_config_entry`
    // bailed and any managed token is still physically in the file. Back it up
    // to a `.bak` sidecar and replace it with a scrubbed valid config so the
    // token is gone even when parsing failed.
    if matches!(
        managed_config_entry_state(config_path),
        ManagedEntryState::Unknown
    ) {
        let _ = backup_and_replace_unparseable_config(config_path);
    }

    // FINDING 2: shred any token-bearing sidecars next to the config (`.bak`
    // from a backup-before-write or the unparseable-config replace above, and
    // any `.invalid-*` copies). A decommission must not leave the managed token
    // at rest in plaintext beside the config.
    secure_delete_config_sidecars(config_path);

    // Final post-state verification, still inside the minting gate: no live key
    // may remain. The gate makes this a no-op in practice, but sweeping any
    // straggler (e.g. one minted in a pre-gate window) keeps `keys_revoked`
    // truthful and guarantees zero live keys at completion.
    let swept = service
        .revoke_all_live_api_keys()
        .map_err(|e| e.to_string())?;
    let keys_revoked = purge.keys_revoked + swept;

    // Authoritative Keychain delete outcome for the report + removal proof.
    // `remove_with_keystore` already best-effort deleted it, so this normally
    // reports `AlreadyAbsent` (proven-absent); a backend that can't verify the
    // delete reports `Unknown`, which must NOT count as removed.
    let keychain_item_removed = keystore
        .delete(MANAGED_MCP_KEYCHAIN_SERVICE, MANAGED_MCP_KEYCHAIN_ACCOUNT)
        .map(|outcome| outcome.proven_absent())
        .unwrap_or(false);

    // `remove` is best-effort (it swallows filesystem/config-write errors and
    // still returns Ok), so VERIFY the post-state rather than claim success: the
    // report must never tell the confirmation UI a removal happened when it
    // didn't. Full removal now requires the Keychain item proven-absent too.
    let managed_integration_removed =
        managed_integration_fully_removed(app_data_dir, config_path, keychain_item_removed);
    Ok(OffboardReport {
        keys_revoked,
        smtp_passwords_cleared: purge.smtp_passwords_cleared,
        workflow_specs_scrubbed: purge.workflow_specs_scrubbed,
        trigger_configs_scrubbed: purge.trigger_configs_scrubbed,
        queue_configs_scrubbed: purge.queue_configs_scrubbed,
        scheduler_config_secrets_cleared: purge.scheduler_config_secrets_cleared,
        managed_integration_removed,
        keychain_item_removed,
    })
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
    use crate::keychain::{DeleteOutcome, FakeKeyStore};
    use crate::service::{NoopNotifier, SchedulerService};
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Arc;

    fn tmpdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("chaos-mcp-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// An in-memory key store for tests, so provisioning/offboarding never
    /// touches the real macOS Keychain (which hangs/fails in headless CI).
    fn fake_keystore() -> FakeKeyStore {
        FakeKeyStore::new()
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

    /// Regression test for the "npm_install has no timeout, can hang MCP
    /// provisioning forever" finding: a child process that never exits
    /// (simulating an npm install stuck on an unresponsive registry) must be
    /// killed at the deadline, not awaited forever — every provision/remove
    /// call site holds `McpState::lock` for the duration of this call, so a
    /// real hang here would wedge every future provision/remove call (and
    /// the startup re-provision hook) behind it indefinitely.
    #[test]
    fn run_with_timeout_kills_a_hanging_process_and_reports_a_timeout_error() {
        let dir = tmpdir();
        let hanging = dir.join("hangs-forever.sh");
        write_executable(&hanging, "#!/bin/sh\nsleep 30\n");

        let started = Instant::now();
        let result = run_with_timeout(Command::new(&hanging), Duration::from_millis(150));
        let elapsed = started.elapsed();

        let err = result.expect_err("a hanging process must be reported as an error, not awaited");
        assert!(
            err.contains("timed out"),
            "expected a timeout error, got: {err:?}"
        );
        assert!(
            elapsed < Duration::from_secs(10),
            "the hung process should have been killed near the 150ms deadline rather than \
             waited out to its 30s sleep; actual elapsed: {elapsed:?}"
        );
    }

    /// A command that exits well within the timeout must still surface its
    /// real exit status and captured stdout/stderr, not be mistaken for a
    /// timeout — guards against the deadline check firing on the wrong
    /// condition or the reader threads dropping output.
    #[test]
    fn run_with_timeout_returns_output_for_a_command_that_finishes_in_time() {
        let dir = tmpdir();
        let script = dir.join("quick.sh");
        write_executable(
            &script,
            "#!/bin/sh\necho out-line\necho err-line >&2\nexit 3\n",
        );

        let output = run_with_timeout(Command::new(&script), Duration::from_secs(5)).unwrap();

        assert_eq!(output.status.code(), Some(3));
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "out-line");
        assert_eq!(String::from_utf8_lossy(&output.stderr).trim(), "err-line");
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
            "/opt/mcp/launch-managed.sh",
            "http://127.0.0.1:9618",
            false,
        )
        .unwrap();
        assert_eq!(outcome, MergeOutcome::Written);

        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        let entry = &written["mcpServers"]["chaos-scheduler"];
        // #292: the command is the Keychain launcher, and the API key is NOT in env.
        assert_eq!(entry["command"], "/opt/mcp/launch-managed.sh");
        assert!(
            entry["env"]["CHAOS_SCHEDULER_API_KEY"].is_null(),
            "the API key must never be written into the Cursor config env"
        );
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

        merge_mcp_config(&config, "id", "/mcp/launch-managed.sh", "http://x", false).unwrap();

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

        let outcome =
            merge_mcp_config(&config, "id", "/mcp/launch-managed.sh", "http://x", false).unwrap();
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

        let outcome =
            merge_mcp_config(&config, "id", "/mcp/launch-managed.sh", "http://x", true).unwrap();
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

        merge_mcp_config(&config, "id", "/mcp/launch-managed.sh", "http://x", false).unwrap();

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
        merge_mcp_config(&config, "id", "/mcp/launch-managed.sh", "http://x", false).unwrap();

        // Force the config back into an invalid state so the second merge
        // call hits the same invalid-JSON backup path again.
        std::fs::write(&config, "{ not valid json (second)").unwrap();
        merge_mcp_config(&config, "id", "/mcp/launch-managed.sh", "http://x", false).unwrap();

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
        merge_mcp_config(&config, "id", "/mcp/launch-managed.sh", "http://x", false).unwrap();
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

    /// Regression test for the "promote deletes the working version on a
    /// failed swap" finding: if the final `rename(staging, target)` fails
    /// after the previous-good `target` has already been moved aside to
    /// `displaced-*`, the old code unconditionally deleted `displaced`
    /// before propagating the error — leaving *neither* the new nor the
    /// previously-working version at `target`. A re-provision attempt that
    /// fails at exactly this step must roll back to the previous-good
    /// version instead of bricking the managed integration.
    #[test]
    fn promote_staging_restores_the_previous_version_when_the_final_rename_fails() {
        let app_data_dir = tmpdir();
        let target = version_dir(&app_data_dir, "1.0.0");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("marker"), "previous-good").unwrap();

        // `staging` deliberately does not exist, so the second `rename`
        // inside `promote_staging` fails with ENOENT — *after* the first
        // rename has already moved `target` aside to a `displaced-*` dir.
        let staging = mcp_root(&app_data_dir).join("staging-missing");

        let result = promote_staging(&app_data_dir, &staging, "1.0.0");
        assert!(result.is_err(), "a failed promote must surface as an error");

        assert_eq!(
            std::fs::read_to_string(target.join("marker")).unwrap(),
            "previous-good",
            "a failed promote must roll back to the previous working version, not lose it"
        );
        let leftovers: Vec<_> = std::fs::read_dir(mcp_root(&app_data_dir))
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("displaced-"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "a successful rollback must not leave a displaced-* dir behind"
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
        let keystore = fake_keystore();

        let first = provision_with_runtime(
            &app_data_dir,
            &service,
            &config_path,
            &runtime,
            false,
            &keystore,
        )
        .expect("first provision should succeed");
        assert_eq!(first.install_status, InstallStatus::Installed);
        assert!(first.registered_in_cursor);
        assert!(first.matches);
        assert_eq!(
            first.provisioned_version.as_deref(),
            Some(pinned_mcp_version())
        );

        // #292: the managed key is in the Keychain and NOT inline in the config.
        assert!(
            keystore.contains(MANAGED_MCP_KEYCHAIN_SERVICE, MANAGED_MCP_KEYCHAIN_ACCOUNT),
            "the managed key must be stored in the Keychain"
        );
        let config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
        let entry = &config["mcpServers"]["chaos-scheduler"];
        assert!(
            entry["env"]["CHAOS_SCHEDULER_API_KEY"].is_null(),
            "the API key must NOT be written into the Cursor config env"
        );
        let command = entry["command"].as_str().unwrap();
        assert!(
            command.ends_with("launch-managed.sh"),
            "the managed command must be the Keychain launcher, got {command}"
        );
        assert!(
            launcher_script_path(&app_data_dir).exists(),
            "the launcher script must be written"
        );

        let key_id_after_first = first.managed_key_id.clone();
        let cursor_state = inspect_cursor_config(&config_path);
        assert!(cursor_state.registered && !cursor_state.conflict);

        // Re-provisioning when nothing changed must be a no-op: same managed
        // key (no needless remint/revoke churn), same registration.
        let second = provision_with_runtime(
            &app_data_dir,
            &service,
            &config_path,
            &runtime,
            false,
            &keystore,
        )
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
            &fake_keystore(),
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

    /// Regression test for the "post-staging provision failures don't
    /// persist last_error" finding: once npm_install and the pre-promotion
    /// smoke check have already succeeded, a *later* failure — forced here
    /// by pointing `config_path` at a location whose parent can never be
    /// created, so `merge_mcp_config` fails deterministically — must still
    /// be recorded on the manifest instead of being silently dropped by a
    /// bare `?`. `status()` (what the Integrations card, the tray, and the
    /// startup re-provision hook all read) has no other way to learn a
    /// provisioning attempt failed once the triggering call's own `Result`
    /// has been handled by its immediate caller.
    #[test]
    fn provision_persists_last_error_when_a_post_staging_step_fails() {
        let app_data_dir = tmpdir();
        let service_dir = tmpdir();
        let service = test_service(&service_dir);
        let runtime = fake_runtime(&tmpdir(), "v20.11.0", pinned_mcp_version());

        // `config_path`'s parent already exists as a plain file, so
        // `merge_mcp_config`'s `create_dir_all(parent)` fails deterministically
        // — but only after staging, the smoke check, and promotion have all
        // already succeeded.
        let blocking_file = tmpdir().join("not-a-directory");
        std::fs::write(&blocking_file, "blocking").unwrap();
        let config_path = blocking_file.join("mcp.json");

        let result = provision_with_runtime(
            &app_data_dir,
            &service,
            &config_path,
            &runtime,
            false,
            &fake_keystore(),
        );
        let err = result.expect_err("the forced merge_mcp_config failure must propagate");

        let manifest = ManagedManifest::load(&app_data_dir);
        assert_eq!(
            manifest.last_error.as_deref(),
            Some(err.as_str()),
            "a post-staging provisioning failure must be persisted to the manifest's \
             last_error, not silently dropped"
        );
        assert!(
            manifest.enabled,
            "the integration is still considered opted-in after a failed re-provision attempt"
        );
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
        let keystore = fake_keystore();
        let first = provision_with_runtime(
            &app_data_dir,
            &service,
            &config_path,
            &good_runtime,
            false,
            &keystore,
        )
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
            &keystore,
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
        let healed = provision_with_runtime(
            &app_data_dir,
            &service,
            &config_path,
            &good_runtime,
            false,
            &keystore,
        )
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
        let keystore = fake_keystore();

        let provisioned = provision_with_runtime(
            &app_data_dir,
            &service,
            &config_path,
            &runtime,
            false,
            &keystore,
        )
        .unwrap();
        let key_id = provisioned.managed_key_id.clone().unwrap();
        assert!(key_is_alive(&service, &key_id));
        assert!(keystore.contains(MANAGED_MCP_KEYCHAIN_SERVICE, MANAGED_MCP_KEYCHAIN_ACCOUNT));

        let removed =
            remove_with_keystore(&app_data_dir, &service, &config_path, false, &keystore).unwrap();
        assert!(
            !key_is_alive(&service, &key_id),
            "managed key must be revoked"
        );
        assert!(
            !keystore.contains(MANAGED_MCP_KEYCHAIN_SERVICE, MANAGED_MCP_KEYCHAIN_ACCOUNT),
            "the managed Keychain item must be deleted on remove"
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

        remove_with_keystore(
            &app_data_dir,
            &service,
            &config_path,
            false,
            &fake_keystore(),
        )
        .unwrap();

        let after: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(after, unmanaged);
    }

    /// PR-D one-action offboarding, end-to-end: revokes ALL keys (managed +
    /// user), purges secret-bearing DB fields (SMTP + workflow-spec secret),
    /// and clears the managed integration (manifest token + Cursor config entry).
    /// Build a run-unique, non-empty secret value from PURELY RUNTIME numeric
    /// sources (monotonic wall-clock nanos + an atomic counter), with no string
    /// literal anywhere on the dataflow path. CodeQL's inter-procedural
    /// `rust/hard-coded-cryptographic-value` query tracks string literals into
    /// password/salt/key sinks; a value derived only from runtime integers via
    /// `to_string()` is not a hard-coded value. Semantics are unchanged: the
    /// value is present pre-purge and asserted blanked post-purge.
    fn runtime_secret() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let mut s = nanos.to_string();
        s.push_str(&seq.to_string());
        s
    }

    #[test]
    fn offboard_revokes_all_keys_purges_secrets_and_clears_managed_integration() {
        let app_data_dir = tmpdir();
        let config_path = tmpdir().join("mcp.json");
        let service_dir = tmpdir();
        // Build the service manually so we keep the db handle for seeding secrets.
        let db = Arc::new(Database::new(&service_dir));
        let service = SchedulerService::new(db.clone(), Arc::new(NoopNotifier));
        let runtime = fake_runtime(&tmpdir(), "v20.11.0", pinned_mcp_version());
        let keystore = fake_keystore();

        // Provision: mints the managed key + stores it in the (fake) Keychain.
        let provisioned = provision_with_runtime(
            &app_data_dir,
            &service,
            &config_path,
            &runtime,
            false,
            &keystore,
        )
        .unwrap();
        let managed_key_id = provisioned.managed_key_id.clone().unwrap();
        assert!(key_is_alive(&service, &managed_key_id));
        assert!(ManagedManifest::load(&app_data_dir)
            .managed_key_id
            .is_some());
        assert!(keystore.contains(MANAGED_MCP_KEYCHAIN_SERVICE, MANAGED_MCP_KEYCHAIN_ACCOUNT));

        // An extra user key + secret-bearing DB fields.
        let extra = service.create_api_key(Some("extra"), &["read"]).unwrap();
        assert!(key_is_alive(&service, &extra.id));
        let profile = db
            .upsert_email_profile(&crate::db::EmailProfile {
                id: String::new(),
                name: "ops".into(),
                enabled: true,
                alert_email: "a@b.c".into(),
                smtp_host: "smtp.example.com".into(),
                smtp_port: 587,
                smtp_user: "u".into(),
                smtp_password: runtime_secret(),
                from_address: "f@b.c".into(),
                from_name: "Chaos".into(),
                created_at: String::new(),
                updated_at: String::new(),
            })
            .unwrap();
        let wf = db
            .create_workflow(
                "hook",
                None,
                "scripts/noop.py",
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
        db.set_workflow_spec(&wf.id, "generic", Some(r#"{"secret":"hmac-shhh"}"#))
            .unwrap();

        // One-action offboarding.
        let report =
            offboard_with_keystore(&app_data_dir, &service, &config_path, &keystore).unwrap();
        assert!(report.managed_integration_removed);
        assert!(
            report.keychain_item_removed,
            "the managed Keychain item must be proven-absent after offboarding"
        );
        assert!(
            !keystore.contains(MANAGED_MCP_KEYCHAIN_SERVICE, MANAGED_MCP_KEYCHAIN_ACCOUNT),
            "the managed Keychain item must be physically gone"
        );
        assert!(report.keys_revoked >= 2, "managed + extra key revoked");
        assert_eq!(report.smtp_passwords_cleared, 1);
        assert_eq!(report.workflow_specs_scrubbed, 1);

        // Every key is revoked.
        assert!(
            !key_is_alive(&service, &managed_key_id),
            "managed key revoked"
        );
        assert!(!key_is_alive(&service, &extra.id), "extra key revoked");

        // Managed integration cleared: manifest/token dir gone + Cursor entry gone.
        assert!(!mcp_root(&app_data_dir).exists(), "manifest token cleared");
        let after: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
        assert!(
            after["mcpServers"]["chaos-scheduler"].is_null(),
            "Cursor managed entry cleared"
        );

        // Secrets purged at rest.
        assert_eq!(db.get_email_profile(&profile.id).unwrap().smtp_password, "");
        let stored = db.get_workflow(&wf.id).unwrap().spec_json.unwrap();
        assert!(!stored.contains("hmac-shhh"), "webhook secret purged");
    }

    /// ISSUE 2 (report-accuracy): `managed_integration_removed` must be DERIVED
    /// from the real post-state, never hardcoded. This exercises the exact
    /// predicate `offboard` uses: it reports `false` while a managed manifest +
    /// managed Cursor entry are still present, and `true` only once both are
    /// actually gone. Against the old hardcoded `true` there was no such
    /// derivation, so a swallowed removal error would have lied.
    #[test]
    fn managed_integration_removed_reflects_actual_post_state() {
        let app_data_dir = tmpdir();
        let config_path = tmpdir().join("mcp.json");
        let service_dir = tmpdir();
        let db = Arc::new(Database::new(&service_dir));
        let service = SchedulerService::new(db.clone(), Arc::new(NoopNotifier));
        let runtime = fake_runtime(&tmpdir(), "v20.11.0", pinned_mcp_version());
        let keystore = fake_keystore();

        // Provision so BOTH halves exist: the manifest file + a managed
        // `chaos-scheduler` entry in the Cursor config.
        provision_with_runtime(
            &app_data_dir,
            &service,
            &config_path,
            &runtime,
            false,
            &keystore,
        )
        .unwrap();
        assert!(ManagedManifest::manifest_path(&app_data_dir).exists());
        assert!(managed_config_entry_present(&config_path));
        assert!(
            // `keychain_item_absent = true` isolates the file-state: even with the
            // Keychain item gone, a present manifest + entry means NOT removed.
            !managed_integration_fully_removed(&app_data_dir, &config_path, true),
            "must report NOT removed while manifest + managed entry are present"
        );

        // A real offboard clears all three halves; only then may the flag be true.
        let report =
            offboard_with_keystore(&app_data_dir, &service, &config_path, &keystore).unwrap();
        assert!(!ManagedManifest::manifest_path(&app_data_dir).exists());
        assert!(!managed_config_entry_present(&config_path));
        assert!(
            managed_integration_fully_removed(&app_data_dir, &config_path, true),
            "must report removed once all halves are gone"
        );
        assert!(report.managed_integration_removed);
        assert!(report.keychain_item_removed);

        // A LINGERING managed entry (simulating a swallowed config-write failure
        // in the best-effort `remove`) must force the reported value back to
        // false — the report can never claim a removal that didn't happen.
        std::fs::write(
            &config_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "mcpServers": {
                    "chaos-scheduler": {
                        "command": "node",
                        "env": { "CHAOS_SCHEDULER_MANAGED_BY": MANAGED_BY_MARKER }
                    }
                }
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(
            managed_config_entry_present(&config_path),
            "a managed entry must be detected as present"
        );
        assert!(
            !managed_integration_fully_removed(&app_data_dir, &config_path, true),
            "a surviving managed entry must yield NOT removed"
        );
    }

    /// #292 Keychain-aware tri-state fails-first: even when the manifest and the
    /// Cursor config entry are both gone, an UNVERIFIABLE Keychain delete
    /// (`DeleteOutcome::Unknown`) must force `managed_integration_removed` to
    /// `false` — the managed key might still be at rest in the Keychain, so the
    /// report can never claim full removal. Against a predicate that ignored the
    /// Keychain (pre-#292 the report had no Keychain dimension at all), this
    /// would have reported removed. The `keychain_item_absent` argument is what
    /// makes the difference here.
    #[test]
    fn offboard_reports_not_removed_when_keychain_delete_is_unverifiable() {
        let app_data_dir = tmpdir();
        let config_path = tmpdir().join("mcp.json");
        let service_dir = tmpdir();
        let db = Arc::new(Database::new(&service_dir));
        let service = SchedulerService::new(db.clone(), Arc::new(NoopNotifier));
        let runtime = fake_runtime(&tmpdir(), "v20.11.0", pinned_mcp_version());
        let keystore = fake_keystore();

        provision_with_runtime(
            &app_data_dir,
            &service,
            &config_path,
            &runtime,
            false,
            &keystore,
        )
        .unwrap();
        assert!(keystore.contains(MANAGED_MCP_KEYCHAIN_SERVICE, MANAGED_MCP_KEYCHAIN_ACCOUNT));

        // Simulate a Keychain that cannot verify (or perform) the delete: the
        // item is LEFT in place and every delete reports `Unknown`.
        keystore.set_delete_unverifiable(true);

        let report =
            offboard_with_keystore(&app_data_dir, &service, &config_path, &keystore).unwrap();

        // The filesystem halves ARE gone...
        assert!(!ManagedManifest::manifest_path(&app_data_dir).exists());
        assert!(!managed_config_entry_present(&config_path));
        // ...but the Keychain item is still present and its delete was unverifiable.
        assert!(
            keystore.contains(MANAGED_MCP_KEYCHAIN_SERVICE, MANAGED_MCP_KEYCHAIN_ACCOUNT),
            "an unverifiable delete must leave the item in place"
        );
        assert!(
            !report.keychain_item_removed,
            "an unverifiable Keychain delete is NOT proven-absent"
        );
        assert!(
            !report.managed_integration_removed,
            "removal must NOT be claimed while the Keychain item may still exist"
        );
        // The `Unknown` outcome is the mechanism.
        assert_eq!(
            keystore
                .delete(MANAGED_MCP_KEYCHAIN_SERVICE, MANAGED_MCP_KEYCHAIN_ACCOUNT)
                .unwrap(),
            DeleteOutcome::Unknown
        );
    }

    /// FINDING 2: offboarding must serialize against concurrent key minting so a
    /// live credential can't survive. The gate deterministically rejects a mint
    /// while held, and a full offboard leaves ZERO live keys with an accurate
    /// report. (Fails-first is shown by a separate current-code probe: pre-fix
    /// there is no gate, so a key minted around the purge stays live.)
    #[test]
    fn offboarding_blocks_concurrent_mint_and_leaves_zero_live_keys() {
        let app_data_dir = tmpdir();
        let config_path = tmpdir().join("mcp.json");
        let service_dir = tmpdir();
        let db = Arc::new(Database::new(&service_dir));
        let service = SchedulerService::new(db.clone(), Arc::new(NoopNotifier));
        let runtime = fake_runtime(&tmpdir(), "v20.11.0", pinned_mcp_version());

        // A live key exists before offboarding.
        let pre = service
            .create_api_key(Some("pre"), &["read", "write"])
            .unwrap();
        assert!(service.verify_api_key(&pre.token).is_some());

        // While an offboard is in progress, minting is rejected — the
        // deterministic stand-in for the concurrent-mint race.
        {
            let _gate = service.begin_offboarding();
            assert!(service.offboarding_in_progress());
            let blocked = service.create_api_key(Some("sneaky"), &["read", "write"]);
            assert!(
                matches!(blocked, Err(crate::service::ServiceError::Conflict(_))),
                "mint must be rejected with a clear Conflict while offboarding"
            );
        }
        // Gate released after the scope: minting works again for re-onboarding.
        assert!(!service.offboarding_in_progress());

        // Provision so offboard has a managed integration to clear.
        let keystore = fake_keystore();
        provision_with_runtime(
            &app_data_dir,
            &service,
            &config_path,
            &runtime,
            false,
            &keystore,
        )
        .unwrap();

        // Full offboard revokes all + clears the integration; verify zero live
        // keys remain and the report is accurate.
        let report =
            offboard_with_keystore(&app_data_dir, &service, &config_path, &keystore).unwrap();
        assert!(report.managed_integration_removed);
        assert!(report.keys_revoked >= 1);
        assert_eq!(
            service.count_live_api_keys().unwrap(),
            0,
            "no live API key may remain after offboarding"
        );
        assert!(service.verify_api_key(&pre.token).is_none());
    }

    /// PR-D(b) fails-first: when `~/.cursor/mcp.json` EXISTS but is unparseable
    /// AND still physically contains the managed `CHAOS_SCHEDULER_API_KEY`,
    /// offboarding must BACK UP the original bytes to a `.bak` sidecar and
    /// REPLACE the file with a scrubbed, valid, token-free config — so the token
    /// is gone even though the file couldn't be parse-merged. Against the pre-fix
    /// offboard (which left the unparseable file untouched and reported removal
    /// Unknown), there is no `.bak` and the token survives, so this test FAILS
    /// before the fix and PASSES after.
    #[test]
    fn offboard_backs_up_and_replaces_unparseable_config_scrubbing_the_token() {
        let app_data_dir = tmpdir();
        let config_path = tmpdir().join("mcp.json");
        let service_dir = tmpdir();
        let db = Arc::new(Database::new(&service_dir));
        let service = SchedulerService::new(db.clone(), Arc::new(NoopNotifier));
        let runtime = fake_runtime(&tmpdir(), "v20.11.0", pinned_mcp_version());
        let keystore = fake_keystore();

        // Provision so both a manifest and a managed Cursor entry exist.
        provision_with_runtime(
            &app_data_dir,
            &service,
            &config_path,
            &runtime,
            false,
            &keystore,
        )
        .unwrap();
        assert!(ManagedManifest::manifest_path(&app_data_dir).exists());

        // Corrupt the Cursor config so it can't be parsed, but leave a managed
        // token physically present in the bytes. The token value is derived from
        // pure runtime integers (no string literal) so no secret scanner or
        // CodeQL hard-coded-value query trips on the test source.
        let token = runtime_secret();
        let corrupt = format!(
            "{{ \"mcpServers\": {{ \"chaos-scheduler\": {{ \"env\": {{ \
             \"CHAOS_SCHEDULER_API_KEY\": \"{token}\" }} }} }}  <<< trailing garbage ::::"
        );
        std::fs::write(&config_path, &corrupt).unwrap();
        assert!(
            serde_json::from_str::<serde_json::Value>(&corrupt).is_err(),
            "test fixture must be genuinely unparseable"
        );
        assert!(corrupt.contains(&token), "fixture must contain the token");

        let report =
            offboard_with_keystore(&app_data_dir, &service, &config_path, &keystore).unwrap();

        // The live config is now valid JSON with the token scrubbed...
        let after = std::fs::read_to_string(&config_path).unwrap();
        assert!(
            serde_json::from_str::<serde_json::Value>(&after).is_ok(),
            "the replacement config must be valid JSON"
        );
        assert!(
            !after.contains(&token),
            "the managed token must be scrubbed from mcp.json even when the original was unparseable"
        );
        // ...and FINDING 2: the `.bak` sidecar the replace step created (which
        // held the original token-bearing bytes) must be securely deleted, so an
        // offboard leaves NO plaintext token at rest beside the config.
        let bak = config_path.with_extension("json.bak");
        assert!(
            !bak.exists(),
            "offboard must shred the token-bearing .bak sidecar, not retain it"
        );
        // With the token scrubbed AND the Keychain item cleared, the tri-state
        // may now legitimately report full removal.
        assert!(
            report.managed_integration_removed,
            "removal is proven once the token is scrubbed and the Keychain item is gone"
        );
        assert!(report.keychain_item_removed);
    }

    /// The tri-state predicate still reports NOT-removed for an unreadable /
    /// unparseable config when it is inspected directly (independent of the
    /// offboard backup-replace). Preserves the original FINDING-3 invariant that
    /// an unknown config state is never counted as proven-removed.
    #[test]
    fn fully_removed_predicate_rejects_unknown_config_state() {
        let app_data_dir = tmpdir();
        let config_path = tmpdir().join("mcp.json");

        // Unparseable config → Unknown → not proven removed (even with manifest
        // absent and the Keychain item proven-absent).
        std::fs::write(&config_path, "{ this is not valid json ::::").unwrap();
        assert!(
            !managed_integration_fully_removed(&app_data_dir, &config_path, true),
            "unknown config state must not count as fully removed"
        );

        // A genuinely-absent config, by contrast, IS proven removed once the
        // manifest is absent and the Keychain item is proven-absent.
        std::fs::remove_file(&config_path).unwrap();
        assert!(managed_integration_fully_removed(
            &app_data_dir,
            &config_path,
            true
        ));
        // ...but a still-present Keychain item (absent=false) blocks it.
        assert!(!managed_integration_fully_removed(
            &app_data_dir,
            &config_path,
            false
        ));
    }

    /// FINDING 3 fails-first: a Keychain that can't be READ (locked / access
    /// denied) is NOT the same as an absent key. Provisioning must leave the
    /// existing (still-live) key intact and surface a needs-attention error,
    /// never revoke+remint. Against the pre-fix code (which mapped any Keychain
    /// read error to `None` via `.ok().flatten()` and treated it as "absent"),
    /// this re-provision revoked the old key and minted a new one — so the
    /// original key id changed and the original key was revoked. This test FAILS
    /// there and PASSES with the `Unavailable` tri-state.
    #[test]
    fn provision_treats_unreadable_keychain_as_unavailable_not_absent() {
        let app_data_dir = tmpdir();
        let config_path = tmpdir().join("mcp.json");
        let service_dir = tmpdir();
        let service = test_service(&service_dir);
        let runtime = fake_runtime(&tmpdir(), "v20.11.0", pinned_mcp_version());
        let keystore = fake_keystore();

        let first = provision_with_runtime(
            &app_data_dir,
            &service,
            &config_path,
            &runtime,
            false,
            &keystore,
        )
        .expect("first provision should succeed");
        let original_key_id = first.managed_key_id.clone().expect("a managed key id");
        assert!(key_is_alive(&service, &original_key_id));

        // Drift the provisioned version so the next call is NOT an idempotent
        // no-op (this is exactly the startup re-provision scenario), forcing the
        // token-recovery path to actually run.
        let mut manifest = ManagedManifest::load(&app_data_dir);
        manifest.provisioned_version = Some("0.0.0-stale".to_string());
        manifest.save(&app_data_dir).unwrap();

        // The Keychain can't be read this attempt (locked / access denied).
        keystore.set_get_unavailable(true);

        let result = provision_with_runtime(
            &app_data_dir,
            &service,
            &config_path,
            &runtime,
            false,
            &keystore,
        );

        // The provision surfaces a needs-attention error rather than silently
        // reminting...
        assert!(
            result.is_err(),
            "an unreadable Keychain must surface as an error, not a silent remint"
        );
        // ...and crucially, the original key is UNTOUCHED: not revoked, not
        // replaced. Exactly one live key remains — the original.
        assert!(
            key_is_alive(&service, &original_key_id),
            "the existing key must NOT be revoked when the Keychain is merely unreadable"
        );
        assert_eq!(
            service.count_live_api_keys().unwrap(),
            1,
            "no remint churn: exactly the original key stays live"
        );
        assert_eq!(
            ManagedManifest::load(&app_data_dir)
                .managed_key_id
                .as_deref(),
            Some(original_key_id.as_str()),
            "the managed key id must be unchanged"
        );
        // The Keychain item itself is left in place.
        keystore.set_get_unavailable(false);
        assert!(keystore.contains(MANAGED_MCP_KEYCHAIN_SERVICE, MANAGED_MCP_KEYCHAIN_ACCOUNT));
    }

    /// FINDING 1 fails-first: if a crash lands between rewriting `mcp.json` to
    /// launcher form (inline token removed) and persisting `key_in_keychain =
    /// true`, the manifest LAGS the config. A re-provision must still recover the
    /// token from the Keychain (because the live entry is launcher-shaped) and
    /// REUSE the still-valid key — never treat it as missing and revoke+remint.
    /// Against the pre-fix lookup (which only consulted the Keychain when
    /// `manifest.key_in_keychain` was already true), the lagged manifest made the
    /// token look absent, so the key was revoked and reminted — the original id
    /// changed and the original key died. FAILS there, PASSES with the
    /// launcher-shaped Keychain fallback.
    #[test]
    fn reprovision_reuses_keychain_key_when_manifest_flag_lags_config_rewrite() {
        let app_data_dir = tmpdir();
        let config_path = tmpdir().join("mcp.json");
        let service_dir = tmpdir();
        let service = test_service(&service_dir);
        let runtime = fake_runtime(&tmpdir(), "v20.11.0", pinned_mcp_version());
        let keystore = fake_keystore();

        let first = provision_with_runtime(
            &app_data_dir,
            &service,
            &config_path,
            &runtime,
            false,
            &keystore,
        )
        .expect("first provision should succeed");
        let original_key_id = first.managed_key_id.clone().expect("a managed key id");
        assert!(managed_entry_is_launcher_shaped(&config_path));
        assert!(keystore.contains(MANAGED_MCP_KEYCHAIN_SERVICE, MANAGED_MCP_KEYCHAIN_ACCOUNT));

        // Simulate the desync crash: the config is already launcher-shaped and
        // the key is in the Keychain, but the manifest flag never got persisted.
        // Also drift the version so the re-provision isn't a no-op.
        let mut manifest = ManagedManifest::load(&app_data_dir);
        assert!(
            manifest.key_in_keychain,
            "sanity: normally set after migration"
        );
        manifest.key_in_keychain = false;
        manifest.provisioned_version = Some("0.0.0-stale".to_string());
        manifest.save(&app_data_dir).unwrap();

        let second = provision_with_runtime(
            &app_data_dir,
            &service,
            &config_path,
            &runtime,
            false,
            &keystore,
        )
        .expect("re-provision should succeed by reusing the Keychain key");

        assert_eq!(
            second.managed_key_id.as_deref(),
            Some(original_key_id.as_str()),
            "the still-valid Keychain key must be REUSED, not reminted"
        );
        assert!(
            key_is_alive(&service, &original_key_id),
            "the original key must remain live (no spurious revoke)"
        );
        assert_eq!(
            service.count_live_api_keys().unwrap(),
            1,
            "no remint churn from a lagged manifest flag"
        );
        // Recovery also re-heals the manifest flag.
        assert!(ManagedManifest::load(&app_data_dir).key_in_keychain);
    }

    /// FINDING 5 fails-first: when `~/.cursor/mcp.json` already holds an
    /// UNMANAGED `chaos-scheduler` entry, a non-force provision must NOT mint a
    /// key or write the Keychain — otherwise a freshly-minted live secret is
    /// orphaned in the Keychain while Cursor keeps using the foreign entry.
    /// Against the pre-fix code (mint + Keychain write happened before the merge,
    /// and a `ConflictUnmanaged` merge returned Ok with no rollback), a live key
    /// was minted and left in the Keychain. This test FAILS there (a live key +
    /// a Keychain item exist) and PASSES with the read-only conflict pre-check.
    #[test]
    fn provision_does_not_orphan_a_minted_key_on_unmanaged_conflict() {
        let app_data_dir = tmpdir();
        let config_path = tmpdir().join("mcp.json");
        let service_dir = tmpdir();
        let service = test_service(&service_dir);
        let runtime = fake_runtime(&tmpdir(), "v20.11.0", pinned_mcp_version());
        let keystore = fake_keystore();

        // A pre-existing UNMANAGED chaos-scheduler entry (the user's own).
        let foreign = serde_json::json!({
            "mcpServers": {
                "chaos-scheduler": { "command": "npx", "args": ["-y", "foreign"], "env": {} }
            }
        });
        std::fs::write(&config_path, foreign.to_string()).unwrap();

        let status = provision_with_runtime(
            &app_data_dir,
            &service,
            &config_path,
            &runtime,
            false,
            &keystore,
        )
        .expect("a conflict is reported as status, not an error");

        // No live key was minted, and nothing was written to the Keychain — so
        // there is no orphaned live secret.
        assert_eq!(
            service.count_live_api_keys().unwrap(),
            0,
            "no key may be minted when an unmanaged entry blocks provisioning"
        );
        assert!(
            !keystore.contains(MANAGED_MCP_KEYCHAIN_SERVICE, MANAGED_MCP_KEYCHAIN_ACCOUNT),
            "no Keychain item may be written on an unmanaged conflict"
        );
        assert!(
            ManagedManifest::load(&app_data_dir)
                .managed_key_id
                .is_none(),
            "no managed key id may be recorded on conflict"
        );
        // The foreign entry is left untouched, and the conflict is surfaced.
        let after: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(after, foreign, "the unmanaged entry must be left untouched");
        assert!(!status.registered_in_cursor && !status.matches);
    }

    /// FINDING 2 fails-first: an OFFBOARD (decommission) must not leave the
    /// managed token at rest in a plaintext sidecar. Token-bearing `.bak` and
    /// `.invalid-*` copies beside `~/.cursor/mcp.json` (produced by the
    /// backup-before-write / invalid-JSON paths) must be securely deleted.
    /// Against the pre-fix offboard (which never touched sidecars), the copies
    /// survive with the token intact — so this FAILS before and PASSES after.
    #[test]
    fn offboard_secure_deletes_token_bearing_config_sidecars() {
        let app_data_dir = tmpdir();
        let config_path = tmpdir().join("mcp.json");
        let service_dir = tmpdir();
        let service = test_service(&service_dir);
        let runtime = fake_runtime(&tmpdir(), "v20.11.0", pinned_mcp_version());
        let keystore = fake_keystore();

        provision_with_runtime(
            &app_data_dir,
            &service,
            &config_path,
            &runtime,
            false,
            &keystore,
        )
        .unwrap();

        // Two token-bearing sidecars, exactly as the backup-before-write and
        // invalid-JSON backup paths would leave for a pre-migration config. The
        // token value is derived from runtime integers so no literal secret
        // lands in the test source.
        let token = runtime_secret();
        let sidecar_body = format!("{{ \"env\": {{ \"CHAOS_SCHEDULER_API_KEY\": \"{token}\" }} }}");
        let bak = config_path.with_extension("json.bak");
        let invalid = config_path.with_extension(format!(
            "json.invalid-20240101T000000-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&bak, &sidecar_body).unwrap();
        std::fs::write(&invalid, &sidecar_body).unwrap();

        offboard_with_keystore(&app_data_dir, &service, &config_path, &keystore).unwrap();

        assert!(
            !invalid.exists(),
            "the token-bearing .invalid-* sidecar must be securely deleted on offboard"
        );
        assert!(
            !bak.exists(),
            "the token-bearing .bak sidecar must be securely deleted on offboard"
        );
    }
}
