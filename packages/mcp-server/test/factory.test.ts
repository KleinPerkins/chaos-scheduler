import { describe, expect, it } from "vitest";
import { ChaosSchedulerClient } from "@chaos-scheduler/sdk";
import { configFromEnv } from "../src/config.js";
import { makeClient } from "../src/factory.js";

describe("makeClient (transport client wiring)", () => {
  it("builds an SDK client from config", () => {
    const config = configFromEnv({
      CHAOS_SCHEDULER_URL: "http://127.0.0.1:9618",
      CHAOS_SCHEDULER_API_KEY: "id.secret",
    });
    expect(makeClient(config)).toBeInstanceOf(ChaosSchedulerClient);
  });

  it("accepts a per-request apiKey override (HTTP transport path)", () => {
    const config = configFromEnv({
      CHAOS_SCHEDULER_URL: "http://127.0.0.1:9618",
      CHAOS_SCHEDULER_API_KEY: "server.fallback",
    });
    expect(makeClient(config, "caller.override")).toBeInstanceOf(
      ChaosSchedulerClient,
    );
  });

  it("throws locally (401) when no api key is available for an authed read", async () => {
    const config = configFromEnv({
      CHAOS_SCHEDULER_URL: "http://127.0.0.1:9618",
    });
    await expect(makeClient(config).listWorkflows()).rejects.toMatchObject({
      status: 401,
    });
  });
});
