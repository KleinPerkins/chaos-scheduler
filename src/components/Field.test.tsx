import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/react";
import Field from "./Field";

afterEach(cleanup);

describe("Field", () => {
  it("renders a <label> wrapping a class-less <span> label then the control child", () => {
    const { container } = render(
      <Field className="step-field" label="Step ID">
        <input />
      </Field>,
    );
    const label = container.firstChild as HTMLElement;
    expect(label.tagName).toBe("LABEL");
    expect(label.getAttribute("class")).toBe("step-field");

    // The label text is wrapped in a *class-less* <span> — the `.X-field > span`
    // selector styles it, so the primitive must NOT emit a `class` on the span
    // (not even `class=""`) to stay byte-identical to the original call sites.
    const span = label.firstElementChild as HTMLElement;
    expect(span.tagName).toBe("SPAN");
    expect(span.hasAttribute("class")).toBe(false);
    expect(span.textContent).toBe("Step ID");

    // The control follows the label span, as a direct child of the <label>.
    expect(label.childElementCount).toBe(2);
    expect(label.children[1].tagName).toBe("INPUT");
  });

  it("passes through multiple label classes (base + modifier)", () => {
    const { container } = render(
      <Field className="step-field step-field-id" label="Step ID">
        <input />
      </Field>,
    );
    const label = container.firstChild as HTMLElement;
    expect(label.getAttribute("class")).toBe("step-field step-field-id");
  });

  it("emits no class attribute on the label when className is omitted", () => {
    const { container } = render(
      <Field label="Bare">
        <input />
      </Field>,
    );
    const label = container.firstChild as HTMLElement;
    expect(label.hasAttribute("class")).toBe(false);
  });

  it("preserves control order for a <select> control", () => {
    const { container } = render(
      <Field className="env-field" label="Environment">
        <select>
          <option value="a">A</option>
        </select>
      </Field>,
    );
    const label = container.firstChild as HTMLElement;
    expect(label.children[0].tagName).toBe("SPAN");
    expect(label.children[0].textContent).toBe("Environment");
    expect(label.children[1].tagName).toBe("SELECT");
  });

  it("renders a ReactNode label inside the span", () => {
    const { container } = render(
      <Field className="intg-field" label={<strong>Rich</strong>}>
        <input />
      </Field>,
    );
    const span = (container.firstChild as HTMLElement)
      .firstElementChild as HTMLElement;
    expect(span.tagName).toBe("SPAN");
    expect(span.innerHTML).toBe("<strong>Rich</strong>");
  });
});
