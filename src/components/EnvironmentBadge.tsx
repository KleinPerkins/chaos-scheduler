import "./EnvironmentBadge.css";

interface Props {
  environment: string;
  /** Render a lock glyph indicating the definition is externally managed. */
  managed?: boolean;
  size?: "sm" | "md";
  title?: string;
}

// Deterministic hue from the environment name so any user-defined environment
// gets a stable, distinct color without a hardcoded source/instance mapping.
function hueFor(name: string): number {
  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = (hash * 31 + name.charCodeAt(i)) % 360;
  }
  return hash;
}

function labelFor(name: string): string {
  if (!name) return "default";
  return name.charAt(0).toUpperCase() + name.slice(1);
}

/**
 * Dynamic environment pill. Replaces the former hardcoded source/instance
 * corpus badge; colors are derived from the environment name so newly created
 * environments render distinctly with no code change.
 */
export default function EnvironmentBadge({
  environment,
  managed,
  size = "md",
  title,
}: Props) {
  const hue = hueFor(environment);
  const style = {
    "--env-hue": String(hue),
  } as React.CSSProperties;
  return (
    <span
      className={`env-badge env-badge--${size}${managed ? " env-badge--managed" : ""}`}
      style={style}
      title={
        title ??
        (managed
          ? `${labelFor(environment)} · managed externally`
          : labelFor(environment))
      }
    >
      {managed && (
        <span className="env-badge-lock" aria-hidden="true">
          &#128274;
        </span>
      )}
      {labelFor(environment)}
      {managed && <span className="env-badge-sr">, managed externally</span>}
    </span>
  );
}
