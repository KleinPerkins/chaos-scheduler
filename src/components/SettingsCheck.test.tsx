import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import SettingsCheck from "./SettingsCheck";

afterEach(cleanup);

function labelOf(container: HTMLElement): HTMLElement {
  return container.firstChild as HTMLElement;
}

describe("SettingsCheck", () => {
  it("renders a `.settings-check` <label> wrapping a checkbox then the label text", () => {
    const { container } = render(
      <SettingsCheck
        checked={false}
        onChange={() => {}}
        label="Launch at login"
      />,
    );
    const label = labelOf(container);
    expect(label.tagName).toBe("LABEL");
    expect(label.className).toBe("settings-check");
    expect(label.textContent).toBe("Launch at login");

    const input = label.firstElementChild as HTMLInputElement;
    expect(input.tagName).toBe("INPUT");
    expect(input.getAttribute("type")).toBe("checkbox");
  });

  it("reflects the `checked` prop on the input", () => {
    const { container } = render(
      <SettingsCheck checked onChange={() => {}} label="On" />,
    );
    const input = labelOf(container).firstElementChild as HTMLInputElement;
    expect(input.checked).toBe(true);
  });

  it("forwards change events to onChange", () => {
    const onChange = vi.fn();
    const { container } = render(
      <SettingsCheck checked={false} onChange={onChange} label="Toggle" />,
    );
    fireEvent.click(labelOf(container).firstElementChild as HTMLInputElement);
    expect(onChange).toHaveBeenCalledTimes(1);
  });

  it("forwards the `disabled` prop to the input", () => {
    const { container } = render(
      <SettingsCheck
        checked
        onChange={() => {}}
        disabled
        label="Disabled row"
      />,
    );
    const input = labelOf(container).firstElementChild as HTMLInputElement;
    expect(input.disabled).toBe(true);
  });
});
