import { useEffect, useId, useRef, useState } from "react";
import Button from "./Button";
import Modal from "./Modal";
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
    <Modal
      onClose={onCancel}
      closeDisabled={busy}
      labelledBy={titleId}
      describedBy={descId}
      backdropClassName="rerun-modal-backdrop"
      scrimClassName="rerun-modal-scrim"
      className="rerun-modal"
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
          <Button
            type="button"
            variant="ghost"
            disabled={busy}
            onClick={onCancel}
          >
            Cancel
          </Button>
          <Button type="submit" variant="primary" disabled={busy}>
            {busy ? "Rerunning…" : "Rerun"}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
