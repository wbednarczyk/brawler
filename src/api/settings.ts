import { callCommand } from "./tauri";
import type { Theme, UserSettings } from "./types";

export type UpdateSettingsInput = {
  theme?: Theme;
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
