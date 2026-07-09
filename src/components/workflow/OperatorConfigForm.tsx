import type { TypedSpec } from "../../lib/commands";
import { OPERATORS, defaultOperatorConfig } from "./specHelpers";
import Input from "../Input";

interface Props {
  spec: TypedSpec;
  onChange: (spec: TypedSpec) => void;
  disabled?: boolean;
}

function str(config: Record<string, unknown>, key: string): string {
  const v = config[key];
  return v == null ? "" : String(v);
}

/** Like `str`, but falls back to a legacy key when the primary key is
 * absent/empty. Used for `cursor_agent`'s `repository` field, which was
 * renamed from `repo` — workflows saved before the rename still have their
 * value stored under the old key, so display it instead of an empty field.
 * Writes still only ever go to the primary key. */
function strWithLegacyFallback(
  config: Record<string, unknown>,
  key: string,
  legacyKey: string,
): string {
  const primary = str(config, key);
  return primary === "" ? str(config, legacyKey) : primary;
}

/** Typed-operator configuration form. Renders known fields per operator and a
 * raw-JSON fallback so operators without a bespoke form (or extra keys) remain
 * editable. */
export default function OperatorConfigForm({
  spec,
  onChange,
  disabled,
}: Props) {
  const config = spec.config ?? {};
  const setConfig = (patch: Record<string, unknown>) =>
    onChange({ ...spec, config: { ...config, ...patch } });

  return (
    <div className="operator-form">
      <label className="editor-field">
        <span className="editor-label">Operator</span>
        <select
          value={spec.operator_type}
          disabled={disabled}
          onChange={(e) =>
            onChange({
              operator_type: e.target.value,
              config: defaultOperatorConfig(e.target.value),
            })
          }
        >
          {OPERATORS.map((op) => (
            <option key={op.value} value={op.value}>
              {op.label}
            </option>
          ))}
          {!OPERATORS.some((op) => op.value === spec.operator_type) && (
            <option value={spec.operator_type}>{spec.operator_type}</option>
          )}
        </select>
        <span className="editor-hint">
          {OPERATORS.find((op) => op.value === spec.operator_type)?.hint ??
            "Custom operator; configure via raw JSON."}
        </span>
      </label>

      {spec.operator_type === "git_pull" && (
        <div className="operator-fields">
          <label className="editor-field">
            <span className="editor-label">Repository URL</span>
            <Input
              type="text"
              value={str(config, "repo_url")}
              disabled={disabled}
              onChange={(e) => setConfig({ repo_url: e.target.value })}
              placeholder="git@github.com:org/repo.git"
            />
          </label>
          <label className="editor-field">
            <span className="editor-label">Local path</span>
            <Input
              type="text"
              value={str(config, "local_path")}
              disabled={disabled}
              onChange={(e) => setConfig({ local_path: e.target.value })}
              placeholder="checkouts/repo"
            />
          </label>
          <div className="editor-field-row">
            <label className="editor-field" style={{ flex: 1 }}>
              <span className="editor-label">Branch</span>
              <Input
                type="text"
                value={str(config, "branch")}
                disabled={disabled}
                onChange={(e) => setConfig({ branch: e.target.value })}
                placeholder="main"
              />
            </label>
            <label className="editor-field">
              <span className="editor-label">Depth (optional)</span>
              <Input
                type="number"
                min={1}
                value={str(config, "depth")}
                disabled={disabled}
                onChange={(e) =>
                  setConfig({
                    depth: e.target.value
                      ? parseInt(e.target.value, 10)
                      : undefined,
                  })
                }
                placeholder="full"
              />
            </label>
          </div>
          <label className="editor-field">
            <span className="editor-label">
              Auth token / SSH key path (optional)
            </span>
            <Input
              type="text"
              value={str(config, "auth")}
              disabled={disabled}
              onChange={(e) => setConfig({ auth: e.target.value || undefined })}
              placeholder="~/.ssh/id_ed25519 or a PAT"
            />
          </label>
          <label className="editor-check">
            <input
              type="checkbox"
              checked={Boolean(config.rebase)}
              disabled={disabled}
              onChange={(e) => setConfig({ rebase: e.target.checked })}
            />
            Rebase instead of merge on pull
          </label>
        </div>
      )}

      {spec.operator_type === "cursor_agent" && (
        <div className="operator-fields">
          <label className="editor-field">
            <span className="editor-label">Mode</span>
            <select
              value={str(config, "mode") || "cloud"}
              disabled={disabled}
              onChange={(e) => setConfig({ mode: e.target.value })}
            >
              <option value="cloud">Cloud (Cursor Cloud Agents API)</option>
              <option value="cli">CLI (cursor-agent)</option>
            </select>
          </label>
          <label className="editor-field">
            <span className="editor-label">Repository</span>
            <Input
              type="text"
              value={strWithLegacyFallback(config, "repository", "repo")}
              disabled={disabled}
              onChange={(e) => setConfig({ repository: e.target.value })}
              placeholder="org/repo"
            />
          </label>
          <label className="editor-field">
            <span className="editor-label">Prompt</span>
            <textarea
              value={str(config, "prompt")}
              rows={3}
              disabled={disabled}
              onChange={(e) => setConfig({ prompt: e.target.value })}
              placeholder="What should the agent do?"
            />
          </label>
          <label className="editor-field">
            <span className="editor-label">Model (optional)</span>
            <Input
              type="text"
              value={str(config, "model")}
              disabled={disabled}
              onChange={(e) =>
                setConfig({ model: e.target.value || undefined })
              }
              placeholder="default"
            />
          </label>
        </div>
      )}
    </div>
  );
}
