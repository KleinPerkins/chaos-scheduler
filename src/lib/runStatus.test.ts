import { describe, expect, it } from "vitest";
import { formatRunStatusLabel } from "./runStatus";

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
