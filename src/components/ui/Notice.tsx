import Button from "../Button";
import "./Notice.css";

export type NoticeVariant = "info" | "success" | "error" | "warning";

interface NoticeProps {
  variant?: NoticeVariant;
  children: React.ReactNode;
  /** When true, uses role="alert" (errors). Otherwise role="status". */
  assertive?: boolean;
  className?: string;
  onDismiss?: () => void;
}

export default function Notice({
  variant = "info",
  children,
  assertive,
  className = "",
  onDismiss,
}: NoticeProps) {
  const isError = variant === "error" || assertive;
  return (
    <div
      className={`ui-notice ui-notice--${variant} ${className}`.trim()}
      role={isError ? "alert" : "status"}
    >
      <span className="ui-notice__body">{children}</span>
      {onDismiss && (
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="ui-notice__dismiss"
          onClick={onDismiss}
          aria-label="Dismiss"
        >
          ×
        </Button>
      )}
    </div>
  );
}
