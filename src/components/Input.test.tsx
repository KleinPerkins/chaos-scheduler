import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import Input from "./Input";

afterEach(cleanup);

describe("Input", () => {
  it("renders a native class-less <input> by default", () => {
    const { container } = render(<Input />);
    const input = container.firstChild as HTMLElement;
    expect(input.tagName).toBe("INPUT");
    // The global `input, select, textarea` element selector styles it — the
    // primitive must NOT emit a `class` attribute (not even `class=""`) so the
    // rendered DOM is byte-identical to the previous raw `<input>` call sites.
    expect(input.hasAttribute("class")).toBe(false);
  });

  it("merges a passthrough className when one is provided", () => {
    const { container } = render(<Input className="sched-input-error" />);
    const input = container.firstChild as HTMLElement;
    expect(input.getAttribute("class")).toBe("sched-input-error");
  });

  it("forwards native input props (type, placeholder, value, disabled)", () => {
    const { container } = render(
      <Input
        type="email"
        placeholder="you@example.com"
        value="hi@example.com"
        onChange={() => {}}
        disabled
      />,
    );
    const input = container.firstChild as HTMLInputElement;
    expect(input).toHaveAttribute("type", "email");
    expect(input).toHaveAttribute("placeholder", "you@example.com");
    expect(input.value).toBe("hi@example.com");
    expect(input).toBeDisabled();
  });

  it("forwards onChange events", () => {
    const onChange = vi.fn();
    const { container } = render(<Input onChange={onChange} />);
    fireEvent.change(container.firstChild as HTMLInputElement, {
      target: { value: "typed" },
    });
    expect(onChange).toHaveBeenCalledTimes(1);
  });
});
