import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import NavItem from "./NavItem";

afterEach(cleanup);

function buttonOf(container: HTMLElement): HTMLButtonElement {
  return container.firstChild as HTMLButtonElement;
}

describe("NavItem", () => {
  it("renders a `.sidebar-link` button with an aria-hidden icon slot and label, inactive by default", () => {
    const { container } = render(
      <NavItem icon={<span data-testid="nav-icon" />} label="Home" />,
    );
    const button = buttonOf(container);
    expect(button.tagName).toBe("BUTTON");
    // Byte-identical to the original inactive markup: `sidebar-link ${""}`
    // leaves a trailing space, and no aria-current attribute is emitted.
    expect(button.className).toBe("sidebar-link ");
    expect(button).not.toHaveAttribute("aria-current");

    const icon = button.firstElementChild as HTMLElement;
    expect(icon.tagName).toBe("SPAN");
    expect(icon.className).toBe("sidebar-icon");
    expect(icon).toHaveAttribute("aria-hidden", "true");
    expect(icon.querySelector('[data-testid="nav-icon"]')).not.toBeNull();

    expect(button.textContent).toBe("Home");
  });

  it("applies the `.active` modifier and aria-current=page when active", () => {
    const { container } = render(
      <NavItem icon={<span />} label="Workflows" active />,
    );
    const button = buttonOf(container);
    expect(button.className).toBe("sidebar-link active");
    expect(button).toHaveAttribute("aria-current", "page");
  });

  it("fires onClick when pressed", () => {
    const onClick = vi.fn();
    const { container } = render(
      <NavItem icon={<span />} label="Queues" onClick={onClick} />,
    );
    fireEvent.click(buttonOf(container));
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it("merges a passthrough className after the base classes", () => {
    const { container } = render(
      <NavItem icon={<span />} label="x" active className="extra" />,
    );
    expect(buttonOf(container).className).toBe("sidebar-link active extra");
  });

  it("forwards native button attributes", () => {
    const { container } = render(
      <NavItem icon={<span />} label="x" title="tip" />,
    );
    expect(buttonOf(container)).toHaveAttribute("title", "tip");
  });
});
