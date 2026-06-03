import { callCommand } from "./tauri";
import type { AppLocale, ShortcutBindingSetting, Theme, UserSettings } from "./types";

export type UpdateSettingsInput = {
  theme?: Theme;
  locale?: AppLocale;
  pollIntervalSeconds?: number;
  youtubeTranscriptionModel?: string;
  youtubeTranscriptionTimeoutSeconds?: number;
  generalAnalysisProvider?: string;
  generalAnalysisModel?: string;
  generalAnalysisTimeoutSeconds?: number;
  shortcutBindings?: Record<string, ShortcutBindingSetting>;
};

export function getSettings() {
  return callCommand<UserSettings>("get_settings");
}

export function updateSettings(input: UpdateSettingsInput) {
  return callCommand<UserSettings>("update_settings", { input });
}
