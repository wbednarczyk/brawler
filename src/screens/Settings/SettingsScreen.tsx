import { AiSettings } from "./AiSettings";
import { AppearanceSettings } from "./AppearanceSettings";
import { CredentialSettings } from "./CredentialSettings";
import { ShortcutSettings } from "./ShortcutSettings";
import { SourceSettings } from "./SourceSettings";
import type { SettingsScreenProps } from "./settingsTypes";
import { makeTranslator } from "../../shared/locale";

export function SettingsScreen({
  theme,
  locale,
  settings,
  settingsError,
  feedPruneRetentionDays,
  feedPruneResult,
  geminiCredentialStatus,
  geminiCredentialError,
  geminiCredentialInFlight,
  geminiApiKeyDraft,
  shortcutBindings,
  shortcutReferences,
  onThemeChange,
  onLocaleChange,
  onPollIntervalChange,
  onShortcutBindingsChange,
  onYoutubeTranscriptionModelChange,
  onYoutubeTranscriptionTimeoutChange,
  onGeminiApiKeyDraftChange,
  onSaveGeminiApiKey,
  onClearGeminiApiKey,
  onOpenGeminiApiKeyPage,
  formatTimestamp,
  formatPollInterval,
  formatAiProvider,
  formatGeminiModel,
  formatCredentialConfigured,
  formatCredentialStorage,
  formatCredentialKind,
}: SettingsScreenProps) {
  const t = makeTranslator(locale);

  return (
    <section className="feed-panel" aria-labelledby="settings-title">
      <div className="panel-header">
        <div>
          <h1 id="settings-title">{t("settings.title")}</h1>
          <p>{t("settings.description")}</p>
        </div>
      </div>

      <div className="settings-layout" aria-label={t("settings.applicationSettings")}>
        <AppearanceSettings
          locale={locale}
          settings={settings}
          theme={theme}
          onLocaleChange={onLocaleChange}
          onThemeChange={onThemeChange}
          t={t}
        />
        <SourceSettings
          feedPruneRetentionDays={feedPruneRetentionDays}
          feedPruneResult={feedPruneResult}
          settings={settings}
          onPollIntervalChange={onPollIntervalChange}
          formatPollInterval={formatPollInterval}
          formatTimestamp={formatTimestamp}
        />
        <AiSettings
          geminiCredentialStatus={geminiCredentialStatus}
          settings={settings}
          onYoutubeTranscriptionModelChange={onYoutubeTranscriptionModelChange}
          onYoutubeTranscriptionTimeoutChange={onYoutubeTranscriptionTimeoutChange}
          formatAiProvider={formatAiProvider}
          formatGeminiModel={formatGeminiModel}
          formatCredentialConfigured={formatCredentialConfigured}
          formatCredentialStorage={formatCredentialStorage}
          formatCredentialKind={formatCredentialKind}
        />
        <CredentialSettings
          geminiApiKeyDraft={geminiApiKeyDraft}
          geminiCredentialError={geminiCredentialError}
          geminiCredentialInFlight={geminiCredentialInFlight}
          geminiCredentialStatus={geminiCredentialStatus}
          onClearGeminiApiKey={onClearGeminiApiKey}
          onGeminiApiKeyDraftChange={onGeminiApiKeyDraftChange}
          onOpenGeminiApiKeyPage={onOpenGeminiApiKeyPage}
          onSaveGeminiApiKey={onSaveGeminiApiKey}
        />
        <ShortcutSettings
          locale={locale}
          shortcutBindings={shortcutBindings}
          shortcutReferences={shortcutReferences}
          onShortcutBindingsChange={onShortcutBindingsChange}
        />

        {settingsError ? (
          <p className="error-text">Settings command failed: {settingsError}</p>
        ) : null}
      </div>
    </section>
  );
}
