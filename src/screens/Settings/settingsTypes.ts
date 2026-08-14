import type { FormEvent } from "react";
import type {
  AccentPalette,
  AppLocale,
  CredentialStatus,
  LicenseStatus,
  ShortcutBindingSetting,
  Theme,
  UserSettings,
} from "../../api/types";
import type { AppShortcutReferenceItem } from "../../app/shortcuts";

export type SettingsScreenProps = {
  theme: Theme;
  accentPalette: AccentPalette;
  locale: AppLocale;
  settings: UserSettings | null;
  settingsError: string | null;
  licenseStatus: LicenseStatus | null;
  licenseError: string | null;
  licenseInFlight: boolean;
  licenseKeyDraft: string;
  geminiCredentialStatus: CredentialStatus | null;
  geminiCredentialError: string | null;
  geminiCredentialInFlight: boolean;
  geminiApiKeyDraft: string;
  shortcutBindings: Record<string, ShortcutBindingSetting>;
  shortcutReferences: AppShortcutReferenceItem[];
  onThemeChange: (theme: Theme) => void;
  onAccentPaletteChange: (accentPalette: AccentPalette) => void;
  onLocaleChange: (locale: AppLocale) => void;
  onPollIntervalChange: (pollIntervalSeconds: number) => void;
  onBackfillYearsChange: (backfillYears: number) => void;
  onMcpPortChange: (port: number) => void;
  onMcpWritesEnabledChange: (enabled: boolean) => void;
  onKpiAcquisitionEnabledChange: (enabled: boolean) => void;
  onShortcutBindingsChange: (
    shortcutBindings: Record<string, ShortcutBindingSetting>,
  ) => void;
  onYoutubeTranscriptionModelChange: (model: string) => void;
  onYoutubeTranscriptionTimeoutChange: (timeoutSeconds: number) => void;
  onLogLevelChange: (level: string) => void;
  onLogMaxFilesChange: (maxFiles: number) => void;
  onLogMaxFileBytesChange: (maxFileBytes: number) => void;
  onDbMaxConnectionsChange: (maxConnections: number) => void;
  onDbBusyTimeoutMsChange: (busyTimeoutMs: number) => void;
  onDbAcquireTimeoutMsChange: (acquireTimeoutMs: number) => void;
  onResetDatabaseSettings: () => void;
  onSourcesWorkersChange: (workers: number) => void;
  onAutopilotWorkersChange: (workers: number) => void;
  onResetQueueSettings: () => void;
  onClearLicenseKey: () => void;
  onLicenseKeyDraftChange: (licenseKey: string) => void;
  onSubmitLicenseKey: (event: FormEvent<HTMLFormElement>) => void;
  onGeminiApiKeyDraftChange: (apiKey: string) => void;
  onSaveGeminiApiKey: (event: FormEvent<HTMLFormElement>) => void;
  onClearGeminiApiKey: () => void;
  onOpenGeminiApiKeyPage: () => void;
  onImportApplied: () => void;
  formatPollInterval: (seconds: number) => string;
  formatGeminiModel: (value: string | null | undefined) => string;
  formatCredentialConfigured: (status: CredentialStatus | null) => string;
  formatCredentialKind: (value: string | null | undefined) => string;
};
