import { useEffect, useRef, useState } from "react";
import Button from "./Button";
import PageHeader from "./PageHeader";
import Input from "./Input";
import {
  getAppConfig,
  getLaunchAtLogin,
  setLaunchAtLogin,
  setNotificationPrefs,
  getNotificationPrefs,
  getEmailConfig,
  setEmailConfig,
  testEmailConfig,
  isCommandUnavailable,
  type EmailConfig,
} from "../lib/commands";
import { PRODUCT_NAME, EMAIL_FROM_NAME, APP_VERSION } from "../lib/branding";
import Notice from "./ui/Notice";
import EmailProfiles from "./EmailProfiles";
import ThemeToggle from "./ThemeToggle";
import { useTheme } from "../hooks/useTheme";
import { useAppUpdate } from "../hooks/useAppUpdate";
import "./Settings.css";

export default function Settings() {
  const { preference: themePreference, setPreference: setThemePreference } =
    useTheme();
  const [workspaceRoot, setWorkspaceRoot] = useState("(detecting...)");
  const [pythonPath, setPythonPath] = useState("(detecting...)");
  const [notifyOnFailure, setNotifyOnFailure] = useState(true);
  const [notifyOnSuccess, setNotifyOnSuccess] = useState(false);
  const [launchAtLogin, setLaunchAtLoginState] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [statusType, setStatusType] = useState<"info" | "error" | "success">(
    "info",
  );
  const statusTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const [updateChecking, setUpdateChecking] = useState(false);
  const [updateInstalling, setUpdateInstalling] = useState(false);
  const [updaterUnavailable, setUpdaterUnavailable] = useState(false);
  const {
    snapshot: updateSnapshot,
    checkNow,
    install: installUpdate,
    setBackgroundCheckEnabled,
    skipVersion,
    clearSkippedVersion,
  } = useAppUpdate();
  const [prefsBusy, setPrefsBusy] = useState(false);

  const [emailConfig, setEmailConfigState] = useState<EmailConfig>({
    enabled: false,
    alert_email: "",
    smtp_host: "smtp.gmail.com",
    smtp_port: 587,
    smtp_user: "",
    smtp_password: "",
    from_address: "",
    from_name: EMAIL_FROM_NAME,
  });
  const [emailDirty, setEmailDirty] = useState(false);
  const [emailSaving, setEmailSaving] = useState(false);
  const [emailTesting, setEmailTesting] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    getAppConfig()
      .then((config) => {
        setWorkspaceRoot(
          config.workspace_root ?? config.chaos_labs_root ?? "(unknown)",
        );
        setPythonPath(config.python_path);
      })
      .catch((e) =>
        setLoadError((prev) => prev ?? `App config failed to load: ${e}`),
      );
    getEmailConfig()
      .then((config) => setEmailConfigState(config))
      .catch((e) =>
        setLoadError((prev) => prev ?? `Email config failed to load: ${e}`),
      );
    getNotificationPrefs()
      .then((prefs) => {
        setNotifyOnFailure(prefs.notify_on_failure);
        setNotifyOnSuccess(prefs.notify_on_success);
      })
      .catch((e) =>
        setLoadError(
          (prev) => prev ?? `Notification prefs failed to load: ${e}`,
        ),
      );
    getLaunchAtLogin()
      .then((enabled) => setLaunchAtLoginState(enabled))
      .catch((e) =>
        setLoadError((prev) => prev ?? `Launch-at-login failed to load: ${e}`),
      );
    return () => {
      if (statusTimerRef.current) {
        clearTimeout(statusTimerRef.current);
      }
    };
  }, []);

  const showStatus = (
    msg: string,
    type: "info" | "error" | "success" = "info",
    duration = 3000,
  ) => {
    if (statusTimerRef.current) {
      clearTimeout(statusTimerRef.current);
      statusTimerRef.current = null;
    }
    setStatus(msg);
    setStatusType(type);
    if (duration > 0) {
      statusTimerRef.current = setTimeout(() => {
        setStatus(null);
        statusTimerRef.current = null;
      }, duration);
    }
  };

  const handleNotifChange = async (failure: boolean, success: boolean) => {
    setNotifyOnFailure(failure);
    setNotifyOnSuccess(success);
    try {
      await setNotificationPrefs(failure, success);
      showStatus("Notification preferences saved", "success");
    } catch (e) {
      showStatus(`Error: ${e}`, "error");
    }
  };

  const handleCheckForUpdate = async () => {
    setUpdateChecking(true);
    setUpdaterUnavailable(false);
    try {
      const result = await checkNow();
      if (result?.phase === "idle") {
        showStatus("You are on the latest version.", "success");
      } else if (result?.phase === "available" && result.latest_version) {
        showStatus(`Update available: v${result.latest_version}`, "info");
      }
      // An "error" phase already renders inline below; no separate toast.
    } catch (e) {
      if (isCommandUnavailable(e)) {
        setUpdaterUnavailable(true);
        showStatus("Auto-update is not wired up in this build yet.", "info");
      } else {
        showStatus(`Update check failed: ${e}`, "error");
      }
    } finally {
      setUpdateChecking(false);
    }
  };

  const handleApplyUpdate = async () => {
    if (!updateSnapshot?.latest_version) return;
    setUpdateInstalling(true);
    try {
      await installUpdate(updateSnapshot.latest_version);
      showStatus("Update downloaded — the app will relaunch.", "success", 0);
    } catch (e) {
      if (isCommandUnavailable(e)) {
        setUpdaterUnavailable(true);
        showStatus("Auto-update is not wired up in this build yet.", "info");
      } else {
        showStatus(`Update failed: ${e}`, "error");
      }
    } finally {
      setUpdateInstalling(false);
    }
  };

  const handleBackgroundCheckToggle = async (enabled: boolean) => {
    setPrefsBusy(true);
    try {
      await setBackgroundCheckEnabled(enabled);
    } catch (e) {
      showStatus(`Error: ${e}`, "error");
    } finally {
      setPrefsBusy(false);
    }
  };

  const handleSkipVersion = async () => {
    const version = updateSnapshot?.latest_version;
    if (!version) return;
    setPrefsBusy(true);
    try {
      await skipVersion(version);
      showStatus(`Skipping v${version}`, "info");
    } catch (e) {
      showStatus(`Error: ${e}`, "error");
    } finally {
      setPrefsBusy(false);
    }
  };

  const handleClearSkip = async () => {
    setPrefsBusy(true);
    try {
      await clearSkippedVersion();
    } catch (e) {
      showStatus(`Error: ${e}`, "error");
    } finally {
      setPrefsBusy(false);
    }
  };

  const handleLaunchToggle = async (enabled: boolean) => {
    setLaunchAtLoginState(enabled);
    try {
      await setLaunchAtLogin(enabled);
      showStatus(
        enabled ? "Launch at login enabled" : "Launch at login disabled",
        "success",
      );
    } catch (e) {
      showStatus(`Error: ${e}`, "error");
    }
  };

  const updateEmailField = <K extends keyof EmailConfig>(
    key: K,
    value: EmailConfig[K],
  ) => {
    setEmailConfigState((prev) => ({ ...prev, [key]: value }));
    setEmailDirty(true);
  };

  const handleEmailSave = async () => {
    setEmailSaving(true);
    try {
      await setEmailConfig(emailConfig);
      setEmailDirty(false);
      showStatus("Email configuration saved", "success");
    } catch (e) {
      showStatus(`Failed to save: ${e}`, "error");
    } finally {
      setEmailSaving(false);
    }
  };

  const handleEmailTest = async () => {
    if (emailDirty) {
      showStatus("Save your changes before testing", "info");
      return;
    }
    setEmailTesting(true);
    showStatus("Sending test email...", "info", 0);
    try {
      const result = await testEmailConfig();
      if (result.success) {
        showStatus(
          "Test email sent successfully — check your inbox",
          "success",
          5000,
        );
      } else {
        showStatus(`Test failed: ${result.error}`, "error", 8000);
      }
    } catch (e) {
      showStatus(`Test failed: ${e}`, "error", 8000);
    } finally {
      setEmailTesting(false);
    }
  };

  const smtpPresets: Record<string, { host: string; port: number }> = {
    Gmail: { host: "smtp.gmail.com", port: 587 },
    Outlook: { host: "smtp.office365.com", port: 587 },
    Yahoo: { host: "smtp.mail.yahoo.com", port: 465 },
  };

  return (
    <div>
      <PageHeader title="Settings" subtitle="Configure the scheduler" />

      {loadError && (
        <Notice variant="error" assertive>
          {loadError} Settings below may show defaults until you reload.
        </Notice>
      )}

      {status && (
        <div className={`settings-status settings-status--${statusType}`}>
          {status}
        </div>
      )}

      <div className="settings-sections">
        <section className="settings-section">
          <h2 className="settings-section-title">Appearance</h2>
          <div className="settings-field">
            <span className="settings-label" id="settings-theme-label">
              Color theme
            </span>
            <div
              className="settings-theme-control"
              aria-labelledby="settings-theme-label"
            >
              <ThemeToggle
                preference={themePreference}
                onChange={setThemePreference}
              />
            </div>
            <span className="settings-hint">
              Light, dark, or match your system appearance. Applies instantly
              and is remembered on this device.
            </span>
          </div>
        </section>

        <section className="settings-section">
          <h2 className="settings-section-title">Paths</h2>
          <div className="settings-field">
            <label className="settings-label" htmlFor="settings-workspace-root">
              Workspace Root
            </label>
            <Input
              id="settings-workspace-root"
              type="text"
              value={workspaceRoot}
              readOnly
            />
            <span className="settings-hint">
              Where relative script paths and per-environment working
              directories resolve. Auto-detected; set
              CHAOS_SCHEDULER_WORKSPACE_ROOT to override.
            </span>
          </div>
          <div className="settings-field">
            <label className="settings-label" htmlFor="settings-python-path">
              Python Path
            </label>
            <Input
              id="settings-python-path"
              type="text"
              value={pythonPath}
              readOnly
            />
            <span className="settings-hint">
              Uses .venv/bin/python3 when available, falls back to system
              python3.
            </span>
          </div>
        </section>

        <section className="settings-section">
          <h2 className="settings-section-title">Notifications</h2>
          <div className="settings-row">
            <label className="settings-check">
              <input
                type="checkbox"
                checked={notifyOnFailure}
                onChange={(e) =>
                  handleNotifChange(e.target.checked, notifyOnSuccess)
                }
              />
              Notify on workflow failure
            </label>
          </div>
          <div className="settings-row">
            <label className="settings-check">
              <input
                type="checkbox"
                checked={notifyOnSuccess}
                onChange={(e) =>
                  handleNotifChange(notifyOnFailure, e.target.checked)
                }
              />
              Notify on workflow success
            </label>
          </div>
        </section>

        <section className="settings-section">
          <h2 className="settings-section-title">Email Alerts</h2>
          <span className="settings-hint" style={{ marginBottom: 8 }}>
            Receive an email when a scheduled workflow fails. Emails are only
            sent on failure, never on success.
          </span>

          <div className="settings-row">
            <label className="settings-check">
              <input
                type="checkbox"
                checked={emailConfig.enabled}
                onChange={(e) => updateEmailField("enabled", e.target.checked)}
              />
              Enable email failure alerts
            </label>
          </div>

          {emailConfig.enabled && (
            <div className="email-config-fields">
              <div className="settings-field">
                <label
                  className="settings-label"
                  htmlFor="settings-alert-email"
                >
                  Alert Email
                </label>
                <Input
                  id="settings-alert-email"
                  type="email"
                  value={emailConfig.alert_email}
                  onChange={(e) =>
                    updateEmailField("alert_email", e.target.value)
                  }
                  placeholder="you@example.com"
                />
                <span className="settings-hint">
                  Where failure alerts will be sent
                </span>
              </div>

              <div className="email-config-divider" />

              <div className="settings-field">
                <span
                  className="settings-label"
                  id="settings-smtp-provider-label"
                >
                  SMTP Provider
                </span>
                <div
                  className="smtp-presets"
                  role="group"
                  aria-labelledby="settings-smtp-provider-label"
                >
                  {Object.entries(smtpPresets).map(([name, preset]) => (
                    <button
                      key={name}
                      className={`smtp-preset-btn ${
                        emailConfig.smtp_host === preset.host
                          ? "smtp-preset-btn--active"
                          : ""
                      }`}
                      onClick={() => {
                        updateEmailField("smtp_host", preset.host);
                        updateEmailField("smtp_port", preset.port);
                      }}
                    >
                      {name}
                    </button>
                  ))}
                  <button
                    className={`smtp-preset-btn ${
                      !Object.values(smtpPresets).some(
                        (p) => p.host === emailConfig.smtp_host,
                      )
                        ? "smtp-preset-btn--active"
                        : ""
                    }`}
                    onClick={() => {
                      updateEmailField("smtp_host", "");
                      updateEmailField("smtp_port", 587);
                    }}
                  >
                    Custom
                  </button>
                </div>
              </div>

              <div className="settings-field-row">
                <div className="settings-field" style={{ flex: 1 }}>
                  <label
                    className="settings-label"
                    htmlFor="settings-smtp-host"
                  >
                    SMTP Host
                  </label>
                  <Input
                    id="settings-smtp-host"
                    type="text"
                    value={emailConfig.smtp_host}
                    onChange={(e) =>
                      updateEmailField("smtp_host", e.target.value)
                    }
                    placeholder="smtp.gmail.com"
                  />
                </div>
                <div className="settings-field" style={{ width: 90 }}>
                  <label
                    className="settings-label"
                    htmlFor="settings-smtp-port"
                  >
                    Port
                  </label>
                  <Input
                    id="settings-smtp-port"
                    type="number"
                    value={emailConfig.smtp_port}
                    onChange={(e) =>
                      updateEmailField(
                        "smtp_port",
                        parseInt(e.target.value) || 587,
                      )
                    }
                  />
                </div>
              </div>

              <div className="settings-field">
                <label className="settings-label" htmlFor="settings-smtp-user">
                  SMTP Username
                </label>
                <Input
                  id="settings-smtp-user"
                  type="text"
                  value={emailConfig.smtp_user}
                  onChange={(e) =>
                    updateEmailField("smtp_user", e.target.value)
                  }
                  placeholder="you@gmail.com"
                />
              </div>

              <div className="settings-field">
                <label
                  className="settings-label"
                  htmlFor="settings-smtp-password"
                >
                  SMTP Password
                </label>
                <Input
                  id="settings-smtp-password"
                  type="password"
                  value={emailConfig.smtp_password}
                  onChange={(e) =>
                    updateEmailField("smtp_password", e.target.value)
                  }
                  placeholder="App password or SMTP password"
                />
                <span className="settings-hint">
                  For Gmail, use an{" "}
                  <a
                    href="https://myaccount.google.com/apppasswords"
                    target="_blank"
                    rel="noopener noreferrer"
                    className="settings-link"
                  >
                    App Password
                  </a>{" "}
                  (requires 2FA enabled on your Google account)
                </span>
              </div>

              <div className="email-config-divider" />

              <div className="settings-field">
                <label
                  className="settings-label"
                  htmlFor="settings-from-address"
                >
                  From Address
                </label>
                <Input
                  id="settings-from-address"
                  type="text"
                  value={emailConfig.from_address}
                  onChange={(e) =>
                    updateEmailField("from_address", e.target.value)
                  }
                  placeholder="noreply@example.com"
                />
                <span className="settings-hint">
                  The sender address shown on alert emails. Some SMTP providers
                  require this to match the authenticated account.
                </span>
              </div>

              <div className="settings-field">
                <label className="settings-label" htmlFor="settings-from-name">
                  From Name
                </label>
                <Input
                  id="settings-from-name"
                  type="text"
                  value={emailConfig.from_name}
                  onChange={(e) =>
                    updateEmailField("from_name", e.target.value)
                  }
                  placeholder={EMAIL_FROM_NAME}
                />
              </div>

              <div className="email-actions">
                <button
                  className="email-save-btn"
                  onClick={handleEmailSave}
                  disabled={!emailDirty || emailSaving}
                >
                  {emailSaving ? "Saving..." : "Save Configuration"}
                </button>
                <button
                  className="email-test-btn"
                  onClick={handleEmailTest}
                  disabled={emailTesting || emailDirty}
                  title={
                    emailDirty
                      ? "Save configuration before testing"
                      : "Send a test failure email"
                  }
                >
                  {emailTesting ? "Sending..." : "Send Test Email"}
                </button>
              </div>

              <div className="email-subject-preview">
                <span className="settings-label">Subject line preview</span>
                <code className="subject-preview-text">
                  [{PRODUCT_NAME}] FAILED: Context Capture |{" "}
                  {new Date().toLocaleDateString("en-CA")}
                </code>
              </div>
            </div>
          )}
        </section>

        <section className="settings-section">
          <h2 className="settings-section-title">Email Profiles</h2>
          <EmailProfiles />
        </section>

        <section className="settings-section">
          <h2 className="settings-section-title">Updates</h2>
          <div className="settings-field">
            <label className="settings-label" htmlFor="settings-app-version">
              Current version
            </label>
            <Input
              id="settings-app-version"
              type="text"
              value={`v${APP_VERSION}`}
              readOnly
            />
          </div>
          <div className="settings-row">
            <label className="settings-check">
              <input
                type="checkbox"
                checked={updateSnapshot?.background_check_enabled ?? true}
                disabled={prefsBusy}
                onChange={(e) => handleBackgroundCheckToggle(e.target.checked)}
              />
              Check for updates automatically
            </label>
          </div>
          <div className="settings-row settings-update-row">
            <Button
              variant="ghost"
              size="sm"
              onClick={handleCheckForUpdate}
              disabled={updateChecking || updateInstalling}
            >
              {updateChecking ? "Checking..." : "Check for updates"}
            </Button>
            {updateSnapshot?.phase === "available" &&
              updateSnapshot.latest_version && (
                <Button
                  variant="primary"
                  size="sm"
                  onClick={handleApplyUpdate}
                  disabled={updateInstalling}
                >
                  {updateInstalling
                    ? "Installing..."
                    : `Install and Restart v${updateSnapshot.latest_version}`}
                </Button>
              )}
          </div>
          {updateSnapshot?.phase === "available" && updateSnapshot.notes && (
            <div className="settings-hint settings-release-notes">
              {updateSnapshot.notes}
            </div>
          )}
          {updateSnapshot?.phase === "error" && updateSnapshot.last_error && (
            <div className="settings-status settings-status--error">
              Update check failed ({updateSnapshot.last_error.kind}):{" "}
              {updateSnapshot.last_error.message}
            </div>
          )}
          {updateSnapshot?.skipped_version ? (
            <div className="settings-row settings-update-row">
              <span className="settings-hint">
                Skipping v{updateSnapshot.skipped_version}
              </span>
              <Button
                variant="ghost"
                size="sm"
                onClick={handleClearSkip}
                disabled={prefsBusy}
              >
                Clear skip
              </Button>
            </div>
          ) : (
            updateSnapshot?.phase === "available" &&
            updateSnapshot.latest_version && (
              <div className="settings-row settings-update-row">
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={handleSkipVersion}
                  disabled={prefsBusy}
                >
                  Skip this version (v{updateSnapshot.latest_version})
                </Button>
              </div>
            )
          )}
          <span className="settings-hint">
            {updaterUnavailable
              ? "Auto-update is not available in this build. Download the latest release manually from GitHub."
              : "Updates are downloaded from GitHub Releases and signature-verified before install."}
          </span>
        </section>

        <section className="settings-section">
          <h2 className="settings-section-title">System</h2>
          <div className="settings-row">
            <label className="settings-check">
              <input
                type="checkbox"
                checked={launchAtLogin}
                onChange={(e) => handleLaunchToggle(e.target.checked)}
              />
              Launch at login
            </label>
            <span className="settings-hint">
              Creates a launchd plist to start the scheduler on macOS login
            </span>
          </div>
        </section>
      </div>
    </div>
  );
}
