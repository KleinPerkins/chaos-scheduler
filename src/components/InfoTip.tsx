import { useEffect, useId, useRef, useState } from "react";
import "./InfoTip.css";

/** One row of the optional glossary table (a term and its meaning). */
export interface GlossaryRow {
  term: string;
  meaning: string;
}

export interface InfoTipProps {
  /** Bold heading shown at the top of the tooltip; also the trigger's a11y name. */
  title: string;
  /** One-line definition of the metric or term. */
  def: string;
  /**
   * Render the glossary table beneath the definition. Maps to the Figma
   * `Glossary` boolean; the section only draws when `glossaryRows` is non-empty.
   */
  glossary?: boolean;
  /** Rows for the glossary table (shown when `glossary` is true). */
  glossaryRows?: GlossaryRow[];
  /** Extra class(es) merged onto the `.info-tip` container. */
  className?: string;
}

/**
 * Hover/focus info affordance matching the Figma `InfoTip` master
 * (node 115:531): a small circular `i` badge that reveals a definition card
 * (bold title + one-line definition, plus an optional glossary table). The
 * reveal is CSS-driven — the Figma `Rest` / `Hover` variants are a pure
 * `:hover` presentation state (mirrored here with `:focus-within` so keyboard
 * users get the same affordance), never a click modal — matching the documented
 * hover-only InfoTip convention. The trigger is a real focusable `<button>`
 * described by the card via `aria-describedby`, and pressing `Escape` while the
 * card is open dismisses it without moving focus (the WAI-ARIA tooltip
 * pattern). Because a pointer-opened tip never focuses the trigger, a
 * document-level `keydown` listener handles Escape for hover- and
 * keyboard-opened tips alike; the dismissed state re-arms once the pointer and
 * focus both leave. All colors/type bind to repo tokens — no raw hex. */
export default function InfoTip({
  title,
  def,
  glossary = false,
  glossaryRows = [],
  className,
}: InfoTipProps) {
  const defId = useId();
  const triggerRef = useRef<HTMLButtonElement>(null);
  const hoveredRef = useRef(false);
  const [dismissed, setDismissed] = useState(false);
  const dismissedRef = useRef(dismissed);

  useEffect(() => {
    dismissedRef.current = dismissed;
  }, [dismissed]);

  // Dismiss on Escape regardless of how the card opened. A hover-opened tip
  // leaves the trigger unfocused, so a trigger-scoped handler would miss it; a
  // document-level listener (attached once, reading the live hover/focus state)
  // covers pointer and keyboard opens. Escape is only swallowed while the card
  // is actually open, so it still propagates (e.g. to a parent) otherwise.
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || dismissedRef.current) return;
      const open =
        hoveredRef.current || document.activeElement === triggerRef.current;
      if (!open) return;
      e.stopPropagation();
      setDismissed(true);
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, []);

  const classes = ["info-tip", dismissed ? "is-dismissed" : null, className]
    .filter(Boolean)
    .join(" ");
  const showGlossary = glossary && glossaryRows.length > 0;

  return (
    <span
      className={classes}
      onMouseEnter={() => {
        hoveredRef.current = true;
      }}
      onMouseLeave={() => {
        hoveredRef.current = false;
        // Re-arm for the next hover, unless the trigger is still focused.
        if (document.activeElement !== triggerRef.current) setDismissed(false);
      }}
    >
      <button
        ref={triggerRef}
        type="button"
        className="info-tip-trigger"
        aria-label={title}
        aria-describedby={defId}
        onBlur={() => {
          // Re-arm once focus leaves, unless the pointer is still hovering.
          if (!hoveredRef.current) setDismissed(false);
        }}
      >
        <span aria-hidden="true">i</span>
      </button>
      <span className="info-tip-card" role="tooltip">
        <span className="info-tip-title">{title}</span>
        <span className="info-tip-def" id={defId}>
          {def}
        </span>
        {showGlossary ? (
          <span className="info-tip-glossary">
            <span className="info-tip-g-row info-tip-g-head">
              <span className="info-tip-g-term">Term</span>
              <span className="info-tip-g-meaning">Meaning</span>
            </span>
            {glossaryRows.map((row, i) => (
              <span key={`${row.term}-${i}`} className="info-tip-g-row">
                <span className="info-tip-g-term">{row.term}</span>
                <span className="info-tip-g-meaning">{row.meaning}</span>
              </span>
            ))}
          </span>
        ) : null}
      </span>
    </span>
  );
}
