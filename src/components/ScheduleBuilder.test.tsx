import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import ScheduleBuilder, { cronToHuman } from "./ScheduleBuilder";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("cronToHuman", () => {
  it.each([
    ["0 9 * * *", "Daily at 9:00 AM"],
    ["0 0 9 * * Mon-Fri", "Weekdays at 9:00 AM"],
    ["0 0 9 * * Mon-Fri *", "Weekdays at 9:00 AM"],
    ["0 0 9 * * 1,3,5 *", "Every Monday, Wednesday and Friday at 9:00 AM"],
  ])("normalizes supported cron field counts: %s", (cron, expected) => {
    expect(cronToHuman(cron)).toBe(expected);
  });

  it("humanizes compatible multi-schedule expressions", () => {
    expect(cronToHuman("0 0 9 * * Mon *; 0 30 17 * * Mon *")).toBe(
      "Every Monday at 9:00 AM and 5:30 PM",
    );
  });

  it("returns unsupported expressions unchanged", () => {
    expect(cronToHuman("not a valid cron")).toBe("not a valid cron");
  });
});

describe("ScheduleBuilder", () => {
  it("does not mutate a valid schedule merely by mounting", () => {
    const onChange = vi.fn();

    render(
      <ScheduleBuilder
        value="0 9 * * *"
        timezone="America/Los_Angeles"
        onChange={onChange}
      />,
    );

    expect(onChange).not.toHaveBeenCalled();
  });

  it("exposes names and selected state for visual controls", () => {
    render(
      <ScheduleBuilder
        value="0 0 9 * * Mon *"
        timezone="UTC"
        onChange={() => {}}
      />,
    );

    expect(screen.getByRole("button", { name: "Weekly" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByRole("button", { name: "Monday" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(
      screen.getByRole("combobox", { name: "Time 1 hour" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("combobox", { name: "Time 1 minute" }),
    ).toBeInTheDocument();

    fireEvent.click(
      screen.getByRole("button", { name: "Edit cron expression" }),
    );
    expect(
      screen.getByRole("textbox", { name: "Cron expression" }),
    ).toHaveValue("0 0 9 * * Mon *");
  });

  it("emits only syntactically valid advanced expressions", () => {
    const onChange = vi.fn();
    render(
      <ScheduleBuilder
        value="not a valid cron"
        timezone="UTC"
        onChange={onChange}
      />,
    );

    const input = screen.getByRole("textbox", { name: "Cron expression" });
    fireEvent.change(input, { target: { value: "still invalid!" } });
    expect(onChange).not.toHaveBeenCalled();

    fireEvent.change(input, { target: { value: "0 0 9 * * Mon *" } });
    expect(onChange).toHaveBeenLastCalledWith("0 0 9 * * Mon *");
  });
});
