import { useCallback, useEffect, useState } from "react";

export type AppTheme = "light" | "dark";

const THEME_STORAGE_KEY = "prompter.theme.v1";

function loadTheme(): AppTheme {
  try {
    const stored = localStorage.getItem(THEME_STORAGE_KEY);
    if (stored === "dark" || stored === "light") return stored;
  } catch {
    // Fall through to the system preference.
  }
  return window.matchMedia?.("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

function saveTheme(theme: AppTheme): void {
  try {
    localStorage.setItem(THEME_STORAGE_KEY, theme);
  } catch {
    // Theme persistence is optional; the active session still keeps the choice.
  }
}

export function useTheme() {
  const [theme, setThemeState] = useState<AppTheme>(loadTheme);

  useEffect(() => {
    document.documentElement.style.colorScheme = theme;
  }, [theme]);

  // Persist only explicit choices so a fresh install keeps following the
  // system appearance until the user picks a theme.
  const setTheme = useCallback((next: AppTheme) => {
    setThemeState(next);
    saveTheme(next);
  }, []);

  return { theme, setTheme };
}
