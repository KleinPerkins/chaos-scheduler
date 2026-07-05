import { describe, expect, it } from "vitest";
import { isActiveRunStatus, nextPollDelayMs } from "./runPolling";

describe("runPolling", () => {
  it("treats queued, admitted, and running as active", () => {
    expect(isActiveRunStatus("queued")).toBe(true);
    expect(isActiveRunStatus("admitted")).toBe(true);
    expect(isActiveRunStatus("running")).toBe(true);
    expect(isActiveRunStatus("failed")).toBe(false);
  });

  it("backs off polling delays with a cap", () => {
    expect(nextPollDelayMs(0)).toBe(2000);
    expect(nextPollDelayMs(1)).toBe(3000);
    expect(nextPollDelayMs(10)).toBe(30000);
  });
});
