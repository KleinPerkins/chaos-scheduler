import { useState, useEffect } from "react";
import { Lock, Sparkles } from "lucide-react";
import {
  createWorkflow,
  updateWorkflow,
  setWorkflowSpec,
  listAvailableScripts,
  listWorkflows,
  listEmailProfiles,
  setWorkflowEmailProfile,
  generateWorkflowDescription,
  environmentOf,
  isCommandUnavailable,
} from "../lib/commands";
import Button from "./Button";
import PageHeader from "./PageHeader";
import Input from "./Input";
import Select from "./Select";
import Textarea from "./Textarea";
import type {
  Workflow,
  AvailableScript,
  WorkflowKind,
  WorkflowSpec,
  StepSpec,
  TypedSpec,
  ActionSpec,
  EmailProfile,
} from "../lib/commands";
import { useEnvironments } from "../hooks/useEnvironments";
import ScheduleBuilder, { cronToHuman } from "./ScheduleBuilder";
import StepFlowBuilder from "./workflow/StepFlowBuilder";
import OperatorConfigForm from "./workflow/OperatorConfigForm";
import ActionsEditor from "./workflow/ActionsEditor";
import {
  emptyStep,
  defaultTypedSpec,
  defaultAction,
} from "./workflow/specHelpers";
import {
  validateRunWorkflowActions,
  validateWorkflowSteps,
} from "../lib/workflowValidation";
import Notice from "./ui/Notice";
import "./WorkflowEditor.css";

interface Props {
  workflow?: Workflow;
  onSaved: () => void;
  onCancel: () => void;
}

const LOCAL_TZ = Intl.DateTimeFormat().resolvedOptions().timeZone;

function parseSpec(workflow?: Workflow): Partial<WorkflowSpec> {
  if (!workflow?.spec_json) return {};
  try {
    return JSON.parse(workflow.spec_json) as WorkflowSpec;
  } catch {
    return {};
  }
}

export default function WorkflowEditor({ workflow, onSaved, onCancel }: Props) {
  const isEdit = !!workflow;
  // Governance is keyed off `managed_externally`, decoupled from the legacy
  // managed_externally: externally-registered definitions are read-only in the app.
  const isManaged = isEdit && (workflow?.managed_externally ?? false);
  const existingSpec = parseSpec(workflow);

  const { environments } = useEnvironments();

  const [name, setName] = useState(workflow?.name ?? "");
  const [description, setDescription] = useState(workflow?.description ?? "");
  const [scriptPath, setScriptPath] = useState(workflow?.script_path ?? "");
  const [environment, setEnvironment] = useState(
    workflow ? environmentOf(workflow) : "production",
  );
  const [cronSchedule, setCronSchedule] = useState(
    workflow?.cron_schedule ?? "0 0 9 * * Mon *",
  );
  const [enabled, setEnabled] = useState(workflow?.enabled ?? true);
  const [asyncMode, setAsyncMode] = useState(workflow?.async_mode ?? false);
  const [emailOnFailure, setEmailOnFailure] = useState(
    workflow?.email_on_failure ?? true,
  );
  const [emailProfileId, setEmailProfileId] = useState(
    workflow?.email_profile_id ?? "",
  );
  const [emailProfiles, setEmailProfiles] = useState<EmailProfile[]>([]);
  const [triggerConfig, setTriggerConfig] = useState(
    workflow?.trigger_config ?? "",
  );
  const [queueConfig, setQueueConfig] = useState(workflow?.queue_config ?? "");

  const [kind, setKind] = useState<WorkflowKind>(workflow?.kind ?? "generic");
  const [steps, setSteps] = useState<StepSpec[]>(
    existingSpec.generic?.steps ?? [emptyStep(0)],
  );
  const [typedSpec, setTypedSpec] = useState<TypedSpec>(
    existingSpec.typed ?? defaultTypedSpec(),
  );
  const [onSuccess, setOnSuccess] = useState<ActionSpec[]>(
    existingSpec.on_success ?? [],
  );
  const [onFailure, setOnFailure] = useState<ActionSpec[]>(
    existingSpec.on_failure ?? (workflow ? [] : [defaultAction("email")]),
  );

  const [isCustomScript, setIsCustomScript] = useState(false);
  const [scripts, setScripts] = useState<AvailableScript[]>([]);
  const [scriptsLoading, setScriptsLoading] = useState(true);
  const [allWorkflows, setAllWorkflows] = useState<Workflow[]>([]);
  const [saving, setSaving] = useState(false);
  const [generatingDesc, setGeneratingDesc] = useState(false);
  const [executionOpen, setExecutionOpen] = useState(!isEdit);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    listAvailableScripts()
      .then((s) => {
        setScripts(s);
        if (
          workflow?.script_path &&
          !s.some((sc) => sc.path === workflow.script_path)
        ) {
          setIsCustomScript(true);
        }
        if (!workflow && s.length > 0 && !scriptPath) {
          setScriptPath(s[0].path);
        }
      })
      .catch(() => setIsCustomScript(true))
      .finally(() => setScriptsLoading(false));
    listWorkflows()
      .then((w) => setAllWorkflows(w))
      .catch(() => setAllWorkflows([]));
    listEmailProfiles()
      .then(setEmailProfiles)
      .catch(() => setEmailProfiles([]));
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const selectedScript = scripts.find((s) => s.path === scriptPath);

  const handleScriptChange = (value: string) => {
    if (value === "__custom__") {
      setIsCustomScript(true);
      setScriptPath("");
    } else {
      setIsCustomScript(false);
      setScriptPath(value);
      // Fill step 1's script path so the discovered script drives execution.
      setSteps((current) =>
        current.length > 0
          ? current.map((s, i) =>
              i === 0 ? { ...s, script: value, command: null } : s,
            )
          : current,
      );
      const matched = scripts.find((s) => s.path === value);
      if (matched && !name) setName(matched.name);
      if (matched?.description && !description)
        setDescription(matched.description);
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

  // The base workflow record still requires a script_path. Derive it from the
  // spec for multi-step / operator workflows so both surfaces stay consistent.
  const derivedScriptPath = (): string => {
    if (kind === "typed") return `operator:${typedSpec.operator_type}`;
    const first = steps[0];
    return (
      first?.script?.trim() || first?.command?.trim() || scriptPath || "generic"
    );
  };

  const buildSpec = (): WorkflowSpec => ({
    kind,
    environment,
    generic: kind === "generic" ? { steps } : null,
    typed: kind === "typed" ? typedSpec : null,
    on_success: onSuccess,
    on_failure: onFailure,
  });

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setNotice(null);
    const stepError = validateWorkflowSteps(kind, steps);
    if (stepError) {
      setError(stepError);
      return;
    }
    const successActionError = validateRunWorkflowActions(
      onSuccess,
      "On-success",
    );
    if (successActionError) {
      setError(successActionError);
      return;
    }
    const failureActionError = validateRunWorkflowActions(
      onFailure,
      "On-failure",
    );
    if (failureActionError) {
      setError(failureActionError);
      return;
    }
    setSaving(true);
    try {
      for (const value of [triggerConfig, queueConfig]) {
        if (value.trim()) JSON.parse(value);
      }
      const effectiveScript = derivedScriptPath();
      let saved: Workflow;
      if (isEdit && workflow) {
        saved = await updateWorkflow({
          id: workflow.id,
          name: isManaged ? workflow.name : name,
          description: isManaged
            ? workflow.description || undefined
            : description || undefined,
          scriptPath: isManaged ? workflow.script_path : effectiveScript,
          cronSchedule: isManaged ? workflow.cron_schedule : cronSchedule,
          enabled,
          asyncMode: isManaged ? workflow.async_mode : asyncMode,
          emailOnFailure,
          timezone: LOCAL_TZ,
          environment: isManaged ? workflow.environment : environment,
          domain: workflow.domain,
          triggerConfig: isManaged
            ? workflow.trigger_config || undefined
            : triggerConfig || undefined,
          queueConfig: isManaged
            ? workflow.queue_config || undefined
            : queueConfig || undefined,
        });
      } else {
        saved = await createWorkflow({
          name,
          description: description || undefined,
          scriptPath: effectiveScript,
          cronSchedule,
          asyncMode,
          emailOnFailure,
          timezone: LOCAL_TZ,
          environment,
          triggerConfig: triggerConfig || undefined,
          queueConfig: queueConfig || undefined,
        });
      }

      // Persist the selected failure-alert email profile (a nullable pointer;
      // null falls back to the global config). Guarded so an older backend
      // without the command still saves the base workflow.
      if (!isManaged) {
        try {
          await setWorkflowEmailProfile(saved.id, emailProfileId || null);
        } catch (profErr) {
          if (!isCommandUnavailable(profErr)) throw profErr;
        }
      }

      // Persist the execution spec (kind + steps/operator + actions +
      // environment). Guarded: if the backend command is not yet registered,
      // the base workflow still saved — surface a non-blocking notice.
      if (!isManaged) {
        try {
          await setWorkflowSpec(saved.id, buildSpec());
        } catch (specErr) {
          if (isCommandUnavailable(specErr)) {
            setNotice(
              "Workflow saved. Step-flow / operator / action details will persist once the backend spec command is available.",
            );
            onSaved();
            return;
          }
          throw specErr;
        }
      }
      onSaved();
    } catch (e) {
      setError(
        e instanceof SyntaxError
          ? "Trigger and queue metadata must be valid JSON."
          : String(e),
      );
    } finally {
      setSaving(false);
    }
  };

  const envOptions = (() => {
    const names = new Set<string>();
    for (const env of environments) names.add(env.name);
    names.add(environment);
    if (names.size === 0) {
      names.add("production");
      names.add("sandbox");
    }
    return Array.from(names).sort((a, b) => a.localeCompare(b));
  })();
  const saveDisabled = saving || (!isManaged && !name) || !cronSchedule;
  const saveLabel = saving
    ? "Saving…"
    : isEdit
      ? "Save changes"
      : "Create workflow";
  const editorSubtitle =
    isEdit && workflow
      ? `${workflow.name} · ${environmentOf(workflow)
          .charAt(0)
          .toUpperCase()}${environmentOf(workflow).slice(1)}`
      : "Configure execution, schedule, and notifications.";

  return (
    <div className="workflow-editor">
      <PageHeader
        title={isEdit ? "Edit workflow" : "New workflow"}
        subtitle={editorSubtitle}
        actions={
          <div className="editor-header-actions">
            <Button variant="ghost" onClick={onCancel}>
              Cancel
            </Button>
            <Button
              type="submit"
              form="workflow-editor-form"
              variant="primary"
              disabled={saveDisabled}
            >
              {saveLabel}
            </Button>
          </div>
        }
      />

      <form
        id="workflow-editor-form"
        className="editor-form"
        onSubmit={handleSubmit}
      >
        {error && (
          <Notice variant="error" assertive>
            {error}
          </Notice>
        )}
        {notice && (
          <div className="editor-hint editor-notice" role="status">
            {notice}
          </div>
        )}
        {isManaged && (
          <div className="editor-hint editor-managed-banner">
            <strong>
              <Lock
                size={13}
                strokeWidth={2.25}
                aria-hidden="true"
                style={{ verticalAlign: "-2px" }}
              />{" "}
              Managed externally.
            </strong>{" "}
            This workflow&rsquo;s definition is owned by an external source of
            truth ({environmentOf(workflow!)} environment) and is read-only
            here. Runtime preferences (enabled state, email alerts, timezone)
            can still be saved.
          </div>
        )}

        <section
          className="editor-section"
          aria-labelledby="editor-general-title"
        >
          <h2 id="editor-general-title" className="editor-section-title">
            General
          </h2>
          <div className="editor-general-grid">
            <div className="editor-field">
              <label className="editor-label" htmlFor="wf-name">
                Name
              </label>
              <Input
                id="wf-name"
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="e.g. Weekly Pod Status"
                required
                disabled={isManaged}
              />
            </div>

            <div className="editor-field editor-field--wide">
              <div className="editor-label-row">
                <label className="editor-label" htmlFor="wf-desc">
                  Description
                </label>
                {kind === "generic" && scriptPath && (
                  <button
                    type="button"
                    className="btn-ai"
                    onClick={handleGenerateDescription}
                    disabled={generatingDesc || isManaged}
                    title="Use AI to generate a description based on the workflow script"
                  >
                    {generatingDesc ? (
                      <span className="btn-ai-loading">Generating...</span>
                    ) : (
                      <>
                        <span className="btn-ai-icon" aria-hidden="true">
                          <Sparkles size={13} strokeWidth={2} />
                        </span>
                        AI Describe
                      </>
                    )}
                  </button>
                )}
              </div>
              <Textarea
                id="wf-desc"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder="What does this workflow do?"
                rows={2}
                disabled={isManaged}
              />
            </div>

            <div className="editor-field">
              <label className="editor-label" htmlFor="wf-env">
                Environment
              </label>
              <Select
                id="wf-env"
                value={environment}
                onChange={(e) => setEnvironment(e.target.value)}
                disabled={isManaged}
              >
                {envOptions.map((env) => (
                  <option key={env} value={env}>
                    {env.charAt(0).toUpperCase() + env.slice(1)}
                  </option>
                ))}
              </Select>
              <span className="editor-hint">
                The partition this workflow runs in. Manage environments from
                the Environments screen.
              </span>
            </div>
          </div>

          <details
            className="editor-execution-details"
            open={executionOpen}
            onToggle={(event) => setExecutionOpen(event.currentTarget.open)}
          >
            <summary>
              Execution details ·{" "}
              {kind === "generic" ? "Generic step flow" : "Typed operator"}
            </summary>
            <div className="editor-execution-body">
              <fieldset
                className="editor-field editor-kind"
                disabled={isManaged}
              >
                <legend className="editor-label">Workflow type</legend>
                <label className="editor-radio">
                  <input
                    type="radio"
                    name="wf-kind"
                    checked={kind === "generic"}
                    onChange={() => setKind("generic")}
                  />
                  Generic — a multi-step flow of commands / scripts
                </label>
                <label className="editor-radio">
                  <input
                    type="radio"
                    name="wf-kind"
                    checked={kind === "typed"}
                    onChange={() => setKind("typed")}
                  />
                  Typed — a single built-in operator (git pull, Cursor agent, …)
                </label>
              </fieldset>

              {kind === "generic" ? (
                <div className="editor-field">
                  <span className="editor-label">Steps</span>
                  <span className="editor-hint">
                    Each step runs a command or script. Use “Depends on” to
                    sequence steps into a DAG; independent steps run in
                    parallel. Cycles are rejected on save.
                  </span>
                  <StepFlowBuilder
                    steps={steps}
                    onChange={setSteps}
                    disabled={isManaged}
                  />
                </div>
              ) : (
                <div className="editor-field">
                  <span className="editor-label">Operator</span>
                  <OperatorConfigForm
                    spec={typedSpec}
                    onChange={setTypedSpec}
                    disabled={isManaged}
                  />
                </div>
              )}
            </div>
          </details>
        </section>

        <section className="editor-section" aria-label="Schedule">
          <div className="editor-field">
            {isManaged ? (
              <>
                <span className="editor-label">Schedule</span>
                <div className="editor-hint">
                  {cronToHuman(
                    workflow?.cron_schedule ?? cronSchedule,
                    workflow?.timezone,
                  )}
                </div>
              </>
            ) : (
              <ScheduleBuilder
                value={cronSchedule}
                onChange={setCronSchedule}
                timezone={workflow?.timezone}
              />
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
        </section>

        <section
          className="editor-section"
          aria-labelledby="editor-runtime-title"
        >
          <h2 id="editor-runtime-title" className="editor-section-title">
            Runtime and notifications
          </h2>
          <div className="editor-field">
            <label className="editor-label">
              <input
                type="checkbox"
                checked={asyncMode}
                onChange={(e) => setAsyncMode(e.target.checked)}
                disabled={isManaged}
                style={{ marginRight: 8 }}
              />
              Async mode
            </label>
            <span className="editor-hint">
              The step spawns a background process; the scheduler monitors the
              PID until completion.
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
              Email on failure (legacy shortcut)
            </label>
            <span className="editor-hint">
              Convenience flag preserved for compatibility. For finer control,
              add an Email action below. Requires email alerts configured in
              Settings.
            </span>
            {emailOnFailure && (
              <div style={{ marginTop: 10 }}>
                <label className="editor-label" htmlFor="wf-email-profile">
                  Delivery profile
                </label>
                <Select
                  id="wf-email-profile"
                  value={emailProfileId}
                  onChange={(e) => setEmailProfileId(e.target.value)}
                  disabled={isManaged}
                >
                  <option value="">Global default (Settings)</option>
                  {emailProfiles.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.name}
                      {p.enabled ? "" : " (disabled)"}
                    </option>
                  ))}
                </Select>
                <span className="editor-hint">
                  Which named email profile receives failure alerts. Manage
                  profiles in Settings → Email Profiles.
                </span>
              </div>
            )}
          </div>

          <div className="editor-field editor-actions-section">
            <ActionsEditor
              title="On success"
              hint="Actions run when a run completes successfully."
              actions={onSuccess}
              onChange={setOnSuccess}
              workflows={allWorkflows}
              disabled={isManaged}
            />
            <ActionsEditor
              title="On failure"
              hint="Actions run when a run fails. Email is the required, always-available capability."
              actions={onFailure}
              onChange={setOnFailure}
              workflows={allWorkflows}
              emailRequired
              disabled={isManaged}
            />
          </div>
        </section>

        <details className="editor-advanced">
          <summary>Advanced trigger &amp; queue metadata (JSON)</summary>
          <div className="editor-field">
            <label className="editor-label" htmlFor="wf-trigger">
              Trigger metadata JSON
            </label>
            <Textarea
              id="wf-trigger"
              value={triggerConfig}
              onChange={(e) => setTriggerConfig(e.target.value)}
              placeholder='{"triggers":[{"kind":"cron","cron":"0 9 * * *"}]}'
              rows={4}
              disabled={isManaged}
            />
          </div>
          <div className="editor-field">
            <label className="editor-label" htmlFor="wf-queue">
              Queue, dependency, and SLA JSON
            </label>
            <Textarea
              id="wf-queue"
              value={queueConfig}
              onChange={(e) => setQueueConfig(e.target.value)}
              placeholder='{"queue":"production-default","priority":0,"depends_on":[],"waits_for":[],"tags":[]}'
              rows={4}
              disabled={isManaged}
            />
          </div>
        </details>

        {kind === "generic" && !isManaged && scripts.length > 0 && (
          <details className="editor-advanced">
            <summary>Pick a step command from discovered scripts</summary>
            <div className="editor-field">
              {scriptsLoading ? (
                <div className="editor-hint">Scanning for scripts...</div>
              ) : (
                <>
                  <Select
                    value={isCustomScript ? "__custom__" : scriptPath}
                    onChange={(e) => handleScriptChange(e.target.value)}
                    aria-label="Discovered scripts"
                  >
                    {scripts.map((s) => (
                      <option key={s.path} value={s.path}>
                        {s.name}
                      </option>
                    ))}
                    <option value="__custom__">Custom path...</option>
                  </Select>
                  {selectedScript?.description && (
                    <span className="editor-script-desc">
                      {selectedScript.description}
                    </span>
                  )}
                  <span className="editor-hint">
                    Selecting a script fills step 1&rsquo;s path (relative to
                    the workspace root).
                  </span>
                </>
              )}
            </div>
          </details>
        )}
      </form>
    </div>
  );
}
