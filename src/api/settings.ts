import { callCommand } from "./tauri";
import type { AppLocale, Theme, UserSettings } from "./types";

export type UpdateSettingsInput = {
  theme?: Theme;
  locale?: AppLocale;
  pollIntervalSeconds?: number;
  youtubeTranscriptionModel?: string;
  youtubeTranscriptionTimeoutSeconds?: number;
};

export function getSettings() {
  return callCommand<UserSettings>("get_settings");
}

export function updateSettings(input: UpdateSettingsInput) {
  return callCommand<UserSettings>("update_settings", { input });
}
