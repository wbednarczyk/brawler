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
import { CapabilityRoutingSettings } from "./CapabilityRoutingSettings";
import { EmbeddingSettings } from "./EmbeddingSettings";
import { AppearanceSettings } from "./AppearanceSettings";
import { CredentialSettings } from "./CredentialSettings";
import { DatabaseSettings } from "./DatabaseSettings";
import { QueueSettings } from "./QueueSettings";
import { ImportExportSettings } from "./ImportExportSettings";
import { LicenseSettings } from "./LicenseSettings";
import { LogSettings } from "./LogSettings";
import { ShortcutSettings } from "./ShortcutSettings";
import { SourceSettings } from "./SourceSettings";
import { makeTextTranslator, makeTranslator, type LocaleKey } from "../../shared/locale";
import { useSettingsScreenViewModel } from "../../app/state/screenViewModels";
import { ErrorText, Panel, PanelHeader, SelectField, Subnav } from "../../ui";

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
  { id: "database", icon: Database, labelText: "Data storage" },
  { id: "license", icon: FileKey2, labelText: "License" },
] satisfies Array<{ id: SettingsTab; icon: LucideIcon; labelKey?: LocaleKey; labelText?: string }>;

export function SettingsScreen() {
  const {
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
  onOpenAiCompatibleBaseUrlChange,
  onCapabilityProvidersChange,
  onLogLevelChange,
  onLogMaxFilesChange,
  onLogMaxFileBytesChange,
  onDbMaxConnectionsChange,
  onDbBusyTimeoutMsChange,
  onDbAcquireTimeoutMsChange,
  onResetDatabaseSettings,
  onSourcesWorkersChange,
  onAutopilotWorkersChange,
  onAiWorkersChange,
  onAiProviderConcurrencyChange,
  onResetQueueSettings,
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
  } = useSettingsScreenViewModel();
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

        {/* U7-E2 density contract (ADR 0076 D6): at the S tier the section tab
            list collapses to this select (same options + shared selection). The
            Subnav is hidden and this shown via container queries (settings.css);
            both always render so the selection state stays a single source. */}
        <div className="settings-section-select">
          <SelectField
            aria-label={text("Settings section")}
            label={text("Section")}
            value={activeSettingsTab}
            onChange={(event) => setActiveSettingsTab(event.target.value as SettingsTab)}
          >
            {settingsTabs.map((tab) => (
              <option key={tab.id} value={tab.id}>
                {tabLabel(tab)}
              </option>
            ))}
          </SelectField>
        </div>

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
            <>
              <AiSettings
                settings={settings}
                onYoutubeTranscriptionModelChange={onYoutubeTranscriptionModelChange}
                onYoutubeTranscriptionTimeoutChange={onYoutubeTranscriptionTimeoutChange}
                onGeneralAnalysisProviderChange={onGeneralAnalysisProviderChange}
                onGeneralAnalysisModelChange={onGeneralAnalysisModelChange}
                onGeneralAnalysisTimeoutChange={onGeneralAnalysisTimeoutChange}
                onEspiAiFallbackChange={onEspiAiFallbackChange}
                onOpenAiCompatibleBaseUrlChange={onOpenAiCompatibleBaseUrlChange}
              />
              <CapabilityRoutingSettings
                capabilityProviders={settings?.capabilityProviders ?? {}}
                onCapabilityProvidersChange={onCapabilityProvidersChange}
              />
              <EmbeddingSettings />
            </>
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
            <>
              <DatabaseSettings
                settings={settings}
                onDbMaxConnectionsChange={onDbMaxConnectionsChange}
                onDbBusyTimeoutMsChange={onDbBusyTimeoutMsChange}
                onDbAcquireTimeoutMsChange={onDbAcquireTimeoutMsChange}
                onResetDatabaseSettings={onResetDatabaseSettings}
              />
              <QueueSettings
                settings={settings}
                onSourcesWorkersChange={onSourcesWorkersChange}
                onAutopilotWorkersChange={onAutopilotWorkersChange}
                onAiWorkersChange={onAiWorkersChange}
                onAiProviderConcurrencyChange={onAiProviderConcurrencyChange}
                onResetQueueSettings={onResetQueueSettings}
              />
            </>
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
          <ErrorText>Settings command failed: {settingsError}</ErrorText>
        ) : null}
      </div>
    </Panel>
  );
}
