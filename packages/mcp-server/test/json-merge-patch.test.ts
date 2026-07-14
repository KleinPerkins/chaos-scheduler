import { describe, expect, it } from "vitest";
import { applyWorkflowSpecMergePatch } from "../src/json-merge-patch.js";

describe("workflow spec JSON Merge Patch", () => {
  it("matches sentinel-bearing webhook actions by stable identity, not array index", () => {
    const current = {
      kind: "generic",
      generic: { steps: [{ id: "run", command: "echo ok" }] },
      on_failure: [
        {
          type: "webhook",
          url: "https://example.com/first",
          secret: "first-secret",
        },
        {
          type: "webhook",
          url: "https://example.com/second",
          secret: "second-secret",
        },
      ],
    };

    const merged = applyWorkflowSpecMergePatch(current, {
      on_failure: [
        {
          type: "webhook",
          url: "https://example.com/second",
          secret: "__redacted__",
          max_retries: 2,
        },
      ],
    }) as typeof current;

    expect(merged.on_failure).toEqual([
      {
        type: "webhook",
        url: "https://example.com/second",
        secret: "second-secret",
        max_retries: 2,
      },
    ]);
  });

  it("rejects ambiguous array sentinel preservation instead of misbinding a secret", () => {
    expect(() =>
      applyWorkflowSpecMergePatch(
        {
          on_failure: [
            {
              type: "webhook",
              url: "https://example.com/old",
              secret: "old-secret",
            },
          ],
        },
        {
          on_failure: [
            {
              type: "webhook",
              url: "https://example.com/new",
              secret: "__redacted__",
            },
          ],
        },
      ),
    ).toThrow(/cannot safely preserve/i);
  });

  it("rejects duplicate sentinel identities instead of copying one secret twice", () => {
    const stored = {
      on_failure: [
        {
          type: "webhook",
          url: "https://example.com/hook",
          secret: "stored-secret",
        },
      ],
    };
    const duplicate = {
      type: "webhook",
      url: "https://example.com/hook",
      secret: "__redacted__",
    };

    expect(() =>
      applyWorkflowSpecMergePatch(stored, {
        on_failure: [duplicate, duplicate],
      }),
    ).toThrow(/duplicate array identity/i);
  });

  it("preserves untouched string bytes during an unrelated patch", () => {
    const current = {
      kind: "generic",
      generic: {
        steps: [{ id: " run ", command: "  printf 'ok'  " }],
      },
      note: "  keep spacing  ",
    };

    expect(applyWorkflowSpecMergePatch(current, { future: true })).toEqual({
      ...current,
      future: true,
    });
  });
});
