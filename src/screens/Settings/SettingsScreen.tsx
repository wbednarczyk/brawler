import { AiSettings } from "./AiSettings";
import { AppearanceSettings } from "./AppearanceSettings";
import { CredentialSettings } from "./CredentialSettings";
import { SourceSettings } from "./SourceSettings";
import type { SettingsScreenProps } from "./settingsTypes";

export function SettingsScreen({
  theme,
  settings,
  settingsError,
  feedPruneRetentionDays,
  feedPruneResult,
  geminiCredentialStatus,
  geminiCredentialError,
  geminiCredentialInFlight,
  geminiApiKeyDraft,
  onThemeChange,
  onPollIntervalChange,
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
  return (
    <section className="feed-panel" aria-labelledby="settings-title">
      <div className="panel-header">
        <div>
          <h1 id="settings-title">Settings</h1>
          <p>SQLite-backed local runtime settings.</p>
        </div>
      </div>

      <div className="settings-layout" aria-label="Application settings">
        <AppearanceSettings
          settings={settings}
          theme={theme}
          onThemeChange={onThemeChange}
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

        {settingsError ? (
          <p className="error-text">Settings command failed: {settingsError}</p>
        ) : null}
      </div>
    </section>
  );
}
