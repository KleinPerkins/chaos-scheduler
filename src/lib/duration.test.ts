import { describe, expect, it } from "vitest";
import { formatDuration, formatDurationBetween } from "./duration";

const S = 1000;
const M = 60 * S;
const H = 60 * M;
const D = 24 * H;

describe("formatDuration", () => {
  it("renders whole seconds below one minute", () => {
    expect(formatDuration(0)).toBe("0s");
    expect(formatDuration(59 * S)).toBe("59s");
  });

  it("switches to `#m #s` at exactly 60s", () => {
    expect(formatDuration(60 * S)).toBe("1m 0s");
    expect(formatDuration(59 * M + 59 * S)).toBe("59m 59s");
  });

  it("switches to `#h #m` at exactly 60m", () => {
    expect(formatDuration(60 * M)).toBe("1h 0m");
    expect(formatDuration(23 * H + 59 * M)).toBe("23h 59m");
  });

  it("switches to `#d #h` at exactly 24h and for multi-day spans", () => {
    expect(formatDuration(24 * H)).toBe("1d 0h");
    expect(formatDuration(2 * D + 3 * H)).toBe("2d 3h");
  });

  it("clamps sub-second and negative inputs to `0s`", () => {
    expect(formatDuration(400)).toBe("0s");
    expect(formatDuration(-5 * S)).toBe("0s");
  });

  it("floors within a tier rather than rounding", () => {
    expect(formatDuration(89 * S)).toBe("1m 29s");
  });
});

describe("formatDurationBetween", () => {
  it("formats the elapsed time between two ISO-8601 timestamps", () => {
    expect(
      formatDurationBetween(
        "2026-01-01T00:00:00.000Z",
        "2026-01-01T00:01:30.000Z",
      ),
    ).toBe("1m 30s");
  });

  it("delegates to the shared ladder for multi-hour spans", () => {
    expect(
      formatDurationBetween(
        "2026-01-01T00:00:00.000Z",
        "2026-01-01T02:05:00.000Z",
      ),
    ).toBe("2h 5m");
  });
});
