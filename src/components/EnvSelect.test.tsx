import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import type { Environment } from "../lib/commands";
import EnvSelect from "./EnvSelect";

afterEach(cleanup);

const ENVIRONMENTS: Environment[] = [
  { id: "env-prod", name: "production" },
  { id: "env-sandbox", name: "sandbox" },
];

describe("EnvSelect", () => {
  it("composes a native class-less <select> (byte-identical to a raw <select>)", () => {
    const { container } = render(
      <EnvSelect
        value="production"
        onChange={() => {}}
        environments={ENVIRONMENTS}
      />,
    );
    const select = container.firstChild as HTMLElement;
    expect(select.tagName).toBe("SELECT");
    // Inherits the global `input, select, textarea` styling via <Select>, so it
    // must NOT emit a `class` attribute (not even `class=""`) — keeping the
    // rendered DOM byte-identical to the previous raw `<select>` call sites.
    expect(select.hasAttribute("class")).toBe(false);
  });

  it("renders one capitalized <option> per environment (value = raw name)", () => {
    const { container } = render(
      <EnvSelect
        value="production"
        onChange={() => {}}
        environments={ENVIRONMENTS}
      />,
    );
    const options = container.querySelectorAll("option");
    expect(options).toHaveLength(2);
    expect(options[0]).toHaveValue("production");
    expect(options[0]).toHaveTextContent("Production");
    expect(options[1]).toHaveValue("sandbox");
    expect(options[1]).toHaveTextContent("Sandbox");
  });

  it("prepends an `All` sentinel option only when includeAllOption is set", () => {
    const { container, rerender } = render(
      <EnvSelect
        value="all"
        onChange={() => {}}
        environments={ENVIRONMENTS}
        includeAllOption
      />,
    );
    let options = container.querySelectorAll("option");
    expect(options).toHaveLength(3);
    expect(options[0]).toHaveValue("all");
    expect(options[0]).toHaveTextContent("All");
    expect(options[1]).toHaveValue("production");

    rerender(
      <EnvSelect
        value="production"
        onChange={() => {}}
        environments={ENVIRONMENTS}
      />,
    );
    options = container.querySelectorAll("option");
    expect(options).toHaveLength(2);
    expect(options[0]).toHaveValue("production");
  });

  it("reflects the current value", () => {
    const { container } = render(
      <EnvSelect
        value="sandbox"
        onChange={() => {}}
        environments={ENVIRONMENTS}
      />,
    );
    expect((container.firstChild as HTMLSelectElement).value).toBe("sandbox");
  });

  it("fires onChange and reflects the newly selected environment", () => {
    const onChange = vi.fn();
    const { container } = render(
      <EnvSelect
        defaultValue="production"
        onChange={onChange}
        environments={ENVIRONMENTS}
      />,
    );
    const select = container.firstChild as HTMLSelectElement;
    fireEvent.change(select, { target: { value: "sandbox" } });
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(select.value).toBe("sandbox");
  });

  it("forwards native select props (id, disabled, aria-label)", () => {
    const { container } = render(
      <EnvSelect
        id="wf-env"
        aria-label="Environment"
        disabled
        value="production"
        onChange={() => {}}
        environments={ENVIRONMENTS}
      />,
    );
    const select = container.firstChild as HTMLSelectElement;
    expect(select).toHaveAttribute("id", "wf-env");
    expect(select).toHaveAttribute("aria-label", "Environment");
    expect(select).toBeDisabled();
  });

  it("renders only the `All` sentinel for an empty environment list", () => {
    const { container } = render(
      <EnvSelect
        value="all"
        onChange={() => {}}
        environments={[]}
        includeAllOption
      />,
    );
    const options = container.querySelectorAll("option");
    expect(options).toHaveLength(1);
    expect(options[0]).toHaveValue("all");
  });
});
