import { useEffect, useId, useRef, useState } from "react";
import "./RerunModal.css";

interface Props {
  workflowName: string;
  initialJson: string;
  busy: boolean;
  error: string | null;
  onCancel: () => void;
  onSubmit: (inputJson: string) => void;
}

export default function RerunModal({
  workflowName,
  initialJson,
  busy,
  error,
  onCancel,
  onSubmit,
}: Props) {
  const titleId = useId();
  const descId = useId();
  const [value, setValue] = useState(initialJson);
  const [parseError, setParseError] = useState<string | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    textareaRef.current?.focus();
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !busy) onCancel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [busy, onCancel]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    setParseError(null);
    try {
      JSON.parse(value || "{}");
    } catch (err) {
      setParseError(`Input override must be valid JSON: ${err}`);
      return;
    }
    onSubmit(value || "{}");
  };

  return (
    <div className="rerun-modal-backdrop">
      <button
        type="button"
        className="rerun-modal-scrim"
        aria-label="Close dialog"
        disabled={busy}
        onClick={onCancel}
      />
      <div
        className="rerun-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={descId}
      >
        <h2 id={titleId} className="rerun-modal-title">
          Rerun {workflowName}
        </h2>
        <p id={descId} className="rerun-modal-desc">
          Optional JSON input override for this rerun. Leave <code>{"{}"}</code>{" "}
          to reuse the original run input.
        </p>
        <form onSubmit={handleSubmit}>
          <label className="rerun-modal-label" htmlFor="rerun-input-json">
            Input override (JSON)
          </label>
          <textarea
            id="rerun-input-json"
            ref={textareaRef}
            className="rerun-modal-textarea"
            value={value}
            disabled={busy}
            rows={8}
            spellCheck={false}
            onChange={(e) => setValue(e.target.value)}
          />
          {(parseError || error) && (
            <div className="rerun-modal-error" role="alert">
              {parseError ?? error}
            </div>
          )}
          <div className="rerun-modal-actions">
            <button
              type="button"
              className="btn btn-ghost"
              disabled={busy}
              onClick={onCancel}
            >
              Cancel
            </button>
            <button type="submit" className="btn btn-primary" disabled={busy}>
              {busy ? "Rerunning…" : "Rerun"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
