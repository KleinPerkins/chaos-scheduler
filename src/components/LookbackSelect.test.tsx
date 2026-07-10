import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import LookbackSelect from "./LookbackSelect";

afterEach(cleanup);

describe("LookbackSelect", () => {
  it("renders a labelled group of the default presets plus Custom", () => {
    render(<LookbackSelect value="1d" onChange={() => {}} />);
    const group = screen.getByRole("group", { name: "Lookback window" });
    const buttons = group.querySelectorAll("button.lookback-option");
    expect(Array.from(buttons).map((b) => b.textContent)).toEqual([
      "1d",
      "3d",
      "7d",
      "30d",
      "Custom",
    ]);
  });

  it("marks only the selected option active and aria-pressed", () => {
    render(<LookbackSelect value="7d" onChange={() => {}} />);
    const seven = screen.getByRole("button", { name: "7d" });
    expect(seven.className).toBe("lookback-option active");
    expect(seven).toHaveAttribute("aria-pressed", "true");

    const one = screen.getByRole("button", { name: "1d" });
    expect(one.className).toBe("lookback-option");
    expect(one).toHaveAttribute("aria-pressed", "false");
  });

  it("calls onChange with the clicked preset value", () => {
    const onChange = vi.fn();
    render(<LookbackSelect value="1d" onChange={onChange} />);
    fireEvent.click(screen.getByRole("button", { name: "30d" }));
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith("30d");
  });

  it("calls onChange with `custom` when the Custom segment is clicked", () => {
    const onChange = vi.fn();
    render(<LookbackSelect value="1d" onChange={onChange} />);
    fireEvent.click(screen.getByRole("button", { name: "Custom" }));
    expect(onChange).toHaveBeenCalledWith("custom");
  });

  it("marks the Custom segment active when it is the selected value", () => {
    render(<LookbackSelect value="custom" onChange={() => {}} />);
    expect(screen.getByRole("button", { name: "Custom" })).toHaveClass(
      "lookback-option",
      "active",
    );
  });

  it("respects a custom `options` list", () => {
    render(
      <LookbackSelect
        value="3d"
        onChange={() => {}}
        options={["3d", "7d"]}
        includeCustom={false}
      />,
    );
    const buttons = screen
      .getByRole("group")
      .querySelectorAll("button.lookback-option");
    expect(Array.from(buttons).map((b) => b.textContent)).toEqual(["3d", "7d"]);
  });

  it("omits the Custom segment when includeCustom is false", () => {
    render(
      <LookbackSelect value="1d" onChange={() => {}} includeCustom={false} />,
    );
    expect(
      screen.queryByRole("button", { name: "Custom" }),
    ).not.toBeInTheDocument();
  });

  it("merges a passthrough className onto the container", () => {
    const { container } = render(
      <LookbackSelect
        value="1d"
        onChange={() => {}}
        className="filter-bar-lb"
      />,
    );
    expect((container.firstChild as HTMLElement).className).toBe(
      "lookback-select filter-bar-lb",
    );
  });
});
