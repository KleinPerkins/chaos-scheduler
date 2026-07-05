import { describe, expect, it } from "vitest";
import { configFromEnv } from "../src/config.js";

describe("stdio transport defaults", () => {
  it("selects stdio unless overridden", () => {
    expect(configFromEnv({}).transport).toBe("stdio");
    expect(
      configFromEnv({ CHAOS_SCHEDULER_MCP_TRANSPORT: "http" }).transport,
    ).toBe("http");
  });

  it("shares protected-environment env naming with hooks", () => {
    const cfg = configFromEnv({
      CHAOS_SCHEDULER_MCP_PROTECTED_ENVIRONMENTS: "staging,prod",
    });
    expect(cfg.protectedEnvironments).toEqual(["staging", "prod"]);
  });
});
