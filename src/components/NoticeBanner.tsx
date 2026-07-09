import Button from "./Button";
import "./NoticeBanner.css";

export type NoticeTone = "info" | "success" | "error";

interface Props {
  message: string;
  tone?: NoticeTone;
  onDismiss?: () => void;
}

export default function NoticeBanner({
  message,
  tone = "info",
  onDismiss,
}: Props) {
  return (
    <div
      className={`notice-banner notice-banner--${tone}`}
      role={tone === "error" ? "alert" : "status"}
    >
      <span className="notice-banner__message">{message}</span>
      {onDismiss && (
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="notice-banner__dismiss"
          onClick={onDismiss}
          aria-label="Dismiss notice"
        >
          Dismiss
        </Button>
      )}
    </div>
  );
}
