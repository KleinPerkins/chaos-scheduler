import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import Button from "./Button";

afterEach(cleanup);

describe("Button", () => {
  it("defaults to the neutral `.btn` primitive with no variant modifier", () => {
    render(<Button>Go</Button>);
    const btn = screen.getByRole("button", { name: "Go" });
    expect(btn.className).toBe("btn");
  });

  it("maps each variant to its shared class", () => {
    const { rerender } = render(<Button variant="primary">P</Button>);
    expect(screen.getByRole("button")).toHaveClass("btn", "btn-primary");
    rerender(<Button variant="ghost">G</Button>);
    expect(screen.getByRole("button")).toHaveClass("btn", "btn-ghost");
    rerender(<Button variant="danger">D</Button>);
    expect(screen.getByRole("button")).toHaveClass("btn", "btn-danger");
  });

  it("applies the compact size modifier", () => {
    render(
      <Button variant="ghost" size="sm">
        S
      </Button>,
    );
    expect(screen.getByRole("button")).toHaveClass(
      "btn",
      "btn-ghost",
      "btn-sm",
    );
  });

  it("merges a passthrough className after the primitive classes", () => {
    render(
      <Button variant="ghost" size="sm" className="step-add">
        A
      </Button>,
    );
    expect(screen.getByRole("button").className).toBe(
      "btn btn-ghost btn-sm step-add",
    );
  });

  it("fires onClick", () => {
    const onClick = vi.fn();
    render(<Button onClick={onClick}>Click</Button>);
    fireEvent.click(screen.getByRole("button"));
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it("does not fire onClick when disabled", () => {
    const onClick = vi.fn();
    render(
      <Button disabled onClick={onClick}>
        Click
      </Button>,
    );
    const btn = screen.getByRole("button");
    expect(btn).toBeDisabled();
    fireEvent.click(btn);
    expect(onClick).not.toHaveBeenCalled();
  });

  it("disables interaction and marks aria-busy while loading", () => {
    const onClick = vi.fn();
    render(
      <Button loading onClick={onClick}>
        Save
      </Button>,
    );
    const btn = screen.getByRole("button");
    expect(btn).toBeDisabled();
    expect(btn).toHaveAttribute("aria-busy", "true");
    fireEvent.click(btn);
    expect(onClick).not.toHaveBeenCalled();
  });

  it("forwards native button props like type", () => {
    render(<Button type="submit">Submit</Button>);
    expect(screen.getByRole("button")).toHaveAttribute("type", "submit");
  });
});
