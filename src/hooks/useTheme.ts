import { useCallback, useSyncExternalStore } from "react";
import {
  getStoredPreference,
  setThemePreference,
  subscribeThemePreference,
  type ThemePreference,
} from "../lib/theme";

export function useTheme() {
  const preference = useSyncExternalStore(
    subscribeThemePreference,
    getStoredPreference,
    (): ThemePreference => "dark",
  );

  const setPreference = useCallback((next: ThemePreference) => {
    setThemePreference(next);
  }, []);

  return { preference, setPreference };
}
