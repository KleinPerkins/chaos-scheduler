import { useId, useState } from "react";
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
 * described by the card via `aria-describedby`, and pressing `Escape` while it
 * is focused dismisses the card without moving focus (the WAI-ARIA tooltip
 * pattern); the dismissed state re-arms once focus leaves the trigger. All
 * colors/type bind to repo tokens — no raw hex. Not yet wired into any screen.
 */
export default function InfoTip({
  title,
  def,
  glossary = false,
  glossaryRows = [],
  className,
}: InfoTipProps) {
  const defId = useId();
  const [dismissed, setDismissed] = useState(false);
  const classes = ["info-tip", dismissed ? "is-dismissed" : null, className]
    .filter(Boolean)
    .join(" ");
  const showGlossary = glossary && glossaryRows.length > 0;

  return (
    <span className={classes}>
      <button
        type="button"
        className="info-tip-trigger"
        aria-label={title}
        aria-describedby={defId}
        onKeyDown={(e) => {
          if (e.key === "Escape" && !dismissed) {
            e.stopPropagation();
            setDismissed(true);
          }
        }}
        onBlur={() => setDismissed(false)}
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
