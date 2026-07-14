import { describe, expect, it } from "vitest";
import {
  IntegrationSchema,
  QueueConfigSchema,
  TriggerConfigSchema,
  WorkflowSpecSchema,
} from "../src/authoring-schemas.js";

describe("MCP authoring schemas", () => {
  it("validates generic workflow structure while preserving additive fields", () => {
    const parsed = WorkflowSpecSchema.parse({
      kind: "generic",
      environment: "sandbox",
      generic: {
        steps: [
          {
            id: "run",
            command: "echo ok",
            future_step_field: true,
          },
        ],
        future_generic_field: "kept",
      },
      future_spec_field: { version: 2 },
    });

    expect(parsed).toMatchObject({
      generic: {
        steps: [{ future_step_field: true }],
        future_generic_field: "kept",
      },
      future_spec_field: { version: 2 },
    });
    expect(
      WorkflowSpecSchema.parse({
        kind: "generic",
        generic: {
          steps: [{ id: " run ", command: "  printf 'ok'  " }],
        },
      }).generic.steps[0],
    ).toMatchObject({
      id: " run ",
      command: "  printf 'ok'  ",
    });
    expect(
      WorkflowSpecSchema.safeParse({
        kind: "generic",
        generic: { steps: [] },
      }).success,
    ).toBe(false);
    expect(
      WorkflowSpecSchema.safeParse({
        kind: "generic",
        generic: {
          steps: [
            {
              id: "invalid",
              command: "echo command",
              script: "script.sh",
            },
          ],
        },
      }).success,
    ).toBe(false);
  });

  it("describes known typed operators without rejecting future fields", () => {
    const gitPull = WorkflowSpecSchema.parse({
      kind: "typed",
      typed: {
        operator_type: "git_pull",
        config: {
          path: "repos/example",
          repo_url: "https://example.com/repo.git",
          depth: 1,
          future_git_option: true,
        },
      },
    });
    expect(gitPull.typed?.config).toMatchObject({ future_git_option: true });

    expect(
      WorkflowSpecSchema.safeParse({
        kind: "typed",
        typed: {
          operator_type: "cursor_agent",
          config: { mode: "cloud", prompt: "Fix it" },
        },
      }).success,
    ).toBe(false);
  });

  it("covers trigger and queue stored-config shapes", () => {
    expect(
      TriggerConfigSchema.parse([
        { kind: "cron", cron: "0 6 * * *" },
        {
          kind: "asset_update",
          asset: { kind: "report", namespace: "cards" },
          future_trigger_field: true,
        },
        {
          kind: "on_completion",
          upstream_workflow_id: "upstream",
          status_filter: ["success"],
        },
      ]),
    ).toHaveLength(3);
    expect(
      TriggerConfigSchema.safeParse([
        { kind: "file_arrival", mode: "mtime_changed" },
      ]).success,
    ).toBe(false);
    expect(
      TriggerConfigSchema.safeParse([
        {
          kind: "on_completion",
          upstream_workflow_id: "upstream",
          status_filter: [],
        },
      ]).success,
    ).toBe(false);
    expect(
      TriggerConfigSchema.safeParse([
        {
          kind: "on_completion",
          upstream_workflow_id: "upstream",
          status_filter: ["succeeded"],
        },
      ]).success,
    ).toBe(false);

    const queue = QueueConfigSchema.parse({
      queue: "sandbox-default",
      depends_on: ["upstream"],
      priority: 10,
      future_queue_field: true,
    });
    expect(queue.future_queue_field).toBe(true);
  });

  it("covers actions, email profiles, and known operator integrations", () => {
    expect(
      IntegrationSchema.parse({
        completion_actions: [
          {
            type: "webhook",
            url: "https://example.com/hook",
            secret: "secret",
            future_action_field: true,
          },
        ],
        email_profile: {
          name: "Primary",
          enabled: true,
          alert_email: "alerts@example.com",
          smtp_host: "smtp.example.com",
          smtp_port: 587,
          smtp_user: "mailer",
          smtp_password: "secret",
          from_address: "scheduler@example.com",
          from_name: "Scheduler",
        },
      }),
    ).toMatchObject({
      completion_actions: [{ future_action_field: true }],
    });
  });
});
