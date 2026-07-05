import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import ErrorBoundary from "./ErrorBoundary";

describe("ErrorBoundary", () => {
  it("renders fallback and recovers on retry", () => {
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    let shouldThrow = true;

    const MaybeBoom = () => {
      if (shouldThrow) throw new Error("render boom");
      return <div>Recovered</div>;
    };

    render(
      <ErrorBoundary viewName="Test view">
        <MaybeBoom />
      </ErrorBoundary>,
    );

    expect(screen.getByRole("alert")).toHaveTextContent("Test view crashed");
    expect(screen.getByText(/render boom/)).toBeInTheDocument();

    shouldThrow = false;
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(screen.getByText("Recovered")).toBeInTheDocument();

    consoleError.mockRestore();
  });
});
