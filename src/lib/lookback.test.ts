import { describe, expect, it } from "vitest";
import {
  DEFAULT_LOOKBACK,
  LOOKBACK_PRESETS,
  lookbackToParam,
  resolveLookbackRange,
} from "./lookback";

describe("Lookback contract", () => {
  it("defaults to 1d", () => {
    expect(DEFAULT_LOOKBACK).toBe("1d");
  });

  it("exposes the preset windows in order (custom excluded)", () => {
    expect(LOOKBACK_PRESETS).toEqual(["1d", "3d", "7d", "30d"]);
  });
});

describe("resolveLookbackRange", () => {
  const now = new Date("2026-07-10T12:00:00.000Z");
  const DAY = 24 * 60 * 60 * 1000;

  it.each([
    ["1d", 1],
    ["3d", 3],
    ["7d", 7],
    ["30d", 30],
  ] as const)(
    "resolves %s to a trailing %i-day window ending at now",
    (lb, days) => {
      const { start, end } = resolveLookbackRange(lb, { now });
      expect(end).toEqual(now);
      expect(start).toEqual(new Date(now.getTime() - days * DAY));
    },
  );

  it("uses the supplied bounds for a custom window", () => {
    const customStart = new Date("2026-06-01T00:00:00.000Z");
    const customEnd = new Date("2026-06-15T00:00:00.000Z");
    expect(resolveLookbackRange("custom", { customStart, customEnd })).toEqual({
      start: customStart,
      end: customEnd,
    });
  });

  it("throws when a custom window is missing its bounds", () => {
    expect(() => resolveLookbackRange("custom", { now })).toThrow();
  });

  it("defaults the window end to the current time when now is omitted", () => {
    const before = Date.now();
    const { end } = resolveLookbackRange("1d");
    const after = Date.now();
    expect(end.getTime()).toBeGreaterThanOrEqual(before);
    expect(end.getTime()).toBeLessThanOrEqual(after);
  });
});

describe("lookbackToParam", () => {
  it.each(["1d", "3d", "7d", "30d"] as const)(
    "passes the preset %s through unchanged (it is already valid grammar)",
    (preset) => {
      expect(lookbackToParam(preset)).toBe(preset);
    },
  );

  it("serializes a whole-day custom range to `<n>h`", () => {
    expect(
      lookbackToParam("custom", {
        customStart: new Date("2026-06-01T00:00:00.000Z"),
        customEnd: new Date("2026-06-04T00:00:00.000Z"),
      }),
    ).toBe("72h");
  });

  it("rounds a partial-hour custom span up to whole hours", () => {
    // 90 minutes → 2h (never drop the earliest bucket).
    expect(
      lookbackToParam("custom", {
        customStart: new Date("2026-06-01T00:00:00.000Z"),
        customEnd: new Date("2026-06-01T01:30:00.000Z"),
      }),
    ).toBe("2h");
  });

  it("rounds a sub-hour custom span up to `1h`", () => {
    expect(
      lookbackToParam("custom", {
        customStart: new Date("2026-06-01T00:00:00.000Z"),
        customEnd: new Date("2026-06-01T00:10:00.000Z"),
      }),
    ).toBe("1h");
  });

  it("throws when a custom window is missing its bounds", () => {
    expect(() => lookbackToParam("custom")).toThrow();
  });

  it("throws when a custom window's end is not after its start", () => {
    const at = new Date("2026-06-01T00:00:00.000Z");
    expect(() =>
      lookbackToParam("custom", { customStart: at, customEnd: at }),
    ).toThrow();
  });
});
