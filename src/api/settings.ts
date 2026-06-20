import { callCommand } from "./tauri";
import type { UserSettings } from "./types";
import type { UpdateSettingsInput } from "./generated/UpdateSettingsInput";

// GENERATED from src-tauri/src/storage/settings.rs (SettingsUpdate) via ts-rs
// (ADR 0048). The Rust struct is the complete set of updatable settings; the
// theme/locale/accentPalette unions come from inline overrides on those fields.
export type { UpdateSettingsInput } from "./generated/UpdateSettingsInput";

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
