import { afterEach, describe, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import type { Environment } from "../lib/commands";
import FilterBar from "./FilterBar";

afterEach(cleanup);

const ENVIRONMENTS: Environment[] = [
  { id: "env-prod", name: "production" },
  { id: "env-sandbox", name: "sandbox" },
];

function renderBar(
  overrides: Partial<React.ComponentProps<typeof FilterBar>> = {},
) {
  const props = {
    environments: ENVIRONMENTS,
    environment: "production",
    onEnvironmentChange: vi.fn(),
    lookback: "1d" as const,
    onLookbackChange: vi.fn(),
    ...overrides,
  };
  return { props, ...render(<FilterBar {...props} />) };
}

describe("FilterBar", () => {
  it("composes the Environment select with an All sentinel + the environments", () => {
    renderBar();
    const select = screen.getByRole("combobox", {
      name: "Environment",
    }) as HTMLSelectElement;
    expect(
      Array.from(select.options).map((o) => [o.value, o.textContent]),
    ).toEqual([
      ["all", "All"],
      ["production", "Production"],
      ["sandbox", "Sandbox"],
    ]);
    expect(select.value).toBe("production");
  });

  it("reports the chosen environment", () => {
    const { props } = renderBar();
    fireEvent.change(screen.getByRole("combobox", { name: "Environment" }), {
      target: { value: "sandbox" },
    });
    expect(props.onEnvironmentChange).toHaveBeenCalledWith("sandbox");
  });

  it("unions an out-of-list environment value so it stays selectable", () => {
    renderBar({ environment: "staging" });
    const select = screen.getByRole("combobox", {
      name: "Environment",
    }) as HTMLSelectElement;
    expect(select.value).toBe("staging");
    expect(Array.from(select.options).some((o) => o.value === "staging")).toBe(
      true,
    );
  });

  it("composes the lookback selector and reports the chosen window", () => {
    const { props } = renderBar({ lookback: "7d" });
    const group = screen.getByRole("group", { name: "Lookback window" });
    expect(within(group).getByRole("button", { name: "7d" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    fireEvent.click(within(group).getByRole("button", { name: "30d" }));
    expect(props.onLookbackChange).toHaveBeenCalledWith("30d");
  });

  it("exposes hover/focus InfoTips for both filters", () => {
    renderBar();
    expect(
      screen.getByRole("button", { name: "Environment" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Lookback window" }),
    ).toBeInTheDocument();
  });

  it("hides the custom date range unless the custom window is selected", () => {
    renderBar({ lookback: "1d" });
    expect(screen.queryByLabelText("From")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("To")).not.toBeInTheDocument();
  });

  it("shows the custom date range and reports edits when custom is selected", () => {
    const onCustomRangeChange = vi.fn();
    renderBar({
      lookback: "custom",
      customRange: { start: "2026-06-01", end: "2026-06-08" },
      onCustomRangeChange,
    });
    const from = screen.getByLabelText("From") as HTMLInputElement;
    const to = screen.getByLabelText("To") as HTMLInputElement;
    expect(from.value).toBe("2026-06-01");
    expect(to.value).toBe("2026-06-08");
    fireEvent.change(from, { target: { value: "2026-06-02" } });
    expect(onCustomRangeChange).toHaveBeenCalledWith({
      start: "2026-06-02",
      end: "2026-06-08",
    });
  });

  it("renders surface-specific extra controls at the trailing edge", () => {
    renderBar({ extras: <button type="button">Domain</button> });
    expect(screen.getByRole("button", { name: "Domain" })).toBeInTheDocument();
  });
});
