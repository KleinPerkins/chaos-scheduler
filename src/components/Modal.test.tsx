import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import Modal from "./Modal";

afterEach(cleanup);

describe("Modal", () => {
  it("renders the backdrop > scrim button + dialog shell with the passed classes", () => {
    const { container } = render(
      <Modal
        onClose={vi.fn()}
        backdropClassName="rerun-modal-backdrop"
        scrimClassName="rerun-modal-scrim"
        className="rerun-modal"
        labelledBy="title-id"
        describedBy="desc-id"
      >
        <p>Body</p>
      </Modal>,
    );

    // Backdrop is the outer element and carries the passthrough class.
    const backdrop = container.firstChild as HTMLElement;
    expect(backdrop.tagName).toBe("DIV");
    expect(backdrop.getAttribute("class")).toBe("rerun-modal-backdrop");
    // Exactly two children: the scrim button then the dialog (order matters so
    // the dialog paints on top of the absolutely-positioned scrim).
    expect(backdrop.childElementCount).toBe(2);

    const scrim = backdrop.children[0] as HTMLButtonElement;
    expect(scrim.tagName).toBe("BUTTON");
    expect(scrim).toHaveAttribute("type", "button");
    expect(scrim.getAttribute("class")).toBe("rerun-modal-scrim");
    expect(scrim).toHaveAttribute("aria-label", "Close dialog");
    expect(scrim).not.toBeDisabled();

    const dialog = backdrop.children[1] as HTMLElement;
    expect(dialog.tagName).toBe("DIV");
    expect(dialog.getAttribute("class")).toBe("rerun-modal");
    expect(dialog).toHaveAttribute("role", "dialog");
    expect(dialog).toHaveAttribute("aria-modal", "true");
    expect(dialog).toHaveAttribute("aria-labelledby", "title-id");
    expect(dialog).toHaveAttribute("aria-describedby", "desc-id");
  });

  it("renders children inside the dialog element", () => {
    render(
      <Modal onClose={vi.fn()} className="rerun-modal">
        <p data-testid="body">Body content</p>
      </Modal>,
    );
    const dialog = screen.getByRole("dialog");
    const body = screen.getByTestId("body");
    expect(dialog).toContainElement(body);
  });

  it("calls onClose when the scrim button is clicked", () => {
    const onClose = vi.fn();
    render(
      <Modal onClose={onClose} scrimClassName="rerun-modal-scrim">
        <p>Body</p>
      </Modal>,
    );
    fireEvent.click(screen.getByRole("button", { name: "Close dialog" }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("does NOT close when the dialog (or its children) are clicked", () => {
    const onClose = vi.fn();
    render(
      <Modal onClose={onClose} className="rerun-modal">
        <button type="button">Inner</button>
      </Modal>,
    );
    fireEvent.click(screen.getByRole("dialog"));
    fireEvent.click(screen.getByRole("button", { name: "Inner" }));
    expect(onClose).not.toHaveBeenCalled();
  });

  it("calls onClose when Escape is pressed", () => {
    const onClose = vi.fn();
    render(
      <Modal onClose={onClose}>
        <p>Body</p>
      </Modal>,
    );
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("disables the scrim and suppresses Escape when closeDisabled is set", () => {
    const onClose = vi.fn();
    render(
      <Modal onClose={onClose} closeDisabled scrimClassName="rerun-modal-scrim">
        <p>Body</p>
      </Modal>,
    );
    const scrim = screen.getByRole("button", { name: "Close dialog" });
    expect(scrim).toBeDisabled();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).not.toHaveBeenCalled();
  });

  it("is class-less by default (no class attribute on any shell element)", () => {
    const { container } = render(
      <Modal onClose={vi.fn()}>
        <p>Body</p>
      </Modal>,
    );
    const backdrop = container.firstChild as HTMLElement;
    expect(backdrop.hasAttribute("class")).toBe(false);
    expect(backdrop.children[0].hasAttribute("class")).toBe(false);
    expect(backdrop.children[1].hasAttribute("class")).toBe(false);
  });

  it("omits aria-labelledby / aria-describedby when not provided", () => {
    render(
      <Modal onClose={vi.fn()}>
        <p>Body</p>
      </Modal>,
    );
    const dialog = screen.getByRole("dialog");
    expect(dialog.hasAttribute("aria-labelledby")).toBe(false);
    expect(dialog.hasAttribute("aria-describedby")).toBe(false);
  });
});
