import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useTheme } from "./useTheme";

beforeEach(() => {
  const values = new Map<string, string>();
  vi.stubGlobal("localStorage", {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => values.set(key, value),
    removeItem: (key: string) => values.delete(key),
    clear: () => values.clear(),
    key: (index: number) => [...values.keys()][index] ?? null,
    get length() {
      return values.size;
    },
  } satisfies Storage);
  document.documentElement.removeAttribute("data-theme");
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

describe("useTheme", () => {
  it("keeps independent consumers synchronized", () => {
    const first = renderHook(() => useTheme());
    const second = renderHook(() => useTheme());

    expect(first.result.current.preference).toBe("dark");
    expect(second.result.current.preference).toBe("dark");

    act(() => first.result.current.setPreference("light"));

    expect(first.result.current.preference).toBe("light");
    expect(second.result.current.preference).toBe("light");
    expect(document.documentElement).toHaveAttribute("data-theme", "light");
  });
});
