import { CalendarClock, type LucideIcon } from "lucide-react";
import NavItem from "./NavItem";
import ThemeToggle from "./ThemeToggle";
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
}

/**
 * Dashboard sidebar navigation container. A typed, reusable extraction of the
 * sidebar block that previously lived inline in `Dashboard.tsx` — it renders
 * the exact same DOM (the `.dashboard-sidebar` aside, the `.sidebar-brand`
 * header, the `.sidebar-nav` list of `NavItem`s, and the `.sidebar-footer` with
 * the `ThemeToggle` + version), reusing the global classes in `Dashboard.css`,
 * so behavior and styling are byte-identical. Active state is derived from each
 * item's `match` list exactly as before (`item.match.includes(currentView)`)
 * and selecting an item calls `onNavigate(item.view)`.
 */
export default function Sidebar<V extends string = string>({
  navItems,
  currentView,
  onNavigate,
  themePreference,
  onThemeChange,
}: SidebarProps<V>) {
  return (
    <aside className="dashboard-sidebar">
      <div className="sidebar-brand">
        <span className="brand-icon" aria-hidden="true">
          <CalendarClock size={18} strokeWidth={2.25} />
        </span>
        <span className="brand-text">{PRODUCT_SHORT_NAME}</span>
      </div>
      <nav className="sidebar-nav" aria-label="Primary navigation">
        {navItems.map((item) => (
          <NavItem
            key={item.view}
            active={item.match.includes(currentView)}
            icon={<item.Icon size={16} strokeWidth={2} />}
            label={item.label}
            onClick={() => onNavigate(item.view)}
          />
        ))}
      </nav>
      <div className="sidebar-footer">
        <ThemeToggle preference={themePreference} onChange={onThemeChange} />
        <span className="sidebar-version">v{APP_VERSION}</span>
      </div>
    </aside>
  );
}
