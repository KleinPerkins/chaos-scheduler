//! Background update-check + apply/restart state machine (updater UX plan,
//! Sections 1-4 & 6).
//!
//! A single Rust-owned snapshot (`Mutex<UpdateSnapshot>`, managed as Tauri
//! state) tracks the updater lifecycle. One background task performs a
//! delayed launch check and a 6h periodic check; the manual Settings "Check
//! for updates" button routes through the exact same [`run_check`] function
//! so there is only one code path that ever talks to the updater plugin for
//! checking, and [`apply`] is the one path for downloading/installing.
//!
//! [`apply`] re-verifies the offered version, refuses if `expected_version`
//! no longer matches (Section 6 consent guard), downloads with progress
//! emitted through the snapshot, then calls [`drain_before_install`] to stop
//! the scheduler admitting new work before the (already signature-verified)
//! artifact is installed and the app restarts.
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
        UpdatePhase::Checking | UpdatePhase::Downloading | UpdatePhase::ReadyToRestart
    ) {
        return false;
    }
    snapshot.phase = UpdatePhase::Checking;
    true
}

/// Attempts to move the snapshot into `Downloading`. Returns `false` (no-op)
/// if a download is already in flight — the single-flight guard for
/// [`apply`], claimed atomically under the lock before any `.await` point
/// (mirrors [`try_begin_check`]) so two concurrent `apply_update` calls can
/// never both proceed past this point into a duplicate download/install.
fn try_begin_apply(snapshot: &mut UpdateSnapshot) -> bool {
    if snapshot.phase == UpdatePhase::Downloading {
        return false;
    }
    snapshot.phase = UpdatePhase::Downloading;
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

/// Tray tooltip text for the current phase (Section 3: native tray affordance
/// alongside the dashboard banner and popup row). Falls back to the plain
/// product tooltip once the phase clears (idle/checking/error), matching how
/// the banner/popup rows disappear.
fn tooltip_for(snapshot: &UpdateSnapshot) -> String {
    match (snapshot.phase, snapshot.latest_version.as_deref()) {
        (UpdatePhase::Available, Some(v)) => {
            format!("{} — update available: v{v}", crate::branding::TRAY_TOOLTIP)
        }
        (UpdatePhase::Downloading, Some(v)) => {
            format!(
                "{} — downloading update v{v}…",
                crate::branding::TRAY_TOOLTIP
            )
        }
        (UpdatePhase::ReadyToRestart, Some(v)) => {
            format!(
                "{} — installing update v{v}, restarting…",
                crate::branding::TRAY_TOOLTIP
            )
        }
        _ => crate::branding::TRAY_TOOLTIP.to_string(),
    }
}

fn emit_snapshot(app: &AppHandle, snapshot: &UpdateSnapshot) {
    if let Err(e) = app.emit(UPDATE_STATUS_EVENT, snapshot) {
        log::warn!("Failed to emit {UPDATE_STATUS_EVENT}: {e}");
    }
    if let Some(tray) = app.tray_by_id(crate::branding::TRAY_ID) {
        let _ = tray.set_tooltip(Some(tooltip_for(snapshot)));
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

/// Signals the scheduler to stop admitting/launching new work, then waits a
/// fixed grace period before the caller proceeds to install + restart
/// (Section 6: "do not install while the scheduler can still admit or launch
/// new work"). Takes an explicit duration so tests can prove the shutdown
/// signal fires without waiting out the real (multi-second) production
/// grace window; [`apply`] always passes [`crate::scheduler::process_exit_grace`].
async fn drain_before_install(grace: Duration) {
    crate::scheduler::initiate_shutdown();
    tokio::time::sleep(grace).await;
}

fn fail_with(
    app: &AppHandle,
    state: &UpdateState,
    phase: UpdatePhase,
    err: &tauri_plugin_updater::Error,
) -> String {
    let mut snapshot = state
        .snapshot
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let info = UpdateErrorInfo {
        kind: classify_updater_error(err).to_string(),
        message: err.to_string(),
    };
    let message = info.message.clone();
    snapshot.phase = phase;
    snapshot.last_error = Some(info);
    snapshot.progress = None;
    let result = snapshot.clone();
    drop(snapshot);
    emit_snapshot(app, &result);
    message
}

/// Downloads, drains, installs, and restarts into the currently-offered
/// update (Sections 3, 4 & 6). `expected_version`, when provided, must match
/// exactly or the call is refused — the exact-version consent guard v5 adds
/// on top of v3's plain "apply the pending update".
///
/// Re-checks the endpoint rather than reusing a cached [`tauri_plugin_updater::Update`]
/// handle, so the artifact that gets installed is always the one just
/// verified against the accepted version, and the download URL/signature
/// pair can never go stale between "banner shown" and "user clicks install".
pub async fn apply(
    app: &AppHandle,
    expected_version: Option<String>,
) -> Result<UpdateSnapshot, String> {
    let state = app.state::<UpdateState>();

    {
        let mut snapshot = state
            .snapshot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !try_begin_apply(&mut snapshot) {
            return Err("An update is already downloading.".to_string());
        }
    }

    // From here on the phase is claimed as `Downloading`; every early-return
    // path below must revert it (via `fail_with` or a direct snapshot write)
    // so a failed/aborted apply never leaves the snapshot stuck reporting an
    // in-progress download.
    let updater = match app.updater() {
        Ok(updater) => updater,
        Err(err) => return Err(fail_with(app, &state, UpdatePhase::Idle, &err)),
    };
    let update = match updater.check().await {
        Ok(Some(update)) => update,
        Ok(None) => {
            let mut snapshot = state
                .snapshot
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            snapshot.phase = UpdatePhase::Idle;
            snapshot.latest_version = None;
            snapshot.notes = None;
            let result = snapshot.clone();
            drop(snapshot);
            emit_snapshot(app, &result);
            return Ok(result);
        }
        // Same error classification as `run_check` (Section 2), so a re-check
        // that fails right as the user clicks "Install" leaves the same kind
        // of actionable `last_error` behind rather than a bare string.
        Err(err) => return Err(fail_with(app, &state, UpdatePhase::Available, &err)),
    };

    if let Some(expected) = expected_version.as_deref() {
        if expected != update.version {
            let mut snapshot = state
                .snapshot
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            snapshot.phase = UpdatePhase::Available;
            snapshot.latest_version = Some(update.version.clone());
            snapshot.notes = update.body.clone();
            drop(snapshot);
            let result = state.snapshot();
            emit_snapshot(app, &result);
            return Err(format!(
                "A newer version (v{}) is now available; refresh before installing.",
                update.version
            ));
        }
    }

    {
        let mut snapshot = state
            .snapshot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // `phase` is already `Downloading` from the atomic claim above; this
        // block just fills in the version/notes/progress now that they're
        // known and re-affirms the phase for readability.
        snapshot.phase = UpdatePhase::Downloading;
        snapshot.latest_version = Some(update.version.clone());
        snapshot.notes = update.body.clone();
        snapshot.progress = Some(UpdateProgress { percent: Some(0.0) });
        snapshot.last_error = None;
        let result = snapshot.clone();
        drop(snapshot);
        emit_snapshot(app, &result);
    }

    let progress_app = app.clone();
    let mut downloaded: usize = 0;
    let download_result = update
        .download(
            move |chunk_len, content_length| {
                downloaded += chunk_len;
                let percent = content_length
                    .filter(|total| *total > 0)
                    .map(|total| (downloaded as f64 / total as f64) * 100.0);
                let state = progress_app.state::<UpdateState>();
                let mut snapshot = state
                    .snapshot
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                snapshot.progress = Some(UpdateProgress { percent });
                let result = snapshot.clone();
                drop(snapshot);
                emit_snapshot(&progress_app, &result);
            },
            || {},
        )
        .await;

    let bytes = match download_result {
        Ok(bytes) => bytes,
        Err(err) => return Err(fail_with(app, &state, UpdatePhase::Available, &err)),
    };

    // Point of no return: stop the scheduler from admitting/launching new
    // work and give in-flight runs a bounded window before the artifact
    // (already signature-verified inside `download()`) gets installed.
    {
        let mut snapshot = state
            .snapshot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        snapshot.phase = UpdatePhase::ReadyToRestart;
        let result = snapshot.clone();
        drop(snapshot);
        emit_snapshot(app, &result);
    }

    drain_before_install(crate::scheduler::process_exit_grace()).await;

    if let Err(err) = update.install(bytes) {
        // The artifact's signature was already verified during `download()`,
        // so an `install()` failure here is an OS-mechanics issue (disk,
        // permissions), not an integrity failure. The scheduler has already
        // stopped admitting work for this process (Section 6 has no "un-
        // shutdown" — matches how a normal quit behaves too), so restarting
        // into the still-intact current binary is the only way back to a
        // healthy scheduler; log loudly and proceed rather than leaving the
        // app running with a permanently-dead scheduler.
        log::error!(
            "Update install failed after scheduler drain; restarting into the current version anyway: {err}"
        );
    }
    app.restart();
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
    fn single_flight_guard_blocks_while_checking_downloading_or_ready_to_restart() {
        let mut checking = snapshot_with_phase(UpdatePhase::Checking);
        assert!(!try_begin_check(&mut checking));
        assert_eq!(checking.phase, UpdatePhase::Checking);

        let mut downloading = snapshot_with_phase(UpdatePhase::Downloading);
        assert!(!try_begin_check(&mut downloading));
        assert_eq!(downloading.phase, UpdatePhase::Downloading);

        // A background/launch check must never overwrite the snapshot during
        // the pre-restart drain window (moments before `app.restart()`).
        let mut ready = snapshot_with_phase(UpdatePhase::ReadyToRestart);
        assert!(!try_begin_check(&mut ready));
        assert_eq!(ready.phase, UpdatePhase::ReadyToRestart);
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
    fn apply_single_flight_guard_blocks_while_downloading() {
        let mut downloading = snapshot_with_phase(UpdatePhase::Downloading);
        assert!(!try_begin_apply(&mut downloading));
        assert_eq!(downloading.phase, UpdatePhase::Downloading);
    }

    #[test]
    fn apply_single_flight_guard_admits_from_idle_available_or_error() {
        for phase in [
            UpdatePhase::Idle,
            UpdatePhase::Available,
            UpdatePhase::Error,
            UpdatePhase::ReadyToRestart,
        ] {
            let mut snapshot = snapshot_with_phase(phase);
            assert!(try_begin_apply(&mut snapshot));
            assert_eq!(snapshot.phase, UpdatePhase::Downloading);
        }
    }

    /// Regression test for the `apply()` race: two concurrent callers racing
    /// on the same `Mutex<UpdateSnapshot>` — one performing the atomic
    /// check-and-set `try_begin_apply` does under the lock, the other
    /// stalled on a simulated `.await` (mirroring `updater.check().await` in
    /// the real `apply()`) before it can even attempt its own claim. Proves
    /// only one caller ever wins the claim, which is the exact mechanism
    /// `apply()` now relies on to stop duplicate concurrent
    /// downloads/installs/restarts.
    #[test]
    fn apply_claim_is_atomic_across_concurrent_callers() {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        rt.block_on(async {
            let state = std::sync::Arc::new(Mutex::new(UpdateSnapshot::new(
                "1.0.0".to_string(),
                true,
                None,
            )));

            let claim = |state: std::sync::Arc<Mutex<UpdateSnapshot>>, delay_ms: u64| async move {
                // Simulate work that happens before a real caller would reach
                // its own lock acquisition (e.g. the `updater.check().await`
                // network round-trip in `apply()`).
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                let mut snapshot = state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                try_begin_apply(&mut snapshot)
            };

            let (first, second) = tokio::join!(
                tokio::spawn(claim(state.clone(), 0)),
                tokio::spawn(claim(state.clone(), 0)),
            );
            let results = [first.unwrap(), second.unwrap()];

            assert_eq!(
                results.iter().filter(|won| **won).count(),
                1,
                "exactly one concurrent caller should win the Downloading claim"
            );
            assert_eq!(
                state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .phase,
                UpdatePhase::Downloading
            );
        });
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

    // Plain (non-`#[tokio::test]`) test so the process-global test-serialization
    // guard — a `std::sync::MutexGuard` — never has to be held across an
    // `.await` point in its own generator, which `clippy::await_holding_lock`
    // (rightly) forbids. The runtime lives entirely inside this sync fn; the
    // guard is never captured by the inner `async move` block.
    #[test]
    fn drain_before_install_signals_shutdown_before_the_grace_elapses() {
        let _guard = crate::scheduler::lock_shutdown_test_state();
        assert!(!crate::scheduler::SHUTDOWN.load(std::sync::atomic::Ordering::Relaxed));

        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        rt.block_on(async {
            let handle = tokio::spawn(drain_before_install(Duration::from_millis(100)));
            tokio::time::sleep(Duration::from_millis(10)).await;
            assert!(
                crate::scheduler::SHUTDOWN.load(std::sync::atomic::Ordering::Relaxed),
                "drain should call initiate_shutdown() well before its grace period elapses"
            );
            handle.await.unwrap();
        });
    }
}
