import { describe, expect, it } from "vitest";
import {
  INVALID_STORED_JSON,
  projectStoredJson,
  projectWorkflowForResource,
  REDACTED_SECRET,
} from "../src/resource-projection.js";

describe("workflow resource projection", () => {
  it("reports absent, parsed, and invalid stored JSON explicitly", () => {
    expect(projectStoredJson(null)).toEqual({ status: "absent", value: null });
    expect(projectStoredJson('{"secret":"value","safe":true}')).toEqual({
      status: "parsed",
      value: { secret: REDACTED_SECRET, safe: true },
    });
    expect(projectStoredJson('{"secret":"value"')).toEqual({
      status: "invalid",
      value: null,
    });
  });

  it("rejects stored JSON beyond the size and traversal bounds", () => {
    const oversized = JSON.stringify({ value: "x".repeat(256 * 1024) });
    expect(projectStoredJson(oversized).status).toBe("invalid");

    let nested: Record<string, unknown> = { leaf: true };
    for (let depth = 0; depth < 40; depth += 1) {
      nested = { child: nested };
    }
    expect(projectStoredJson(JSON.stringify(nested)).status).toBe("invalid");

    const tooManyNodes = JSON.stringify(
      Array.from({ length: 10_001 }, () => null),
    );
    expect(projectStoredJson(tooManyNodes).status).toBe("invalid");
  });

  it("treats prototype-shaped JSON keys as inert data", () => {
    const projection = projectStoredJson(
      '{"__proto__":{"secret":"hidden"},"constructor":{"safe":true}}',
    );
    expect(projection.status).toBe("parsed");
    expect(Object.hasOwn(projection.value as object, "__proto__")).toBe(true);
    expect(JSON.stringify(projection.value)).toBe(
      `{"__proto__":{"secret":"${REDACTED_SECRET}"},"constructor":{"safe":true}}`,
    );
    expect(Object.prototype).not.toHaveProperty("secret");
  });

  it("keeps the legacy workflow envelope while suppressing invalid JSON", () => {
    expect(
      projectWorkflowForResource({
        id: "w1",
        name: "Example",
        spec_json: '{"secret":"do-not-echo"',
        unknown_backend_field: "private",
      }),
    ).toEqual({
      id: "w1",
      name: "Example",
      spec_json: INVALID_STORED_JSON,
    });
  });
});
