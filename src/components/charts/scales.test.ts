import { describe, expect, it } from "vitest";
import { linearAxisTicks, niceLinearDomain, timeTicks } from "./scales";

describe("niceLinearDomain", () => {
  it("anchors to zero and rounds the max up to a nice bound", () => {
    // d3 `.nice()` rounds [0, 42] up to a friendly step boundary (multiple of 5).
    expect(niceLinearDomain([3, 42, 17])).toEqual([0, 45]);
  });

  it("returns [0, 1] for empty input", () => {
    expect(niceLinearDomain([])).toEqual([0, 1]);
  });

  it("widens a flat series so the range is never zero", () => {
    const [min, max] = niceLinearDomain([5, 5, 5]);
    expect(max).toBeGreaterThan(min);
  });

  it("can include negative values when zero-anchoring is disabled", () => {
    const [min, max] = niceLinearDomain([-8, -3], { zero: false });
    expect(min).toBeLessThan(0);
    expect(max).toBeLessThanOrEqual(0);
  });

  it("spans zero when values straddle it", () => {
    const [min, max] = niceLinearDomain([-4, 9]);
    expect(min).toBeLessThanOrEqual(-4);
    expect(max).toBeGreaterThanOrEqual(9);
  });
});

describe("linearAxisTicks", () => {
  it("maps each tick value to a pixel offset within the range", () => {
    const ticks = linearAxisTicks([0, 100], [0, 200], 5);
    expect(ticks.length).toBeGreaterThan(0);
    const first = ticks[0];
    const last = ticks[ticks.length - 1];
    expect(first.value).toBe(0);
    expect(first.offset).toBe(0);
    expect(last.value).toBe(100);
    expect(last.offset).toBe(200);
  });

  it("respects an inverted range (SVG y grows downward)", () => {
    // value 0 -> bottom (offset 100), value max -> top (offset 0)
    const ticks = linearAxisTicks([0, 50], [100, 0], 5);
    const zero = ticks.find((t) => t.value === 0)!;
    const top = ticks.find((t) => t.value === 50)!;
    expect(zero.offset).toBe(100);
    expect(top.offset).toBe(0);
  });

  it("does not re-nice the caller-owned domain", () => {
    // A deliberately un-nice domain must keep its endpoints pinned to the range.
    const ticks = linearAxisTicks([0, 37], [0, 100]);
    expect(ticks.every((t) => t.offset >= 0 && t.offset <= 100)).toBe(true);
  });
});

describe("timeTicks", () => {
  it("lands hourly ticks on whole hours across a one-day span", () => {
    const start = Date.UTC(2026, 6, 4, 0, 0, 0);
    const end = Date.UTC(2026, 6, 5, 0, 0, 0);
    const ticks = timeTicks(start, end, 6);
    expect(ticks.length).toBeGreaterThan(1);
    // every hourly-granularity label is HH:00
    expect(ticks.every((t) => /^\d{2}:00$/.test(t.label))).toBe(true);
    // ascending, in-range
    for (let i = 1; i < ticks.length; i++) {
      expect(ticks[i].value).toBeGreaterThan(ticks[i - 1].value);
    }
    expect(ticks[0].value).toBeGreaterThanOrEqual(start);
  });

  it("uses day granularity for multi-day spans", () => {
    const start = Date.UTC(2026, 6, 1);
    const end = Date.UTC(2026, 6, 30);
    const ticks = timeTicks(start, end, 6);
    // day labels are not HH:MM
    expect(ticks.some((t) => !/^\d{2}:\d{2}$/.test(t.label))).toBe(true);
  });

  it("degrades to a single tick for a non-positive span", () => {
    const ticks = timeTicks(1000, 1000);
    expect(ticks).toHaveLength(1);
  });
});
