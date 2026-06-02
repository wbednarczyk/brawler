import type { DatabaseStatus, Theme } from "../api/types";

export function databaseIndicatorClass(status: DatabaseStatus | null, error: string | null) {
  if (error) {
    return "status-dot status-danger";
  }

  if (status) {
    return "status-dot status-ok";
  }

  return "status-dot status-warn";
}

export function resolveTheme(theme: Theme) {
  if (theme === "system") {
    return window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
  }

  return theme;
}
