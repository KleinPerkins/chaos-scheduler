/** Build an SDK client from config, optionally overriding the API key. */
import { ChaosSchedulerClient } from "@chaos-scheduler/sdk";
import type { ChaosMcpConfig } from "./config.js";

export function makeClient(
  config: ChaosMcpConfig,
  apiKeyOverride?: string,
): ChaosSchedulerClient {
  return new ChaosSchedulerClient({
    baseUrl: config.baseUrl,
    apiKey: apiKeyOverride ?? config.apiKey,
    timeoutMs: config.requestTimeoutMs,
  });
}
