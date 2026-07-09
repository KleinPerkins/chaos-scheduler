import type { ActionSpec, ActionKind, Workflow } from "../../lib/commands";
import Button from "../Button";
import Input from "../Input";
import { defaultAction } from "./specHelpers";

interface Props {
  title: string;
  hint: string;
  actions: ActionSpec[];
  onChange: (actions: ActionSpec[]) => void;
  workflows: Workflow[];
  /** When true, an email action is treated as required and cannot be removed. */
  emailRequired?: boolean;
  disabled?: boolean;
}

const ACTION_TYPES: { value: ActionKind; label: string }[] = [
  { value: "email", label: "Email" },
  { value: "webhook", label: "Webhook (POST result)" },
  { value: "run_workflow", label: "Run another workflow" },
  { value: "desktop_notification", label: "Desktop notification" },
];

/** Editor for a list of on-success / on-failure actions. `email` is always an
 * available action type; when `emailRequired` is set the last email action is
 * protected from removal (the "email required capability" from the plan). */
export default function ActionsEditor({
  title,
  hint,
  actions,
  onChange,
  workflows,
  emailRequired,
  disabled,
}: Props) {
  const emailCount = actions.filter((a) => a.type === "email").length;

  const update = (index: number, next: ActionSpec) =>
    onChange(actions.map((a, i) => (i === index ? next : a)));

  const remove = (index: number) =>
    onChange(actions.filter((_, i) => i !== index));

  const changeType = (index: number, kind: ActionKind) =>
    update(index, defaultAction(kind));

  return (
    <div className="actions-editor">
      <div className="actions-header">
        <span className="editor-label">{title}</span>
        {!disabled && (
          <div className="actions-add">
            {ACTION_TYPES.map((t) => (
              <Button
                key={t.value}
                type="button"
                variant="ghost"
                size="sm"
                onClick={() => onChange([...actions, defaultAction(t.value)])}
              >
                + {t.label}
              </Button>
            ))}
          </div>
        )}
      </div>
      <span className="editor-hint">{hint}</span>

      {actions.length === 0 ? (
        <div className="editor-hint actions-empty">No actions configured.</div>
      ) : (
        <div className="action-list">
          {actions.map((action, index) => {
            const protectedEmail =
              emailRequired && action.type === "email" && emailCount <= 1;
            return (
              <div className="action-row" key={index}>
                <select
                  value={action.type}
                  disabled={disabled}
                  aria-label={`Action ${index + 1} type`}
                  onChange={(e) =>
                    changeType(index, e.target.value as ActionKind)
                  }
                >
                  {ACTION_TYPES.map((t) => (
                    <option key={t.value} value={t.value}>
                      {t.label}
                    </option>
                  ))}
                </select>

                {action.type === "email" && (
                  <Input
                    type="email"
                    value={action.to ?? ""}
                    disabled={disabled}
                    placeholder="defaults to configured alert email"
                    aria-label="Email recipient override"
                    onChange={(e) =>
                      update(index, { ...action, to: e.target.value || null })
                    }
                  />
                )}
                {action.type === "webhook" && (
                  <>
                    <Input
                      type="url"
                      value={action.url}
                      disabled={disabled}
                      placeholder="https://example.com/hook"
                      aria-label="Webhook URL"
                      onChange={(e) =>
                        update(index, { ...action, url: e.target.value })
                      }
                    />
                    <Input
                      type="text"
                      value={action.secret ?? ""}
                      disabled={disabled}
                      placeholder="HMAC secret (optional)"
                      aria-label="Webhook signing secret"
                      onChange={(e) =>
                        update(index, {
                          ...action,
                          secret: e.target.value || null,
                        })
                      }
                    />
                    <Input
                      type="number"
                      min={0}
                      value={action.max_retries ?? 0}
                      disabled={disabled}
                      aria-label="Webhook max retries"
                      onChange={(e) =>
                        update(index, {
                          ...action,
                          max_retries: Math.max(
                            0,
                            parseInt(e.target.value, 10) || 0,
                          ),
                        })
                      }
                    />
                  </>
                )}
                {action.type === "run_workflow" && (
                  <>
                    <select
                      value={action.workflow_id}
                      disabled={disabled}
                      aria-label="Workflow to run"
                      onChange={(e) =>
                        update(index, {
                          ...action,
                          workflow_id: e.target.value,
                        })
                      }
                    >
                      <option value="">Select workflow…</option>
                      {workflows.map((w) => (
                        <option key={w.id} value={w.id}>
                          {w.name}
                        </option>
                      ))}
                    </select>
                    <label className="action-inline-check">
                      <input
                        type="checkbox"
                        checked={Boolean(action.wait)}
                        disabled={disabled}
                        onChange={(e) =>
                          update(index, { ...action, wait: e.target.checked })
                        }
                      />
                      wait
                    </label>
                  </>
                )}
                {action.type === "desktop_notification" && (
                  <Input
                    type="text"
                    value={action.title ?? ""}
                    disabled={disabled}
                    placeholder="notification title (optional)"
                    aria-label="Notification title"
                    onChange={(e) =>
                      update(index, {
                        ...action,
                        title: e.target.value || null,
                      })
                    }
                  />
                )}

                {!disabled && (
                  <Button
                    type="button"
                    variant="danger"
                    size="sm"
                    disabled={protectedEmail}
                    title={
                      protectedEmail
                        ? "Email is required and cannot be removed"
                        : "Remove action"
                    }
                    aria-label={`Remove action ${index + 1}`}
                    onClick={() => remove(index)}
                  >
                    Remove
                  </Button>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
