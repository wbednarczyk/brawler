import { callCommand } from "./tauri";
import type { AccentPalette, AppLocale, ShortcutBindingSetting, Theme, UserSettings } from "./types";

export type UpdateSettingsInput = {
  theme?: Theme;
  accentPalette?: AccentPalette;
  locale?: AppLocale;
  pollIntervalSeconds?: number;
  youtubeTranscriptionModel?: string;
  youtubeTranscriptionTimeoutSeconds?: number;
  generalAnalysisProvider?: string;
  generalAnalysisModel?: string;
  generalAnalysisTimeoutSeconds?: number;
  logLevel?: string;
  logMaxFiles?: number;
  logMaxFileBytes?: number;
  shortcutBindings?: Record<string, ShortcutBindingSetting>;
  dbMaxConnections?: number;
  dbBusyTimeoutMs?: number;
  dbAcquireTimeoutMs?: number;
};

export function getSettings() {
  return callCommand<UserSettings>("get_settings");
}

export function updateSettings(input: UpdateSettingsInput) {
  return callCommand<UserSettings>("update_settings", { input });
}

export function disableDeveloperMode() {
  return callCommand<UserSettings>("disable_developer_mode");
}

export function unlockDeveloperMode(passphrase: string) {
  return callCommand<UserSettings>("unlock_developer_mode", { input: { passphrase } });
}
