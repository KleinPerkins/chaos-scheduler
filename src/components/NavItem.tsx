export interface NavItemProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  /** Icon element rendered inside the aria-hidden `.sidebar-icon` slot. */
  icon: React.ReactNode;
  /** Text label rendered after the icon. */
  label: React.ReactNode;
  /** Highlighted state: applies the `.active` modifier and `aria-current="page"`. */
  active?: boolean;
}

/**
 * Shared sidebar navigation-item primitive. A thin, typed wrapper over the
 * global `.sidebar-link` / `.sidebar-icon` classes (see `Dashboard.css` /
 * DESIGN-SYSTEM.md) — it renders the exact same markup the sidebar used before,
 * so behavior and styling are unchanged. `active` toggles the `.active`
 * modifier class and `aria-current="page"`, byte-identically to the original
 * `` `sidebar-link ${active ? "active" : ""}` `` markup (the inactive class
 * string keeps its trailing space). Building block for the Sidebar component.
 */
export default function NavItem({
  icon,
  label,
  active = false,
  className,
  ...rest
}: NavItemProps) {
  const classes = [`sidebar-link ${active ? "active" : ""}`, className]
    .filter(Boolean)
    .join(" ");

  return (
    <button
      {...rest}
      className={classes}
      aria-current={active ? "page" : undefined}
    >
      <span className="sidebar-icon" aria-hidden="true">
        {icon}
      </span>
      {label}
    </button>
  );
}
