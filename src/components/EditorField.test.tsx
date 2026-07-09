import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/react";
import EditorField from "./EditorField";

afterEach(cleanup);

function fieldOf(container: HTMLElement): HTMLElement {
  return container.firstChild as HTMLElement;
}

describe("EditorField", () => {
  it("renders a `.editor-field` <label> with an `.editor-label` span then the control", () => {
    const { container } = render(
      <EditorField label="Branch">
        <input />
      </EditorField>,
    );
    const label = fieldOf(container);
    expect(label.tagName).toBe("LABEL");
    expect(label.className).toBe("editor-field");

    const labelSpan = label.children[0] as HTMLElement;
    expect(labelSpan.tagName).toBe("SPAN");
    expect(labelSpan.className).toBe("editor-label");
    expect(labelSpan.textContent).toBe("Branch");

    // The control follows the label span.
    expect(label.children[1].tagName).toBe("INPUT");
  });

  it("omits the `.editor-hint` entirely when no hint is provided", () => {
    const { container } = render(
      <EditorField label="Branch">
        <input />
      </EditorField>,
    );
    const label = fieldOf(container);
    expect(label.querySelector(".editor-hint")).toBeNull();
    expect(label.childElementCount).toBe(2);
  });

  it("renders a trailing `.editor-hint` span (last child) when hint is provided", () => {
    const { container } = render(
      <EditorField label="Operator" hint="Custom operator">
        <input />
      </EditorField>,
    );
    const label = fieldOf(container);
    expect(label.childElementCount).toBe(3);
    const hint = label.children[2] as HTMLElement;
    expect(hint.tagName).toBe("SPAN");
    expect(hint.className).toBe("editor-hint");
    expect(hint.textContent).toBe("Custom operator");
  });

  it("merges a passthrough className after the base `editor-field` class", () => {
    const { container } = render(
      <EditorField label="x" className="extra">
        <input />
      </EditorField>,
    );
    expect(fieldOf(container).className).toBe("editor-field extra");
  });

  it("forwards native label attributes (e.g. style) onto the `<label>`", () => {
    const { container } = render(
      <EditorField label="Branch" style={{ width: 120 }}>
        <input />
      </EditorField>,
    );
    expect(fieldOf(container).style.width).toBe("120px");
  });
});
