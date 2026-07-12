import { useState } from "react";
import { ChevronsLeft, ChevronsRight, type LucideIcon } from "lucide-react";
import NavItem from "./NavItem";
import ThemeToggle from "./ThemeToggle";
import BrandMark from "./BrandMark";
import type { ThemePreference } from "../lib/theme";
import { PRODUCT_SHORT_NAME, APP_VERSION } from "../lib/branding";

/**
 * A single primary-navigation entry. `match` is the set of routing views that
 * keep this entry highlighted — e.g. the Workflows entry stays active across
 * its editor / per-workflow-history / run-detail sub-views.
 */
export interface SidebarNavItem<V extends string = string> {
  /** The view this entry navigates to when selected. */
  view: V;
  /** Text label rendered after the icon. */
  label: string;
  /** Lucide icon component rendered in the aria-hidden `.sidebar-icon` slot. */
  Icon: LucideIcon;
  /** Views for which this entry renders as active. */
  match: V[];
}

export interface SidebarProps<V extends string = string> {
  /** Primary navigation entries, rendered in order as `NavItem`s. */
  navItems: SidebarNavItem<V>[];
  /** Active view; an entry is active when its `match` list includes it. */
  currentView: V;
  /** Called with an entry's `view` when that entry is selected. */
  onNavigate: (view: V) => void;
  /** Current color-theme preference, forwarded to the footer `ThemeToggle`. */
  themePreference: ThemePreference;
  /** Theme-change handler, forwarded to the footer `ThemeToggle`. */
  onThemeChange: (preference: ThemePreference) => void;
  /** Controlled collapsed state. Omit to let the Sidebar manage its own state. */
  collapsed?: boolean;
  /** Reports collapse-toggle intent in both controlled and uncontrolled modes. */
  onCollapsedChange?: (collapsed: boolean) => void;
}

/**
 * Dashboard navigation matching the Figma Sidebar master (node 305:6378).
 * Expanded mode includes the brand, named navigation items, theme controls,
 * version, and collapse affordance. Collapsed mode preserves every navigation
 * button's accessible name while reducing the rail to icon-only presentation.
 * It can be controlled by a caller or manage its own toggle state.
 */
export default function Sidebar<V extends string = string>({
  navItems,
  currentView,
  onNavigate,
  themePreference,
  onThemeChange,
  collapsed,
  onCollapsedChange,
}: SidebarProps<V>) {
  const [internalCollapsed, setInternalCollapsed] = useState(false);
  const isCollapsed = collapsed ?? internalCollapsed;

  const toggleCollapsed = () => {
    const next = !isCollapsed;
    if (collapsed === undefined) setInternalCollapsed(next);
    onCollapsedChange?.(next);
  };

  return (
    <aside className={`dashboard-sidebar${isCollapsed ? " is-collapsed" : ""}`}>
      <div className="sidebar-brand">
        <span className="brand-icon" aria-hidden="true">
          <BrandMark size={20} title="" className="sidebar-brand-mark" />
        </span>
        <span className="brand-text">{PRODUCT_SHORT_NAME}</span>
      </div>
      <nav className="sidebar-nav" aria-label="Primary navigation">
        {navItems.map((item) => (
          <NavItem
            key={item.view}
            active={item.match.includes(currentView)}
            icon={<item.Icon size={16} strokeWidth={2} />}
            label={<span className="sidebar-link-label">{item.label}</span>}
            aria-label={item.label}
            title={isCollapsed ? item.label : undefined}
            onClick={() => onNavigate(item.view)}
          />
        ))}
      </nav>
      <div className="sidebar-footer">
        <div className="sidebar-footer-main">
          <ThemeToggle preference={themePreference} onChange={onThemeChange} />
          <span className="sidebar-version">v{APP_VERSION}</span>
        </div>
        <button
          type="button"
          className="sidebar-collapse-toggle"
          aria-label={isCollapsed ? "Expand sidebar" : "Collapse sidebar"}
          aria-expanded={!isCollapsed}
          title={isCollapsed ? "Expand sidebar" : "Collapse sidebar"}
          onClick={toggleCollapsed}
        >
          {isCollapsed ? (
            <ChevronsRight size={15} aria-hidden="true" />
          ) : (
            <ChevronsLeft size={15} aria-hidden="true" />
          )}
        </button>
      </div>
    </aside>
  );
}
