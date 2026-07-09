import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/react";
import SettingsField from "./SettingsField";

afterEach(cleanup);

function fieldOf(container: HTMLElement): HTMLElement {
  return container.firstChild as HTMLElement;
}

describe("SettingsField", () => {
  it("renders a `.settings-field` with a `.settings-label`[htmlFor] then the control", () => {
    const { container } = render(
      <SettingsField label="Workspace root" htmlFor="workspace-root">
        <input id="workspace-root" />
      </SettingsField>,
    );
    const field = fieldOf(container);
    expect(field.tagName).toBe("DIV");
    expect(field.className).toBe("settings-field");

    const label = field.children[0] as HTMLElement;
    expect(label.tagName).toBe("LABEL");
    expect(label.className).toBe("settings-label");
    expect(label.getAttribute("for")).toBe("workspace-root");
    expect(label.textContent).toBe("Workspace root");

    // The control follows the label as the next child.
    expect(field.children[1].tagName).toBe("INPUT");
  });

  it("omits the `.settings-hint` entirely when no hint is provided", () => {
    const { container } = render(
      <SettingsField label="Port" htmlFor="port">
        <input id="port" />
      </SettingsField>,
    );
    const field = fieldOf(container);
    expect(field.querySelector(".settings-hint")).toBeNull();
    expect(field.childElementCount).toBe(2);
  });

  it("renders a trailing `<span.settings-hint>` after the control when hint is provided", () => {
    const { container } = render(
      <SettingsField label="Port" htmlFor="port" hint="Default 587">
        <input id="port" />
      </SettingsField>,
    );
    const field = fieldOf(container);
    expect(field.childElementCount).toBe(3);
    const hint = field.children[2] as HTMLElement;
    expect(hint.tagName).toBe("SPAN");
    expect(hint.className).toBe("settings-hint");
    expect(hint.textContent).toBe("Default 587");
  });

  it("merges a passthrough className after the base `settings-field` class", () => {
    const { container } = render(
      <SettingsField label="x" htmlFor="x" className="extra">
        <input id="x" />
      </SettingsField>,
    );
    expect(fieldOf(container).className).toBe("settings-field extra");
  });

  it("forwards native div attributes (e.g. style) onto the `.settings-field`", () => {
    const { container } = render(
      <SettingsField label="Port" htmlFor="port" style={{ width: 90 }}>
        <input id="port" />
      </SettingsField>,
    );
    expect(fieldOf(container).style.width).toBe("90px");
  });
});
