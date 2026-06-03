import type { Dispatch, FormEvent, SetStateAction } from "react";
import * as credentialsApi from "../api/credentials";
import * as settingsApi from "../api/settings";
import type { AppLocale, CredentialStatus, ShortcutBindingSetting, Theme, UserSettings } from "../api/types";

type SettingsControllerInput = {
  geminiApiKeyDraft: string;
  setGeminiApiKeyDraft: Dispatch<SetStateAction<string>>;
  setGeminiCredentialError: Dispatch<SetStateAction<string | null>>;
  setGeminiCredentialInFlight: Dispatch<SetStateAction<boolean>>;
  setGeminiCredentialStatus: Dispatch<SetStateAction<CredentialStatus | null>>;
  setSettings: Dispatch<SetStateAction<UserSettings | null>>;
  setSettingsError: Dispatch<SetStateAction<string | null>>;
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
  setLocale,
  setTheme,
  text,
}: SettingsControllerInput) {
  function updateSettings(input: settingsApi.UpdateSettingsInput) {
    settingsApi.updateSettings(input)
      .then((response) => {
        setSettings(response);
        setLocale(response.locale);
        setTheme(response.theme);
        setSettingsError(null);
      })
      .catch((error) => {
        setSettingsError(String(error));
      });
  }

  function updateTheme(nextTheme: Theme) {
    setTheme(nextTheme);
    updateSettings({ theme: nextTheme });
  }

  function updateLocale(nextLocale: AppLocale) {
    setLocale(nextLocale);
    updateSettings({ locale: nextLocale });
  }

  function updatePollInterval(nextPollIntervalSeconds: number) {
    updateSettings({ pollIntervalSeconds: nextPollIntervalSeconds });
  }

  function updateYoutubeTranscriptionModel(nextModel: string) {
    updateSettings({ youtubeTranscriptionModel: nextModel });
  }

  function updateYoutubeTranscriptionTimeout(nextTimeoutSeconds: number) {
    updateSettings({ youtubeTranscriptionTimeoutSeconds: nextTimeoutSeconds });
  }

  function updateShortcutBindings(nextShortcutBindings: Record<string, ShortcutBindingSetting>) {
    updateSettings({ shortcutBindings: nextShortcutBindings });
  }

  function saveGeminiApiKey(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const apiKey = geminiApiKeyDraft.trim();
    if (!apiKey) {
      setGeminiCredentialError(text("Gemini API key is required."));
      return;
    }

    setGeminiCredentialInFlight(true);
    credentialsApi.setGeminiTranscriptionApiKey(apiKey)
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
    credentialsApi.clearGeminiTranscriptionApiKey()
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
    saveGeminiApiKey,
    updateLocale,
    updatePollInterval,
    updateShortcutBindings,
    updateTheme,
    updateYoutubeTranscriptionModel,
    updateYoutubeTranscriptionTimeout,
  };
}
