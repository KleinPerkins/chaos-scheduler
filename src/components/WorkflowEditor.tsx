import { useState, useEffect } from "react";
import { createWorkflow, updateWorkflow, listAvailableScripts, generateWorkflowDescription } from "../lib/commands";
import type { Workflow, AvailableScript } from "../lib/commands";
import ScheduleBuilder from "./ScheduleBuilder";
import "./WorkflowEditor.css";

interface Props {
  workflow?: Workflow;
  onSaved: () => void;
  onCancel: () => void;
}

const LOCAL_TZ = Intl.DateTimeFormat().resolvedOptions().timeZone;

export default function WorkflowEditor({ workflow, onSaved, onCancel }: Props) {
  const isEdit = !!workflow;
  const [name, setName] = useState(workflow?.name ?? "");
  const [description, setDescription] = useState(workflow?.description ?? "");
  const [scriptPath, setScriptPath] = useState(workflow?.script_path ?? "");
  const [cronSchedule, setCronSchedule] = useState(workflow?.cron_schedule ?? "0 0 9 * * Mon *");
  const [enabled, setEnabled] = useState(workflow?.enabled ?? true);
  const [asyncMode, setAsyncMode] = useState(workflow?.async_mode ?? false);
  const [emailOnFailure, setEmailOnFailure] = useState(workflow?.email_on_failure ?? true);
  const [isCustomScript, setIsCustomScript] = useState(false);
  const [scripts, setScripts] = useState<AvailableScript[]>([]);
  const [scriptsLoading, setScriptsLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [generatingDesc, setGeneratingDesc] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    listAvailableScripts()
      .then((s) => {
        setScripts(s);
        if (workflow?.script_path && !s.some((sc) => sc.path === workflow.script_path)) {
          setIsCustomScript(true);
        }
        if (!workflow && s.length > 0 && !scriptPath) {
          setScriptPath(s[0].path);
        }
      })
      .catch(() => setIsCustomScript(true))
      .finally(() => setScriptsLoading(false));
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const selectedScript = scripts.find((s) => s.path === scriptPath);

  const handleScriptChange = (value: string) => {
    if (value === "__custom__") {
      setIsCustomScript(true);
      setScriptPath("");
    } else {
      setIsCustomScript(false);
      setScriptPath(value);
      const matched = scripts.find((s) => s.path === value);
      if (matched && !name) {
        setName(matched.name);
      }
      if (matched?.description && !description) {
        setDescription(matched.description);
      }
    }
  };

  const handleGenerateDescription = async () => {
    if (!scriptPath) return;
    setGeneratingDesc(true);
    setError(null);
    try {
      const desc = await generateWorkflowDescription(scriptPath);
      setDescription(desc);
    } catch (e) {
      setError(`AI description failed: ${String(e)}`);
    } finally {
      setGeneratingDesc(false);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setSaving(true);
    try {
      if (isEdit && workflow) {
        await updateWorkflow({
          id: workflow.id,
          name,
          description: description || undefined,
          scriptPath,
          cronSchedule,
          enabled,
          asyncMode,
          emailOnFailure,
          timezone: LOCAL_TZ,
          corpus: workflow.corpus ?? "source",
          domain: workflow.domain,
        });
      } else {
        await createWorkflow({
          name,
          description: description || undefined,
          scriptPath,
          cronSchedule,
          asyncMode,
          emailOnFailure,
          timezone: LOCAL_TZ,
          corpus: "instance",
        });
      }
      onSaved();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div>
      <div className="page-header">
        <div>
          <h1 className="page-title">
            {isEdit ? "Edit Workflow" : "New Workflow"}
          </h1>
        </div>
        <button className="btn btn-ghost" onClick={onCancel}>
          Cancel
        </button>
      </div>

      <form className="editor-form" onSubmit={handleSubmit}>
        {error && <div className="editor-error">{error}</div>}

        <div className="editor-field">
          <label className="editor-label">Script</label>
          {scriptsLoading ? (
            <div className="editor-hint">Scanning for scripts...</div>
          ) : scripts.length === 0 && !isCustomScript ? (
            <>
              <div className="editor-hint">
                No scripts found in scripts/workflows/. Enter a path manually.
              </div>
              <input
                type="text"
                value={scriptPath}
                onChange={(e) => setScriptPath(e.target.value)}
                placeholder="scripts/workflows/my_script.py"
                required
              />
            </>
          ) : (
            <>
              <select
                value={isCustomScript ? "__custom__" : scriptPath}
                onChange={(e) => handleScriptChange(e.target.value)}
              >
                {scripts.map((s) => (
                  <option key={s.path} value={s.path}>
                    {s.name}
                  </option>
                ))}
                <option value="__custom__">Custom path...</option>
              </select>
              {isCustomScript ? (
                <input
                  type="text"
                  value={scriptPath}
                  onChange={(e) => setScriptPath(e.target.value)}
                  placeholder="scripts/workflows/my_script.py"
                  required
                  style={{ marginTop: 8 }}
                />
              ) : selectedScript?.description ? (
                <span className="editor-script-desc">{selectedScript.description}</span>
              ) : null}
              <span className="editor-hint">
                {isCustomScript
                  ? "Path relative to chaos-labs root"
                  : `${scriptPath}`}
              </span>
            </>
          )}
        </div>

        <div className="editor-field">
          <label className="editor-label">Name</label>
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="e.g. Weekly Pod Status"
            required
          />
        </div>

        <div className="editor-field">
          <div className="editor-label-row">
            <label className="editor-label">Description</label>
            {scriptPath && (
              <button
                type="button"
                className="btn-ai"
                onClick={handleGenerateDescription}
                disabled={generatingDesc}
                title="Use AI to generate a description based on the workflow script"
              >
                {generatingDesc ? (
                  <span className="btn-ai-loading">Generating...</span>
                ) : (
                  <>
                    <span className="btn-ai-icon">&#10022;</span>
                    AI Describe
                  </>
                )}
              </button>
            )}
          </div>
          <textarea
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder="What does this workflow do?"
            rows={2}
          />
        </div>

        <div className="editor-field">
          <ScheduleBuilder value={cronSchedule} onChange={setCronSchedule} timezone={workflow?.timezone} />
        </div>

        {isEdit && (
          <div className="editor-field">
            <label className="editor-label">
              <input
                type="checkbox"
                checked={enabled}
                onChange={(e) => setEnabled(e.target.checked)}
                style={{ marginRight: 8 }}
              />
              Enabled
            </label>
          </div>
        )}

        <div className="editor-field">
          <label className="editor-label">
            <input
              type="checkbox"
              checked={asyncMode}
              onChange={(e) => setAsyncMode(e.target.checked)}
              style={{ marginRight: 8 }}
            />
            Async mode
          </label>
          <span className="editor-hint">
            Script spawns a background process (e.g. context capture launcher). The scheduler monitors the PID until completion.
          </span>
        </div>

        <div className="editor-field">
          <label className="editor-label">
            <input
              type="checkbox"
              checked={emailOnFailure}
              onChange={(e) => setEmailOnFailure(e.target.checked)}
              style={{ marginRight: 8 }}
            />
            Email on failure
          </label>
          <span className="editor-hint">
            Send an email alert when this workflow fails. Requires email alerts to be configured in Settings.
          </span>
        </div>

        <div className="editor-actions">
          <button
            type="submit"
            className="btn btn-primary"
            disabled={saving || !name || !scriptPath || !cronSchedule}
          >
            {saving ? "Saving..." : isEdit ? "Save Changes" : "Create Workflow"}
          </button>
          <button type="button" className="btn btn-ghost" onClick={onCancel}>
            Cancel
          </button>
        </div>
      </form>
    </div>
  );
}
