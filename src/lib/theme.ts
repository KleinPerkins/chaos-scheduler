export type ThemePreference = "dark" | "light" | "system";
export type ResolvedTheme = "dark" | "light";

const STORAGE_KEY = "chaos-theme";
const LIGHT_QUERY = "(prefers-color-scheme: light)";

export function getStoredPreference(): ThemePreference {
  try {
    const value = localStorage.getItem(STORAGE_KEY);
    if (value === "light" || value === "dark" || value === "system") {
      return value;
    }
  } catch {
    // localStorage unavailable (private mode / SSR) — fall through.
  }
  return "dark";
}

export function resolveTheme(pref: ThemePreference): ResolvedTheme {
  if (pref === "system") {
    return typeof window !== "undefined" &&
      window.matchMedia?.(LIGHT_QUERY).matches
      ? "light"
      : "dark";
  }
  return pref;
}

export function applyTheme(pref: ThemePreference): void {
  if (typeof document === "undefined") return;
  document.documentElement.setAttribute("data-theme", resolveTheme(pref));
}

export function setThemePreference(pref: ThemePreference): void {
  try {
    localStorage.setItem(STORAGE_KEY, pref);
  } catch {
    // Preference won't persist, but the applied theme still takes effect.
  }
  applyTheme(pref);
}

/**
 * Apply the stored preference and keep "system" in sync with OS changes.
 * Called once, synchronously, before the app renders.
 */
export function initTheme(): void {
  applyTheme(getStoredPreference());
  window.matchMedia?.(LIGHT_QUERY).addEventListener?.("change", () => {
    if (getStoredPreference() === "system") applyTheme("system");
  });
}
