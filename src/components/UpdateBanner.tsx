import { useState } from "react";
import Notice from "./ui/Notice";
import { useAppUpdate } from "../hooks/useAppUpdate";
import { openExternalSafe } from "../lib/openExternalSafe";
import { RELEASES_URL } from "../lib/branding";
import "./UpdateBanner.css";

/**
 * Persistent, low-noise update affordance shown at the top of every
 * dashboard view (updater UX plan, Section 3). Purely a view over
 * `useAppUpdate()`'s snapshot — silent background-check failures resolve
 * back to `idle` with no banner (Section 1: background checks stay quiet);
 * only a genuinely offered update, an in-progress install, or a *manual*
 * check failure (`phase === "error"`) render anything. No dismiss action —
 * "Skip this version" (Section 2) is the only escape hatch.
 */
export default function UpdateBanner() {
  const { snapshot, install, skipVersion } = useAppUpdate();
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  if (!snapshot) return null;
  const {
    phase,
    latest_version,
    skipped_version,
    notes,
    progress,
    last_error,
  } = snapshot;

  if (phase === "error") {
    return (
      <Notice variant="error" assertive className="update-banner">
        <span>
          Update check failed
          {last_error ? ` (${last_error.kind}): ${last_error.message}` : "."}
        </span>
      </Notice>
    );
  }

  // Give instant feedback on skip rather than waiting for the next check
  // (the backend only re-derives `phase` from `skipped_version` on the next
  // check — Section 2).
  const justSkipped =
    phase === "available" &&
    !!latest_version &&
    latest_version === skipped_version;

  if (
    justSkipped ||
    (phase !== "available" &&
      phase !== "downloading" &&
      phase !== "ready_to_restart")
  ) {
    return null;
  }

  const downloading = phase === "downloading" || phase === "ready_to_restart";
  const percent = progress?.percent;

  const handleInstall = async () => {
    setActionError(null);
    setBusy(true);
    try {
      // A real install restarts the process and this promise never
      // resolves; it only returns for the rare "nothing to install" race.
      await install(latest_version ?? undefined);
    } catch (e) {
      setActionError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleSkip = async () => {
    if (!latest_version) return;
    setActionError(null);
    try {
      await skipVersion(latest_version);
    } catch (e) {
      setActionError(String(e));
    }
  };

  const handleReleaseNotes = () => {
    if (!latest_version) return;
    void openExternalSafe(
      `${RELEASES_URL}/tag/chaos-scheduler-v${latest_version}`,
    );
  };

  return (
    <Notice
      variant={actionError ? "error" : "info"}
      assertive={!!actionError}
      className="update-banner"
    >
      <div className="update-banner__row" aria-busy={downloading || undefined}>
        <div className="update-banner__text">
          <strong>Update available: v{latest_version ?? "?"}</strong>
          {notes && <p className="update-banner__notes">{notes}</p>}
          {downloading && (
            <p className="update-banner__progress">
              {phase === "ready_to_restart"
                ? "Installing update…"
                : `Downloading update…${percent != null ? ` ${Math.round(percent)}%` : ""}`}
            </p>
          )}
          {actionError && <p className="update-banner__error">{actionError}</p>}
          {!actionError && last_error && (
            <p className="update-banner__error">
              Last attempt failed ({last_error.kind}): {last_error.message}
            </p>
          )}
        </div>
        <div className="update-banner__actions">
          <button
            type="button"
            className="btn btn-primary btn-sm"
            disabled={downloading || busy}
            onClick={handleInstall}
          >
            {downloading ? "Installing…" : "Install and Restart"}
          </button>
          <button
            type="button"
            className="btn btn-ghost btn-sm"
            disabled={!latest_version}
            onClick={handleReleaseNotes}
          >
            Release notes
          </button>
          <button
            type="button"
            className="btn btn-ghost btn-sm"
            disabled={downloading}
            onClick={handleSkip}
          >
            Skip this version
          </button>
        </div>
      </div>
    </Notice>
  );
}
