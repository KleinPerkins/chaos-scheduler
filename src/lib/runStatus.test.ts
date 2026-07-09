import { describe, expect, it } from "vitest";
import { formatRunStatusLabel, statusKey } from "./runStatus";

describe("formatRunStatusLabel", () => {
  it("maps poll_exhausted to a spaced label", () => {
    expect(formatRunStatusLabel("poll_exhausted")).toBe("poll exhausted");
  });

  it("maps timed_out to a spaced label", () => {
    expect(formatRunStatusLabel("timed_out")).toBe("timed out");
  });

  it("passes through simple statuses", () => {
    expect(formatRunStatusLabel("failed")).toBe("failed");
    expect(formatRunStatusLabel("success")).toBe("success");
  });
});

describe("statusKey", () => {
  it("collapses the succeeded alias onto success", () => {
    expect(statusKey("succeeded")).toBe("success");
    expect(statusKey("success")).toBe("success");
  });

  it("passes through all other status tokens unchanged", () => {
    expect(statusKey("running")).toBe("running");
    expect(statusKey("failed")).toBe("failed");
    expect(statusKey("queued")).toBe("queued");
    expect(statusKey("poll_exhausted")).toBe("poll_exhausted");
  });

  it("is idempotent", () => {
    expect(statusKey(statusKey("succeeded"))).toBe("success");
  });
});
