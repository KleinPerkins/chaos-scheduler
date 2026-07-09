import type { StepSpec } from "../../lib/commands";
import Button from "../Button";
import { emptyStep } from "./specHelpers";

interface Props {
  steps: StepSpec[];
  onChange: (steps: StepSpec[]) => void;
  disabled?: boolean;
}

type Mode = "command" | "script";

function modeOf(step: StepSpec): Mode {
  return step.script != null && step.command == null ? "script" : "command";
}

/**
 * Guided builder for a generic workflow's step DAG. Each step runs a command or
 * a script and may depend on earlier steps (`depends_on`), mirroring
 * `workflow_spec::StepSpec`. Cycles and command/script exclusivity are validated
 * server-side on save; this UI surfaces the fields and dependency options.
 */
export default function StepFlowBuilder({ steps, onChange, disabled }: Props) {
  const update = (index: number, patch: Partial<StepSpec>) => {
    onChange(steps.map((s, i) => (i === index ? { ...s, ...patch } : s)));
  };

  const remove = (index: number) => {
    const removedId = steps[index]?.id;
    onChange(
      steps
        .filter((_, i) => i !== index)
        .map((s) => ({
          ...s,
          depends_on: s.depends_on.filter((d) => d !== removedId),
        })),
    );
  };

  const add = () => onChange([...steps, emptyStep(steps.length)]);

  const setMode = (index: number, mode: Mode) => {
    if (mode === "command") {
      update(index, { command: steps[index].command ?? "", script: null });
    } else {
      update(index, { script: steps[index].script ?? "", command: null });
    }
  };

  return (
    <div className="step-builder">
      {steps.map((step, index) => {
        const mode = modeOf(step);
        const dependencyChoices = steps.filter((_, i) => i !== index);
        return (
          <fieldset className="step-card" key={index} disabled={disabled}>
            <legend className="step-card-legend">Step {index + 1}</legend>
            <div className="step-row">
              <label className="step-field step-field-id">
                <span>Step ID</span>
                <input
                  type="text"
                  value={step.id}
                  onChange={(e) => update(index, { id: e.target.value })}
                  placeholder="build"
                />
              </label>
              <label className="step-field step-field-mode">
                <span>Runs</span>
                <select
                  value={mode}
                  onChange={(e) => setMode(index, e.target.value as Mode)}
                >
                  <option value="command">Shell command</option>
                  <option value="script">Script path</option>
                </select>
              </label>
              {steps.length > 1 && (
                <Button
                  type="button"
                  variant="danger"
                  size="sm"
                  className="step-remove"
                  onClick={() => remove(index)}
                  aria-label={`Remove step ${index + 1}`}
                >
                  Remove
                </Button>
              )}
            </div>

            <label className="step-field">
              <span>{mode === "command" ? "Command" : "Script path"}</span>
              <input
                type="text"
                value={(mode === "command" ? step.command : step.script) ?? ""}
                onChange={(e) =>
                  update(
                    index,
                    mode === "command"
                      ? { command: e.target.value }
                      : { script: e.target.value },
                  )
                }
                placeholder={
                  mode === "command"
                    ? "npm run build"
                    : "scripts/workflows/build.sh"
                }
              />
            </label>

            <div className="step-row">
              <label className="step-field">
                <span>Args (space-separated)</span>
                <input
                  type="text"
                  value={step.args.join(" ")}
                  onChange={(e) =>
                    update(index, {
                      args: e.target.value.split(/\s+/).filter(Boolean),
                    })
                  }
                  placeholder="--flag value"
                />
              </label>
              <label className="step-field">
                <span>Working dir (optional)</span>
                <input
                  type="text"
                  value={step.working_dir ?? ""}
                  onChange={(e) =>
                    update(index, { working_dir: e.target.value || null })
                  }
                  placeholder="relative to workspace root"
                />
              </label>
            </div>

            <div className="step-row">
              <label className="step-field">
                <span>Retries</span>
                <input
                  type="number"
                  min={0}
                  value={step.retry?.max_retries ?? 0}
                  onChange={(e) => {
                    const max = Math.max(0, parseInt(e.target.value, 10) || 0);
                    update(index, {
                      retry:
                        max === 0
                          ? null
                          : {
                              max_retries: max,
                              backoff_seconds: step.retry?.backoff_seconds ?? 0,
                            },
                    });
                  }}
                />
              </label>
              <label className="step-field">
                <span>Backoff (s)</span>
                <input
                  type="number"
                  min={0}
                  value={step.retry?.backoff_seconds ?? 0}
                  disabled={!step.retry || step.retry.max_retries === 0}
                  onChange={(e) =>
                    update(index, {
                      retry: {
                        max_retries: step.retry?.max_retries ?? 1,
                        backoff_seconds: Math.max(
                          0,
                          parseInt(e.target.value, 10) || 0,
                        ),
                      },
                    })
                  }
                />
              </label>
              <label className="step-field">
                <span>Timeout (s)</span>
                <input
                  type="number"
                  min={0}
                  value={step.timeout_seconds ?? ""}
                  onChange={(e) =>
                    update(index, {
                      timeout_seconds: e.target.value
                        ? parseInt(e.target.value, 10)
                        : null,
                    })
                  }
                  placeholder="none"
                />
              </label>
            </div>

            {dependencyChoices.length > 0 && (
              <div className="step-field">
                <span className="step-field-label">Depends on</span>
                <div className="step-deps">
                  {dependencyChoices.map((dep) => (
                    <label key={dep.id} className="step-dep-chip">
                      <input
                        type="checkbox"
                        checked={step.depends_on.includes(dep.id)}
                        onChange={(e) =>
                          update(index, {
                            depends_on: e.target.checked
                              ? [...step.depends_on, dep.id]
                              : step.depends_on.filter((d) => d !== dep.id),
                          })
                        }
                      />
                      {dep.id}
                    </label>
                  ))}
                </div>
              </div>
            )}

            <label className="step-check">
              <input
                type="checkbox"
                checked={step.continue_on_error}
                onChange={(e) =>
                  update(index, { continue_on_error: e.target.checked })
                }
              />
              Continue the run even if this step fails
            </label>
          </fieldset>
        );
      })}
      {!disabled && (
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="step-add"
          onClick={add}
        >
          + Add step
        </Button>
      )}
    </div>
  );
}
