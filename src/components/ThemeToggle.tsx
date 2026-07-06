import { Sun, Moon, Monitor } from "lucide-react";
import type { ThemePreference } from "../lib/theme";
import "./ThemeToggle.css";

interface Props {
  preference: ThemePreference;
  onChange: (preference: ThemePreference) => void;
}

const OPTIONS: {
  value: ThemePreference;
  label: string;
  Icon: typeof Sun;
}[] = [
  { value: "light", label: "Light", Icon: Sun },
  { value: "system", label: "System", Icon: Monitor },
  { value: "dark", label: "Dark", Icon: Moon },
];

export default function ThemeToggle({ preference, onChange }: Props) {
  return (
    <div className="theme-toggle" role="group" aria-label="Color theme">
      {OPTIONS.map(({ value, label, Icon }) => {
        const active = preference === value;
        return (
          <button
            key={value}
            type="button"
            className={`theme-toggle-option ${active ? "active" : ""}`}
            aria-pressed={active}
            title={`${label} theme`}
            onClick={() => onChange(value)}
          >
            <Icon size={14} strokeWidth={2} aria-hidden="true" />
            <span className="theme-toggle-label">{label}</span>
          </button>
        );
      })}
    </div>
  );
}
