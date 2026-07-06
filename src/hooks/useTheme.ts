import { useCallback, useState } from "react";
import {
  getStoredPreference,
  setThemePreference,
  type ThemePreference,
} from "../lib/theme";

export function useTheme() {
  const [preference, setPreferenceState] =
    useState<ThemePreference>(getStoredPreference);

  const setPreference = useCallback((next: ThemePreference) => {
    setThemePreference(next);
    setPreferenceState(next);
  }, []);

  return { preference, setPreference };
}
