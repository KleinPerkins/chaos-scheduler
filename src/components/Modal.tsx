import { useEffect } from "react";

export interface ModalProps {
  /** Called when the shell requests a close (scrim click or Escape). */
  onClose: () => void;
  /**
   * When true the scrim close-button is `disabled` and the Escape key is
   * ignored (mirrors RerunModal's `busy` guard — a modal that is mid-submit
   * must not be dismissable).
   */
  closeDisabled?: boolean;
  /** `id` of the element that labels the dialog (`aria-labelledby`). */
  labelledBy?: string;
  /** `id` of the element that describes the dialog (`aria-describedby`). */
  describedBy?: string;
  /** Class(es) applied to the dialog container (`role="dialog"`). */
  className?: string;
  /** Class(es) applied to the fixed backdrop wrapper. */
  backdropClassName?: string;
  /** Class(es) applied to the full-bleed scrim close `<button>`. */
  scrimClassName?: string;
  /** Accessible label for the scrim close-button. */
  scrimLabel?: string;
  /** Dialog body. */
  children: React.ReactNode;
}

/**
 * Shared modal / dialog SHELL primitive. Renders the exact overlay skeleton the
 * Chaos Scheduler modals use — a fixed backdrop wrapper containing a full-bleed
 * scrim close-`<button>` (the click-to-close affordance) followed by the
 * `role="dialog"` container — and owns the shell BEHAVIOR: scrim-click and
 * Escape both call `onClose` (both suppressed while `closeDisabled`).
 *
 * It is intentionally STRUCTURE + BEHAVIOR only and CLASS-LESS by default: the
 * scrim/backdrop/dialog classes live in each consumer's own CSS (e.g.
 * `RerunModal.css`) and are passed in, so this stays BYTE-IDENTICAL to the
 * hand-written wrapper it replaces (when a `*ClassName` is omitted the element
 * emits no `class` attribute, not even `class=""`).
 *
 * Deliberately NOT included (the current modals do not use them, so adding them
 * would change behavior): React portal, body-scroll-lock, and focus-trap. The
 * dialog has no click handler — close-by-click is the explicit scrim button, a
 * sibling of the dialog, so clicks inside the dialog never reach it (no
 * `stopPropagation` needed). Consumers keep their own content-specific effects
 * (e.g. autofocusing a field) and control mount/unmount themselves — there is
 * no `isOpen` prop, matching the `{cond && <Modal …/>}` render idiom.
 */
export default function Modal({
  onClose,
  closeDisabled = false,
  labelledBy,
  describedBy,
  className,
  backdropClassName,
  scrimClassName,
  scrimLabel = "Close dialog",
  children,
}: ModalProps) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !closeDisabled) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [closeDisabled, onClose]);

  const backdropClasses =
    [backdropClassName].filter(Boolean).join(" ") || undefined;
  const scrimClasses = [scrimClassName].filter(Boolean).join(" ") || undefined;
  const dialogClasses = [className].filter(Boolean).join(" ") || undefined;

  return (
    <div className={backdropClasses}>
      <button
        type="button"
        className={scrimClasses}
        aria-label={scrimLabel}
        disabled={closeDisabled}
        onClick={onClose}
      />
      <div
        className={dialogClasses}
        role="dialog"
        aria-modal="true"
        aria-labelledby={labelledBy}
        aria-describedby={describedBy}
      >
        {children}
      </div>
    </div>
  );
}
