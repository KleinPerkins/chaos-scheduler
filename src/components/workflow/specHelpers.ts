import type {
  StepSpec,
  TypedSpec,
  ActionSpec,
  ActionKind,
} from "../../lib/commands";

/** Built-in operators exposed in the editor (mirrors the Rust operator
 * registry). Advanced keys can be added via the raw JSON escape hatch. */
export const OPERATORS: { value: string; label: string; hint: string }[] = [
  {
    value: "git_pull",
    label: "Git pull",
    hint: "Fetch/pull a git repository via system git",
  },
  {
    value: "cursor_agent",
    label: "Cursor agent",
    hint: "Launch a Cursor Cloud Agent or the cursor-agent CLI",
  },
];

export function emptyStep(index: number): StepSpec {
  return {
    id: `step_${index + 1}`,
    command: "",
    script: null,
    args: [],
    working_dir: null,
    depends_on: [],
    retry: null,
    timeout_seconds: null,
    continue_on_error: false,
  };
}

export function defaultOperatorConfig(
  operatorType: string,
): Record<string, unknown> {
  switch (operatorType) {
    case "git_pull":
      return { repo_url: "", local_path: "", branch: "main", rebase: false };
    case "cursor_agent":
      return { mode: "cloud", prompt: "", repo: "", model: "" };
    default:
      return {};
  }
}

export function defaultTypedSpec(): TypedSpec {
  return {
    operator_type: "git_pull",
    config: defaultOperatorConfig("git_pull"),
  };
}

export function defaultAction(kind: ActionKind): ActionSpec {
  switch (kind) {
    case "email":
      return { type: "email", to: null };
    case "webhook":
      return { type: "webhook", url: "", secret: null, max_retries: 0 };
    case "run_workflow":
      return { type: "run_workflow", workflow_id: "", wait: false };
    case "desktop_notification":
      return { type: "desktop_notification", title: null };
  }
}
