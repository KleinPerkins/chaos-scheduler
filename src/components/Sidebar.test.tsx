import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import { Gauge, Workflow as WorkflowIcon } from "lucide-react";
import Sidebar, { type SidebarNavItem, type SidebarProps } from "./Sidebar";
import { PRODUCT_SHORT_NAME, APP_VERSION } from "../lib/branding";

afterEach(cleanup);

type TestView = "mission" | "workflows" | "editor";

const NAV_ITEMS: SidebarNavItem<TestView>[] = [
  { view: "mission", label: "Home", Icon: Gauge, match: ["mission"] },
  {
    view: "workflows",
    label: "Workflows",
    Icon: WorkflowIcon,
    // "editor" is a sub-view that keeps "Workflows" highlighted, mirroring the
    // real WORKFLOW_VIEWS grouping in Dashboard.
    match: ["workflows", "editor"],
  },
];

function renderSidebar(props: Partial<SidebarProps<TestView>> = {}) {
  return render(
    <Sidebar
      navItems={NAV_ITEMS}
      currentView="mission"
      onNavigate={() => {}}
      themePreference="dark"
      onThemeChange={() => {}}
      {...props}
    />,
  );
}

function sidebarOf(container: HTMLElement): HTMLElement {
  return container.firstChild as HTMLElement;
}

describe("Sidebar", () => {
  it("renders the `<aside.dashboard-sidebar>` container with brand, primary nav, and footer", () => {
    const { container } = renderSidebar();
    const aside = sidebarOf(container);
    expect(aside.tagName).toBe("ASIDE");
    expect(aside.className).toBe("dashboard-sidebar");

    // Brand header: aria-hidden icon slot + product short name.
    const brand = aside.querySelector(":scope > .sidebar-brand");
    expect(brand).not.toBeNull();
    const brandIcon = brand!.querySelector(".brand-icon");
    expect(brandIcon).toHaveAttribute("aria-hidden", "true");
    expect(brand!.querySelector(".brand-text")?.textContent).toBe(
      PRODUCT_SHORT_NAME,
    );

    // Primary navigation landmark.
    const nav = aside.querySelector(":scope > nav.sidebar-nav");
    expect(nav).not.toBeNull();
    expect(nav).toHaveAttribute("aria-label", "Primary navigation");

    // Footer: theme toggle (a labelled radio-like group) + version.
    const footer = aside.querySelector(":scope > .sidebar-footer");
    expect(footer).not.toBeNull();
    expect(footer!.querySelector('[role="group"]')).not.toBeNull();
    expect(footer!.querySelector(".sidebar-version")?.textContent).toBe(
      `v${APP_VERSION}`,
    );
  });

  it("renders one NavItem button per nav item, in order, with icon + label", () => {
    const { container } = renderSidebar();
    const links = container.querySelectorAll(
      "nav.sidebar-nav > button.sidebar-link",
    );
    expect(links).toHaveLength(NAV_ITEMS.length);
    expect(links[0].textContent).toBe("Home");
    expect(links[1].textContent).toBe("Workflows");
    // Each NavItem keeps its aria-hidden `.sidebar-icon` slot.
    expect(links[0].querySelector(".sidebar-icon")).toHaveAttribute(
      "aria-hidden",
      "true",
    );
  });

  it("marks only the nav item whose `match` includes the current view as active", () => {
    const { container } = renderSidebar({ currentView: "workflows" });
    const links = container.querySelectorAll("button.sidebar-link");
    // Byte-identical to the original inactive markup (trailing space, no
    // aria-current) vs. active markup.
    expect(links[0].className).toBe("sidebar-link ");
    expect(links[0]).not.toHaveAttribute("aria-current");
    expect(links[1].className).toBe("sidebar-link active");
    expect(links[1]).toHaveAttribute("aria-current", "page");
  });

  it("uses the item's `match` list (not exact view equality) for active state", () => {
    // "editor" is not the "workflows" item's own view, but is in its match list.
    const { container } = renderSidebar({ currentView: "editor" });
    const links = container.querySelectorAll("button.sidebar-link");
    expect(links[0].className).toBe("sidebar-link ");
    expect(links[1].className).toBe("sidebar-link active");
  });

  it("calls onNavigate with the item's view when its NavItem is clicked", () => {
    const onNavigate = vi.fn();
    const { container } = renderSidebar({ onNavigate });
    const links = container.querySelectorAll("button.sidebar-link");
    fireEvent.click(links[1]);
    expect(onNavigate).toHaveBeenCalledTimes(1);
    expect(onNavigate).toHaveBeenCalledWith("workflows");
  });

  it("forwards the theme preference and change handler to the footer ThemeToggle", () => {
    const onThemeChange = vi.fn();
    const { container } = renderSidebar({
      themePreference: "light",
      onThemeChange,
    });
    const pressed = container.querySelector(
      '.theme-toggle-option[aria-pressed="true"]',
    );
    expect(pressed?.getAttribute("title")).toBe("Light theme");

    const darkOption = Array.from(
      container.querySelectorAll(".theme-toggle-option"),
    ).find((el) => el.getAttribute("title") === "Dark theme");
    fireEvent.click(darkOption as HTMLElement);
    expect(onThemeChange).toHaveBeenCalledWith("dark");
  });

  it("toggles between expanded and collapsed layouts without hiding navigation names", () => {
    const { container, getByRole } = renderSidebar();
    const aside = sidebarOf(container);
    const toggle = getByRole("button", { name: "Collapse sidebar" });

    expect(aside).not.toHaveClass("is-collapsed");
    expect(toggle).toHaveAttribute("aria-expanded", "true");

    fireEvent.click(toggle);

    expect(aside).toHaveClass("is-collapsed");
    expect(getByRole("button", { name: "Expand sidebar" })).toHaveAttribute(
      "aria-expanded",
      "false",
    );
    expect(getByRole("button", { name: "Home" })).toBeInTheDocument();
    expect(getByRole("button", { name: "Workflows" })).toBeInTheDocument();
  });

  it("supports a controlled collapsed layout and reports toggle intent", () => {
    const onCollapsedChange = vi.fn();
    const { container, getByRole } = renderSidebar({
      collapsed: true,
      onCollapsedChange,
    });

    expect(sidebarOf(container)).toHaveClass("is-collapsed");
    fireEvent.click(getByRole("button", { name: "Expand sidebar" }));
    expect(onCollapsedChange).toHaveBeenCalledWith(false);
    expect(sidebarOf(container)).toHaveClass("is-collapsed");
  });
});
