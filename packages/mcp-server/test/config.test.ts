import { describe, expect, it } from "vitest";
import { applyCliOverrides, configFromEnv } from "../src/config.js";
import {
  assertEnvironmentWritable,
  GuardrailError,
  ToolBudget,
} from "../src/guardrails.js";

describe("config", () => {
  it("uses defaults for an empty environment", () => {
    const cfg = configFromEnv({});
    expect(cfg.baseUrl).toBe("http://127.0.0.1:9618");
    expect(cfg.transport).toBe("stdio");
    expect(cfg.protectedEnvironments).toEqual(["prod", "production"]);
    expect(cfg.allowProtectedWrites).toBe(false);
    expect(cfg.allowRemoteHttp).toBe(false);
    expect(cfg.httpMaxBodyBytes).toBe(1024 * 1024);
    expect(cfg.maxToolCalls).toBe(0);
  });

  it("parses env overrides", () => {
    const cfg = configFromEnv({
      CHAOS_SCHEDULER_URL: "http://host:1234",
      CHAOS_SCHEDULER_API_KEY: "id.secret",
      CHAOS_SCHEDULER_MCP_TRANSPORT: "http",
      CHAOS_SCHEDULER_MCP_HTTP_PORT: "9800",
      CHAOS_SCHEDULER_MCP_ALLOW_REMOTE_HTTP: "true",
      CHAOS_SCHEDULER_MCP_HTTP_MAX_BODY_BYTES: "4096",
      CHAOS_SCHEDULER_MCP_PROTECTED_ENVIRONMENTS: "prod, staging",
      CHAOS_SCHEDULER_MCP_ALLOW_PROTECTED_WRITES: "true",
      CHAOS_SCHEDULER_MCP_MAX_TOOL_CALLS: "50",
    });
    expect(cfg.baseUrl).toBe("http://host:1234");
    expect(cfg.apiKey).toBe("id.secret");
    expect(cfg.transport).toBe("http");
    expect(cfg.httpPort).toBe(9800);
    expect(cfg.allowRemoteHttp).toBe(true);
    expect(cfg.httpMaxBodyBytes).toBe(4096);
    expect(cfg.protectedEnvironments).toEqual(["prod", "staging"]);
    expect(cfg.allowProtectedWrites).toBe(true);
    expect(cfg.maxToolCalls).toBe(50);
  });

  it("empty protected list disables protection", () => {
    const cfg = configFromEnv({
      CHAOS_SCHEDULER_MCP_PROTECTED_ENVIRONMENTS: "",
    });
    expect(cfg.protectedEnvironments).toEqual([]);
  });

  it("applies CLI overrides on top of env", () => {
    const base = configFromEnv({});
    const cfg = applyCliOverrides(base, [
      "--http",
      "--port",
      "9999",
      "--allow-remote-http",
      "--http-max-body-bytes",
      "4096",
      "--allow-protected-writes",
    ]);
    expect(cfg.transport).toBe("http");
    expect(cfg.httpPort).toBe(9999);
    expect(cfg.allowRemoteHttp).toBe(true);
    expect(cfg.httpMaxBodyBytes).toBe(4096);
    expect(cfg.allowProtectedWrites).toBe(true);
  });
});

describe("guardrails", () => {
  it("ToolBudget is unlimited when max <= 0", () => {
    const b = new ToolBudget(0);
    for (let i = 0; i < 1000; i++) b.consume("x");
    expect(b.remaining).toBe(Number.POSITIVE_INFINITY);
  });

  it("ToolBudget throws once exhausted", () => {
    const b = new ToolBudget(2);
    b.consume("a");
    b.consume("b");
    expect(() => b.consume("c")).toThrow(GuardrailError);
    expect(b.remaining).toBe(0);
  });

  it("blocks writes to protected environments", () => {
    const cfg = configFromEnv({
      CHAOS_SCHEDULER_MCP_PROTECTED_ENVIRONMENTS: "prod",
    });
    expect(() => assertEnvironmentWritable("prod", cfg)).toThrow(
      GuardrailError,
    );
    expect(() => assertEnvironmentWritable("PROD", cfg)).toThrow(
      GuardrailError,
    );
    expect(() => assertEnvironmentWritable("instance", cfg)).not.toThrow();
    expect(() => assertEnvironmentWritable(undefined, cfg)).not.toThrow();
  });

  it("allows protected writes when overridden", () => {
    const cfg = configFromEnv({
      CHAOS_SCHEDULER_MCP_PROTECTED_ENVIRONMENTS: "prod",
      CHAOS_SCHEDULER_MCP_ALLOW_PROTECTED_WRITES: "1",
    });
    expect(() => assertEnvironmentWritable("prod", cfg)).not.toThrow();
  });
});
