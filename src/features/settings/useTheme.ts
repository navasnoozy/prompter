import { useEffect, useState } from "react";

export type AppTheme = "light" | "dark";

const THEME_STORAGE_KEY = "prompter.theme.v1";

function loadTheme(): AppTheme {
  try {
    return localStorage.getItem(THEME_STORAGE_KEY) === "dark" ? "dark" : "light";
  } catch {
    return "light";
  }
}

function saveTheme(theme: AppTheme): void {
  try {
    localStorage.setItem(THEME_STORAGE_KEY, theme);
  } catch {
    // Theme persistence is optional; the active session still keeps the choice.
  }
}

export function useTheme() {
  const [theme, setTheme] = useState<AppTheme>(loadTheme);

  useEffect(() => {
    saveTheme(theme);
    document.documentElement.style.colorScheme = theme;
  }, [theme]);

  return { theme, setTheme };
}
