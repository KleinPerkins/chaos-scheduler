import { afterEach, describe, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import WorkflowEditor from "./WorkflowEditor";
import { sampleWorkflow } from "../test/fixtures/data";
import {
  createDefaultIpcRegistry,
  resolveIpcInvoke,
} from "../test/fixtures/ipc-registry";
import type { Workflow } from "../lib/commands";

function installStrictIpcMocks(): void {
  const registry = createDefaultIpcRegistry();
  mockIPC(
    (cmd, args) =>
      resolveIpcInvoke(cmd, (args ?? {}) as Record<string, unknown>, registry),
    { shouldMockEvents: true },
  );
}

const typedWorkflow: Workflow = {
  ...sampleWorkflow,
  kind: "typed",
  spec_json: JSON.stringify({
    kind: "typed",
    environment: "production",
    generic: null,
    typed: { operator_type: "git_pull", config: {} },
    on_success: [],
    on_failure: [],
  }),
};

describe("WorkflowEditor environment persistence", () => {
  afterEach(() => {
    cleanup();
    clearMocks();
    delete window.__CHAOS_IPC_OVERRIDES__;
  });

  it("persists the selected environment when creating a workflow", async () => {
    installStrictIpcMocks();
    let createArgs: Record<string, unknown> | undefined;
    window.__CHAOS_IPC_OVERRIDES__ = {
      create_workflow: (args) => {
        createArgs = args;
        return { ...typedWorkflow, environment: String(args.environment) };
      },
    };
    const onSaved = vi.fn();

    render(<WorkflowEditor onSaved={onSaved} onCancel={() => {}} />);

    fireEvent.change(await screen.findByLabelText("Name"), {
      target: { value: "Sandbox operator" },
    });
    await screen.findByRole("option", { name: "Sandbox" });
    fireEvent.change(screen.getByLabelText("Environment"), {
      target: { value: "sandbox" },
    });
    fireEvent.click(screen.getByRole("radio", { name: /^Typed/ }));
    fireEvent.click(screen.getByRole("button", { name: "Create Workflow" }));

    await waitFor(() => expect(onSaved).toHaveBeenCalled());
    expect(createArgs?.environment).toBe("sandbox");
  });

  it("persists the selected environment when editing a workflow", async () => {
    installStrictIpcMocks();
    let updateArgs: Record<string, unknown> | undefined;
    window.__CHAOS_IPC_OVERRIDES__ = {
      update_workflow: (args) => {
        updateArgs = args;
        return { ...typedWorkflow, environment: String(args.environment) };
      },
    };
    const onSaved = vi.fn();

    render(
      <WorkflowEditor
        workflow={typedWorkflow}
        onSaved={onSaved}
        onCancel={() => {}}
      />,
    );

    await screen.findByRole("option", { name: "Sandbox" });
    fireEvent.change(screen.getByLabelText("Environment"), {
      target: { value: "sandbox" },
    });
    expect(screen.getByLabelText("Environment")).toHaveValue("sandbox");
    fireEvent.click(screen.getByRole("button", { name: "Save Changes" }));

    await waitFor(() => expect(onSaved).toHaveBeenCalled());
    expect(updateArgs?.environment).toBe("sandbox");
  });
});
