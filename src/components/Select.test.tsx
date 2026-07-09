import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import Select from "./Select";

afterEach(cleanup);

describe("Select", () => {
  it("renders a native class-less <select> by default", () => {
    const { container } = render(
      <Select>
        <option value="a">A</option>
      </Select>,
    );
    const select = container.firstChild as HTMLElement;
    expect(select.tagName).toBe("SELECT");
    // The global `input, select, textarea` element selector plus contextual
    // parent selectors (e.g. `.sched-row select`, `.action-row select`) style
    // it — the primitive must NOT emit a `class` attribute (not even
    // `class=""`) so the rendered DOM is byte-identical to the previous raw
    // `<select>` call sites.
    expect(select.hasAttribute("class")).toBe(false);
  });

  it("merges a passthrough className when one is provided", () => {
    const { container } = render(
      <Select className="custom-select">
        <option value="a">A</option>
      </Select>,
    );
    const select = container.firstChild as HTMLElement;
    expect(select.getAttribute("class")).toBe("custom-select");
  });

  it("renders its <option> children", () => {
    const { container } = render(
      <Select>
        <option value="a">Alpha</option>
        <option value="b">Bravo</option>
      </Select>,
    );
    const options = container.querySelectorAll("option");
    expect(options).toHaveLength(2);
    expect(options[0]).toHaveValue("a");
    expect(options[0]).toHaveTextContent("Alpha");
    expect(options[1]).toHaveValue("b");
    expect(options[1]).toHaveTextContent("Bravo");
  });

  it("forwards native select props (name, disabled) and reflects selected value", () => {
    const { container } = render(
      <Select name="env" value="b" onChange={() => {}} disabled>
        <option value="a">A</option>
        <option value="b">B</option>
      </Select>,
    );
    const select = container.firstChild as HTMLSelectElement;
    expect(select).toHaveAttribute("name", "env");
    expect(select).toBeDisabled();
    expect(select.value).toBe("b");
  });

  it("supports defaultValue (uncontrolled)", () => {
    const { container } = render(
      <Select defaultValue="b">
        <option value="a">A</option>
        <option value="b">B</option>
      </Select>,
    );
    const select = container.firstChild as HTMLSelectElement;
    expect(select.value).toBe("b");
  });

  it("forwards onChange events", () => {
    const onChange = vi.fn();
    const { container } = render(
      <Select value="a" onChange={onChange}>
        <option value="a">A</option>
        <option value="b">B</option>
      </Select>,
    );
    fireEvent.change(container.firstChild as HTMLSelectElement, {
      target: { value: "b" },
    });
    expect(onChange).toHaveBeenCalledTimes(1);
  });
});
