import { describe, expect, it } from "vitest";
import { applyCliOverrides, configFromEnv } from "../src/config.js";

describe("CLI config overrides", () => {
  it("switches transport via --http/--stdio", () => {
    const base = configFromEnv({});
    expect(applyCliOverrides(base, ["--http"]).transport).toBe("http");
    expect(applyCliOverrides(base, ["--stdio"]).transport).toBe("stdio");
  });
});
