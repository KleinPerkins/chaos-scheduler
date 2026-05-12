import { useEffect, useRef, useState } from "react";
import {
  getAppConfig,
  getLaunchAtLogin,
  setLaunchAtLogin,
  setNotificationPrefs,
  getNotificationPrefs,
  getEmailConfig,
  setEmailConfig,
  testEmailConfig,
  type EmailConfig,
} from "../lib/commands";
import "./Settings.css";

export default function Settings() {
  const [chaosLabsRoot, setChaosLabsRoot] = useState("(detecting...)");
  const [pythonPath, setPythonPath] = useState("(detecting...)");
  const [notifyOnFailure, setNotifyOnFailure] = useState(true);
  const [notifyOnSuccess, setNotifyOnSuccess] = useState(false);
  const [launchAtLogin, setLaunchAtLoginState] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [statusType, setStatusType] = useState<"info" | "error" | "success">("info");
  const statusTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const [emailConfig, setEmailConfigState] = useState<EmailConfig>({
    enabled: false,
    alert_email: "",
    smtp_host: "smtp.gmail.com",
    smtp_port: 587,
    smtp_user: "",
    smtp_password: "",
    from_address: "",
    from_name: "Chaos Labs Scheduler",
  });
  const [emailDirty, setEmailDirty] = useState(false);
  const [emailSaving, setEmailSaving] = useState(false);
  const [emailTesting, setEmailTesting] = useState(false);

  useEffect(() => {
    getAppConfig()
      .then((config) => {
        setChaosLabsRoot(config.chaos_labs_root);
        setPythonPath(config.python_path);
      })
      .catch(() => {});
    getEmailConfig()
      .then((config) => setEmailConfigState(config))
      .catch(() => {});
    getNotificationPrefs()
      .then((prefs) => {
        setNotifyOnFailure(prefs.notify_on_failure);
        setNotifyOnSuccess(prefs.notify_on_success);
      })
      .catch(() => {});
    getLaunchAtLogin()
      .then((enabled) => setLaunchAtLoginState(enabled))
      .catch(() => {});
    return () => {
      if (statusTimerRef.current) {
        clearTimeout(statusTimerRef.current);
      }
    };
  }, []);

  const showStatus = (msg: string, type: "info" | "error" | "success" = "info", duration = 3000) => {
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

  const handleLaunchToggle = async (enabled: boolean) => {
    setLaunchAtLoginState(enabled);
    try {
      await setLaunchAtLogin(enabled);
      showStatus(
        enabled ? "Launch at login enabled" : "Launch at login disabled",
        "success"
      );
    } catch (e) {
      showStatus(`Error: ${e}`, "error");
    }
  };

  const updateEmailField = <K extends keyof EmailConfig>(key: K, value: EmailConfig[K]) => {
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
        showStatus("Test email sent successfully — check your inbox", "success", 5000);
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
      <div className="page-header">
        <div>
          <h1 className="page-title">Settings</h1>
          <p className="page-subtitle">Configure the scheduler</p>
        </div>
      </div>

      {status && (
        <div className={`settings-status settings-status--${statusType}`}>
          {status}
        </div>
      )}

      <div className="settings-sections">
        <section className="settings-section">
          <h2 className="settings-section-title">Paths</h2>
          <div className="settings-field">
            <label className="settings-label">Chaos Labs Root</label>
            <input type="text" value={chaosLabsRoot} readOnly />
            <span className="settings-hint">
              Auto-detected. Restart the app to change.
            </span>
          </div>
          <div className="settings-field">
            <label className="settings-label">Python Path</label>
            <input type="text" value={pythonPath} readOnly />
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
                <label className="settings-label">Alert Email</label>
                <input
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
                <label className="settings-label">SMTP Provider</label>
                <div className="smtp-presets">
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
                        (p) => p.host === emailConfig.smtp_host
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
                  <label className="settings-label">SMTP Host</label>
                  <input
                    type="text"
                    value={emailConfig.smtp_host}
                    onChange={(e) =>
                      updateEmailField("smtp_host", e.target.value)
                    }
                    placeholder="smtp.gmail.com"
                  />
                </div>
                <div className="settings-field" style={{ width: 90 }}>
                  <label className="settings-label">Port</label>
                  <input
                    type="number"
                    value={emailConfig.smtp_port}
                    onChange={(e) =>
                      updateEmailField("smtp_port", parseInt(e.target.value) || 587)
                    }
                  />
                </div>
              </div>

              <div className="settings-field">
                <label className="settings-label">SMTP Username</label>
                <input
                  type="text"
                  value={emailConfig.smtp_user}
                  onChange={(e) =>
                    updateEmailField("smtp_user", e.target.value)
                  }
                  placeholder="you@gmail.com"
                />
              </div>

              <div className="settings-field">
                <label className="settings-label">SMTP Password</label>
                <input
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
                <label className="settings-label">From Address</label>
                <input
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
                <label className="settings-label">From Name</label>
                <input
                  type="text"
                  value={emailConfig.from_name}
                  onChange={(e) =>
                    updateEmailField("from_name", e.target.value)
                  }
                  placeholder="Chaos Labs Scheduler"
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
                  [Chaos Labs] FAILED: Context Capture |{" "}
                  {new Date().toLocaleDateString("en-CA")}
                </code>
              </div>
            </div>
          )}
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
