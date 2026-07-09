import { useEffect, useState } from "react";
import { Mail, Plus, Trash2, Pencil, Send } from "lucide-react";
import {
  listEmailProfiles,
  saveEmailProfile,
  deleteEmailProfile,
  testEmailProfile,
  isCommandUnavailable,
  type EmailProfile,
} from "../lib/commands";
import { EMAIL_FROM_NAME } from "../lib/branding";
import Notice from "./ui/Notice";
import Input from "./Input";

const MASK = "••••••••";

function blankProfile(): EmailProfile {
  return {
    id: "",
    name: "",
    enabled: true,
    alert_email: "",
    smtp_host: "smtp.gmail.com",
    smtp_port: 587,
    smtp_user: "",
    smtp_password: "",
    from_address: "",
    from_name: EMAIL_FROM_NAME,
  };
}

const smtpPresets: Record<string, { host: string; port: number }> = {
  Gmail: { host: "smtp.gmail.com", port: 587 },
  Outlook: { host: "smtp.office365.com", port: 587 },
  Yahoo: { host: "smtp.mail.yahoo.com", port: 465 },
};

/**
 * Manager for named, reusable email-delivery profiles. Workflows select a
 * profile for their failure alerts; the global email config remains the master
 * enable switch and the fallback when no profile is chosen.
 */
export default function EmailProfiles() {
  const [profiles, setProfiles] = useState<EmailProfile[]>([]);
  const [unavailable, setUnavailable] = useState(false);
  const [editing, setEditing] = useState<EmailProfile | null>(null);
  const [saving, setSaving] = useState(false);
  const [testingId, setTestingId] = useState<string | null>(null);
  const [status, setStatus] = useState<{
    text: string;
    type: "info" | "error" | "success";
  } | null>(null);

  const refresh = () =>
    listEmailProfiles()
      .then(setProfiles)
      .catch((e) => {
        if (isCommandUnavailable(e)) setUnavailable(true);
        else
          setStatus({ text: `Failed to load profiles: ${e}`, type: "error" });
      });

  useEffect(() => {
    void refresh();
  }, []);

  if (unavailable) return null;

  const update = <K extends keyof EmailProfile>(
    key: K,
    value: EmailProfile[K],
  ) => setEditing((prev) => (prev ? { ...prev, [key]: value } : prev));

  const handleSave = async () => {
    if (!editing) return;
    if (!editing.name.trim()) {
      setStatus({ text: "Profile name is required.", type: "error" });
      return;
    }
    setSaving(true);
    try {
      await saveEmailProfile(editing);
      setStatus({ text: `Saved profile "${editing.name}".`, type: "success" });
      setEditing(null);
      await refresh();
    } catch (e) {
      setStatus({ text: `Save failed: ${e}`, type: "error" });
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (profile: EmailProfile) => {
    try {
      await deleteEmailProfile(profile.id);
      setStatus({ text: `Deleted "${profile.name}".`, type: "info" });
      if (editing?.id === profile.id) setEditing(null);
      await refresh();
    } catch (e) {
      setStatus({ text: `Delete failed: ${e}`, type: "error" });
    }
  };

  const handleTest = async (profile: EmailProfile) => {
    setTestingId(profile.id);
    try {
      const result = await testEmailProfile(profile.id);
      if (result.success)
        setStatus({
          text: `Test email sent via "${profile.name}".`,
          type: "success",
        });
      else
        setStatus({
          text: `Test failed: ${result.error ?? "unknown error"}`,
          type: "error",
        });
    } catch (e) {
      setStatus({ text: `Test failed: ${e}`, type: "error" });
    } finally {
      setTestingId(null);
    }
  };

  return (
    <div className="email-profiles">
      <div className="email-profiles-header">
        <p className="settings-hint" style={{ margin: 0 }}>
          Named delivery profiles let different workflows send alerts to
          different recipients or mailboxes. Workflows pick a profile in the
          editor; the global config above is used when none is selected.
        </p>
        {!editing && (
          <button
            className="email-save-btn"
            onClick={() => setEditing(blankProfile())}
          >
            <Plus size={14} /> New Profile
          </button>
        )}
      </div>

      {status && (
        <Notice variant={status.type} onDismiss={() => setStatus(null)}>
          {status.text}
        </Notice>
      )}

      {profiles.length === 0 && !editing && (
        <div className="email-profiles-empty">
          <Mail size={20} />
          <span>No email profiles yet.</span>
        </div>
      )}

      {profiles.length > 0 && (
        <ul className="email-profiles-list">
          {profiles.map((p) => (
            <li key={p.id} className="email-profile-row">
              <div className="email-profile-meta">
                <span className="email-profile-name">
                  {p.name}
                  {!p.enabled && (
                    <span className="email-profile-disabled"> (disabled)</span>
                  )}
                </span>
                <span className="email-profile-recipient">{p.alert_email}</span>
              </div>
              <div className="email-profile-row-actions">
                <button
                  className="icon-btn"
                  title="Send test email"
                  onClick={() => handleTest(p)}
                  disabled={testingId === p.id}
                >
                  <Send size={14} />
                </button>
                <button
                  className="icon-btn"
                  title="Edit profile"
                  onClick={() => setEditing({ ...p })}
                >
                  <Pencil size={14} />
                </button>
                <button
                  className="icon-btn icon-btn--danger"
                  title="Delete profile"
                  onClick={() => handleDelete(p)}
                >
                  <Trash2 size={14} />
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}

      {editing && (
        <div className="email-profile-form">
          <div className="settings-field">
            <label className="settings-label" htmlFor="profile-name">
              Profile Name
            </label>
            <Input
              id="profile-name"
              type="text"
              value={editing.name}
              onChange={(e) => update("name", e.target.value)}
              placeholder="e.g. Production alerts"
            />
          </div>

          <label className="settings-check">
            <input
              type="checkbox"
              checked={editing.enabled}
              onChange={(e) => update("enabled", e.target.checked)}
            />
            Enabled
          </label>

          <div className="settings-field">
            <label className="settings-label" htmlFor="profile-recipient">
              Recipient
            </label>
            <Input
              id="profile-recipient"
              type="email"
              value={editing.alert_email}
              onChange={(e) => update("alert_email", e.target.value)}
              placeholder="alerts@example.com"
            />
          </div>

          <div
            className="smtp-presets"
            role="group"
            aria-label="SMTP provider preset"
          >
            {Object.entries(smtpPresets).map(([name, preset]) => (
              <button
                key={name}
                type="button"
                className={`smtp-preset-btn ${
                  editing.smtp_host === preset.host
                    ? "smtp-preset-btn--active"
                    : ""
                }`}
                onClick={() => {
                  update("smtp_host", preset.host);
                  update("smtp_port", preset.port);
                }}
              >
                {name}
              </button>
            ))}
          </div>

          <div className="settings-field-row">
            <div className="settings-field">
              <label className="settings-label" htmlFor="profile-host">
                SMTP Host
              </label>
              <Input
                id="profile-host"
                type="text"
                value={editing.smtp_host}
                onChange={(e) => update("smtp_host", e.target.value)}
                placeholder="smtp.gmail.com"
              />
            </div>
            <div className="settings-field">
              <label className="settings-label" htmlFor="profile-port">
                Port
              </label>
              <Input
                id="profile-port"
                type="number"
                value={editing.smtp_port}
                onChange={(e) =>
                  update("smtp_port", parseInt(e.target.value, 10) || 587)
                }
              />
            </div>
          </div>

          <div className="settings-field">
            <label className="settings-label" htmlFor="profile-user">
              SMTP Username
            </label>
            <Input
              id="profile-user"
              type="text"
              value={editing.smtp_user}
              onChange={(e) => update("smtp_user", e.target.value)}
              placeholder="you@example.com"
            />
          </div>

          <div className="settings-field">
            <label className="settings-label" htmlFor="profile-password">
              SMTP Password
            </label>
            <Input
              id="profile-password"
              type="password"
              value={editing.smtp_password}
              onChange={(e) => update("smtp_password", e.target.value)}
              placeholder={editing.id ? MASK : "app password"}
            />
          </div>

          <div className="settings-field-row">
            <div className="settings-field">
              <label className="settings-label" htmlFor="profile-from-address">
                From Address
              </label>
              <Input
                id="profile-from-address"
                type="text"
                value={editing.from_address}
                onChange={(e) => update("from_address", e.target.value)}
                placeholder="noreply@example.com"
              />
            </div>
            <div className="settings-field">
              <label className="settings-label" htmlFor="profile-from-name">
                From Name
              </label>
              <Input
                id="profile-from-name"
                type="text"
                value={editing.from_name}
                onChange={(e) => update("from_name", e.target.value)}
                placeholder={EMAIL_FROM_NAME}
              />
            </div>
          </div>

          <div className="email-actions">
            <button
              className="email-save-btn"
              onClick={handleSave}
              disabled={saving}
            >
              {saving ? "Saving..." : "Save Profile"}
            </button>
            <button
              className="email-test-btn"
              onClick={() => setEditing(null)}
              disabled={saving}
            >
              Cancel
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
