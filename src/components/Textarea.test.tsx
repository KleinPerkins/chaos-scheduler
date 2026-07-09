import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import Textarea from "./Textarea";

afterEach(cleanup);

describe("Textarea", () => {
  it("renders a native class-less <textarea> by default", () => {
    const { container } = render(<Textarea />);
    const textarea = container.firstChild as HTMLElement;
    expect(textarea.tagName).toBe("TEXTAREA");
    // The global `input, select, textarea` element selector styles it — the
    // primitive must NOT emit a `class` attribute (not even `class=""`) so the
    // rendered DOM is byte-identical to the previous raw `<textarea>` call sites.
    expect(textarea.hasAttribute("class")).toBe(false);
  });

  it("merges a passthrough className when one is provided", () => {
    const { container } = render(<Textarea className="rerun-modal-textarea" />);
    const textarea = container.firstChild as HTMLElement;
    expect(textarea.getAttribute("class")).toBe("rerun-modal-textarea");
  });

  it("forwards native textarea props (name, placeholder, value, rows, disabled)", () => {
    const { container } = render(
      <Textarea
        name="prompt"
        placeholder="What should the agent do?"
        value="hello"
        onChange={() => {}}
        rows={3}
        disabled
      />,
    );
    const textarea = container.firstChild as HTMLTextAreaElement;
    expect(textarea).toHaveAttribute("name", "prompt");
    expect(textarea).toHaveAttribute(
      "placeholder",
      "What should the agent do?",
    );
    expect(textarea).toHaveAttribute("rows", "3");
    expect(textarea.value).toBe("hello");
    expect(textarea).toBeDisabled();
  });

  it("supports defaultValue (uncontrolled)", () => {
    const { container } = render(<Textarea defaultValue="seed" />);
    const textarea = container.firstChild as HTMLTextAreaElement;
    expect(textarea.value).toBe("seed");
  });

  it("forwards onChange events", () => {
    const onChange = vi.fn();
    const { container } = render(<Textarea value="" onChange={onChange} />);
    fireEvent.change(container.firstChild as HTMLTextAreaElement, {
      target: { value: "typed" },
    });
    expect(onChange).toHaveBeenCalledTimes(1);
  });
});
