//! Background update-check state machine (updater UX plan, Sections 1-2, 4).
//!
//! A single Rust-owned snapshot (`Arc<Mutex<UpdateSnapshot>>`, managed as
//! Tauri state) tracks the updater lifecycle. One background task performs a
//! delayed launch check and a 6h periodic check; the manual Settings "Check
//! for updates" button routes through the exact same [`run_check`] function
//! so there is only one code path that ever talks to the updater plugin.
//!
//! Persisted preferences (`updater.background_check_enabled`,
//! `updater.skipped_version`) live in `scheduler_config` via [`crate::db`];
//! everything else (phase, latest version, progress, last error) is
//! in-memory only and resets on restart.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

/// Broadcast (all windows) whenever the snapshot changes.
pub const UPDATE_STATUS_EVENT: &str = "update-status";

/// Delay before the first background check after launch.
const LAUNCH_CHECK_DELAY: Duration = Duration::from_secs(30);
/// Interval between subsequent background checks.
const BACKGROUND_CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdatePhase {
    Idle,
    Checking,
    Available,
    Downloading,
    ReadyToRestart,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateErrorInfo {
    /// One of "network" | "endpoint" | "verification" | "install" | "unknown".
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct UpdateProgress {
    pub percent: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSnapshot {
    pub updater_available: bool,
    pub phase: UpdatePhase,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub notes: Option<String>,
    pub last_checked_at: Option<String>,
    pub last_error: Option<UpdateErrorInfo>,
    pub progress: Option<UpdateProgress>,
    pub background_check_enabled: bool,
    pub skipped_version: Option<String>,
}

impl UpdateSnapshot {
    fn new(
        current_version: String,
        background_check_enabled: bool,
        skipped_version: Option<String>,
    ) -> Self {
        Self {
            // Optimistic until the first check proves otherwise; a failed
            // `app.updater()` lookup flips this to false (Section 6: "offline
            // /unconfigured" keeps the graceful Settings fallback).
            updater_available: true,
            phase: UpdatePhase::Idle,
            current_version,
            latest_version: None,
            notes: None,
            last_checked_at: None,
            last_error: None,
            progress: None,
            background_check_enabled,
            skipped_version,
        }
    }
}

/// Tauri-managed state holding the shared snapshot.
pub struct UpdateState {
    pub snapshot: Mutex<UpdateSnapshot>,
}

impl UpdateState {
    pub fn new(
        current_version: String,
        background_check_enabled: bool,
        skipped_version: Option<String>,
    ) -> Self {
        Self {
            snapshot: Mutex::new(UpdateSnapshot::new(
                current_version,
                background_check_enabled,
                skipped_version,
            )),
        }
    }

    pub fn snapshot(&self) -> UpdateSnapshot {
        self.snapshot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

/// Attempts to move the snapshot into `Checking`. Returns `false` (no-op) if
/// a check or download is already in flight — the single-flight guard
/// (Section 1: "ignore a check while phase in {checking, downloading}").
fn try_begin_check(snapshot: &mut UpdateSnapshot) -> bool {
    if matches!(
        snapshot.phase,
        UpdatePhase::Checking | UpdatePhase::Downloading
    ) {
        return false;
    }
    snapshot.phase = UpdatePhase::Checking;
    true
}

/// Whether `candidate_version` should be suppressed because the user
/// explicitly skipped that exact version (Section 2: skip is a per-exact-
/// version escape hatch; a newer version always overrides a stale skip).
fn is_skipped(skipped_version: Option<&str>, candidate_version: &str) -> bool {
    skipped_version == Some(candidate_version)
}

/// Best-effort classification of updater errors into the buckets the
/// frontend distinguishes. Used by the check path here and by the
/// download/install path added in the follow-up apply/restart PR.
pub fn classify_updater_error(err: &tauri_plugin_updater::Error) -> &'static str {
    use tauri_plugin_updater::Error;
    match err {
        Error::Network(_) | Error::Reqwest(_) | Error::Io(_) => "network",
        Error::EmptyEndpoints
        | Error::ReleaseNotFound
        | Error::UnsupportedArch
        | Error::UnsupportedOs
        | Error::UrlParse(_)
        | Error::TargetNotFound(_)
        | Error::TargetsNotFound(_)
        | Error::InsecureTransportProtocol
        | Error::Semver(_)
        | Error::Serialization(_) => "endpoint",
        Error::Minisign(_) | Error::SignatureUtf8(_) | Error::Base64(_) => "verification",
        Error::PackageInstallFailed
        | Error::DebInstallFailed
        | Error::InvalidUpdaterFormat
        | Error::TempDirNotFound
        | Error::TempDirNotOnSameMountPoint
        | Error::BinaryNotFoundInArchive
        | Error::FailedToDetermineExtractPath => "install",
        _ => "unknown",
    }
}

fn emit_snapshot(app: &AppHandle, snapshot: &UpdateSnapshot) {
    if let Err(e) = app.emit(UPDATE_STATUS_EVENT, snapshot) {
        log::warn!("Failed to emit {UPDATE_STATUS_EVENT}: {e}");
    }
}

/// Runs the shared check path used by both the background timer and the
/// manual Settings "Check for updates" button (Section 1: "one code path").
///
/// Manual invocations surface a failed check as `Error`; background
/// invocations quietly return to `Idle` and record `last_error` (Section 2)
/// so a transient network blip on the timer never leaves the user staring at
/// a dead-end error state.
pub async fn run_check(app: &AppHandle, manual: bool) -> UpdateSnapshot {
    let state = app.state::<UpdateState>();

    {
        let mut snapshot = state
            .snapshot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !try_begin_check(&mut snapshot) {
            return snapshot.clone();
        }
    }

    let now = chrono::Utc::now().to_rfc3339();

    let updater = match app.updater() {
        Ok(updater) => updater,
        Err(_) => {
            let mut snapshot = state
                .snapshot
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            snapshot.updater_available = false;
            snapshot.phase = UpdatePhase::Idle;
            snapshot.last_checked_at = Some(now);
            let result = snapshot.clone();
            drop(snapshot);
            emit_snapshot(app, &result);
            return result;
        }
    };

    let outcome = updater.check().await;

    let mut snapshot = state
        .snapshot
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    snapshot.updater_available = true;
    snapshot.last_checked_at = Some(now);

    match outcome {
        Ok(Some(update)) => {
            if is_skipped(snapshot.skipped_version.as_deref(), &update.version) {
                snapshot.phase = UpdatePhase::Idle;
                snapshot.latest_version = None;
                snapshot.notes = None;
            } else {
                snapshot.phase = UpdatePhase::Available;
                snapshot.latest_version = Some(update.version.clone());
                snapshot.notes = update.body.clone();
            }
            snapshot.last_error = None;
        }
        Ok(None) => {
            snapshot.phase = UpdatePhase::Idle;
            snapshot.latest_version = None;
            snapshot.notes = None;
            snapshot.last_error = None;
        }
        Err(err) => {
            snapshot.phase = if manual {
                UpdatePhase::Error
            } else {
                UpdatePhase::Idle
            };
            snapshot.last_error = Some(UpdateErrorInfo {
                kind: classify_updater_error(&err).to_string(),
                message: err.to_string(),
            });
        }
    }

    let result = snapshot.clone();
    drop(snapshot);
    emit_snapshot(app, &result);
    result
}

async fn run_background_tick(app: &AppHandle) {
    let should_check = {
        let state = app.state::<UpdateState>();
        let snapshot = state
            .snapshot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        snapshot.updater_available && snapshot.background_check_enabled
    };
    if should_check {
        run_check(app, false).await;
    }
}

/// Spawns the single Rust-owned background-check task: one delayed launch
/// check, then a fixed 6h tick, for the lifetime of the process.
pub fn spawn_background_task(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(LAUNCH_CHECK_DELAY).await;
        run_background_tick(&app).await;

        let mut interval = tokio::time::interval(BACKGROUND_CHECK_INTERVAL);
        interval.tick().await; // first tick fires immediately; already handled above
        loop {
            interval.tick().await;
            run_background_tick(&app).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot_with_phase(phase: UpdatePhase) -> UpdateSnapshot {
        let mut snapshot = UpdateSnapshot::new("1.0.0".to_string(), true, None);
        snapshot.phase = phase;
        snapshot
    }

    #[test]
    fn single_flight_guard_blocks_while_checking_or_downloading() {
        let mut checking = snapshot_with_phase(UpdatePhase::Checking);
        assert!(!try_begin_check(&mut checking));
        assert_eq!(checking.phase, UpdatePhase::Checking);

        let mut downloading = snapshot_with_phase(UpdatePhase::Downloading);
        assert!(!try_begin_check(&mut downloading));
        assert_eq!(downloading.phase, UpdatePhase::Downloading);
    }

    #[test]
    fn single_flight_guard_admits_from_idle_available_or_error() {
        for phase in [
            UpdatePhase::Idle,
            UpdatePhase::Available,
            UpdatePhase::Error,
        ] {
            let mut snapshot = snapshot_with_phase(phase);
            assert!(try_begin_check(&mut snapshot));
            assert_eq!(snapshot.phase, UpdatePhase::Checking);
        }
    }

    #[test]
    fn skip_suppresses_only_the_exact_skipped_version() {
        assert!(is_skipped(Some("1.2.0"), "1.2.0"));
        assert!(!is_skipped(Some("1.2.0"), "1.3.0"));
        assert!(!is_skipped(None, "1.2.0"));
    }

    #[test]
    fn classifies_network_errors() {
        let err = tauri_plugin_updater::Error::Network("boom".to_string());
        assert_eq!(classify_updater_error(&err), "network");
    }

    #[test]
    fn classifies_endpoint_errors() {
        assert_eq!(
            classify_updater_error(&tauri_plugin_updater::Error::ReleaseNotFound),
            "endpoint"
        );
        assert_eq!(
            classify_updater_error(&tauri_plugin_updater::Error::EmptyEndpoints),
            "endpoint"
        );
    }

    #[test]
    fn classifies_verification_errors() {
        let err = tauri_plugin_updater::Error::SignatureUtf8("bad sig".to_string());
        assert_eq!(classify_updater_error(&err), "verification");
    }

    #[test]
    fn classifies_install_errors() {
        assert_eq!(
            classify_updater_error(&tauri_plugin_updater::Error::PackageInstallFailed),
            "install"
        );
    }

    #[test]
    fn unmapped_errors_fall_back_to_unknown() {
        assert_eq!(
            classify_updater_error(&tauri_plugin_updater::Error::AuthenticationFailed),
            "unknown"
        );
    }

    #[test]
    fn new_snapshot_is_optimistically_available_and_idle() {
        let snapshot = UpdateSnapshot::new("1.0.0".to_string(), true, Some("0.9.0".to_string()));
        assert!(snapshot.updater_available);
        assert_eq!(snapshot.phase, UpdatePhase::Idle);
        assert_eq!(snapshot.current_version, "1.0.0");
        assert!(snapshot.background_check_enabled);
        assert_eq!(snapshot.skipped_version, Some("0.9.0".to_string()));
        assert!(snapshot.latest_version.is_none());
    }

    #[test]
    fn update_state_snapshot_returns_a_clone() {
        let state = UpdateState::new("2.0.0".to_string(), false, None);
        let snap = state.snapshot();
        assert_eq!(snap.current_version, "2.0.0");
        assert!(!snap.background_check_enabled);
    }
}
