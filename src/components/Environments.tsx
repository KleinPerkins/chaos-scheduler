import { useState } from "react";
import { useEnvironments } from "../hooks/useEnvironments";
import {
  createEnvironment,
  deleteEnvironment,
  updateEnvironment,
  isCommandUnavailable,
} from "../lib/commands";
import type { EnvironmentPayload } from "../lib/commands";
import EnvironmentBadge from "./EnvironmentBadge";
import Button from "./Button";
import PageHeader from "./PageHeader";
import Field from "./Field";
import Input from "./Input";
import "./Environments.css";

interface DraftForm {
  name: string;
  description: string;
  workingDir: string;
  defaultQueueCapacity: string;
  defaultTagCap: string;
  defaultMaxQueued: string;
}

const EMPTY_FORM: DraftForm = {
  name: "",
  description: "",
  workingDir: "",
  defaultQueueCapacity: "",
  defaultTagCap: "",
  defaultMaxQueued: "",
};

function toPayload(form: DraftForm): EnvironmentPayload {
  const num = (v: string) => (v.trim() ? Number.parseInt(v, 10) : null);
  return {
    name: form.name.trim(),
    description: form.description.trim() || null,
    workingDir: form.workingDir.trim() || null,
    defaultQueueCapacity: num(form.defaultQueueCapacity),
    defaultTagCap: num(form.defaultTagCap),
    defaultMaxQueued: num(form.defaultMaxQueued),
  };
}

export default function Environments() {
  const { environments, loading, error, refresh } = useEnvironments();
  const [form, setForm] = useState<DraftForm>(EMPTY_FORM);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editForm, setEditForm] = useState<DraftForm>(EMPTY_FORM);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [statusType, setStatusType] = useState<"info" | "error" | "success">(
    "info",
  );
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null);

  const notify = (msg: string, type: "info" | "error" | "success" = "info") => {
    setStatus(msg);
    setStatusType(type);
  };

  const set = <K extends keyof DraftForm>(key: K, value: string) =>
    setForm((f) => ({ ...f, [key]: value }));

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!form.name.trim()) {
      notify("Environment name is required.", "error");
      return;
    }
    setBusy(true);
    try {
      await createEnvironment(toPayload(form));
      setForm(EMPTY_FORM);
      notify(`Environment "${form.name.trim()}" created.`, "success");
      await refresh();
    } catch (err) {
      notify(String(err), "error");
    } finally {
      setBusy(false);
    }
  };

  const startEdit = (id: string) => {
    const env = environments.find((e) => e.id === id);
    if (!env) return;
    setEditingId(id);
    setEditForm({
      name: env.name,
      description: env.description ?? "",
      workingDir: env.working_dir ?? "",
      defaultQueueCapacity: env.default_queue_capacity?.toString() ?? "",
      defaultTagCap: env.default_tag_cap?.toString() ?? "",
      defaultMaxQueued: env.default_max_queued?.toString() ?? "",
    });
  };

  const saveEdit = async (id: string) => {
    setBusy(true);
    try {
      await updateEnvironment(id, toPayload(editForm));
      setEditingId(null);
      notify("Environment updated.", "success");
      await refresh();
    } catch (err) {
      if (isCommandUnavailable(err)) {
        notify(
          "Editing environments needs a backend update command that is not available yet.",
          "info",
        );
      } else {
        notify(String(err), "error");
      }
    } finally {
      setBusy(false);
    }
  };

  const handleDelete = async (id: string) => {
    if (pendingDeleteId !== id) {
      setPendingDeleteId(id);
      window.setTimeout(
        () => setPendingDeleteId((cur) => (cur === id ? null : cur)),
        3000,
      );
      return;
    }
    setPendingDeleteId(null);
    setBusy(true);
    try {
      await deleteEnvironment(id);
      notify("Environment deleted.", "success");
      await refresh();
    } catch (err) {
      notify(String(err), "error");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="environments">
      <PageHeader
        title="Environments"
        subtitle="Partitions that scope queues, working directories, and workflow filters."
        actions={
          <Button variant="ghost" onClick={refresh} disabled={busy}>
            Refresh
          </Button>
        }
      />

      {status && (
        <div className={`env-status env-status--${statusType}`} role="status">
          {status}
        </div>
      )}
      {error && (
        <div className="env-status env-status--error" role="alert">
          {error}
        </div>
      )}

      <section className="env-section">
        <h2 className="env-section-title">Create environment</h2>
        <form className="env-create-form" onSubmit={handleCreate}>
          <div className="env-form-row">
            <Field className="env-field" label="Name">
              <Input
                type="text"
                value={form.name}
                onChange={(e) => set("name", e.target.value)}
                placeholder="staging"
                required
              />
            </Field>
            <Field className="env-field env-field-grow" label="Description">
              <Input
                type="text"
                value={form.description}
                onChange={(e) => set("description", e.target.value)}
                placeholder="Pre-production runs"
              />
            </Field>
          </div>
          <Field className="env-field" label="Working directory (optional)">
            <Input
              type="text"
              value={form.workingDir}
              onChange={(e) => set("workingDir", e.target.value)}
              placeholder="overrides the workspace root for this environment"
            />
          </Field>
          <div className="env-form-row">
            <Field className="env-field" label="Default queue capacity">
              <Input
                type="number"
                min={1}
                value={form.defaultQueueCapacity}
                onChange={(e) => set("defaultQueueCapacity", e.target.value)}
                placeholder="inherit"
              />
            </Field>
            <Field className="env-field" label="Default tag cap">
              <Input
                type="number"
                min={1}
                value={form.defaultTagCap}
                onChange={(e) => set("defaultTagCap", e.target.value)}
                placeholder="inherit"
              />
            </Field>
            <Field className="env-field" label="Default max queued">
              <Input
                type="number"
                min={0}
                value={form.defaultMaxQueued}
                onChange={(e) => set("defaultMaxQueued", e.target.value)}
                placeholder="unbounded"
              />
            </Field>
          </div>
          <Button
            type="submit"
            variant="primary"
            disabled={busy || !form.name.trim()}
          >
            {busy ? "Working..." : "Create environment"}
          </Button>
        </form>
      </section>

      <section className="env-section">
        <h2 className="env-section-title">Existing environments</h2>
        {loading ? (
          <div className="env-empty">Loading environments...</div>
        ) : environments.length === 0 ? (
          <div className="env-empty">
            No environments yet. Create one above.
          </div>
        ) : (
          <div className="env-grid">
            {environments.map((env) => (
              <div key={env.id} className="env-card">
                {editingId === env.id ? (
                  <div className="env-card-edit">
                    <Field className="env-field" label="Description">
                      <Input
                        type="text"
                        value={editForm.description}
                        onChange={(e) =>
                          setEditForm((f) => ({
                            ...f,
                            description: e.target.value,
                          }))
                        }
                      />
                    </Field>
                    <Field className="env-field" label="Working directory">
                      <Input
                        type="text"
                        value={editForm.workingDir}
                        onChange={(e) =>
                          setEditForm((f) => ({
                            ...f,
                            workingDir: e.target.value,
                          }))
                        }
                      />
                    </Field>
                    <div className="env-card-actions">
                      <Button
                        variant="primary"
                        size="sm"
                        onClick={() => saveEdit(env.id)}
                        disabled={busy}
                      >
                        Save
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => setEditingId(null)}
                      >
                        Cancel
                      </Button>
                    </div>
                  </div>
                ) : (
                  <>
                    <div className="env-card-header">
                      <EnvironmentBadge
                        environment={env.name}
                        managed={env.managed_externally}
                      />
                      {typeof env.workflow_count === "number" && (
                        <span className="env-count">
                          {env.workflow_count} workflow(s)
                        </span>
                      )}
                    </div>
                    {env.description && (
                      <p className="env-desc">{env.description}</p>
                    )}
                    <dl className="env-meta">
                      {env.working_dir && (
                        <>
                          <dt>Working dir</dt>
                          <dd>{env.working_dir}</dd>
                        </>
                      )}
                      {env.default_queue_capacity != null && (
                        <>
                          <dt>Queue cap</dt>
                          <dd>{env.default_queue_capacity}</dd>
                        </>
                      )}
                    </dl>
                    <div className="env-card-actions">
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => startEdit(env.id)}
                      >
                        Edit
                      </Button>
                      <Button
                        size="sm"
                        className={
                          pendingDeleteId === env.id
                            ? "btn-danger-confirm"
                            : "btn-danger"
                        }
                        onClick={() => handleDelete(env.id)}
                        disabled={busy || env.managed_externally}
                        title={
                          env.managed_externally
                            ? "Managed environments cannot be deleted here"
                            : "Delete environment"
                        }
                      >
                        {pendingDeleteId === env.id ? "Confirm?" : "Delete"}
                      </Button>
                    </div>
                  </>
                )}
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
