export interface PageHeaderProps {
  /** Heading rendered in the `.page-title` h1. */
  title: React.ReactNode;
  /** Optional caption rendered in a `.page-subtitle` p; omitted entirely when falsy. */
  subtitle?: React.ReactNode;
  /** Optional right-side content (buttons/action group) rendered after the title block. */
  actions?: React.ReactNode;
  /** Optional extra class(es) merged onto the `.page-header` container. */
  className?: string;
}

/**
 * Shared page-header primitive. A typed extraction of the repeated
 * `.page-header > div > (h1.page-title + p.page-subtitle?)` block (see the
 * per-surface CSS / DESIGN-SYSTEM.md) — renders the exact same DOM the call
 * sites used before, so styling is byte-identical. `subtitle`/`actions` are
 * optional and, when omitted, emit no extra DOM (matching the inline sites).
 */
export default function PageHeader({
  title,
  subtitle,
  actions,
  className,
}: PageHeaderProps) {
  const classes = ["page-header", className].filter(Boolean).join(" ");
  return (
    <div className={classes}>
      <div>
        <h1 className="page-title">{title}</h1>
        {subtitle ? <p className="page-subtitle">{subtitle}</p> : null}
      </div>
      {actions}
    </div>
  );
}
