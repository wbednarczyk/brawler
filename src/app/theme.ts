import type { AccentPalette, DatabaseStatus, Theme } from "../api/types";

export const accentPaletteOptions = [
  { value: "night-neon", label: "Night Neon" },
  { value: "midnight-horizon", label: "Midnight Horizon" },
] satisfies Array<{ value: AccentPalette; label: string }>;

export function isAccentPalette(value: string): value is AccentPalette {
  return accentPaletteOptions.some((option) => option.value === value);
}

export function normalizeAccentPalette(value: string | null | undefined): AccentPalette {
  return value && isAccentPalette(value) ? value : "night-neon";
}

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
