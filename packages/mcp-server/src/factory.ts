/** Build an SDK client from config, optionally overriding the API key. */
import { ChaosSchedulerClient, type FetchLike } from "@chaos-scheduler/sdk";
import type { ChaosMcpConfig } from "./config.js";

export function makeClient(
  config: ChaosMcpConfig,
  apiKeyOverride?: string,
  fetchOverride?: FetchLike,
): ChaosSchedulerClient {
  return new ChaosSchedulerClient({
    baseUrl: config.baseUrl,
    apiKey: apiKeyOverride ?? config.apiKey,
    timeoutMs: config.requestTimeoutMs,
    ...(fetchOverride ? { fetch: fetchOverride } : {}),
  });
}
