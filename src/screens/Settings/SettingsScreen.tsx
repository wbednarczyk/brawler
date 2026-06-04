import { useState } from "react";
import { AiSettings } from "./AiSettings";
import { AppearanceSettings } from "./AppearanceSettings";
import { CredentialSettings } from "./CredentialSettings";
import { LicenseSettings } from "./LicenseSettings";
import { LogSettings } from "./LogSettings";
import { ShortcutSettings } from "./ShortcutSettings";
import { SourceSettings } from "./SourceSettings";
import type { SettingsScreenProps } from "./settingsTypes";
import { makeTextTranslator, makeTranslator, type LocaleKey } from "../../shared/locale";

type SettingsTab =
  | "appearance"
  | "sources"
  | "ai"
  | "credentials"
  | "shortcuts"
  | "logs"
  | "license";

const settingsTabs = [
  { id: "appearance", labelKey: "settings.appearance.title" },
  { id: "sources", labelKey: "settings.sources.title" },
  { id: "ai", labelKey: "settings.ai.title" },
  { id: "credentials", labelKey: "settings.credentials.title" },
  { id: "shortcuts", labelText: "Keyboard shortcuts" },
  { id: "logs", labelText: "Logs" },
  { id: "license", labelText: "License" },
] satisfies Array<{ id: SettingsTab; labelKey?: LocaleKey; labelText?: string }>;

export function SettingsScreen({
  theme,
  accentPalette,
  locale,
  settings,
  settingsError,
  licenseStatus,
  licenseError,
  licenseInFlight,
  licenseKeyDraft,
  feedPruneRetentionDays,
  feedPruneResult,
  geminiCredentialStatus,
  geminiCredentialError,
  geminiCredentialInFlight,
  geminiApiKeyDraft,
  shortcutBindings,
  shortcutReferences,
  onThemeChange,
  onAccentPaletteChange,
  onLocaleChange,
  onPollIntervalChange,
  onShortcutBindingsChange,
  onYoutubeTranscriptionModelChange,
  onYoutubeTranscriptionTimeoutChange,
  onGeneralAnalysisProviderChange,
  onGeneralAnalysisModelChange,
  onGeneralAnalysisTimeoutChange,
  onLogLevelChange,
  onLogMaxFilesChange,
  onLogMaxFileBytesChange,
  onClearLicenseKey,
  onLicenseKeyDraftChange,
  onSubmitLicenseKey,
  onGeminiApiKeyDraftChange,
  onSaveGeminiApiKey,
  onClearGeminiApiKey,
  onOpenGeminiApiKeyPage,
  formatTimestamp,
  formatPollInterval,
  formatAiProvider,
  formatGeminiModel,
  formatCredentialConfigured,
  formatCredentialKind,
}: SettingsScreenProps) {
  const t = makeTranslator(locale);
  const text = makeTextTranslator(locale);
  const [activeSettingsTab, setActiveSettingsTab] = useState<SettingsTab>("appearance");
  const tabLabel = (tab: (typeof settingsTabs)[number]) =>
    tab.labelKey ? t(tab.labelKey) : text(tab.labelText ?? "");

  return (
    <section className="feed-panel" aria-labelledby="settings-title">
      <div className="panel-header">
        <div>
          <h1 id="settings-title">{t("settings.title")}</h1>
          <p>{t("settings.description")}</p>
        </div>
      </div>

      <div className="settings-layout" aria-label={t("settings.applicationSettings")}>
        <nav className="settings-subnav" aria-label={t("settings.sections")}>
          {settingsTabs.map((tab) => (
            <button
              className={activeSettingsTab === tab.id ? "settings-subnav-active" : undefined}
              key={tab.id}
              onClick={() => setActiveSettingsTab(tab.id)}
              type="button"
            >
              {tabLabel(tab)}
            </button>
          ))}
        </nav>

        <div className="settings-tab-panel">
          {activeSettingsTab === "appearance" ? (
            <AppearanceSettings
              accentPalette={accentPalette}
              locale={locale}
              settings={settings}
              theme={theme}
              onAccentPaletteChange={onAccentPaletteChange}
              onLocaleChange={onLocaleChange}
              onThemeChange={onThemeChange}
              t={t}
            />
          ) : null}
          {activeSettingsTab === "sources" ? (
            <SourceSettings
              feedPruneRetentionDays={feedPruneRetentionDays}
              feedPruneResult={feedPruneResult}
              settings={settings}
              onPollIntervalChange={onPollIntervalChange}
              formatPollInterval={formatPollInterval}
              formatTimestamp={formatTimestamp}
            />
          ) : null}
          {activeSettingsTab === "ai" ? (
            <AiSettings
              geminiCredentialStatus={geminiCredentialStatus}
              settings={settings}
              onYoutubeTranscriptionModelChange={onYoutubeTranscriptionModelChange}
              onYoutubeTranscriptionTimeoutChange={onYoutubeTranscriptionTimeoutChange}
              onGeneralAnalysisProviderChange={onGeneralAnalysisProviderChange}
              onGeneralAnalysisModelChange={onGeneralAnalysisModelChange}
              onGeneralAnalysisTimeoutChange={onGeneralAnalysisTimeoutChange}
              formatAiProvider={formatAiProvider}
              formatGeminiModel={formatGeminiModel}
              formatCredentialConfigured={formatCredentialConfigured}
              formatCredentialKind={formatCredentialKind}
            />
          ) : null}
          {activeSettingsTab === "credentials" ? (
            <CredentialSettings
              formatCredentialConfigured={formatCredentialConfigured}
              formatCredentialKind={formatCredentialKind}
              geminiApiKeyDraft={geminiApiKeyDraft}
              geminiCredentialError={geminiCredentialError}
              geminiCredentialInFlight={geminiCredentialInFlight}
              geminiCredentialStatus={geminiCredentialStatus}
              onClearGeminiApiKey={onClearGeminiApiKey}
              onGeminiApiKeyDraftChange={onGeminiApiKeyDraftChange}
              onOpenGeminiApiKeyPage={onOpenGeminiApiKeyPage}
              onSaveGeminiApiKey={onSaveGeminiApiKey}
            />
          ) : null}
          {activeSettingsTab === "shortcuts" ? (
            <ShortcutSettings
              locale={locale}
              shortcutBindings={shortcutBindings}
              shortcutReferences={shortcutReferences}
              onShortcutBindingsChange={onShortcutBindingsChange}
            />
          ) : null}
          {activeSettingsTab === "logs" ? (
            <LogSettings
              settings={settings}
              onLogLevelChange={onLogLevelChange}
              onLogMaxFilesChange={onLogMaxFilesChange}
              onLogMaxFileBytesChange={onLogMaxFileBytesChange}
            />
          ) : null}
          {activeSettingsTab === "license" ? (
            <LicenseSettings
              licenseError={licenseError}
              licenseInFlight={licenseInFlight}
              licenseKeyDraft={licenseKeyDraft}
              licenseStatus={licenseStatus}
              onClearLicenseKey={onClearLicenseKey}
              onLicenseKeyDraftChange={onLicenseKeyDraftChange}
              onSubmitLicenseKey={onSubmitLicenseKey}
            />
          ) : null}
        </div>

        {settingsError ? (
          <p className="error-text">Settings command failed: {settingsError}</p>
        ) : null}
      </div>
    </section>
  );
}
