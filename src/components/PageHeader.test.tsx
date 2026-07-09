import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/react";
import PageHeader from "./PageHeader";

afterEach(cleanup);

function headerOf(container: HTMLElement): HTMLElement {
  return container.firstChild as HTMLElement;
}

describe("PageHeader", () => {
  it("renders a `.page-header` wrapping a title block with an `<h1.page-title>`", () => {
    const { container } = render(<PageHeader title="Workflows" />);
    const header = headerOf(container);
    expect(header.tagName).toBe("DIV");
    expect(header.className).toBe("page-header");

    // The h1 lives inside the title-block wrapper div (first child).
    const titleBlock = header.firstElementChild as HTMLElement;
    expect(titleBlock.tagName).toBe("DIV");
    const heading = titleBlock.firstElementChild as HTMLElement;
    expect(heading.tagName).toBe("H1");
    expect(heading.className).toBe("page-title");
    expect(heading.textContent).toBe("Workflows");
  });

  it("omits the `.page-subtitle` entirely when no subtitle is provided", () => {
    const { container } = render(<PageHeader title="Settings" />);
    expect(headerOf(container).querySelector(".page-subtitle")).toBeNull();
  });

  it("renders a `<p.page-subtitle>` after the title when a subtitle is provided", () => {
    const { container } = render(
      <PageHeader title="Settings" subtitle="Configure the scheduler" />,
    );
    const titleBlock = headerOf(container).firstElementChild as HTMLElement;
    expect(titleBlock.children[0].className).toBe("page-title");

    const subtitle = titleBlock.children[1] as HTMLElement;
    expect(subtitle.tagName).toBe("P");
    expect(subtitle.className).toBe("page-subtitle");
    expect(subtitle.textContent).toBe("Configure the scheduler");
  });

  it("renders `actions` as a sibling after the title block", () => {
    const { container } = render(
      <PageHeader
        title="Workflows"
        actions={<button data-testid="action">+ Add</button>}
      />,
    );
    const header = headerOf(container);
    expect(header.children).toHaveLength(2);
    const action = header.children[1] as HTMLElement;
    expect(action.tagName).toBe("BUTTON");
    expect(action).toHaveAttribute("data-testid", "action");
  });

  it("merges a passthrough className after the base `page-header` class", () => {
    const { container } = render(<PageHeader title="x" className="extra" />);
    expect(headerOf(container).className).toBe("page-header extra");
  });
});
