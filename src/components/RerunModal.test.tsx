import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import RerunModal from "./RerunModal";

describe("RerunModal", () => {
  afterEach(() => cleanup());

  it("submits valid JSON overrides", () => {
    const onSubmit = vi.fn();
    render(
      <RerunModal
        workflowName="Daily digest"
        initialJson="{}"
        busy={false}
        error={null}
        onCancel={vi.fn()}
        onSubmit={onSubmit}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /^Rerun$/i }));
    expect(onSubmit).toHaveBeenCalledWith("{}");
  });

  it("shows JSON parse errors inline", () => {
    render(
      <RerunModal
        workflowName="Daily digest"
        initialJson="{bad"
        busy={false}
        error={null}
        onCancel={vi.fn()}
        onSubmit={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /^Rerun$/i }));
    expect(screen.getByRole("alert")).toHaveTextContent(/valid JSON/i);
  });
});
