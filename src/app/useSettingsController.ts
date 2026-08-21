import type { Dispatch, FormEvent, SetStateAction } from "react";
import * as credentialsApi from "../api/credentials";
import * as settingsApi from "../api/settings";
import type {
  AccentPalette,
  AppLocale,
  CredentialStatus,
  ShortcutBindingSetting,
  Theme,
  UserSettings,
} from "../api/types";

type SettingsControllerInput = {
  geminiApiKeyDraft: string;
  setGeminiApiKeyDraft: Dispatch<SetStateAction<string>>;
  setGeminiCredentialError: Dispatch<SetStateAction<string | null>>;
  setGeminiCredentialInFlight: Dispatch<SetStateAction<boolean>>;
  setGeminiCredentialStatus: Dispatch<SetStateAction<CredentialStatus | null>>;
  setSettings: Dispatch<SetStateAction<UserSettings | null>>;
  setSettingsError: Dispatch<SetStateAction<string | null>>;
  setAccentPalette: Dispatch<SetStateAction<AccentPalette>>;
  setLocale: Dispatch<SetStateAction<AppLocale>>;
  setTheme: Dispatch<SetStateAction<Theme>>;
  text: (value: string) => string;
};

export function useSettingsController({
  geminiApiKeyDraft,
  setGeminiApiKeyDraft,
  setGeminiCredentialError,
  setGeminiCredentialInFlight,
  setGeminiCredentialStatus,
  setSettings,
  setSettingsError,
  setAccentPalette,
  setLocale,
  setTheme,
  text,
}: SettingsControllerInput) {
  function applySettingsResponse(response: UserSettings) {
    setSettings(response);
    setAccentPalette(response.accentPalette);
    setLocale(response.locale);
    setTheme(response.theme);
    setSettingsError(null);
  }

  function updateSettings(input: settingsApi.UpdateSettingsInput) {
    settingsApi
      .updateSettings(input)
      .then(applySettingsResponse)
      .catch((error) => {
        setSettingsError(String(error));
      });
  }

  function updateTheme(nextTheme: Theme) {
    setTheme(nextTheme);
    updateSettings({ theme: nextTheme });
  }

  function updateAccentPalette(nextAccentPalette: AccentPalette) {
    setAccentPalette(nextAccentPalette);
    updateSettings({ accentPalette: nextAccentPalette });
  }

  function updateLocale(nextLocale: AppLocale) {
    setLocale(nextLocale);
    updateSettings({ locale: nextLocale });
  }

  function updatePollInterval(nextPollIntervalSeconds: number) {
    updateSettings({ pollIntervalSeconds: nextPollIntervalSeconds });
  }

  // Backfill depth in years (ADR 0077 §3). The backend clamps to [1, 10]; the UI
  // offers presets + a bound input, so callers pass an already-sane value.
  function updateBackfillYears(nextBackfillYears: number) {
    updateSettings({ backfillYears: nextBackfillYears });
  }

  // MCP listen port (ADR 0078). The backend clamps to [1024, 65535]; the UI
  // commits an already-clamped value on blur (never per keystroke). Enable/token
  // lifecycle is owned by McpSettings via its dedicated commands.
  function updateMcpPort(nextPort: number) {
    updateSettings({ mcpPort: nextPort });
  }

  // MCP write tier (ADR 0088 M3). Off by default; when on, write tools require
  // citations and deletes/settings stay UI-only. This is the only toggle —
  // `update_settings` is itself MCP-excluded, so an agent can never enable its
  // own writes.
  function updateMcpWritesEnabled(nextEnabled: boolean) {
    updateSettings({ mcpWritesEnabled: nextEnabled });
  }

  // The kpi_acquisition scope gate (ADR 0099 dec. 2). Off by default; off means
  // the acquisition token is rejected at auth entirely (reads included). Same
  // MCP-excluded posture as the writes toggle.
  function updateKpiAcquisitionEnabled(nextEnabled: boolean) {
    updateSettings({ kpiAcquisitionEnabled: nextEnabled });
  }

  function updateYoutubeTranscriptionModel(nextModel: string) {
    updateSettings({ youtubeTranscriptionModel: nextModel });
  }

  function updateYoutubeTranscriptionTimeout(nextTimeoutSeconds: number) {
    updateSettings({ youtubeTranscriptionTimeoutSeconds: nextTimeoutSeconds });
  }

  function updateShortcutBindings(
    nextShortcutBindings: Record<string, ShortcutBindingSetting>,
  ) {
    updateSettings({ shortcutBindings: nextShortcutBindings });
  }

  // Replace the full pinned-company list (ADR 0054 sidebar spine). Callers pass
  // the complete desired order; the backend overwrites and de-duplicates.
  function updatePinnedCompanyIds(nextPinnedCompanyIds: string[]) {
    updateSettings({ pinnedCompanyIds: nextPinnedCompanyIds });
  }

  // Dziś v2 "Oznacz dzień jako przejrzany" set (F2 S4, plan decision 5) — same
  // replace-the-array precedent as `pinnedCompanyIds`.
  function updateTodayReviewedDays(nextTodayReviewedDays: string[]) {
    updateSettings({ todayReviewedDays: nextTodayReviewedDays });
  }

  function updateLogLevel(nextLevel: string) {
    updateSettings({ logLevel: nextLevel });
  }

  function updateLogMaxFiles(nextMaxFiles: number) {
    updateSettings({ logMaxFiles: nextMaxFiles });
  }

  function updateLogMaxFileBytes(nextMaxFileBytes: number) {
    updateSettings({ logMaxFileBytes: nextMaxFileBytes });
  }

  function updateDbMaxConnections(nextMaxConnections: number) {
    updateSettings({ dbMaxConnections: nextMaxConnections });
  }

  function updateDbBusyTimeoutMs(nextBusyTimeoutMs: number) {
    updateSettings({ dbBusyTimeoutMs: nextBusyTimeoutMs });
  }

  function updateDbAcquireTimeoutMs(nextAcquireTimeoutMs: number) {
    updateSettings({ dbAcquireTimeoutMs: nextAcquireTimeoutMs });
  }

  function resetDatabaseSettings() {
    updateSettings({
      dbMaxConnections: 4,
      dbBusyTimeoutMs: 5000,
      dbAcquireTimeoutMs: 10000,
    });
  }

  function updateSourcesWorkers(nextWorkers: number) {
    updateSettings({ sourcesWorkers: nextWorkers });
  }

  function updateAutopilotWorkers(nextWorkers: number) {
    updateSettings({ autopilotWorkers: nextWorkers });
  }

  function resetQueueSettings() {
    updateSettings({
      sourcesWorkers: 2,
      autopilotWorkers: 3,
    });
  }

  function disableDeveloperMode() {
    settingsApi
      .disableDeveloperMode()
      .then(applySettingsResponse)
      .catch((error) => {
        setSettingsError(String(error));
      });
  }

  function unlockDeveloperMode(passphrase: string) {
    settingsApi
      .unlockDeveloperMode(passphrase)
      .then(applySettingsResponse)
      .catch((error) => {
        setSettingsError(String(error));
      });
  }

  function saveGeminiApiKey(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const apiKey = geminiApiKeyDraft.trim();
    if (!apiKey) {
      setGeminiCredentialError(text("Gemini API key is required."));
      return;
    }

    setGeminiCredentialInFlight(true);
    credentialsApi
      .setGeminiTranscriptionApiKey(apiKey)
      .then((response) => {
        setGeminiCredentialStatus(response);
        setGeminiCredentialError(null);
        setGeminiApiKeyDraft("");
      })
      .catch((error) => {
        setGeminiCredentialError(String(error));
      })
      .finally(() => {
        setGeminiCredentialInFlight(false);
      });
  }

  function clearGeminiApiKey() {
    setGeminiCredentialInFlight(true);
    credentialsApi
      .clearGeminiTranscriptionApiKey()
      .then((response) => {
        setGeminiCredentialStatus(response);
        setGeminiCredentialError(null);
        setGeminiApiKeyDraft("");
      })
      .catch((error) => {
        setGeminiCredentialError(String(error));
      })
      .finally(() => {
        setGeminiCredentialInFlight(false);
      });
  }

  return {
    clearGeminiApiKey,
    disableDeveloperMode,
    saveGeminiApiKey,
    unlockDeveloperMode,
    updateAccentPalette,
    updateDbAcquireTimeoutMs,
    updateDbBusyTimeoutMs,
    updateDbMaxConnections,
    resetDatabaseSettings,
    updateSourcesWorkers,
    updateAutopilotWorkers,
    resetQueueSettings,
    updateLocale,
    updateLogLevel,
    updateLogMaxFileBytes,
    updateLogMaxFiles,
    updateBackfillYears,
    updateKpiAcquisitionEnabled,
    updateMcpPort,
    updateMcpWritesEnabled,
    updatePinnedCompanyIds,
    updatePollInterval,
    updateShortcutBindings,
    updateTodayReviewedDays,
    updateTheme,
    updateYoutubeTranscriptionModel,
    updateYoutubeTranscriptionTimeout,
  };
}
