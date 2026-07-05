import { useEffect, useState } from "react";
import {
  createApiKey,
  listApiKeys,
  revokeApiKey,
  isCommandUnavailable,
} from "../lib/commands";
import type { ApiKey, ApiKeyScope, NewApiKey } from "../lib/commands";
import { PRODUCT_NAME, REPO_SLUG, RELEASES_URL } from "../lib/branding";
import Notice from "./ui/Notice";
import { openExternalSafe } from "../lib/openExternalSafe";
import "./Integrations.css";

const ALL_SCOPES: ApiKeyScope[] = ["read", "write", "admin"];

function mcpConfigSnippet(token: string): string {
  return JSON.stringify(
    {
      mcpServers: {
        "chaos-scheduler": {
          command: "npx",
          args: ["-y", "@chaos-scheduler/mcp-server"],
          env: {
            CHAOS_SCHEDULER_API_KEY: token || "<your-api-key>",
            CHAOS_SCHEDULER_API_URL: "http://127.0.0.1:9618",
          },
        },
      },
    },
    null,
    2,
  );
}

function addToCursorLink(token: string): string {
  const config = {
    command: "npx",
    args: ["-y", "@chaos-scheduler/mcp-server"],
    env: {
      CHAOS_SCHEDULER_API_KEY: token || "<your-api-key>",
      CHAOS_SCHEDULER_API_URL: "http://127.0.0.1:9618",
    },
  };
  const encoded = btoa(JSON.stringify(config));
  return `cursor://anysphere.cursor-deeplink/mcp/install?name=chaos-scheduler&config=${encoded}`;
}

export default function Integrations() {
  const [keys, setKeys] = useState<ApiKey[]>([]);
  const [keysUnavailable, setKeysUnavailable] = useState(false);
  const [keyName, setKeyName] = useState("");
  const [scopes, setScopes] = useState<ApiKeyScope[]>(["read"]);
  const [newKey, setNewKey] = useState<NewApiKey | null>(null);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [statusType, setStatusType] = useState<"info" | "error" | "success">(
    "info",
  );
  const [copied, setCopied] = useState<string | null>(null);
  const [revokePendingId, setRevokePendingId] = useState<string | null>(null);

  const notify = (msg: string, type: "info" | "error" | "success" = "info") => {
    setStatus(msg);
    setStatusType(type);
  };

  const loadKeys = async () => {
    try {
      const rows = await listApiKeys();
      setKeys(rows);
      setKeysUnavailable(false);
    } catch (err) {
      if (isCommandUnavailable(err)) {
        setKeysUnavailable(true);
      } else {
        notify(String(err), "error");
      }
    }
  };

  // Defer the initial load to a macrotask so the fetch's state updates do
  // not run inside the effect body (avoids react-hooks/set-state-in-effect).
  // Mirrors the established pattern in useSchedulerStatus.
  useEffect(() => {
    const id = setTimeout(() => void loadKeys(), 0);
    return () => clearTimeout(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const copy = async (label: string, value: string) => {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(label);
      window.setTimeout(() => setCopied((c) => (c === label ? null : c)), 1500);
    } catch {
      notify("Copy failed — select and copy manually.", "error");
    }
  };

  const toggleScope = (scope: ApiKeyScope) =>
    setScopes((current) =>
      current.includes(scope)
        ? current.filter((s) => s !== scope)
        : [...current, scope],
    );

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    try {
      const key = await createApiKey(
        keyName.trim() || undefined,
        scopes.length ? scopes : ["read"],
      );
      setNewKey(key);
      setKeyName("");
      notify(
        "API key created. Copy it now — it is shown only once.",
        "success",
      );
      await loadKeys();
    } catch (err) {
      notify(String(err), "error");
    } finally {
      setBusy(false);
    }
  };

  const handleRevoke = async (id: string) => {
    if (revokePendingId !== id) {
      setRevokePendingId(id);
      return;
    }
    setRevokePendingId(null);
    setBusy(true);
    try {
      await revokeApiKey(id);
      notify("API key revoked.", "success");
      await loadKeys();
    } catch (err) {
      if (isCommandUnavailable(err)) {
        notify(
          "Revoking keys needs a backend command that is not available yet.",
          "info",
        );
      } else {
        notify(String(err), "error");
      }
    } finally {
      setBusy(false);
    }
  };

  const tokenForSnippet = newKey?.token ?? "";
  const writeScopeSelected =
    scopes.includes("write") || scopes.includes("admin");

  return (
    <div className="integrations">
      <div className="page-header">
        <div>
          <h1 className="page-title">Integrations</h1>
          <p className="page-subtitle">
            API keys, webhooks, and the Cursor MCP connection for {PRODUCT_NAME}
            .
          </p>
        </div>
      </div>

      {status && (
        <Notice variant={statusType} onDismiss={() => setStatus(null)}>
          {status}
        </Notice>
      )}

      <section className="intg-section">
        <h2 className="intg-section-title">API keys</h2>
        <p className="intg-copy">
          Keys authenticate the REST API and the MCP server. The plaintext token
          is shown once at creation and stored only as a salted hash.
        </p>

        <form className="intg-key-form" onSubmit={handleCreate}>
          <label className="intg-field">
            <span>Key name (optional)</span>
            <input
              type="text"
              value={keyName}
              onChange={(e) => setKeyName(e.target.value)}
              placeholder="ci-runner"
            />
          </label>
          <fieldset className="intg-scopes">
            <legend>Scopes</legend>
            {ALL_SCOPES.map((scope) => (
              <label key={scope} className="intg-scope">
                <input
                  type="checkbox"
                  checked={scopes.includes(scope)}
                  onChange={() => toggleScope(scope)}
                />
                {scope}
              </label>
            ))}
          </fieldset>
          {writeScopeSelected && (
            <p className="intg-hint" role="note">
              <strong>Write/admin keys can execute local code.</strong> Treat
              them like machine-owner credentials: store them in a secret
              manager, revoke unused keys, and avoid sharing them in chat or
              logs. Protected environments require an explicit backend override.
            </p>
          )}
          <button type="submit" className="btn btn-primary" disabled={busy}>
            {busy ? "Working..." : "Create key"}
          </button>
        </form>

        {newKey && (
          <div className="intg-new-key" role="status">
            <span className="intg-new-key-label">
              New key ({newKey.scopes}) — copy it now:
            </span>
            <div className="intg-token-row">
              <code className="intg-token">{newKey.token}</code>
              <button
                type="button"
                className="btn btn-ghost btn-sm"
                onClick={() => copy("token", newKey.token)}
              >
                {copied === "token" ? "Copied" : "Copy"}
              </button>
            </div>
          </div>
        )}

        {keysUnavailable ? (
          <p className="intg-hint">
            Listing existing keys requires a backend command that is not
            available yet. Newly created keys still work.
          </p>
        ) : keys.length === 0 ? (
          <p className="intg-hint">No API keys yet.</p>
        ) : (
          <table className="intg-key-table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Scopes</th>
                <th>Created</th>
                <th>Last used</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              {keys.map((key) => (
                <tr key={key.id}>
                  <td>{key.name ?? key.id}</td>
                  <td>{key.scopes}</td>
                  <td>{key.created_at ?? "—"}</td>
                  <td>{key.last_used_at ?? "never"}</td>
                  <td>
                    <button
                      className="btn btn-danger btn-sm"
                      onClick={() => handleRevoke(key.id)}
                      disabled={busy}
                      aria-label={
                        revokePendingId === key.id
                          ? "Confirm revoke API key"
                          : "Revoke API key"
                      }
                    >
                      {revokePendingId === key.id
                        ? "Confirm revoke?"
                        : "Revoke"}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>

      <section className="intg-section">
        <h2 className="intg-section-title">Result webhooks</h2>
        <p className="intg-copy">
          Outbound result webhooks are configured per workflow as an{" "}
          <strong>on-success</strong> or
          <strong> on-failure</strong> action in the Workflow editor. Each
          delivery is signed with HMAC-SHA256 (
          <code>x-chaos-signature: sha256=…</code>) using the secret you set on
          the action, with bounded retries and dead-letter capture.
        </p>
        <p className="intg-copy">
          Inbound event triggers post to{" "}
          <code>POST /workflows/&#123;id&#125;/dispatch</code> on the embedded
          API (default <code>http://127.0.0.1:9618</code>) with an API key.
        </p>
      </section>

      <section className="intg-section">
        <h2 className="intg-section-title">Cursor MCP server</h2>
        <p className="intg-copy">
          Connect Cursor to {PRODUCT_NAME} so agents can register, run, and
          inspect workflows. Create an API key above, then add the MCP server to
          Cursor.
        </p>
        <div className="intg-mcp-actions">
          <button
            type="button"
            className="btn btn-primary"
            onClick={() =>
              openExternalSafe(addToCursorLink(tokenForSnippet)).catch(() =>
                notify("Could not open Cursor.", "error"),
              )
            }
          >
            Add to Cursor
          </button>
          <button
            type="button"
            className="btn btn-ghost"
            onClick={() => copy("mcp", mcpConfigSnippet(tokenForSnippet))}
          >
            {copied === "mcp" ? "Copied" : "Copy .cursor/mcp.json"}
          </button>
        </div>
        <pre className="intg-snippet" aria-label="Cursor MCP configuration">
          {mcpConfigSnippet(tokenForSnippet)}
        </pre>
        {!newKey && (
          <p className="intg-hint">
            Create a key above to embed it in the snippet; otherwise replace
            <code>&lt;your-api-key&gt;</code> manually.
          </p>
        )}
      </section>

      <section className="intg-section">
        <h2 className="intg-section-title">SDK &amp; docs</h2>
        <ul className="intg-links">
          <li>
            <button
              className="intg-link"
              onClick={() =>
                openExternalSafe(`https://github.com/${REPO_SLUG}#readme`)
              }
            >
              Integration guide (README)
            </button>
          </li>
          <li>
            <button
              className="intg-link"
              onClick={() =>
                openExternalSafe(
                  `https://github.com/${REPO_SLUG}/tree/main/packages/sdk-ts`,
                )
              }
            >
              TypeScript SDK
            </button>
          </li>
          <li>
            <button
              className="intg-link"
              onClick={() => openExternalSafe(RELEASES_URL)}
            >
              Releases &amp; downloads
            </button>
          </li>
        </ul>
      </section>
    </div>
  );
}
