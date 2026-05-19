import { useState, useEffect } from "react";
import { createWorkflow, updateWorkflow, listAvailableScripts, generateWorkflowDescription } from "../lib/commands";
import type { Workflow, AvailableScript } from "../lib/commands";
import ScheduleBuilder, { cronToHuman } from "./ScheduleBuilder";
import "./WorkflowEditor.css";

interface Props {
  workflow?: Workflow;
  onSaved: () => void;
  onCancel: () => void;
}

const LOCAL_TZ = Intl.DateTimeFormat().resolvedOptions().timeZone;

export default function WorkflowEditor({ workflow, onSaved, onCancel }: Props) {
  const isEdit = !!workflow;
  const isSourceControlled = isEdit && (workflow?.corpus ?? "source") === "source";
  const [name, setName] = useState(workflow?.name ?? "");
  const [description, setDescription] = useState(workflow?.description ?? "");
  const [scriptPath, setScriptPath] = useState(workflow?.script_path ?? "");
  const [cronSchedule, setCronSchedule] = useState(workflow?.cron_schedule ?? "0 0 9 * * Mon *");
  const [enabled, setEnabled] = useState(workflow?.enabled ?? true);
  const [asyncMode, setAsyncMode] = useState(workflow?.async_mode ?? false);
  const [emailOnFailure, setEmailOnFailure] = useState(workflow?.email_on_failure ?? true);
  const [triggerConfig, setTriggerConfig] = useState(workflow?.trigger_config ?? "");
  const [queueConfig, setQueueConfig] = useState(workflow?.queue_config ?? "");
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
      for (const value of [triggerConfig, queueConfig]) {
        if (value.trim()) {
          JSON.parse(value);
        }
      }
      if (isEdit && workflow) {
        await updateWorkflow({
          id: workflow.id,
          name: isSourceControlled ? workflow.name : name,
          description: isSourceControlled ? workflow.description || undefined : description || undefined,
          scriptPath: isSourceControlled ? workflow.script_path : scriptPath,
          cronSchedule: isSourceControlled ? workflow.cron_schedule : cronSchedule,
          enabled,
          asyncMode: isSourceControlled ? workflow.async_mode : asyncMode,
          emailOnFailure,
          timezone: LOCAL_TZ,
          corpus: workflow.corpus ?? "source",
          domain: workflow.domain,
          triggerConfig: isSourceControlled ? workflow.trigger_config || undefined : triggerConfig || undefined,
          queueConfig: isSourceControlled ? workflow.queue_config || undefined : queueConfig || undefined,
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
          triggerConfig: triggerConfig || undefined,
          queueConfig: queueConfig || undefined,
        });
      }
      onSaved();
    } catch (e) {
      setError(e instanceof SyntaxError ? "Trigger and queue metadata must be valid JSON." : String(e));
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
        {isSourceControlled && (
          <div className="editor-hint">
            Source workflow definitions are managed in git. Runtime preferences
            such as enabled state, email alerts, and display timezone can be
            saved here; definition fields are read-only.
          </div>
        )}

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
                disabled={isSourceControlled}
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
                  disabled={isSourceControlled}
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
            disabled={isSourceControlled}
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
                disabled={generatingDesc || isSourceControlled}
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
            disabled={isSourceControlled}
          />
        </div>

        <div className="editor-field">
          {isSourceControlled ? (
            <>
              <label className="editor-label">Schedule</label>
              <div className="editor-hint">
                {cronToHuman(workflow?.cron_schedule ?? cronSchedule, workflow?.timezone)}
              </div>
            </>
          ) : (
            <ScheduleBuilder value={cronSchedule} onChange={setCronSchedule} timezone={workflow?.timezone} />
          )}
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
                disabled={isSourceControlled}
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

        <div className="editor-field">
          <label className="editor-label">Trigger metadata JSON</label>
          <textarea
            value={triggerConfig}
            onChange={(e) => setTriggerConfig(e.target.value)}
            placeholder='{"triggers":[{"kind":"cron","cron":"0 9 * * *"}]}'
            rows={4}
            disabled={isSourceControlled}
          />
          <span className="editor-hint">
            Use SDK-compatible trigger metadata. Source workflow triggers are read-only and come from git.
          </span>
        </div>

        <div className="editor-field">
          <label className="editor-label">Queue, dependency, and SLA JSON</label>
          <textarea
            value={queueConfig}
            onChange={(e) => setQueueConfig(e.target.value)}
            placeholder='{"queue":"instance-default","priority":0,"depends_on":[],"waits_for":[],"tags":[]}'
            rows={4}
            disabled={isSourceControlled}
          />
          <span className="editor-hint">
            Instance workflows can declare queue, priority, dependency, mutex/tag, and SLA metadata here.
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
