import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import Button from "./Button";
import Field from "./Field";
import Input from "./Input";
import {
  createApiKey,
  listApiKeys,
  revokeApiKey,
  getMcpIntegrationStatus,
  provisionMcpIntegration,
  removeMcpIntegration,
  isCommandUnavailable,
} from "../lib/commands";
import type {
  ApiKey,
  ApiKeyScope,
  McpInstallStatus,
  McpIntegrationStatus,
  NewApiKey,
} from "../lib/commands";
import { PRODUCT_NAME, REPO_SLUG, RELEASES_URL } from "../lib/branding";
import Notice from "./ui/Notice";
import { openExternalSafe } from "../lib/openExternalSafe";
import "./Integrations.css";

const ALL_SCOPES: ApiKeyScope[] = ["read", "write", "admin"];

// Mirrors the Rust-side `mcp::MCP_STATUS_EVENT` constant — Rust emits this
// whenever provision/remove completes or the background startup
// re-provision hook finishes, so this card stays live even when that
// background thread completes after the page has already mounted and
// fetched its initial status.
const MCP_STATUS_EVENT = "mcp-status-changed";

function mcpConfigSnippet(token: string): string {
  return JSON.stringify(
    {
      mcpServers: {
        "chaos-scheduler": {
          command: "npx",
          args: ["-y", "@chaos-scheduler/mcp-server"],
          env: {
            CHAOS_SCHEDULER_API_KEY: token || "<your-api-key>",
            CHAOS_SCHEDULER_URL: "http://127.0.0.1:9618",
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
      CHAOS_SCHEDULER_URL: "http://127.0.0.1:9618",
    },
  };
  const encoded = btoa(JSON.stringify(config));
  return `cursor://anysphere.cursor-deeplink/mcp/install?name=chaos-scheduler&config=${encoded}`;
}

const MCP_STATUS_LABEL: Record<McpInstallStatus, string> = {
  not_installed: "Not installed",
  installed: "Installed",
  stale: "Update available",
  node_unavailable: "Node.js not found",
};

const MCP_STATUS_VARIANT: Record<McpInstallStatus, "good" | "warn" | "bad"> = {
  not_installed: "warn",
  installed: "good",
  stale: "warn",
  node_unavailable: "bad",
};

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
  const revokeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const [mcpStatus, setMcpStatus] = useState<McpIntegrationStatus | null>(null);
  const [mcpUnavailable, setMcpUnavailable] = useState(false);
  const [mcpBusy, setMcpBusy] = useState(false);
  // "Remove" and "Prepare to uninstall" are two distinct destructive
  // actions that happen to share a backend command — each gets its own
  // independent two-step confirm state so arming one can never be
  // misread as confirming the other (see handleMcpRemoveClick /
  // handleMcpPrepareUninstallClick below).
  const [mcpRemovePending, setMcpRemovePending] = useState(false);
  const mcpRemoveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [mcpPrepareUninstallPending, setMcpPrepareUninstallPending] =
    useState(false);
  const mcpPrepareUninstallTimerRef = useRef<ReturnType<
    typeof setTimeout
  > | null>(null);

  const clearRevokeTimer = () => {
    if (revokeTimerRef.current) {
      clearTimeout(revokeTimerRef.current);
      revokeTimerRef.current = null;
    }
  };

  const clearMcpRemoveTimer = () => {
    if (mcpRemoveTimerRef.current) {
      clearTimeout(mcpRemoveTimerRef.current);
      mcpRemoveTimerRef.current = null;
    }
  };

  const clearMcpPrepareUninstallTimer = () => {
    if (mcpPrepareUninstallTimerRef.current) {
      clearTimeout(mcpPrepareUninstallTimerRef.current);
      mcpPrepareUninstallTimerRef.current = null;
    }
  };

  useEffect(() => () => clearRevokeTimer(), []);
  useEffect(() => () => clearMcpRemoveTimer(), []);
  useEffect(() => () => clearMcpPrepareUninstallTimer(), []);

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

  const loadMcpStatus = async () => {
    try {
      const s = await getMcpIntegrationStatus();
      setMcpStatus(s);
      setMcpUnavailable(false);
    } catch (err) {
      if (isCommandUnavailable(err)) {
        setMcpUnavailable(true);
      } else {
        notify(String(err), "error");
      }
    }
  };

  // Defer the initial load to a macrotask so the fetch's state updates do
  // not run inside the effect body (avoids react-hooks/set-state-in-effect).
  // Mirrors the established pattern in useSchedulerStatus.
  useEffect(() => {
    const id = setTimeout(() => {
      void loadKeys();
      void loadMcpStatus();
    }, 0);
    return () => clearTimeout(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Stay live if the managed integration's status changes on the Rust side
  // without this component driving it — most notably the background
  // startup re-provision hook, which can complete after this card has
  // already mounted and fetched its initial (now-stale) status.
  useEffect(() => {
    const unlisten = listen<McpIntegrationStatus>(MCP_STATUS_EVENT, (event) => {
      setMcpStatus(event.payload);
      setMcpUnavailable(false);
    });
    return () => {
      // Best-effort: if the unlisten resolves after test/teardown mocks (or,
      // in principle, a real Tauri event bridge) have already gone away,
      // swallow it rather than surfacing an unhandled rejection — a stray
      // listener reference is harmless once the component is unmounted.
      unlisten.then((fn) => fn()).catch(() => {});
    };
  }, []);

  const handleMcpProvision = async (force = false) => {
    setMcpBusy(true);
    try {
      const s = await provisionMcpIntegration(force);
      setMcpStatus(s);
      notify(
        s.matches
          ? "Managed integration is provisioned and registered in Cursor."
          : (s.last_error ??
              "Managed integration needs attention — see status below."),
        s.matches ? "success" : "error",
      );
    } catch (err) {
      notify(String(err), "error");
    } finally {
      setMcpBusy(false);
    }
  };

  const performMcpRemove = async (prepareToUninstall: boolean) => {
    setMcpBusy(true);
    try {
      const s = await removeMcpIntegration(prepareToUninstall);
      setMcpStatus(s);
      notify("Managed integration removed.", "success");
    } catch (err) {
      notify(String(err), "error");
    } finally {
      setMcpBusy(false);
    }
  };

  const handleMcpRemoveClick = () => {
    if (!mcpRemovePending) {
      clearMcpRemoveTimer();
      setMcpRemovePending(true);
      mcpRemoveTimerRef.current = setTimeout(
        () => setMcpRemovePending(false),
        3000,
      );
      return;
    }
    clearMcpRemoveTimer();
    setMcpRemovePending(false);
    void performMcpRemove(false);
  };

  const handleMcpPrepareUninstallClick = () => {
    if (!mcpPrepareUninstallPending) {
      clearMcpPrepareUninstallTimer();
      setMcpPrepareUninstallPending(true);
      mcpPrepareUninstallTimerRef.current = setTimeout(
        () => setMcpPrepareUninstallPending(false),
        3000,
      );
      return;
    }
    clearMcpPrepareUninstallTimer();
    setMcpPrepareUninstallPending(false);
    void performMcpRemove(true);
  };

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
      clearRevokeTimer();
      setRevokePendingId(id);
      revokeTimerRef.current = setTimeout(() => setRevokePendingId(null), 3000);
      return;
    }
    clearRevokeTimer();
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
          <Field className="intg-field" label="Key name (optional)">
            <Input
              type="text"
              value={keyName}
              onChange={(e) => setKeyName(e.target.value)}
              placeholder="ci-runner"
            />
          </Field>
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
          <Button type="submit" variant="primary" disabled={busy}>
            {busy ? "Working..." : "Create key"}
          </Button>
        </form>

        {newKey && (
          <div className="intg-new-key" role="status">
            <span className="intg-new-key-label">
              New key ({newKey.scopes}) — copy it now:
            </span>
            <div className="intg-token-row">
              <code className="intg-token">{newKey.token}</code>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={() => copy("token", newKey.token)}
              >
                {copied === "token" ? "Copied" : "Copy"}
              </Button>
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
                <tr
                  key={key.id}
                  className={key.revoked ? "intg-key-row--revoked" : undefined}
                >
                  <td>{key.name ?? key.id}</td>
                  <td>{key.scopes}</td>
                  <td>{key.created_at ?? "—"}</td>
                  <td>{key.last_used_at ?? "never"}</td>
                  <td>
                    {key.revoked ? (
                      <span className="intg-revoked-badge">Revoked</span>
                    ) : (
                      <Button
                        variant="danger"
                        size="sm"
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
                      </Button>
                    )}
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

      <section className="intg-section" data-testid="mcp-managed-card">
        <h2 className="intg-section-title">Managed Cursor/MCP integration</h2>
        <p className="intg-copy">
          Let {PRODUCT_NAME} install, register, and keep the Cursor MCP server
          up to date for you — a pinned version, an app-owned API key, and a
          non-destructive <code>~/.cursor/mcp.json</code> entry, with no manual
          snippet or floating <code>npx</code>.
        </p>

        {mcpUnavailable ? (
          <p className="intg-hint">
            The managed integration needs a backend command that is not
            available yet. Use the manual setup below instead.
          </p>
        ) : !mcpStatus ? (
          <p className="intg-hint">Loading status…</p>
        ) : (
          <>
            <div className="mcp-status-row">
              <span
                className={`mcp-badge mcp-badge--${MCP_STATUS_VARIANT[mcpStatus.install_status]}`}
              >
                {MCP_STATUS_LABEL[mcpStatus.install_status]}
              </span>
              {mcpStatus.enabled && (
                <span
                  className={`mcp-badge mcp-badge--${mcpStatus.matches ? "good" : "warn"}`}
                >
                  {mcpStatus.matches ? "Healthy" : "Needs attention"}
                </span>
              )}
            </div>

            <dl className="mcp-detail-grid">
              <dt>Provisioned version</dt>
              <dd>{mcpStatus.provisioned_version ?? "none"}</dd>
              <dt>Pinned version</dt>
              <dd>{mcpStatus.pinned_version}</dd>
              <dt>Node.js</dt>
              <dd>
                {mcpStatus.node_available
                  ? (mcpStatus.node_path ?? "available")
                  : "Not found — install Node ≥18 to enable this"}
              </dd>
              <dt>npm</dt>
              <dd>
                {mcpStatus.npm_available
                  ? (mcpStatus.npm_path ?? "available")
                  : "Not found"}
              </dd>
              <dt>Cursor registration</dt>
              <dd>
                {mcpStatus.cursor_config_conflict
                  ? "Conflict — an unmanaged chaos-scheduler entry already exists"
                  : mcpStatus.registered_in_cursor
                    ? "Registered"
                    : "Not registered"}
              </dd>
              <dt>API reachable</dt>
              <dd>{mcpStatus.api_reachable ? "Yes" : "No"}</dd>
            </dl>

            {mcpStatus.last_error && (
              <Notice variant="error">{mcpStatus.last_error}</Notice>
            )}

            <div className="intg-mcp-actions">
              {!mcpStatus.enabled ? (
                <Button
                  type="button"
                  variant="primary"
                  disabled={mcpBusy}
                  onClick={() => handleMcpProvision(false)}
                >
                  {mcpBusy ? "Working…" : "Enable managed integration"}
                </Button>
              ) : (
                <>
                  <Button
                    type="button"
                    variant="primary"
                    disabled={mcpBusy}
                    onClick={() => handleMcpProvision(false)}
                  >
                    {mcpBusy ? "Working…" : "Re-provision"}
                  </Button>
                  {mcpStatus.cursor_config_conflict && (
                    <Button
                      type="button"
                      variant="ghost"
                      disabled={mcpBusy}
                      onClick={() => handleMcpProvision(true)}
                    >
                      Take over conflicting entry
                    </Button>
                  )}
                  <Button
                    type="button"
                    variant="danger"
                    disabled={mcpBusy}
                    onClick={handleMcpRemoveClick}
                    aria-label={
                      mcpRemovePending
                        ? "Confirm remove managed integration"
                        : "Remove managed integration"
                    }
                  >
                    {mcpRemovePending ? "Confirm remove?" : "Remove"}
                  </Button>
                  <Button
                    type="button"
                    variant="ghost"
                    disabled={mcpBusy}
                    onClick={handleMcpPrepareUninstallClick}
                    aria-label={
                      mcpPrepareUninstallPending
                        ? "Confirm prepare to uninstall"
                        : "Prepare to uninstall"
                    }
                  >
                    {mcpPrepareUninstallPending
                      ? "Confirm?"
                      : "Prepare to uninstall"}
                  </Button>
                </>
              )}
            </div>
          </>
        )}
      </section>

      <section className="intg-section">
        <h2 className="intg-section-title">Advanced: manual MCP setup</h2>
        <p className="intg-copy">
          Prefer to manage the connection yourself, or connecting from another
          machine? Create an API key above, then add the MCP server to Cursor
          manually.
        </p>
        <div className="intg-mcp-actions">
          <Button
            type="button"
            variant="primary"
            onClick={() =>
              openExternalSafe(addToCursorLink(tokenForSnippet)).catch(() =>
                notify("Could not open Cursor.", "error"),
              )
            }
          >
            Add to Cursor
          </Button>
          <Button
            type="button"
            variant="ghost"
            onClick={() => copy("mcp", mcpConfigSnippet(tokenForSnippet))}
          >
            {copied === "mcp" ? "Copied" : "Copy .cursor/mcp.json"}
          </Button>
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
