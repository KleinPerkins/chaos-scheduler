//! Centralized product branding + runtime configuration constants.
//!
//! Every user-visible name, bundle identifier, network port, and environment
//! variable prefix lives here so that rebranding (or repointing to a different
//! workspace root / port) is a single-file change rather than a scattered
//! find-and-replace. The scheduler engine, Tauri shell, and HTTP API all read
//! from this module.
//!
//! This is a canonical name registry: some constants (legacy env-var
//! prefixes/names kept for the transition window, reserved ports) document the
//! scheme and are referenced by tooling/tests or emitted as literals elsewhere,
//! so unused-in-code entries are intentional rather than dead.
#![allow(dead_code)]

/// Human-facing product name.
pub const PRODUCT_NAME: &str = "Chaos Scheduler";

/// Current macOS bundle identifier. Must match `tauri.conf.json`.
pub const BUNDLE_ID: &str = "com.chaosscheduler.app";

/// Previous bundle identifier, retained so the legacy-DB relocation can find a
/// pre-rebrand install's data directory.
pub const LEGACY_BUNDLE_ID: &str = "com.chaoslabs.scheduler";

/// Stable tray icon id.
pub const TRAY_ID: &str = "chaos-scheduler-tray";

/// Canonical installed executable path (used for launch-at-login registration).
pub const CANONICAL_EXECUTABLE_PATH: &str =
    "/Applications/Chaos Scheduler.app/Contents/MacOS/chaos-scheduler";

/// Environment-variable prefix exported to child workflow processes.
pub const ENV_PREFIX: &str = "CHAOS_SCHEDULER_";

/// Legacy environment-variable prefixes, still dual-emitted to child processes
/// for one minor version so external scripts keep working during migration.
pub const LEGACY_ROOT_ENV: &str = "CHAOS_LABS_ROOT";
pub const LEGACY_SCHEDULER_ENV_PREFIX: &str = "CHAOS_LABS_SCHEDULER_";
pub const LEGACY_WORKFLOW_INPUT_ENV: &str = "CHAOS_LABS_WORKFLOW_INPUT_JSON";
pub const LEGACY_TASK_CHANNEL_FD_ENV: &str = "CHAOS_LABS_TASK_CHANNEL_FD";

/// Environment variable that overrides the detected workspace root.
pub const WORKSPACE_ROOT_ENV: &str = "CHAOS_SCHEDULER_WORKSPACE_ROOT";

/// Single-instance guard socket (localhost only).
pub const SINGLE_INSTANCE_ADDR: &str = "127.0.0.1:9616";
/// Prometheus `/metrics` endpoint (localhost only).
pub const METRICS_ADDR: &str = "127.0.0.1:9617";
/// Default bind address for the versioned HTTP API (loopback by default).
pub const DEFAULT_API_ADDR: &str = "127.0.0.1:9618";

/// Default scheduler environment for real workflows (UI + API when omitted).
pub const DEFAULT_ENVIRONMENT: &str = "production";

/// Isolated environment for integration tests, SDK demos, and MCP smoke runs.
pub const SANDBOX_ENVIRONMENT: &str = "sandbox";

/// Default queue name for an environment (`{environment}-default`).
pub fn default_queue_name(environment: &str) -> String {
    format!("{}-default", environment.trim())
}

/// Window title for the popup shell.
pub const POPUP_TITLE: &str = "Chaos Scheduler";
/// Tray tooltip.
pub const TRAY_TOOLTIP: &str = "Chaos Scheduler";
/// Default email `from` display name.
pub const EMAIL_FROM_NAME: &str = "Chaos Scheduler";

/// Detect the workspace root the scheduler resolves relative script paths and
/// per-environment working directories against.
///
/// Resolution order:
/// 1. `CHAOS_SCHEDULER_WORKSPACE_ROOT` (explicit override, e.g. from launchd)
/// 2. `CHAOS_LABS_ROOT` (legacy override, honored for one minor version)
/// 3. the provided app-data fallback (the standalone default — no longer the
///    chaos-labs repo)
pub fn detect_workspace_root(app_data_fallback: &str) -> String {
    for key in [WORKSPACE_ROOT_ENV, LEGACY_ROOT_ENV] {
        if let Ok(root) = std::env::var(key) {
            let root = root.trim();
            if !root.is_empty() {
                return root.to_string();
            }
        }
    }
    app_data_fallback.to_string()
}
