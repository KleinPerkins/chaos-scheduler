import { useEffect, useRef } from "react";

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

const FOCUSABLE_SELECTOR = [
  "a[href]",
  "button:not([disabled])",
  "textarea:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  '[tabindex]:not([tabindex="-1"])',
].join(",");

/** Tabbable descendants of `container`, in DOM order (excludes disabled/hidden). */
function getFocusable(container: HTMLElement): HTMLElement[] {
  return Array.from(
    container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR),
  ).filter((el) => !el.hasAttribute("disabled") && !el.hasAttribute("hidden"));
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
 * `RerunModal.css`) and are passed in, so its class emission stays
 * byte-identical to the hand-written wrapper it replaces (when a `*ClassName`
 * is omitted the element emits no `class` attribute, not even `class=""`); the
 * dialog additionally carries a non-visual `tabIndex={-1}` for focus management.
 *
 * Owns modal FOCUS MANAGEMENT: on open it moves focus into the dialog; it TRAPS
 * Tab / Shift+Tab within the dialog (so focus cannot escape to the background
 * that `aria-modal="true"` claims inert); and on close (unmount) it restores
 * focus to the element that was focused before open (the trigger). Consumers
 * may still autofocus a specific field in their own effect — it runs after this
 * one and harmlessly wins.
 *
 * Deliberately NOT included (the current modals do not need them): React portal
 * and body-scroll-lock. The dialog has no click handler — close-by-click is the
 * explicit scrim button, a sibling of the dialog, so clicks inside the dialog
 * never reach it (no `stopPropagation` needed). Consumers control mount/unmount
 * themselves — there is no `isOpen` prop, matching the `{cond && <Modal …/>}`
 * render idiom.
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
  const dialogRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !closeDisabled) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [closeDisabled, onClose]);

  // Move focus INTO the dialog on open, and restore it to the element that had
  // focus before open (the trigger) when the dialog unmounts (close). A
  // consumer that autofocuses a specific field does so in its own effect, which
  // runs after this one and harmlessly overrides the initial target chosen here.
  useEffect(() => {
    const previouslyFocused = document.activeElement as HTMLElement | null;
    const dialog = dialogRef.current;
    if (dialog && !dialog.contains(document.activeElement)) {
      const focusables = getFocusable(dialog);
      (focusables[0] ?? dialog).focus();
    }
    return () => {
      if (
        previouslyFocused &&
        typeof previouslyFocused.focus === "function" &&
        document.contains(previouslyFocused)
      ) {
        previouslyFocused.focus();
      }
    };
  }, []);

  // Trap Tab / Shift+Tab within the dialog so keyboard focus cannot leave it
  // for the background that `aria-modal="true"` claims inert.
  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== "Tab") return;
      const focusables = getFocusable(dialog);
      if (focusables.length === 0) {
        e.preventDefault();
        dialog.focus();
        return;
      }
      const first = focusables[0];
      const last = focusables[focusables.length - 1];
      const active = document.activeElement;
      if (e.shiftKey) {
        if (active === first || !dialog.contains(active)) {
          e.preventDefault();
          last.focus();
        }
      } else if (active === last || !dialog.contains(active)) {
        e.preventDefault();
        first.focus();
      }
    };
    dialog.addEventListener("keydown", onKeyDown);
    return () => dialog.removeEventListener("keydown", onKeyDown);
  }, []);

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
        ref={dialogRef}
        className={dialogClasses}
        role="dialog"
        aria-modal="true"
        aria-labelledby={labelledBy}
        aria-describedby={describedBy}
        tabIndex={-1}
      >
        {children}
      </div>
    </div>
  );
}
