import { useState } from "react";
import {
  Bot,
  Database,
  Download,
  FileKey2,
  KeyRound,
  Keyboard,
  Logs,
  Palette,
  RadioTower,
  type LucideIcon,
} from "lucide-react";
import { AiSettings } from "./AiSettings";
import { AppearanceSettings } from "./AppearanceSettings";
import { CredentialSettings } from "./CredentialSettings";
import { DatabaseSettings } from "./DatabaseSettings";
import { ImportExportSettings } from "./ImportExportSettings";
import { LicenseSettings } from "./LicenseSettings";
import { LogSettings } from "./LogSettings";
import { ShortcutSettings } from "./ShortcutSettings";
import { SourceSettings } from "./SourceSettings";
import type { SettingsScreenProps } from "./settingsTypes";
import { makeTextTranslator, makeTranslator, type LocaleKey } from "../../shared/locale";
import { Panel, PanelHeader, Subnav } from "../../ui";

type SettingsTab =
  | "appearance"
  | "sources"
  | "ai"
  | "credentials"
  | "importExport"
  | "shortcuts"
  | "logs"
  | "database"
  | "license";

const settingsTabs = [
  { id: "appearance", icon: Palette, labelKey: "settings.appearance.title" },
  { id: "sources", icon: RadioTower, labelKey: "settings.sources.title" },
  { id: "ai", icon: Bot, labelKey: "settings.ai.title" },
  { id: "credentials", icon: KeyRound, labelKey: "settings.credentials.title" },
  { id: "importExport", icon: Download, labelKey: "settings.importExport.title" },
  { id: "shortcuts", icon: Keyboard, labelText: "Keyboard shortcuts" },
  { id: "logs", icon: Logs, labelText: "Logs" },
  { id: "database", icon: Database, labelText: "Database" },
  { id: "license", icon: FileKey2, labelText: "License" },
] satisfies Array<{ id: SettingsTab; icon: LucideIcon; labelKey?: LocaleKey; labelText?: string }>;

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
  onEspiAiFallbackChange,
  onLogLevelChange,
  onLogMaxFilesChange,
  onLogMaxFileBytesChange,
  onDbMaxConnectionsChange,
  onDbBusyTimeoutMsChange,
  onDbAcquireTimeoutMsChange,
  onResetDatabaseSettings,
  onClearLicenseKey,
  onLicenseKeyDraftChange,
  onSubmitLicenseKey,
  onGeminiApiKeyDraftChange,
  onSaveGeminiApiKey,
  onClearGeminiApiKey,
  onOpenGeminiApiKeyPage,
  onImportApplied,
  formatTimestamp,
  formatPollInterval,
  formatCredentialConfigured,
  formatCredentialKind,
}: SettingsScreenProps) {
  const t = makeTranslator(locale);
  const text = makeTextTranslator(locale);
  const [activeSettingsTab, setActiveSettingsTab] = useState<SettingsTab>("appearance");
  const tabLabel = (tab: (typeof settingsTabs)[number]) =>
    tab.labelKey ? t(tab.labelKey) : text(tab.labelText ?? "");

  return (
    <Panel ariaLabelledBy="settings-title">
      <PanelHeader
        description={t("settings.description")}
        title={t("settings.title")}
        titleId="settings-title"
      />

      <div className="settings-layout" aria-label={t("settings.applicationSettings")}>
        <Subnav
          activeId={activeSettingsTab}
          ariaLabel={t("settings.sections")}
          className="settings-subnav"
          items={settingsTabs.map((tab) => ({
            id: tab.id,
            icon: <tab.icon size={18} aria-hidden="true" />,
            label: tabLabel(tab),
          }))}
          onSelect={setActiveSettingsTab}
        />

        <div className="settings-tab-panel">
          {activeSettingsTab === "appearance" ? (
            <AppearanceSettings
              accentPalette={accentPalette}
              locale={locale}
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
              settings={settings}
              onYoutubeTranscriptionModelChange={onYoutubeTranscriptionModelChange}
              onYoutubeTranscriptionTimeoutChange={onYoutubeTranscriptionTimeoutChange}
              onGeneralAnalysisProviderChange={onGeneralAnalysisProviderChange}
              onGeneralAnalysisModelChange={onGeneralAnalysisModelChange}
              onGeneralAnalysisTimeoutChange={onGeneralAnalysisTimeoutChange}
              onEspiAiFallbackChange={onEspiAiFallbackChange}
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
          {activeSettingsTab === "importExport" ? (
            <ImportExportSettings onImportApplied={onImportApplied} />
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
          {activeSettingsTab === "database" ? (
            <DatabaseSettings
              settings={settings}
              onDbMaxConnectionsChange={onDbMaxConnectionsChange}
              onDbBusyTimeoutMsChange={onDbBusyTimeoutMsChange}
              onDbAcquireTimeoutMsChange={onDbAcquireTimeoutMsChange}
              onResetDatabaseSettings={onResetDatabaseSettings}
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
    </Panel>
  );
}
